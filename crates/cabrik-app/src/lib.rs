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
    Bekannt, Bereinigung, Fassung, Identitaet, KdfStufe, Kontakt, Nutzlastbefund, Sendedatei,
    Sitzungsstand, Sperrfrist, Verifikationsweg,
};
use cabrik_core::Error;
use cabrik_core::fingerprint::{Fingerprint, safety_number};
use cabrik_core::keyfile::{self, Identity, KdfStufe as KernStufe};
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
    /// Worauf sich der Fehler bezieht.
    ///
    /// Diese Schicht kennt keine Pfade — sie sieht Bytes. Der Aufrufer
    /// kennt sie und kann sie ergänzen, aber nur, wenn er weiß, wovon die
    /// Rede ist. Die Meldung danach abzusuchen wäre die schlechtere
    /// Lösung: Sie ändert sich, sobald jemand einen Satz umformuliert.
    pub betrifft: Betroffen,
}

/// Worauf ein Fehler sich bezieht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Betroffen {
    /// Nichts Bestimmtes.
    Allgemein,
    /// Die Kontaktdatei — der Aufrufer darf ihren Pfad nennen.
    Kontaktspeicher,
}

impl Befehlsfehler {
    fn neu(meldung: &str) -> Self {
        Self {
            meldung: meldung.to_owned(),
            betrifft: Betroffen::Allgemein,
        }
    }

