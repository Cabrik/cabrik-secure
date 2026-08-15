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
    Absender, Bekannt, Bereinigung, Fassung, Geoeffnet, Identitaet, Inhaltsart, KdfStufe, Kontakt,
    Nutzlastbefund, Sendedatei, Sitzungsstand, Sperrfrist, Verifikationsweg, Versandergebnis,
};
use cabrik_core::Error;
use cabrik_core::fingerprint::{Fingerprint, safety_number};
use cabrik_core::keyfile::{self, Identity, KdfStufe as KernStufe};
use cabrik_core::Suite;
use cabrik_core::envelope::{self, ContentType, SealOptions};
use cabrik_core::trust::{self, TrustState, TrustStore, VerifiedVia};
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

/// Prüft ein neu gewähltes Schlüsselpasswort — an einer Stelle, für beide
/// Türen.
///
/// Die Meldung nennt die Zahl **und** den Grund. „Zu kurz" allein klänge
/// nach Schikane; mit dem Grund ist es eine Auskunft.
fn passwort_pruefen(passwort: &[u8]) -> Befehlsergebnis<()> {
    cabrik_core::passwort::pruefe(passwort, cabrik_core::passwort::MIN_SCHLUESSEL).map_err(
        |_| {
            Befehlsfehler::neu(&format!(
                "Das Passwort muss mindestens {} Zeichen haben. Erst ab dieser \
                 Länge ist ein reines Durchprobieren aller Zeichenfolgen \
                 aussichtslos — gegen ein erratbares Passwort hilft sie nicht.",
                cabrik_core::passwort::MIN_SCHLUESSEL
            ))
        },
    )
}

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
    /// Der zuletzt geöffnete Klartext — **verlässt Rust nie**.
    ///
    /// Er liegt hier, weil er irgendwo liegen muss: Zwischen „öffnen“ und
    /// „speichern unter“ liegt ein Dateidialog, und solange steht der
    /// entschlüsselte Inhalt im Speicher. Ihn stattdessen über die Brücke
    /// zu reichen hieße, ihn in eine Webansicht zu legen, die wir weder
    /// überschreiben noch begrenzen können.
    ///
    /// `Zeroizing`, und er geht mit der Sperre: `Offen` wird beim Sperren
    /// fallengelassen, und mit ihm dieser Puffer.
    nutzlast: Option<Nutzlast>,
}

