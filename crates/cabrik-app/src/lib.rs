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
//! # Wie die Sperre erzwungen wird
//!
//! **Über den Typ, nicht über eine Prüfung.** Die Befehle, die Kontakte
//! anfassen, stehen nicht auf [`Sitzung`], sondern auf [`Offen`] — und an
//! ein `&mut Offen` kommt man nur durch [`Sitzung::offen`], das vorher die
//! Frist prüft und gegebenenfalls sperrt.
//!
//! Eine Prüfung am Anfang jeder Methode täte scheinbar dasselbe. Sie wäre
//! aber beim nächsten hinzugefügten Befehl zu vergessen, und niemandem
//! fiele es auf. Hier kann man sie nicht vergessen: Ohne den Weg über
//! `offen` gibt es den Empfänger gar nicht.
//!
//! # Was nie herausgeht
//!
//! Schlüsselmaterial. Die Rückgabetypen stammen sämtlich aus
//! `cabrik-bruecke`, und dort gibt es kein Feld dafür. Das Passwort geht in
//! die **andere** Richtung: Es kommt als `Zeroizing<String>` herein, wird
//! an `keyfile::read` gereicht und danach fallengelassen — die Sitzung hat
//! kein Feld dafür (`spec/entsperrung.md` §2.1).

#![forbid(unsafe_code)]

use cabrik_bruecke::{
    Bekannt, Kontakt, Nutzlastbefund, Sitzungsstand, Sperrfrist, Verifikationsweg,
};
use cabrik_core::Error;
use cabrik_core::fingerprint::{Fingerprint, safety_number};
use cabrik_core::keyfile::{self, Identity};
use cabrik_core::trust::{self, TrustStore, VerifiedVia};
use zeroize::Zeroizing;

/// Was schiefgehen kann — in Worten, die eine Oberfläche zeigen darf.
///
/// Der Kern gibt technische Fehler zurück. Die Oberfläche braucht Sätze,
/// die ein Mensch lesen kann, und zwar **ohne** die technische Ursache zu
/// verschweigen: Wer nur „Fehler“ liest, kann nichts tun.
#[derive(Debug)]
pub struct Befehlsfehler {
    /// Was dem Nutzer gesagt wird.
    pub meldung: String,
}

impl Befehlsfehler {
    fn neu(meldung: &str) -> Self {
        Self {
            meldung: meldung.to_owned(),
        }
    }
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
/// Ein Passwort. Version 1 hielt es dauerhaft im Klartext in ihrem Zustand
/// — der schwerwiegendste Befund der Nachprüfung. Diese Sitzung hat kein
/// Feld dafür, und damit ist die Frage „wie lange halten wir es“ nicht
/// beantwortet, sondern **weggefallen**.
pub struct Sitzung {
    /// Die verschlüsselte Schlüsseldatei, wie sie auf der Platte liegt.
    schluesseldatei: Vec<u8>,
    /// Die verschlüsselte Kontaktdatei, sofern es schon eine gibt.
    kontaktdatei: Option<Vec<u8>>,
    frist: Sperrfrist,
    offen: Option<Offen>,
}

/// Der entsperrte Teil.
///
/// **Existiert nur, solange entsperrt ist.** Beim Sperren wird er
/// fallengelassen; `Identity` ist `ZeroizeOnDrop`, der Speicher also
/// überschrieben.
#[derive(Debug)]
pub struct Offen {
    identitaet: Identity,
    speicher: TrustStore,
    eigener: Fingerprint,
    /// Unix-Sekunden der letzten Handlung.
    letzte_handlung: u64,
}

impl Sitzung {
    /// Eine gesperrte Sitzung über den Dateien, wie sie auf der Platte liegen.
    #[must_use]
    pub const fn neu(
        schluesseldatei: Vec<u8>,
        kontaktdatei: Option<Vec<u8>>,
        frist: Sperrfrist,
    ) -> Self {
        Self {
            schluesseldatei,
            kontaktdatei,
            frist,
            offen: None,
        }
    }

