//! Wo Dateien liegen und wie der Kontaktspeicher verschlüsselt wird.
//!
//! Der Kern kennt bewusst keine Dateien (`crates/README.md`). Alles, was mit
//! Pfaden und Datenträgern zu tun hat, steht deshalb hier.

use crate::fehler::{Ergebnis, Fehler};

use cabrik_core::keyfile;
use cabrik_core::trust::{self, ContactsKey, TrustStore};
use cabrik_core::{Identity, OsRandom, Randomness as _};

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use std::path::{Path, PathBuf};

/// Magic-Bytes des Kontaktspeichers.
const KONTAKT_MAGIC: [u8; 2] = [0xCA, 0x43];
/// Formatversion des Kontaktspeichers.
const KONTAKT_VERSION: u8 = 0x02;
/// Länge des Klartextkopfs: Magic, Version, Nonce.
const KOPF_LEN: usize = 15;

/// Verzeichnis für Schlüssel und Kontakte.
///
/// # Fehler
///
/// [`Fehler::Bedienung`], wenn kein Heimatverzeichnis feststellbar ist.
pub fn verzeichnis() -> Ergebnis<PathBuf> {
    // Windows: %APPDATA%. Unix: $XDG_CONFIG_HOME, sonst ~/.config.
    if let Ok(appdata) = std::env::var("APPDATA")
        && !appdata.is_empty()
    {
        return Ok(Path::new(&appdata).join("CabrikSecure"));
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        return Ok(Path::new(&xdg).join("cabrik"));
    }
    if let Ok(home) = std::env::var("HOME")
        && !home.is_empty()
    {
        return Ok(Path::new(&home).join(".config").join("cabrik"));
    }
    Err(Fehler::bedienung(
        "Kein Konfigurationsverzeichnis feststellbar — bitte --keyfile angeben",
    ))
}

/// Voreingestellter Pfad des Keyfiles.
///
/// # Fehler
///
/// Siehe [`verzeichnis`].
pub fn keyfile_pfad(angabe: Option<&Path>) -> Ergebnis<PathBuf> {
    match angabe {
        Some(p) => Ok(p.to_path_buf()),
        None => Ok(verzeichnis()?.join("identity.cabrik-key")),
    }
}

/// Voreingestellter Pfad des Kontaktspeichers.
///
/// # Fehler
///
/// Siehe [`verzeichnis`].
pub fn kontakte_pfad(angabe: Option<&Path>) -> Ergebnis<PathBuf> {
    match angabe {
        Some(p) => Ok(p.to_path_buf()),
        None => Ok(verzeichnis()?.join("contacts.cabrik-contacts")),
    }
}