/// Was von einem geöffneten Envelope im Speicher bleibt.
#[derive(Debug)]
struct Nutzlast {
    inhalt: Zeroizing<Vec<u8>>,
    dateiname: Option<String>,
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
        passwort_pruefen(passwort.as_bytes())?;

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
                nutzlast: None,
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
            nutzlast: None,
        });
        Ok(())
    }

    /// Ändert das Passwort. **Die Identität bleibt dieselbe.**
    ///
    /// # Was sich dabei nicht ändert
    ///
    /// Der Schlüssel. Es wird nur die Hülle neu verschlossen: derselbe
    /// Fingerprint, dieselben Kontakte, dieselben alten Envelopes gehen
    /// weiter auf. Wer einen **neuen Schlüssel** will, legt eine neue
    /// Identität an — und muss dann allen den neuen Fingerprint geben.
    ///
    /// Das ist die Erwartung, die am häufigsten danebenliegt, und sie
    /// gehört ausgesprochen: Ein geändertes Passwort schützt nicht davor,
    /// dass jemand den privaten Schlüssel schon hat.
    ///
    /// # Warum das alte Passwort verlangt wird, obwohl entsperrt ist
    ///
    /// Weil „entsperrt" nicht heißt, dass der Berechtigte davorsitzt. Wer
    /// an einen offenen Rechner tritt, könnte sonst in zwei Klicks das
    /// Passwort ändern und den Eigentümer aussperren.
    ///
    /// # Die Stärke der Ableitung bleibt, wie sie war
    ///
    /// „Passwort ändern" ändert das Passwort. Die Ableitung dabei
    /// stillschweigend zu verschieben wäre eine zweite Entscheidung unter
    /// der Flagge der ersten — und beim Entsperren fiele plötzlich eine
    /// andere Wartezeit an, ohne dass jemand wüsste, warum.
    ///
    /// # Was diese Schicht nicht tut
    ///
    /// Schreiben. Die neue Datei steht danach in
    /// [`Sitzung::schluesseldatei`]; der Aufrufer legt sie ab. Und zwar
    /// **überschreibend** — dies ist die eine Stelle, an der das richtig
    /// ist: Es ist dieselbe Identität, nur anders verschlossen.
    ///
    /// # Fehler
    ///
    /// Wenn das alte Passwort nicht passt, oder das neue leer ist. Wie
    /// **gut** das neue ist, beurteilt dieses Programm nicht — es kennt
    /// die Liste nicht, in der es vielleicht steht.
    pub fn passwort_aendern<R: cabrik_core::Randomness>(
        &mut self,
        alt: &Zeroizing<String>,
        neu: &Zeroizing<String>,
        rng: &mut R,
    ) -> Befehlsergebnis<()> {
        // Dieselbe Schwelle wie beim Anlegen. Sie stand bis eben allein im
        // Einrichtungsbildschirm -- und damit hatte das Aendern keine.
        passwort_pruefen(neu.as_bytes())?;

        // Das alte Passwort wird geprüft, indem damit gelesen wird -- eine
        // eigene Prüfung daneben wäre eine zweite Wahrheit über dieselbe
        // Frage.
        let identitaet = keyfile::read(&self.schluesseldatei, alt.as_bytes())
            .map_err(|_| Befehlsfehler::neu("Das bisherige Passwort passt nicht."))?;

        let params = keyfile::params_of(&self.schluesseldatei)?;
        let neue_datei = keyfile::write(&identitaet, neu.as_bytes(), &params, rng)?;

        // Erst wenn alles gelungen ist. Ein Fehlschlag dazwischen ließe
        // sonst eine Sitzung über einer Datei zurück, die es so nicht gibt.
        self.schluesseldatei = neue_datei;
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

    /// Die eigene Austausch-Nutzlast — zum Weitergeben.
    ///
    /// # Warum das die fehlende Hälfte war
    ///
    /// Ohne sie ist das Programm einseitig: Man kann Kontakte aufnehmen,
    /// aber niemand kann einem schreiben. Wer nur das Fenster hat, konnte
    /// sich bisher niemandem mitteilen.
    ///
    /// # Was drinsteht
    ///
    /// **Ausschließlich öffentliche Angaben** — die drei öffentlichen
    /// Schlüssel und der daraus berechnete Fingerprint. Kein Name, keine
    /// Bezeichnung: Die vergibt der Empfänger selbst, und sie steht auch
    /// nirgends drin, wo sie jemand mitlesen könnte.
    ///
    /// Sie darf über jeden Weg gehen — Mail, Messenger, Aushang. Der Weg
    /// entscheidet allerdings nichts über Echtheit; dafür ist der
    /// Fingerprint-Vergleich da.
    ///
    /// # Fehler
    ///
    /// Wenn sich aus der Identität kein öffentlicher Schlüssel ableiten
    /// lässt.
    pub fn eigene_nutzlast(&self) -> Befehlsergebnis<String> {
        Ok(trust::qr_payload(
            &self.identitaet.enc_pub()?,
            self.identitaet.sig_pub().as_ref(),
            Some(&self.identitaet.xwing_pub()),
        ))
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
    // **Erst prüfen, ob es überhaupt ein PDF ist.** Ohne das lief der
    // PDF-Leser über jede Datei -- er suchte in einem 1,3-MB-Foto nach
    // `%%EOF` und versuchte dann, das Rauschen als Objektgraph zu lesen.
    // Das kostet bei jedem Bild Zeit und stellt einen Leser auf Daten an,
    // für die er nie gedacht war.
    if !cabrik_metadata::pdf::looks_like_pdf(daten) {
        return Vec::new();
    }
    cabrik_metadata::pdf::fassungen(daten, None)
        .map(|f| f.iter().map(Fassung::from).collect())
        .unwrap_or_default()
}

/// Die bereinigte Fassung einer Datei — samt dem Befund dazu.
///
/// # Warum es dieselbe Bereinigung ist wie beim Senden
///
/// Weil es sonst zwei bereinigte Fassungen derselben Datei gäbe: die
/// gespeicherte und die verschickte. Sie liefen beim nächsten Formatzusatz
/// auseinander, und niemand könnte sagen, welche von beiden das ist, was
/// jemand geprüft hat. Hier läuft `strip` mit denselben Voreinstellungen
/// wie in [`datei_pruefen`] — der Befund, den die Anzeige zeigt, gilt für
/// genau diese Bytes.
///
/// # Wann es keine bereinigte Fassung gibt
///
/// Wenn das Format nicht verstanden wurde. Dann wüsste das Programm nicht,
/// was es entfernen sollte, und eine Kopie mit demselben Inhalt „bereinigt"
/// zu nennen wäre eine Falschaussage. Der Aufrufer bekommt `None` und den
/// Befund dazu, damit er sagen kann, warum.
///
/// # Fehler
///
/// Gibt es nicht: Auch ein nicht lesbares Format ist ein Befund, keine
/// Störung.
#[must_use]
pub fn datei_bereinigen(daten: &[u8]) -> (Option<Vec<u8>>, Bereinigung) {
    let format = cabrik_metadata::inspect(daten)
        .ok()
        .and_then(|i| i.format)
        .unwrap_or_else(|| "unbekannt".to_owned());

    match cabrik_metadata::strip(daten) {
        Ok((sauber, ergebnis)) => {
            let befund = Bereinigung::aus(&ergebnis, &format);
            // Bei `Unbekannt` gibt `strip` die Bytes unveraendert zurueck.
            // Sie als bereinigt anzubieten hiesse, eine Kopie fuer eine
            // Leistung auszugeben.
            let inhalt = matches!(
                befund,
                Bereinigung::Vollstaendig { .. } | Bereinigung::Teilweise { .. }
            )
            .then_some(sauber);
            (inhalt, befund)
        }
        Err(e) => (
            None,
            Bereinigung::Fehler {
                grund: e.to_string(),
            },
        ),
    }
}

// ---------------------------------------------------------------------------
// Verschlüsseln
// ---------------------------------------------------------------------------

/// Der geprüfte Plan für einen Versand.
///
/// # Warum das ein eigener Schritt ist
///
/// Weil die Prüfungen **einmal** stattfinden müssen, nicht je Datei. Ob ein
/// Empfänger widerrufen ist, hängt nicht davon ab, welche Datei gerade
/// dran ist — und ein Stapel, der bei Datei siebenunddreißig abbricht,
/// hätte sechsunddreißig Envelopes hinterlassen, die niemand bestellt hat.
///
/// Wer den Plan hat, hat die Erlaubnis. Wer sie nicht bekommt, schreibt
/// keine einzige Datei.
pub struct Versandplan {
    schluessel: Vec<Vec<u8>>,
    namen: Vec<String>,
    suite: Suite,
    signieren: bool,
    /// Was der Nutzer wissen sollte, ohne dass es den Versand verhindert.
    pub vorbehalte: Vec<String>,
}

impl core::fmt::Debug for Versandplan {
    /// Gibt **keine Schlüssel** aus, auch keine öffentlichen.
    ///
    /// Sie sind nicht geheim, aber sie gehören nicht in ein Protokoll: Wer
    /// sie dort findet, erfährt, mit wem jemand spricht. Dieselbe
    /// Zurückhaltung wie bei `Identity`.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Versandplan")
            .field("empfaenger", &self.namen)
            .field("suite", &self.suite_name())
            .field("signiert", &self.signieren)
            .field("vorbehalte", &self.vorbehalte)
            .finish()
    }
}

