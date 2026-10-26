//! Wo Dateien liegen und wie der Kontaktspeicher verschlüsselt wird.
//!
//! Der Kern kennt bewusst keine Dateien (`crates/README.md`). Alles, was mit
//! Pfaden und Datenträgern zu tun hat, steht deshalb hier.

use crate::fehler::{Ergebnis, Fehler};

use cabrik_core::keyfile::{self, KdfParams};
use cabrik_core::trust::{self, TrustStore};
use cabrik_core::{Identity, OsRandom, Speichergrenze};

use std::path::{Path, PathBuf};

/// Was eine Passwortableitung auf diesem Rechner darf
/// (`spec/keyfile-v2.md` §4.2, `spec/entsperrung.md` §4.5).
///
/// # Warum ein Schalter und kein Dialog
///
/// Das Fenster fragt nach, wenn eine Datei ungewöhnlich viel verlangt. Eine
/// Befehlszeile kann das nicht verlässlich — sie läuft auch in Skripten, und
/// die Standardeingabe ist womöglich schon für das Passwort belegt. Hier ist
/// die Rückfrage deshalb ein ausdrücklicher Schalter, wie `--confirm` beim
/// Löschen: `--allow-unusual-kdf`.
#[derive(Debug, Clone, Copy)]
pub struct Ableitung {
    /// Wie viel Arbeitsspeicher die Ableitung belegen darf.
    pub grenze: Speichergrenze,
    /// Ob ungewöhnliche Werte ausdrücklich zugelassen sind.
    pub ungewoehnliche_erlaubt: bool,
}

impl Ableitung {
    /// Die Grenze dieses Rechners, in diesem Augenblick gefragt.
    #[must_use]
    pub fn hier(ungewoehnliche_erlaubt: bool) -> Self {
        Self {
            grenze: cabrik_speicher::ableitungsgrenze()
                .map_or(Speichergrenze::UNBEKANNT, Speichergrenze::bytes),
            ungewoehnliche_erlaubt,
        }
    }

    /// Ob mit diesen Werten abgeleitet werden darf — **bevor** jemand nach
    /// dem Passwort gefragt wird.
    ///
    /// # Fehler
    ///
    /// Bei ungewöhnlichen Werten ohne `--allow-unusual-kdf`, oder wenn die
    /// Ableitung nicht in den Speicher passt.
    pub fn pruefe(&self, params: &KdfParams) -> Ergebnis<()> {
        if params.ist_ungewoehnlich() && !self.ungewoehnliche_erlaubt {
            // Die Vergleichswerte aus dem Kern, nicht abgeschrieben: Wird die
            // staerkste Stufe je angehoben, stuende hier sonst die alte Zahl.
            let stark = keyfile::KdfStufe::Stark.params();
            return Err(Fehler::bedienung(format!(
                "Diese Datei verlangt ungewöhnlich viel: {} MiB Arbeitsspeicher und {} \
                 Durchgänge.\n\
                 Cabrik selbst schreibt höchstens {} MiB mit {} Durchgängen. Stammt die \
                 Datei\n\
                 von Ihnen oder aus einer anderen Anwendung, öffnen Sie sie mit\n  \
                 --allow-unusual-kdf\n\
                 Wenn nicht, ist sie womöglich verändert worden — dann schlägt das \
                 Öffnen ohnehin\n\
                 fehl, nur eben erst nach der langen Ableitung.",
                params.m_cost / 1024,
                params.t_cost,
                stark.m_cost / 1024,
                stark.t_cost
            )));
        }
        params.pruefe_speicher(self.grenze)?;
        Ok(())
    }
}

/// Voreingestellter Pfad des Keyfiles.
///
/// # Fehler
///
/// Siehe [`verzeichnis`].
pub fn keyfile_pfad(angabe: Option<&Path>) -> Ergebnis<PathBuf> {
    cabrik_ablage::keyfile_pfad(angabe).map_err(|e| Fehler::bedienung(&e.meldung))
}