    /// Entsperrt mit einem Passwort.
    ///
    /// Das Passwort kommt als [`Zeroizing<String>`] herein und wird nach
    /// dem Ableiten fallengelassen. Woher es stammt, weiß diese Funktion
    /// nicht — heute aus der Webansicht, später aus einem nativen Fenster
    /// (`spec/entsperrung.md` §5.2).
    ///
    /// # Fehler
    ///
    /// Ein falsches Passwort ergibt eine Meldung, die **nicht** sagt, wie
    /// falsch es war (§4.3).
    pub fn entsperren(
        &mut self,
        passwort: &Zeroizing<String>,
        jetzt: u64,
    ) -> Befehlsergebnis<()> {
        let identitaet = keyfile::read(&self.schluesseldatei, passwort.as_bytes())
            .map_err(|_| Befehlsfehler::neu("Das Passwort passt nicht."))?;

        // Der Kontaktspeicher hängt an der Identität: Ohne sie ist er nicht
        // lesbar (`spec/trust-store.md` §6). Gibt es noch keine Datei, ist
        // ein leeres Verzeichnis richtig -- und kein Fehler.
        let speicher = match &self.kontaktdatei {
            Some(daten) => trust::open_store(daten, &identitaet).map_err(|_| {
                Befehlsfehler::neu(
                    "Der Kontaktspeicher ließ sich nicht lesen. Er gehört zu \
                     einer anderen Identität oder ist beschädigt.",
                )
            })?,
            None => TrustStore::new(),
        };

        let eigener = Fingerprint::compute(
            &identitaet.enc_pub()?,
            identitaet.sig_pub().as_ref(),
            Some(&identitaet.xwing_pub()),
        );

        self.offen = Some(Offen {
            identitaet,
            speicher,
            eigener,
            letzte_handlung: jetzt,
        });
        Ok(())
    }

    /// Sperrt sofort.
    ///
    /// Fallenlassen genügt: `Identity` ist `ZeroizeOnDrop`.
    pub fn sperren(&mut self) {
        self.offen = None;
    }

    /// Ob gerade gesperrt ist.
    #[must_use]
    pub const fn ist_gesperrt(&self) -> bool {
        self.offen.is_none()
    }

    /// Stellt die Frist ein.
    ///
    /// Sperrt dabei nicht: Wer von einer Stunde auf eine Minute wechselt,
    /// hat gerade gehandelt — die Messung beginnt von vorn.
    pub fn frist_setzen(&mut self, frist: Sperrfrist, jetzt: u64) {
        self.frist = frist;
        if let Some(o) = self.offen.as_mut() {
            o.letzte_handlung = jetzt;
        }
    }

    /// Der entsperrte Teil — **der einzige Weg zu den Kontaktbefehlen**.
    ///
    /// Prüft zuerst die Frist und sperrt, wenn sie abgelaufen ist. Damit
    /// kann kein Befehl die Sperre umgehen: Ohne diesen Weg gibt es den
    /// Empfänger gar nicht.
    ///
    /// # Fehler
    ///
    /// Wenn gesperrt ist — auch wenn dieser Aufruf es gerade ausgelöst hat.
    pub fn offen(&mut self, jetzt: u64) -> Befehlsergebnis<&mut Offen> {
        self.sperre_pruefen(jetzt);
        let Some(o) = self.offen.as_mut() else {
            return Err(Befehlsfehler::neu(
                "Die Sitzung ist gesperrt. Geben Sie Ihr Passwort ein.",
            ));
        };
        o.letzte_handlung = jetzt;
        Ok(o)
    }

    /// Wie es um die Sitzung steht.
    ///
    /// Prüft dabei ebenfalls die Frist — sonst zeigte die Oberfläche
    /// „entsperrt“ an, bis jemand etwas anfasst.
    pub fn stand(&mut self, jetzt: u64) -> Sitzungsstand {
        self.sperre_pruefen(jetzt);
        Sitzungsstand {
            gesperrt: self.ist_gesperrt(),
            frist: self.frist,
            restsekunden: self.restsekunden(jetzt),
        }
    }