impl Versandplan {
    /// Das Verfahren, in Worten für die Anzeige.
    #[must_use]
    pub const fn suite_name(&self) -> &'static str {
        match self.suite {
            Suite::Hybrid => "Post-Quantum-Hybrid (X-Wing, 0x0002)",
            _ => "klassisch (X25519, 0x0001)",
        }
    }

    /// Die Empfängernamen, in der Reihenfolge der Kapseln.
    #[must_use]
    pub fn empfaenger(&self) -> Vec<String> {
        self.namen.clone()
    }

    /// Ob tatsächlich signiert wird.
    #[must_use]
    pub const fn signiert(&self) -> bool {
        self.signieren
    }
}

impl Offen {
    /// Prüft die Empfänger und wählt das Verfahren — **bevor** etwas entsteht.
    ///
    /// # Was zum Abbruch führt
    ///
    /// - Kein Empfänger. Ein Envelope ohne Kapsel und ohne Passwort ließe
    ///   sich von niemandem öffnen.
    /// - Ein **widerrufener** Schlüssel. Sie haben ihn als kompromittiert
    ///   markiert; wer den privaten Teil hat, läse mit. Das ist kein
    ///   Vorbehalt, sondern ein Nein.
    /// - Ein Fingerprint, zu dem es keinen Kontakt gibt.
    ///
    /// # Was nur vermerkt wird
    ///
    /// Ein gewechselter oder nicht verifizierter Schlüssel. Beides kann
    /// harmlos sein — ein neues Gerät, ein frischer Kontakt —, und beides
    /// gehört gesagt, ohne den Weg zu versperren.
    ///
    /// # Die Wahl des Verfahrens
    ///
    /// Post-Quantum, sobald **alle** Empfänger es können; sonst klassisch,
    /// mit Vermerk. Ein Envelope trägt ein Verfahren für alle Kapseln —
    /// einen Empfänger stillschweigend schwächer zu bedienen geht nicht,
    /// und die halbe Wahrheit „Post-Quantum" wäre schlimmer als die ganze.
    ///
    /// # Fehler
    ///
    /// Siehe oben. Es entsteht dabei nichts und wird nichts geschrieben.
    pub fn versand_planen(
        &self,
        empfaenger: &[String],
        signieren: bool,
    ) -> Befehlsergebnis<Versandplan> {
        if empfaenger.is_empty() {
            return Err(Befehlsfehler::neu(
                "Ohne Empfänger ließe sich der Envelope von niemandem öffnen.",
            ));
        }

        let mut kontakte = Vec::with_capacity(empfaenger.len());
        for fp in empfaenger {
            let k = self
                .speicher
                .contacts()
                .iter()
                .find(|c| c.fingerprint().display_full() == *fp)
                .ok_or_else(|| {
                    Befehlsfehler::neu(
                        "Einer der Empfänger steht nicht mehr im Verzeichnis. \
                         Bitte die Auswahl erneut treffen.",
                    )
                })?;
            if k.state == TrustState::Revoked {
                return Err(Befehlsfehler::neu(&format!(
                    "„{}“ ist als kompromittiert markiert. An diesen Schlüssel \
                     wird nicht verschlüsselt — wer den privaten Teil hat, läse \
                     mit. Erst neu austauschen und verifizieren.",
                    k.name
                )));
            }
            kontakte.push(k);
        }

        let mut vorbehalte = Vec::new();
        for k in &kontakte {
            match k.state {
                TrustState::Changed => vorbehalte.push(format!(
                    "„{}“ tritt mit einem anderen Schlüssel auf als zuvor. Das \
                     kann ein neues Gerät sein — oder ein Angriff. Vor dem \
                     Senden über einen zweiten Weg klären.",
                    k.name
                )),
                TrustState::Seen => vorbehalte.push(format!(
                    "„{}“ ist nicht verifiziert. Sie wissen nicht sicher, wem \
                     dieser Schlüssel gehört.",
                    k.name
                )),
                TrustState::Verified | TrustState::Revoked => {}
            }
        }

        let ohne_pq: Vec<&str> = kontakte
            .iter()
            .filter(|k| k.xwing_pub.is_none())
            .map(|k| k.name.as_str())
            .collect();
        let suite = if ohne_pq.is_empty() {
            Suite::Hybrid
        } else {
            vorbehalte.push(format!(
                "Klassisches Verfahren, weil {} keinen Post-Quantum-Schlüssel \
                 hat. Ein Envelope trägt ein Verfahren für alle.",
                ohne_pq.join(", ")
            ));
            Suite::Classical
        };

        // Für Suite 0x0002 ist der Empfängerschlüssel der X-Wing-Schlüssel,
        // für 0x0001 der X25519-Schlüssel. Die Längen prüft der Kern.
        let schluessel = kontakte
            .iter()
            .map(|k| match suite {
                Suite::Hybrid => k
                    .xwing_pub
                    .as_deref()
                    .map_or_else(|| k.enc_pub.to_vec(), |x| x.to_vec()),
                _ => k.enc_pub.to_vec(),
            })
            .collect();

        // Signieren kann nur, wer einen Signierschlüssel hat. Eine
        // Anonymitätsidentität hat keinen -- das ist ein gewählter Modus,
        // und der Bericht sagt es, statt es stillschweigend zu übergehen.
        let signiert = signieren && self.identitaet.can_sign();

        Ok(Versandplan {
            schluessel,
            namen: kontakte.iter().map(|k| k.name.clone()).collect(),
            suite,
            signieren: signiert,
            vorbehalte,
        })
    }