/// Voreingestellter Pfad des Kontaktspeichers.
///
/// # Fehler
///
/// Siehe [`verzeichnis`].
pub fn kontakte_pfad(angabe: Option<&Path>) -> Ergebnis<PathBuf> {
    cabrik_ablage::kontakte_pfad(angabe).map_err(|e| Fehler::bedienung(&e.meldung))
}

/// Legt das Verzeichnis an, falls nötig.
///
/// # Fehler
///
/// Dateisystemfehler.
pub fn stelle_verzeichnis_sicher(pfad: &Path) -> Ergebnis<()> {
    cabrik_ablage::verzeichnis_sicherstellen(pfad).map_err(|e| Fehler::bedienung(&e.meldung))
}

// ---------------------------------------------------------------------------
// Keyfile
// ---------------------------------------------------------------------------

/// Prüft ein Keyfile, **bevor** nach dem Passwort gefragt wird: Ist es
/// lesbar, sind seine Werte zugelassen, passt die Ableitung in den
/// Speicher? Wer das erst nach dem Tippen erfährt, hat umsonst getippt.
///
/// # Fehler
///
/// Wie [`lies_keyfile`], nur ohne den Fall „falsches Passwort“.
pub fn pruefe_keyfile(pfad: &Path, ableitung: &Ableitung) -> Ergebnis<()> {
    let daten = std::fs::read(pfad).map_err(|e| Fehler::datei(pfad, e))?;
    vor_der_ableitung(pfad, &daten, ableitung)
}

/// Liest und entsperrt ein Keyfile.
///
/// # Fehler
///
/// - Dateizugriff
/// - ungewöhnliche Werte ohne `--allow-unusual-kdf`
/// - [`cabrik_core::Error::InsufficientMemory`], wenn die Ableitung nicht in
///   den Speicher passt
/// - [`cabrik_core::Error::KeyfileAuthFailed`] bei falschem Passwort
pub fn lies_keyfile(pfad: &Path, passwort: &[u8], ableitung: &Ableitung) -> Ergebnis<Identity> {
    let daten = std::fs::read(pfad).map_err(|e| Fehler::datei(pfad, e))?;
    vor_der_ableitung(pfad, &daten, ableitung)?;
    Ok(keyfile::read(&daten, passwort, ableitung.grenze)?)
}

/// Was sich am Klartextkopf prüfen lässt, ohne Passwort.
fn vor_der_ableitung(pfad: &Path, daten: &[u8], ableitung: &Ableitung) -> Ergebnis<()> {
    // Ein v1-Keyfile hier freundlich abfangen: Der Nutzer bekommt sonst nur
    // „beschädigt" zu sehen und weiß nicht, dass es einen Weg gibt.
    if cabrik_v1::keyfile::looks_like_v1(daten) {
        return Err(Fehler::bedienung(format!(
            "{} ist ein Schlüssel aus Version 1. Zum Übernehmen:\n  \
             cabrik migrate \"{}\" --out <neue-datei>",
            pfad.display(),
            pfad.display()
        )));
    }
    ableitung.pruefe(&keyfile::params_of(daten)?)
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
    params: &KdfParams,
    grenze: Speichergrenze,
) -> Ergebnis<()> {
    if pfad.exists() {
        return Err(Fehler::bedienung(format!(
            "{} existiert bereits. Ein überschriebener Schlüssel ist unwiederbringlich —\n\
             bitte einen anderen Pfad wählen oder die Datei vorher wegsichern.",
            pfad.display()
        )));
    }
    stelle_verzeichnis_sicher(pfad)?;
    let daten = keyfile::write(identity, passwort, params, grenze, &mut OsRandom)?;
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
    let daten = kontakte_verschluesseln(store, identity)?;
    cabrik_ablage::schreib_atomar(pfad, &daten).map_err(|e| Fehler::bedienung(&e.meldung))?;
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
            kontakte_entschluesseln(&daten, &mallory)
                .unwrap_err()
                .code(),
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