    /// Sperrt, wenn die Frist abgelaufen ist.
    ///
    /// **Setzt die Messung nicht zurück.** Diese Prüfung ist keine Handlung
    /// des Nutzers; sonst hielte allein das Nachfragen die Sitzung offen.
    fn sperre_pruefen(&mut self, jetzt: u64) {
        let Some(grenze) = self.frist.sekunden() else {
            return;
        };
        let abgelaufen = self
            .offen
            .as_ref()
            .is_some_and(|o| jetzt.saturating_sub(o.letzte_handlung) >= grenze);
        if abgelaufen {
            self.sperren();
        }
    }

    /// Sekunden bis zur Sperre.
    fn restsekunden(&self, jetzt: u64) -> Option<u64> {
        let grenze = self.frist.sekunden()?;
        let o = self.offen.as_ref()?;
        Some(grenze.saturating_sub(jetzt.saturating_sub(o.letzte_handlung)))
    }

    /// Der Kontaktspeicher, verschlüsselt für die Ablage.
    ///
    /// Der Aufrufer schreibt ihn — diese Schicht fasst kein Dateisystem an.
    ///
    /// # Fehler
    ///
    /// Wenn gesperrt ist: Ohne Identität lässt sich nichts verschlüsseln.
    pub fn kontakte_sichern<R: cabrik_core::Randomness>(
        &mut self,
        jetzt: u64,
        rng: &mut R,
    ) -> Befehlsergebnis<Vec<u8>> {
        let o = self.offen(jetzt)?;
        Ok(trust::seal_store(&o.speicher, &o.identitaet, rng)?)
    }
}

// ---------------------------------------------------------------------------
// Die Befehle — nur im entsperrten Zustand erreichbar
// ---------------------------------------------------------------------------

impl Offen {
    /// Alle Kontakte, wie die Oberfläche sie sieht.
    #[must_use]
    pub fn kontakte(&self) -> Vec<Kontakt> {
        self.speicher
            .contacts()
            .iter()
            .map(|k| Kontakt::aus(k, self.nummer_zu(k)))
            .collect()
    }

    /// Liest eine Austausch-Nutzlast, **ohne** etwas aufzunehmen.
    ///
    /// Getrennt vom Aufnehmen, weil es zwei Vorgänge sind: erst ansehen,
    /// was drinsteht, dann entscheiden.
    #[must_use]
    pub fn nutzlast_lesen(&self, nutzlast: &str) -> Nutzlastbefund {
        let gelesen = match trust::parse_qr(nutzlast.trim()) {
            Ok(q) => q,
            Err(trust::QrFehler::Beschaedigt) => {
                return Nutzlastbefund::Beschaedigt {
                    grund: "Es ist erkennbar eine Cabrik-Nutzlast, aber sie \
                            lässt sich nicht lesen. Beim Kopieren ist etwas \
                            verlorengegangen, oder ein Mailprogramm hat einen \
                            Zeilenumbruch eingefügt."
                        .to_owned(),
                };
            }
            Err(_) => {
                return Nutzlastbefund::Unlesbar {
                    grund: "Das ist keine Cabrik-Austausch-Nutzlast. Sie \
                            beginnt mit `cabrik:v2:` und ist rund 2050 \
                            Zeichen lang."
                        .to_owned(),
                };
            }
        };

        // Der Fingerprint entsteht aus den Schlüsseln, nicht aus der
        // Prüfsumme in der Nutzlast (`spec/trust-store.md` §5.1).
        let fp = Fingerprint::compute(
            &gelesen.enc_pub,
            gelesen.sig_pub.as_ref(),
            gelesen.xwing_pub.as_deref(),
        );

        Nutzlastbefund::Gelesen {
            fingerprint: fp.display_full(),
            hat_signierschluessel: gelesen.sig_pub.is_some(),
            hat_post_quantum: gelesen.xwing_pub.is_some(),
            schon_bekannt: self.speicher.find_by_fingerprint(&fp).map(|k| Bekannt {
                name: k.name.clone(),
                gleicher_schluessel: true,
            }),
        }
    }