    /// Verschlüsselt **eine** Datei nach einem geprüften Plan.
    ///
    /// # Warum der Dateiname mitgeht
    ///
    /// Er liegt **verschlüsselt** im Envelope (`spec/envelope-v2.md` §7.2).
    /// Ohne ihn wüsste der Empfänger nicht, wie die Datei heißen soll, und
    /// müsste raten — bei einem Stapel aus vierzig ist das aussichtslos.
    ///
    /// # Fehler
    ///
    /// Fehler des Kerns. Es wird dabei nichts geschrieben; diese Schicht
    /// fasst kein Dateisystem an.
    pub fn verschluesseln<R: cabrik_core::Randomness>(
        &self,
        plan: &Versandplan,
        name: &str,
        klartext: &[u8],
        rng: &mut R,
    ) -> Befehlsergebnis<Vec<u8>> {
        let schluessel: Vec<&[u8]> = plan.schluessel.iter().map(Vec::as_slice).collect();
        let opts = SealOptions {
            content_type: ContentType::File,
            filename: Some(name),
            // Kein Zeitstempel. Er ist keine Sicherheitseigenschaft, aber
            // eine Angabe über den Absender -- und wer ihn will, soll ihn
            // wählen, statt ihn ungefragt mitzuschicken.
            timestamp: None,
            padding: None,
            dummy_stanzas: false,
        };
        let signierer = plan.signieren.then_some(&self.identitaet);

        envelope::seal(
            plan.suite,
            &schluessel,
            None,
            klartext,
            signierer,
            &opts,
            rng,
        )
        .map_err(Into::into)
    }
}