/// Legt das Verzeichnis an, falls nötig.
///
/// # Fehler
///
/// Dateisystemfehler.
pub fn stelle_verzeichnis_sicher(pfad: &Path) -> Ergebnis<()> {
    if let Some(v) = pfad.parent()
        && !v.as_os_str().is_empty()
    {
        std::fs::create_dir_all(v).map_err(|e| Fehler::datei(v, e))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Keyfile
// ---------------------------------------------------------------------------

/// Liest und entsperrt ein Keyfile.
///
/// # Fehler
///
/// - Dateizugriff
/// - [`cabrik_core::Error::KeyfileAuthFailed`] bei falschem Passwort
pub fn lies_keyfile(pfad: &Path, passwort: &[u8]) -> Ergebnis<Identity> {
    let daten = std::fs::read(pfad).map_err(|e| Fehler::datei(pfad, e))?;

    // Ein v1-Keyfile hier freundlich abfangen: Der Nutzer bekommt sonst nur
    // „beschädigt" zu sehen und weiß nicht, dass es einen Weg gibt.
    if cabrik_v1::keyfile::looks_like_v1(&daten) {
        return Err(Fehler::bedienung(format!(
            "{} ist ein Schlüssel aus Version 1. Zum Übernehmen:\n  \
             cabrik migrate \"{}\" --out <neue-datei>",
            pfad.display(),
            pfad.display()
        )));
    }

    Ok(keyfile::read(&daten, passwort)?)
}

/// Schreibt ein Keyfile.
///
/// Schreibt **nicht** über eine bestehende Datei: Ein überschriebenes Keyfile
/// ist unwiederbringlich, und der Verlust fällt oft erst Wochen später auf.
///
/// # Fehler
///
/// - [`Fehler::Bedienung`], wenn die Datei bereits existiert
/// - Dateisystemfehler
pub fn schreib_keyfile(
    pfad: &Path,
    identity: &Identity,
    passwort: &[u8],
    params: &keyfile::KdfParams,
) -> Ergebnis<()> {
    if pfad.exists() {
        return Err(Fehler::bedienung(format!(
            "{} existiert bereits. Ein überschriebener Schlüssel ist unwiederbringlich —\n\
             bitte einen anderen Pfad wählen oder die Datei vorher wegsichern.",
            pfad.display()
        )));
    }
    stelle_verzeichnis_sicher(pfad)?;
    let daten = keyfile::write(identity, passwort, params, &mut OsRandom)?;
    std::fs::write(pfad, &daten).map_err(|e| Fehler::datei(pfad, e))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Kontaktspeicher (spec/trust-store.md §6)
// ---------------------------------------------------------------------------

/// Liest den Kontaktspeicher. Fehlt die Datei, ist er leer.
///
/// # Fehler
///
/// - Dateizugriff
/// - [`cabrik_core::Error::AuthFailed`], wenn die Datei nicht zu dieser
///   Identität gehört oder verändert wurde
pub fn lies_kontakte(pfad: &Path, identity: &Identity) -> Ergebnis<TrustStore> {
    if !pfad.exists() {
        return Ok(TrustStore::new());
    }
    let daten = std::fs::read(pfad).map_err(|e| Fehler::datei(pfad, e))?;
    entschluessle_kontakte(&daten, identity)
}

/// Schreibt den Kontaktspeicher.
///
/// # Fehler
///
/// Dateisystemfehler oder Serialisierungsfehler.
pub fn schreib_kontakte(pfad: &Path, store: &TrustStore, identity: &Identity) -> Ergebnis<()> {
    stelle_verzeichnis_sicher(pfad)?;
    let daten = verschluessle_kontakte(store, identity)?;

    // Erst danebenschreiben, dann umbenennen. Ein Absturz mitten im Schreiben
    // darf nicht alle Kontakte vernichten.
    let temp = pfad.with_extension("tmp");
    std::fs::write(&temp, &daten).map_err(|e| Fehler::datei(&temp, e))?;
    std::fs::rename(&temp, pfad).map_err(|e| Fehler::datei(pfad, e))?;
    Ok(())
}

/// Verschlüsselt den Kontaktspeicher.
///
/// # Der Nonce ist zufällig, nicht null
///
/// Das Keyfile darf einen Null-Nonce führen, weil bei jedem Schreiben ein
/// frisches Salz einen **neuen** Schlüssel erzeugt. Hier ist das anders: Der
/// Schlüssel stammt aus `HKDF(enc_sk)` und ist bei jedem Schreiben
/// **derselbe**. Ein fester Nonce hieße Nonce-Wiederverwendung über alle
/// Fassungen der Datei hinweg — bei ChaCha20-Poly1305 gibt das den
/// XOR-Unterschied zweier Fassungen preis und erlaubt zusätzlich, den
/// Authentisierungsschlüssel zu berechnen und Fälschungen zu bauen.
///
/// # Fehler
///
/// Serialisierungs- oder Zufallsfehler.
fn verschluessle_kontakte(store: &TrustStore, identity: &Identity) -> Ergebnis<Vec<u8>> {
    let klartext = trust::serialize(store)?;

    let mut nonce = [0u8; 12];
    OsRandom.fill(&mut nonce)?;

    let mut aus = Vec::with_capacity(KOPF_LEN.saturating_add(klartext.len()).saturating_add(16));
    aus.extend_from_slice(&KONTAKT_MAGIC);
    aus.push(KONTAKT_VERSION);
    aus.extend_from_slice(&nonce);
    debug_assert_eq!(aus.len(), KOPF_LEN, "Kopfaufbau weicht von der Spec ab");

    let schluessel = ContactsKey::derive(identity);
    let cipher = ChaCha20Poly1305::new(&Key::from(*schluessel.as_bytes()));
    let kopf = aus.clone();
    let ct = cipher
        .encrypt(
            &Nonce::from(nonce),
            Payload {
                msg: &klartext,
                aad: &kopf,
            },
        )
        .map_err(|_| Fehler::from(cabrik_core::Error::AuthFailed))?;

    aus.extend_from_slice(&ct);
    Ok(aus)
}

/// Entschlüsselt den Kontaktspeicher.
fn entschluessle_kontakte(daten: &[u8], identity: &Identity) -> Ergebnis<TrustStore> {
    let kopf = daten
        .get(..KOPF_LEN)
        .ok_or(cabrik_core::Error::Malformed("contacts: truncated header"))?;

    if kopf.get(..2) != Some(&KONTAKT_MAGIC[..]) {
        return Err(cabrik_core::Error::Malformed("contacts: bad magic").into());
    }
    if kopf.get(2) != Some(&KONTAKT_VERSION) {
        return Err(cabrik_core::Error::UnsupportedVersion.into());
    }
    let nonce: [u8; 12] = kopf
        .get(3..KOPF_LEN)
        .and_then(|s| s.try_into().ok())
        .ok_or(cabrik_core::Error::Malformed("contacts: truncated nonce"))?;
    let ct = daten
        .get(KOPF_LEN..)
        .ok_or(cabrik_core::Error::Malformed("contacts: no ciphertext"))?;

    let schluessel = ContactsKey::derive(identity);
    let cipher = ChaCha20Poly1305::new(&Key::from(*schluessel.as_bytes()));
    let klartext = cipher
        .decrypt(&Nonce::from(nonce), Payload { msg: ct, aad: kopf })
        .map_err(|_| cabrik_core::Error::AuthFailed)?;

    Ok(trust::deserialize(&klartext)?)
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Fehlschlag soll den Test abbrechen")]
mod tests {
    use super::*;
    use cabrik_core::trust::Contact;

    fn identitaet(seed: u8) -> Identity {
        let mut id = Identity::generate(&mut OsRandom, true, 0).unwrap();
        id.enc_sk = [seed; 32];
        id
    }

    #[test]
    fn kontakte_ueberstehen_verschluesseln_und_lesen() {
        let id = identitaet(0x11);
        let mut store = TrustStore::new();
        store
            .add(Contact::new_seen("Bob", [0x22; 32], Some([0x33; 32]), None, 7).unwrap())
            .unwrap();

        let daten = verschluessle_kontakte(&store, &id).unwrap();
        let zurueck = entschluessle_kontakte(&daten, &id).unwrap();

        assert_eq!(zurueck.len(), 1);
        assert_eq!(zurueck.contacts().first().unwrap().name, "Bob");
    }

    /// Der Zweck der Verschlüsselung: Wer die Datei hat, aber den Schlüssel
    /// nicht, erfährt nicht, mit wem kommuniziert wird.
    #[test]
    fn fremde_identitaet_kann_nicht_lesen() {
        let alice = identitaet(0x11);
        let mallory = identitaet(0x99);

        let mut store = TrustStore::new();
        store
            .add(Contact::new_seen("Bob", [0x22; 32], None, None, 7).unwrap())
            .unwrap();
        let daten = verschluessle_kontakte(&store, &alice).unwrap();

        assert_eq!(
            entschluessle_kontakte(&daten, &mallory).unwrap_err().code(),
            "AUTH_FAILED"
        );
    }

    #[test]
    fn namen_stehen_nicht_im_klartext_in_der_datei() {
        let id = identitaet(0x11);
        let mut store = TrustStore::new();
        store
            .add(Contact::new_seen("Rechtsanwalt", [0x22; 32], None, None, 7).unwrap())
            .unwrap();

        let daten = verschluessle_kontakte(&store, &id).unwrap();
        assert!(
            !daten.windows(12).any(|f| f == b"Rechtsanwalt"),
            "der Name stand lesbar in der Datei"
        );
    }

    /// Zweimal derselbe Inhalt, zweimal andere Bytes — sonst waere der Nonce
    /// wiederverwendet und die Verschluesselung gebrochen.
    #[test]
    fn zwei_schreibvorgaenge_verwenden_verschiedene_nonces() {
        let id = identitaet(0x11);
        let store = TrustStore::new();

        let a = verschluessle_kontakte(&store, &id).unwrap();
        let b = verschluessle_kontakte(&store, &id).unwrap();

        assert_ne!(
            a.get(3..15),
            b.get(3..15),
            "gleicher Nonce bei gleichem Schluessel — das bricht ChaCha20-Poly1305"
        );
        assert_ne!(a, b);
    }

    /// Ein veraenderter Kopf muss auffallen; er ist AAD.
    #[test]
    fn manipulierter_kopf_faellt_auf() {
        let id = identitaet(0x11);
        let mut daten = verschluessle_kontakte(&TrustStore::new(), &id).unwrap();
        if let Some(b) = daten.get_mut(5) {
            *b ^= 0xFF;
        }
        assert!(entschluessle_kontakte(&daten, &id).is_err());
    }

    #[test]
    fn fehlende_datei_ergibt_leeren_speicher() {
        let id = identitaet(0x11);
        let pfad = std::env::temp_dir().join("cabrik-gibt-es-nicht-12345.contacts");
        let store = lies_kontakte(&pfad, &id).unwrap();
        assert!(store.is_empty());
    }
}