    /// Nimmt einen Kontakt aus einer Austausch-Nutzlast auf.
    ///
    /// **Immer als `gesehen`.** Es gibt keinen Parameter, mit dem sich das
    /// umgehen ließe: Wer eine Nutzlast einliest, hat sie erhalten, nicht
    /// geprüft.
    pub fn kontakt_aus_nutzlast(
        &mut self,
        name: &str,
        nutzlast: &str,
        jetzt: u64,
    ) -> Befehlsergebnis<Kontakt> {
        let gelesen = trust::parse_qr(nutzlast.trim()).map_err(|e| match e {
            trust::QrFehler::Beschaedigt => Befehlsfehler::neu(
                "Die Austausch-Nutzlast ist beschädigt angekommen. Lassen Sie \
                 sie sich noch einmal schicken.",
            ),
            _ => Befehlsfehler::neu("Das ist keine Cabrik-Austausch-Nutzlast."),
        })?;

        let kontakt = trust::Contact::new_seen(
            name,
            gelesen.enc_pub,
            gelesen.sig_pub,
            gelesen.xwing_pub,
            jetzt,
        )?;
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
        let kern_weg = match weg {
            Verifikationsweg::Qr => VerifiedVia::QrCode,
            Verifikationsweg::SafetyNumber => VerifiedVia::SafetyNumber,
            Verifikationsweg::Fingerprint => VerifiedVia::Fingerprint,
        };
        self.aendern(fingerprint, |k| k.verify(kern_weg, jetzt))
    }

    /// Nimmt eine Verifikation zurück — der Kontakt gilt wieder als
    /// **gesehen**.
    ///
    /// Für den misslungenen Vergleich. **Nicht widerrufen:** Das hieße
    /// „dieser Schlüssel ist kompromittiert“, und das weiß niemand.
    pub fn kontakt_zuruecksetzen(&mut self, fingerprint: &str) -> Befehlsergebnis<Kontakt> {
        self.aendern(fingerprint, trust::Contact::unverify)
    }

    /// Markiert einen Schlüssel lokal als kompromittiert.
    pub fn kontakt_widerrufen(
        &mut self,
        fingerprint: &str,
        jetzt: u64,
        grund: Option<&str>,
    ) -> Befehlsergebnis<Kontakt> {
        self.aendern(fingerprint, |k| k.revoke(jetzt, grund))
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

    /// Wendet eine Änderung auf einen Kontakt an und gibt ihn zurück.
    fn aendern<F>(&mut self, fingerprint: &str, tun: F) -> Befehlsergebnis<Kontakt>
    where
        F: FnOnce(&mut trust::Contact) -> cabrik_core::Result<()>,
    {
        let fp = self.zeige_auf(fingerprint)?;
        let index = self.index_von(&fp)?;
        let k = self
            .speicher
            .contacts_mut()
            .get_mut(index)
            .ok_or_else(|| Befehlsfehler::neu("Diesen Kontakt gibt es nicht mehr."))?;
        tun(k)?;
        self.finde(&fp)
    }

    /// Die Safety Number gegenüber der eigenen Identität.
    fn nummer_zu(&self, k: &trust::Contact) -> String {
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
            .map(trust::Contact::fingerprint)
            .find(|fp| fp.display_full() == anzeige)
            .ok_or_else(|| {
                Befehlsfehler::neu(&format!("Kein Kontakt mit dem Fingerprint {anzeige}."))
            })
    }

    fn index_von(&self, fp: &Fingerprint) -> Befehlsergebnis<usize> {
        self.speicher
            .contacts()
            .iter()
            .position(|k| &k.fingerprint() == fp)
            .ok_or_else(|| Befehlsfehler::neu("Diesen Kontakt gibt es nicht mehr."))
    }

    fn finde(&self, fp: &Fingerprint) -> Befehlsergebnis<Kontakt> {
        self.speicher
            .find_by_fingerprint(fp)
            .map(|k| Kontakt::aus(k, self.nummer_zu(k)))
            .ok_or_else(|| Befehlsfehler::neu("Diesen Kontakt gibt es nicht mehr."))
    }
}