/// Der Name des Envelopes zu einer Datei.
///
/// **Angehängt, nicht ersetzt**: `bericht.pdf` wird zu `bericht.pdf.cab`.
/// Ersetzte man die Endung, kollidierten `bericht.pdf` und `bericht.docx`
/// in derselben Datei — und die zweite überschriebe die erste.
///
/// Dieselbe Regel wie in der CLI; dort steht sie als `ausgabename`.
#[must_use]
pub fn envelope_name(dateiname: &str) -> String {
    format!("{dateiname}.cab")
}

/// Ein leeres Ergebnis für eine Datei, die gar nicht erst drankam.
#[must_use]
pub fn versand_fehler(quelle: &str, grund: String) -> Versandergebnis {
    Versandergebnis {
        quelle: quelle.to_owned(),
        ziel: None,
        bytes: 0,
        befund: None,
        fehler: Some(grund),
    }
}

// ---------------------------------------------------------------------------
// Entschlüsseln
// ---------------------------------------------------------------------------

impl Offen {
    /// Öffnet einen Envelope. **Der Klartext bleibt hier.**
    ///
    /// Zurück geht ein Bericht: Wer geschickt hat, wie die Datei heißt, wie
    /// groß sie ist. Der Inhalt bleibt in Rust, bis jemand sagt, wohin er
    /// soll — ihn über die Brücke zu reichen hieße, ihn in eine Webansicht
    /// zu legen, die wir weder überschreiben noch begrenzen können.
    ///
    /// # Warum der Absender aus zwei Quellen kommt
    ///
    /// Der Envelope sagt nur, **welcher Schlüssel** signiert hat. Wem der
    /// gehört, weiß allein der Kontaktspeicher. Erst beides zusammen ergibt
    /// eine Aussage — und der Unterschied zwischen „gültig signiert“ und
    /// „von Anna“ ist genau der, an dem sich Sicherheit entscheidet.
    ///
    /// # Fehler
    ///
    /// Wenn der Envelope nicht für diese Identität bestimmt ist oder
    /// beschädigt wurde. **Beides ist nach außen ununterscheidbar** — der
    /// Kern formuliert es so, und diese Schicht ändert daran nichts.
    ///
    /// `signatur_verlangt` macht aus einer unsignierten Nachricht einen
    /// Fehler. Das ist eine Entscheidung des Nutzers, keine des Programms.
    pub fn envelope_oeffnen(
        &mut self,
        daten: &[u8],
        signatur_verlangt: bool,
    ) -> Befehlsergebnis<Geoeffnet> {
        let opener = envelope::Opener::Identity(&self.identitaet);
        let auf = envelope::open(&opener, daten, signatur_verlangt).map_err(|e| match e {
            Error::AuthFailed | Error::NoMatchingRecipient => Befehlsfehler::neu(
                "Diese Datei ließ sich nicht öffnen. Sie ist nicht an Ihre \
                 Identität gerichtet oder wurde verändert.",
            ),
            anderer => Befehlsfehler::from(anderer),
        })?;

        let absender = Absender::aus(&self.speicher.resolve(&auf.signer));
        let art = match auf.content_type {
            envelope::ContentType::Text => Inhaltsart::Text,
            _ => Inhaltsart::Datei,
        };
        let text = matches!(art, Inhaltsart::Text)
            .then(|| String::from_utf8_lossy(&auf.plaintext).into_owned());

        let bericht = Geoeffnet {
            art,
            text,
            dateiname: auf.filename.clone(),
            groesse_bytes: auf.plaintext.len(),
            zeitpunkt: auf.timestamp,
            absender,
            // Noch keine Metadatenprüfung auf Empfangenes. `Bereinigung`
            // beschreibt, was ein Bereinigen ergäbe -- für eine Datei, die
            // gerade ankommt, wäre das die falsche Frage. Was in ihr steht,
            // ist eine eigene Auskunft und braucht einen eigenen Typ.
            metadaten: None,
        };

        self.nutzlast = Some(Nutzlast {
            inhalt: auf.plaintext,
            dateiname: auf.filename,
        });
        Ok(bericht)
    }

