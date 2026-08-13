//! Die Befehle, die die Oberfläche auslöst.
//!
//! # Warum ohne Tauri
//!
//! Dieselbe Reihenfolge wie im Frontend, und aus demselben Grund
//! (Leitprinzip 2): Erst die Befehle, geprüft und lauffähig, dann die
//! Hülle darum. Ein `#[tauri::command]` ist danach eine Zeile über einer
//! Funktion, die bereits tut, was sie soll — statt einer Funktion, die
//! zugleich neu ist und in einer neuen Umgebung läuft.
//!
//! Diese Schicht kennt Tauri nicht. Sie lässt sich mit `cargo test`
//! ausführen, ohne Fenster, ohne Webansicht, ohne Ereignisschleife.
//!
//! # Was sie ist
//!
//! Die Gegenseite von `kern/bruecke.ts`. Jede Methode hier entspricht
//! genau einer dort, und beide geben dieselben Typen aus
//! `cabrik-bruecke` heraus — die durch Prüfmuster festgenagelt sind.
//!
//! # Was sie nie herausgibt
//!
//! Schlüsselmaterial. Die Rückgabetypen stammen sämtlich aus
//! `cabrik-bruecke`, und dort gibt es kein Feld dafür. Ein Passwort geht in
//! die **andere** Richtung: Es kommt als Parameter herein, wird
//! durchgereicht und nirgends behalten.

#![forbid(unsafe_code)]

use cabrik_bruecke::{Kontakt, Verifikationsweg};
use cabrik_core::fingerprint::{Fingerprint, safety_number};
use cabrik_core::trust::{TrustStore, VerifiedVia};
use cabrik_core::Error;

/// Was schiefgehen kann — in Worten, die eine Oberfläche zeigen darf.
///
/// Der Kern gibt technische Fehler zurück. Die Oberfläche braucht Sätze,
/// die ein Mensch lesen kann, und zwar **ohne** die technische Ursache zu
/// verschweigen: Wer nur „Fehler" liest, kann nichts tun.
#[derive(Debug)]
pub struct Befehlsfehler {
    /// Was dem Nutzer gesagt wird.
    pub meldung: String,
}

impl core::fmt::Display for Befehlsfehler {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.meldung)
    }
}

impl core::error::Error for Befehlsfehler {}

impl From<Error> for Befehlsfehler {
    fn from(e: Error) -> Self {
        Self {
            meldung: e.to_string(),
        }
    }
}

/// Ergebnis eines Befehls.
pub type Befehlsergebnis<T> = core::result::Result<T, Befehlsfehler>;

// ---------------------------------------------------------------------------
// Sitzung
// ---------------------------------------------------------------------------

/// Was zwischen zwei Befehlen bestehen bleibt.
///
/// # Was hier fehlt
///
/// Ein Passwort. v1 hielt es dauerhaft im Klartext in seinem Zustand — der
/// schwerwiegendste Befund der ganzen Nachprüfung. Diese Sitzung hat kein
/// Feld dafür, und damit ist die Frage „wie lange halten wir es" nicht
/// beantwortet, sondern **weggefallen**.
///
/// Der eigene Fingerprint steht hier, weil die Safety Number eine
/// **paarweise** Ableitung ist: Ohne die eigene Identität gibt es keine.
pub struct Sitzung {
    speicher: TrustStore,
    eigener: Fingerprint,
}

impl Sitzung {
    /// Eine Sitzung über einem geladenen Speicher.
    #[must_use]
    pub const fn neu(speicher: TrustStore, eigener: Fingerprint) -> Self {
        Self { speicher, eigener }
    }

    /// Der Speicher, für den Aufrufer, der ihn sichern muss.
    #[must_use]
    pub const fn speicher(&self) -> &TrustStore {
        &self.speicher
    }

    // -----------------------------------------------------------------------
    // Kontakte
    // -----------------------------------------------------------------------

    /// Alle Kontakte, wie die Oberfläche sie sieht.
    #[must_use]
    pub fn kontakte(&self) -> Vec<Kontakt> {
        self.speicher
            .contacts()
            .iter()
            .map(|k| Kontakt::aus(k, self.nummer_zu(k)))
            .collect()
    }

