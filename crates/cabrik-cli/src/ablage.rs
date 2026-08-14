//! Wo Dateien liegen und wie der Kontaktspeicher verschlüsselt wird.
//!
//! Der Kern kennt bewusst keine Dateien (`crates/README.md`). Alles, was mit
//! Pfaden und Datenträgern zu tun hat, steht deshalb hier.

use crate::fehler::{Ergebnis, Fehler};

use cabrik_core::keyfile;
use cabrik_core::trust::{self, TrustStore};
use cabrik_core::{Identity, OsRandom};

use std::path::{Path, PathBuf};

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
    kontakte_entschluesseln(&daten, identity)
}

/// Schreibt den Kontaktspeicher.
///
/// # Fehler
///
/// Dateisystemfehler oder Serialisierungsfehler.
pub fn schreib_kontakte(pfad: &Path, store: &TrustStore, identity: &Identity) -> Ergebnis<()> {
    stelle_verzeichnis_sicher(pfad)?;
    let daten = kontakte_verschluesseln(store, identity)?;

    // Erst danebenschreiben, dann umbenennen. Ein Absturz mitten im Schreiben
    // darf nicht alle Kontakte vernichten.
    let temp = pfad.with_extension("tmp");
    std::fs::write(&temp, &daten).map_err(|e| Fehler::datei(&temp, e))?;
    std::fs::rename(&temp, pfad).map_err(|e| Fehler::datei(pfad, e))?;
    Ok(())
}

/// Verschlüsselt den Kontaktspeicher.
///
/// Reicht an `cabrik_core::trust::seal_store` weiter. Das Format steht dort,
/// weil `spec/trust-store.md` §6 es festlegt: Zwei Umsetzungen desselben
/// Formats laufen früher oder später auseinander, und dann liest die eine
/// nicht mehr, was die andere schreibt.
fn kontakte_verschluesseln(store: &TrustStore, identity: &Identity) -> Ergebnis<Vec<u8>> {
    Ok(trust::seal_store(store, identity, &mut OsRandom)?)
}

/// Liest den Kontaktspeicher.
fn kontakte_entschluesseln(daten: &[u8], identity: &Identity) -> Ergebnis<TrustStore> {
    Ok(trust::open_store(daten, identity)?)
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

        let daten = kontakte_verschluesseln(&store, &id).unwrap();
        let zurueck = kontakte_entschluesseln(&daten, &id).unwrap();

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
        let daten = kontakte_verschluesseln(&store, &alice).unwrap();

        assert_eq!(
            kontakte_entschluesseln(&daten, &mallory).unwrap_err().code(),
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

        let daten = kontakte_verschluesseln(&store, &id).unwrap();
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

        let a = kontakte_verschluesseln(&store, &id).unwrap();
        let b = kontakte_verschluesseln(&store, &id).unwrap();

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
        let mut daten = kontakte_verschluesseln(&TrustStore::new(), &id).unwrap();
        if let Some(b) = daten.get_mut(5) {
            *b ^= 0xFF;
        }
        assert!(kontakte_entschluesseln(&daten, &id).is_err());
    }

    #[test]
    fn fehlende_datei_ergibt_leeren_speicher() {
        let id = identitaet(0x11);
        let pfad = std::env::temp_dir().join("cabrik-gibt-es-nicht-12345.contacts");
        let store = lies_kontakte(&pfad, &id).unwrap();
        assert!(store.is_empty());
    }
}