    /// Verschlüsselt einen Text und gibt ihn als Armor zurück.
    ///
    /// # Warum Text anders behandelt wird als eine Datei
    ///
    /// Zwei Unterschiede, beide aus `spec/envelope-v2.md`:
    ///
    /// 1. **Padding ist an.** Bei Text ist die Länge die Aussage: „ja“ und
    ///    „auf keinen Fall, und zwar aus folgenden Gründen“ sind sonst von
    ///    außen zu unterscheiden. Bei Dateien wäre dasselbe Padding teuer
    ///    und nutzlos — deshalb steht es dort aus.
    /// 2. **Kein Dateiname.** Es gibt keinen, und einen zu erfinden hieße,
    ///    eine Angabe mitzuschicken, die niemand gemacht hat.
    ///
    /// # Warum das Ergebnis Text ist und keine Datei
    ///
    /// Weil der Zweck das Einfügen ist — in ein Chatfenster, eine E-Mail,
    /// ein Ticket. Eine Datei müsste erst irgendwo abgelegt und dann
    /// angehängt werden; wer diesen Weg will, verschickt eine Datei.
    ///
    /// Der Preis steht in §14 und wird bewusst gezahlt: ein Drittel mehr
    /// Umfang, und die Rahmenzeilen nennen das Produkt.
    ///
    /// # Fehler
    ///
    /// Fehler des Kerns. Ein leerer Text wird abgelehnt: Ein Envelope über
    /// nichts ist keine Nachricht.
    pub fn text_verschluesseln<R: cabrik_core::Randomness>(
        &self,
        plan: &Versandplan,
        text: &str,
        rng: &mut R,
    ) -> Befehlsergebnis<String> {
        if text.trim().is_empty() {
            return Err(Befehlsfehler::neu("Es gibt nichts zu verschlüsseln."));
        }

        let schluessel: Vec<&[u8]> = plan.schluessel.iter().map(Vec::as_slice).collect();
        let opts = SealOptions {
            content_type: ContentType::Text,
            filename: None,
            timestamp: None,
            // `None` heißt: die Voreinstellung des Formats -- und die ist
            // bei Text „an". Sie hier auszuschreiben hieße, sie an zwei
            // Stellen zu führen.
            padding: None,
            dummy_stanzas: false,
        };
        let signierer = plan.signieren.then_some(&self.identitaet);

        let envelope = envelope::seal(
            plan.suite,
            &schluessel,
            None,
            text.as_bytes(),
            signierer,
            &opts,
            rng,
        )?;
        Ok(cabrik_core::armor::encode(&envelope))
    }