    /// Nimmt einen Kontakt auf — **immer als `gesehen`**.
    ///
    /// Es gibt keinen Parameter, mit dem sich das umgehen ließe. Wer eine
    /// Austausch-Nutzlast einliest, hat sie erhalten, nicht geprüft; diese
    /// Unterscheidung an der ersten Stelle aufzuweichen machte sie überall
    /// wertlos.
    pub fn kontakt_aufnehmen(
        &mut self,
        name: &str,
        enc_pub: [u8; 32],
        sig_pub: Option<[u8; 32]>,
        xwing_pub: Option<Box<[u8; cabrik_core::trust::PQ_PUB_LEN]>>,
        jetzt: u64,
    ) -> Befehlsergebnis<Kontakt> {
        let kontakt =
            cabrik_core::trust::Contact::new_seen(name, enc_pub, sig_pub, xwing_pub, jetzt)?;
        let fp = kontakt.fingerprint();
        self.speicher.add(kontakt)?;
        self.finde(&fp)
    }

    /// Markiert einen Kontakt als verifiziert, mit dem benutzten Weg.
    pub fn kontakt_verifizieren(
        &mut self,
        fingerprint: &str,
        weg: Verifikationsweg,
        jetzt: u64,
    ) -> Befehlsergebnis<Kontakt> {
        let fp = self.zeige_auf(fingerprint)?;
        let index = self.index_von(&fp)?;
        let kern_weg = match weg {
            Verifikationsweg::Qr => VerifiedVia::QrCode,
            Verifikationsweg::SafetyNumber => VerifiedVia::SafetyNumber,
            Verifikationsweg::Fingerprint => VerifiedVia::Fingerprint,
        };
        self.speicher
            .contacts_mut()
            .get_mut(index)
            .ok_or_else(|| Befehlsfehler {
                meldung: "Diesen Kontakt gibt es nicht mehr.".to_owned(),
            })?
            .verify(kern_weg, jetzt)?;
        self.finde(&fp)
    }

    /// Markiert einen Schlüssel lokal als kompromittiert.
    pub fn kontakt_widerrufen(
        &mut self,
        fingerprint: &str,
        jetzt: u64,
        grund: Option<&str>,
    ) -> Befehlsergebnis<Kontakt> {
        let fp = self.zeige_auf(fingerprint)?;
        let index = self.index_von(&fp)?;
        self.speicher
            .contacts_mut()
            .get_mut(index)
            .ok_or_else(|| Befehlsfehler {
                meldung: "Diesen Kontakt gibt es nicht mehr.".to_owned(),
            })?
            .revoke(jetzt, grund)?;
        self.finde(&fp)
    }

    /// Entfernt einen Kontakt aus dem Verzeichnis.
    ///
    /// **Nicht dasselbe wie widerrufen.** Widerrufen lässt den Eintrag
    /// stehen und warnt künftig; Löschen entfernt ihn **und mit ihm die
    /// Warnung**. Wer einen verdächtigen Schlüssel löscht, sieht ihn beim
    /// nächsten Mal als unbekannten Absender wieder.
    pub fn kontakt_loeschen(&mut self, fingerprint: &str) -> Befehlsergebnis<()> {
        let fp = self.zeige_auf(fingerprint)?;
        let index = self.index_von(&fp)?;
        self.speicher.remove(index)?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Kleinkram
    // -----------------------------------------------------------------------

    /// Die Safety Number gegenüber der eigenen Identität.
    fn nummer_zu(&self, k: &cabrik_core::trust::Contact) -> String {
        safety_number(&self.eigener, &k.fingerprint())
    }

    /// Löst eine Anzeigeform des Fingerprints auf einen echten auf.
    ///
    /// Die Oberfläche schickt zurück, was sie angezeigt bekommen hat — also
    /// die Fassung mit Leerzeichen. Sie zu vergleichen, statt sie zu
    /// zerlegen, spart eine Umkehrfunktion, die nur eine weitere Stelle
    /// wäre, an der etwas auseinanderlaufen kann.
    fn zeige_auf(&self, anzeige: &str) -> Befehlsergebnis<Fingerprint> {
        self.speicher
            .contacts()
            .iter()
            .map(cabrik_core::trust::Contact::fingerprint)
            .find(|fp| fp.display_full() == anzeige)
            .ok_or_else(|| Befehlsfehler {
                meldung: format!("Kein Kontakt mit dem Fingerprint {anzeige}."),
            })
    }

    fn index_von(&self, fp: &Fingerprint) -> Befehlsergebnis<usize> {
        self.speicher
            .contacts()
            .iter()
            .position(|k| &k.fingerprint() == fp)
            .ok_or_else(|| Befehlsfehler {
                meldung: "Diesen Kontakt gibt es nicht mehr.".to_owned(),
            })
    }

    fn finde(&self, fp: &Fingerprint) -> Befehlsergebnis<Kontakt> {
        self.speicher
            .find_by_fingerprint(fp)
            .map(|k| Kontakt::aus(k, self.nummer_zu(k)))
            .ok_or_else(|| Befehlsfehler {
                meldung: "Diesen Kontakt gibt es nicht mehr.".to_owned(),
            })
    }
}