    fn wegen(meldung: &str, betrifft: Betroffen) -> Self {
        Self {
            meldung: meldung.to_owned(),
            betrifft,
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
            betrifft: Betroffen::Allgemein,
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

    /// Legt eine Identität an — und ist danach **offen**.
    ///
    /// # Warum nicht anschließend sperren
    ///
    /// Weil das Theater wäre. Wer gerade ein Passwort gesetzt hat, hat es
    /// eben getippt; ihn danach auf den Sperrbildschirm zu schicken,
    /// verlangt dieselbe Eingabe ein zweites Mal und schützt vor nichts.
    /// Die Frist beginnt in diesem Augenblick zu laufen.
    ///
    /// # Was diese Schicht nicht tut
    ///
    /// Schreiben. Die erzeugte Datei steht danach in [`Sitzung::schluesseldatei`];
    /// der Aufrufer legt sie ab — und zwar mit `cabrik_ablage::schreib_neu`,
    /// das sich weigert, eine bestehende zu überschreiben.
    ///
    /// # Fehler
    ///
    /// Wenn kein Zufall zu bekommen ist oder das Ableiten scheitert.
    pub fn anlegen<R: cabrik_core::Randomness>(
        bezeichnung: Option<String>,
        passwort: &Zeroizing<String>,
        mit_signierschluessel: bool,
        stufe: KdfStufe,
        frist: Sperrfrist,
        jetzt: u64,
        rng: &mut R,
    ) -> Befehlsergebnis<Self> {
        let mut identitaet = Identity::generate(rng, mit_signierschluessel, jetzt)?;
        identitaet.label = bezeichnung;

        let params = KernStufe::from(stufe).params();
        let schluesseldatei = keyfile::write(&identitaet, passwort.as_bytes(), &params, rng)?;

        let eigener = Fingerprint::compute(
            &identitaet.enc_pub()?,
            identitaet.sig_pub().as_ref(),
            Some(&identitaet.xwing_pub()),
        );

        Ok(Self {
            schluesseldatei,
            kontaktdatei: None,
            frist,
            offen: Some(Offen {
                identitaet,
                // Ein frisches Verzeichnis. Es gibt noch niemanden darin,
                // und das ist der Normalfall, kein Mangel.
                speicher: TrustStore::new(),
                eigener,
                letzte_handlung: jetzt,
            }),
        })
    }

    /// Die Schlüsseldatei, wie sie auf die Platte gehört.
    ///
    /// Verschlüsselt — ohne das Passwort ist daraus nichts zu gewinnen.
    #[must_use]
    pub fn schluesseldatei(&self) -> &[u8] {
        &self.schluesseldatei
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
                // Der Fall entsteht, wenn eine frühere Identität verschwand
                // und eine neue angelegt wurde, ohne dass der Speicher
                // mitging: Er ist an die alte versiegelt und **dauerhaft**
                // nicht mehr zu öffnen.
                //
                // Ohne den Pfad in der Meldung säße der Nutzer mit dem
                // richtigen Passwort vor einer verschlossenen Tür und
                // wüsste nicht, welche Datei im Weg liegt. Den Pfad kennt
                // diese Schicht nicht — sie sieht Bytes; deshalb sagt sie
                // dem Aufrufer, wovon die Rede ist.
                Befehlsfehler::wegen(
                    "Der Kontaktspeicher ließ sich nicht lesen. Er gehört zu \
                     einer anderen Identität oder ist beschädigt.",
                    Betroffen::Kontaktspeicher,
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

    /// Meldet, dass jemand gehandelt hat — Taste, Klick, Rollen.
    ///
    /// # Warum es diesen Weg überhaupt gibt
    ///
    /// Die Befehle allein reichen nicht. Wer zehn Minuten an einer langen
    /// Nachricht schreibt, ruft in dieser Zeit keinen einzigen auf — und
    /// säße plötzlich vor dem Sperrbildschirm, obwohl er die ganze Zeit vor
    /// dem Rechner saß. Tätigkeit im Fenster erreicht diese Schicht sonst
    /// nie.
    ///
    /// **Bloße Mausbewegung zählt nicht** (`spec/entsperrung.md` §9.2). Eine
    /// verschobene Maus sagt nichts darüber, ob noch jemand da ist; ein
    /// Ärmel oder ein ruckelnder Tisch genügt.
    ///
    /// **Im gesperrten Zustand ohne Wirkung.** Sonst hielte Tippen auf dem
    /// Sperrbildschirm die Frist offen, obwohl niemand angemeldet ist. Und
    /// eine Meldung, die nach Fristablauf eintrifft, weckt nichts auf: Die
    /// Prüfung läuft zuerst.
    pub fn taetigkeit(&mut self, jetzt: u64) {
        self.sperre_pruefen(jetzt);
        if let Some(o) = self.offen.as_mut() {
            o.letzte_handlung = jetzt;
        }
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
    /// Die eigene Identität, so wie die Oberfläche sie zeigen darf.
    ///
    /// **Auf `Offen` und nicht auf `Sitzung`** — aus demselben Grund wie
    /// die Kontaktbefehle, aber hier mit einer zusätzlichen Pointe: Die
    /// Bezeichnung steht im *verschlüsselten* Teil der Datei. Ohne Passwort
    /// liegt sie niemandem vor, auch uns nicht. Der Sperrbildschirm kann
    /// also gar nicht verraten, wessen Rechner das ist
    /// (`spec/entsperrung.md` §4.1) — das ist keine Zurückhaltung der
    /// Anzeige, sondern eine Eigenschaft des Formats.
    ///
    /// `pfad` kommt von außen: Diese Schicht fasst kein Dateisystem an und
    /// weiß deshalb nicht, wo die Datei liegt.
    ///
    /// # Fehler
    ///
    /// Wenn sich aus der Identität kein öffentlicher Schlüssel ableiten
    /// lässt, oder der Kopf der Datei nicht lesbar ist.
    pub fn identitaet(&self, schluesseldatei: &[u8], pfad: String) -> Befehlsergebnis<Identitaet> {
        let params = keyfile::params_of(schluesseldatei)?;
        Ok(Identitaet {
            bezeichnung: self.identitaet.label.clone(),
            fingerprint: self.eigener.display_full(),
            // `short()` und keine eigene Verkuerzung: Der Kern haelt fest,
            // dass diese Form nur zum Unterscheiden in Listen taugt und
            // **nie** Grundlage einer Verifikation sein darf. Wer hier
            // selbst abschneidet, verliert diese Zusicherung.
            fingerprint_kurz: self.eigener.short(),
            erzeugt_am: self.identitaet.created,
            kdf: KernStufe::von_params(&params).map(Into::into),
            // Aufgerundet waere geschmeichelt: 200_000 KiB sind nicht 196
            // MiB, sondern 195,3 -- und die Zahl soll die Wahrheit sein.
            kdf_speicher_mib: params.m_cost / 1024,
            hat_signierschluessel: self.identitaet.sig_pub().is_some(),
            // Ab v2 Pflicht. `false` kaeme nur bei einer Uebernahme aus v1
            // vor -- und dann ist es eine Aussage, keine Nachlaessigkeit.
            hat_post_quantum: true,
            pfad,
        })
    }

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

// ---------------------------------------------------------------------------
// Dateien ansehen, bevor etwas geschieht
// ---------------------------------------------------------------------------

/// Was über eine Datei zu sagen ist, **bevor** irgendetwas geschrieben wird.
///
/// # Warum die Bereinigung wirklich läuft
///
/// Weil eine Vorhersage, die nicht der Vorgang selbst ist, ihm irgendwann
/// davonläuft. Diese Funktion ruft `strip` auf und wirft das Ergebnis weg;
/// was die Anzeige zeigt, ist damit **genau** das, was beim Senden
/// geschieht — nicht eine zweite Einschätzung derselben Frage, die beim
/// nächsten Formatzusatz stehenbleibt.
///
/// Der Preis ist eine zusätzliche Runde über die Bytes. Bei einem Stapel
/// aus vierzig Bildern ist das spürbar, und es ist den Preis wert: Der
/// Bildschirm, auf dem jemand entscheidet, ob er etwas verschickt, darf
/// nicht raten.
///
/// # Warum es keine Sitzung braucht
///
/// Weil hier nichts Geheimes vorkommt. Metadaten einer Datei zu lesen hat
/// mit der Identität nichts zu tun — und diese Funktion nicht an `Offen` zu
/// hängen, hält sie aus dem Weg der Sperre heraus.
///
/// # Was sie nicht tut
///
/// Dateien lesen. Sie bekommt Bytes; wer sie holt, weiß, wo sie liegen.
#[must_use]
pub fn datei_pruefen(pfad: &str, name: &str, daten: &[u8]) -> Sendedatei {
    // Das Format kommt aus der Erkennung, nicht aus dem Bereinigungs-
    // ergebnis: `StripResult::Complete` führt es nicht mit, die Anzeige
    // braucht es aber zwingend (`spec/anzeige.md` §4.1).
    let format = cabrik_metadata::inspect(daten)
        .ok()
        .and_then(|i| i.format)
        .unwrap_or_else(|| "unbekannt".to_owned());

    let befund = match cabrik_metadata::strip(daten) {
        Ok((_sauber, ergebnis)) => Bereinigung::aus(&ergebnis, &format),
        // Nicht `Unbekannt`: „Format nicht verstanden" ist keine Aussage
        // über die Datei, „ließ sich nicht lesen" ist eine.
        Err(e) => Bereinigung::Fehler {
            grund: e.to_string(),
        },
    };

    Sendedatei {
        pfad: pfad.to_owned(),
        name: name.to_owned(),
        groesse_bytes: daten.len(),
        befund,
        // Nur PDF trägt Fassungen. Bei allem anderen ist die leere Liste
        // die richtige Aussage und kein fehlendes Ergebnis.
        fassungen: fassungen_von(daten),
    }
}

/// Frühere Fassungen eines PDF, sofern es eins ist.
///
/// Ein Fehlschlag ergibt eine leere Liste und **keine** Meldung: Er heißt
/// nur, dass sich der Änderungsverlauf nicht lesen ließ. Das ist der
/// Normalfall für jedes Format außer PDF.
fn fassungen_von(daten: &[u8]) -> Vec<Fassung> {
    cabrik_metadata::pdf::fassungen(daten, None)
        .map(|f| f.iter().map(Fassung::from).collect())
        .unwrap_or_default()
}