    /// Öffnet einen eingefügten Armor-Text.
    ///
    /// Derselbe Weg wie bei einer Datei, nur mit einem Schritt davor. Was
    /// um den Envelope herum steht — Anrede, Grußformel, Zitatzeichen —
    /// stört nicht: Wer ihn aus einer E-Mail herauskopiert, hat selten
    /// saubere Zeilen.
    ///
    /// # Fehler
    ///
    /// Wenn kein Envelope im Text steckt, oder was auch beim Öffnen einer
    /// Datei schiefgehen kann.
    pub fn text_oeffnen(
        &mut self,
        text: &str,
        signatur_verlangt: bool,
    ) -> Befehlsergebnis<Geoeffnet> {
        let daten = cabrik_core::armor::decode(text).map_err(|_| {
            Befehlsfehler::neu(
                "In diesem Text steckt kein Cabrik-Envelope. Er beginnt mit \
                 „-----BEGIN CABRIK ENVELOPE-----“ und endet mit der \
                 passenden Schlusszeile.",
            )
        })?;
        self.envelope_oeffnen(&daten, signatur_verlangt)
    }

    /// Der zuletzt geöffnete Klartext, zum Ablegen durch den Aufrufer.
    ///
    /// Diese Schicht fasst kein Dateisystem an; wer schreibt, bekommt die
    /// Bytes geliehen und gibt sie sofort wieder her.
    #[must_use]
    pub fn nutzlast(&self) -> Option<(&[u8], Option<&str>)> {
        self.nutzlast
            .as_ref()
            .map(|n| (n.inhalt.as_slice(), n.dateiname.as_deref()))
    }

    /// Wirft den geöffneten Klartext weg.
    ///
    /// **Nach dem Speichern und beim Verlassen des Bildschirms.** Ein
    /// entschlüsselter Inhalt, der liegen bleibt, ist eine Kopie ohne
    /// Zweck — und `Zeroizing` überschreibt ihn erst, wenn er fällt.
    pub fn nutzlast_verwerfen(&mut self) {
        self.nutzlast = None;
    }
}
