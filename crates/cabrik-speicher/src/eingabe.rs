//! Was ein Tastendruck im Passwortfeld bewirkt.
//!
//! # Warum das ein eigenes Modul ist
//!
//! Weil es der einzige Teil des nativen Passwortfensters ist, der sich
//! **ohne Fenster** prüfen lässt — und weil er der Teil ist, in dem ein
//! Fehler stillschweigend teuer wird.
//!
//! Ein Fenster lässt sich nicht in einem Test aufmachen, eine Taste nicht
//! drücken. Die Entscheidung dahinter dagegen sehr wohl: Was bedeutet
//! diese Zeicheneinheit? Was gehört in den Puffer, was nicht? Genau diese
//! Fragen beantwortet dieses Modul, und die Fensterhülle daneben besteht
//! aus Zeichnen und Nachrichtenschleife.
//!
//! # Die zwei Fallen von `WM_CHAR`
//!
//! **Erstens Steuerzeichen.** Windows schickt Rücktaste, Eingabe und
//! Escape als gewöhnliche Zeichennachricht — `0x08`, `0x0D`, `0x1B`. Wer
//! sie nicht abfängt, hat sie im Passwort stehen, und niemand sieht es:
//! Die Punkte sehen genauso aus.
//!
//! **Zweitens Ersatzzeichen.** `WM_CHAR` liefert UTF-16-Einheiten. Ein
//! Zeichen außerhalb der Grundebene — jedes Emoji — kommt als **zwei**
//! Nachrichten. Wer jede einzeln als Zeichen nimmt, schreibt zwei
//! ungültige Hälften in den Puffer; das Passwort ist dann ein anderes als
//! das getippte, und beim nächsten Entsperren geht es nicht mehr auf.

use crate::{Festgenagelt, Voll};

/// Was ein Tastendruck bewirken soll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wirkung {
    /// Der Puffer hat sich geändert — die Punkte sind neu zu zeichnen.
    Geaendert,
    /// Der Nutzer ist fertig.
    Bestaetigt,
    /// Der Nutzer will nicht mehr.
    Abgebrochen,
    /// Nichts zu tun.
    ///
    /// Ausdrücklich benannt statt als `Option`: Der häufigste Fall ist
    /// „diese Taste bedeutet hier nichts", und ein `None` dafür lüde dazu
    /// ein, ihn mit einem Fehler zu verwechseln.
    Nichts,
    /// Der Puffer ist voll — das Zeichen wurde **nicht** aufgenommen.
    ///
    /// Kein Fehler, aber auch kein stilles Verschlucken: Wer weitertippt,
    /// ohne dass sich etwas rührt, muss es merken.
    Voll,
}

/// Der Zustand des Eingabefeldes.
///
/// Enthält den festgenagelten Puffer und die eine Zusatzangabe, die
/// UTF-16 nötig macht.
pub struct Eingabe {
    puffer: Festgenagelt,
    /// Eine erste Hälfte, die auf ihre zweite wartet.
    ///
    /// Steht hier etwas, kam gerade ein hohes Ersatzzeichen. Es ist für
    /// sich genommen **kein** Zeichen und darf nicht in den Puffer.
    schwebend: Option<u16>,
    /// Wie viele Zeichen der Nutzer getippt hat.
    ///
    /// Nicht aus dem Puffer gezählt, sondern mitgeführt: Die Punkte
    /// dürfen nicht davon abhängen, dass jemand die Bytes liest.
    zeichen: usize,
}

impl Eingabe {
    /// Ein leeres Feld mit festgenageltem Puffer.
    ///
    /// `kapazitaet` ist die Obergrenze in **Bytes**, nicht in Zeichen.
    #[must_use]
    pub fn neu(kapazitaet: usize) -> Self {
        Self {
            puffer: Festgenagelt::neu(kapazitaet),
            schwebend: None,
            zeichen: 0,
        }
    }

    /// Ob die Seiten des Puffers festgenagelt sind.
    ///
    /// Wird durchgereicht, damit der Aufrufer es sagen kann — und nicht
    /// behaupten muss.
    #[must_use]
    pub const fn ist_festgenagelt(&self) -> bool {
        self.puffer.ist_festgenagelt()
    }

    /// Wie viele Punkte zu zeichnen sind.
    #[must_use]
    pub const fn punkte(&self) -> usize {
        self.zeichen
    }

    /// Ob nichts drinsteht.
    #[must_use]
    pub const fn ist_leer(&self) -> bool {
        self.zeichen == 0
    }

    /// Das Getippte, zum Weiterreichen an den Kern.
    #[must_use]
    pub fn als_bytes(&self) -> &[u8] {
        self.puffer.als_bytes()
    }

    /// Gibt den Puffer heraus und lässt das Feld zurück.
    #[must_use]
    pub fn nehmen(self) -> Festgenagelt {
        self.puffer
    }

    /// Eine Zeichennachricht von Windows — eine **UTF-16-Einheit**.
    ///
    /// Behandelt Steuerzeichen und Ersatzzeichen; siehe den Modulkopf.
    pub fn zeichen(&mut self, einheit: u16) -> Wirkung {
        // Erst die Steuerzeichen, und zwar VOR allem anderen. Sie kommen
        // als gewoehnliche Zeichennachricht, und wer sie durchlaesst, hat
        // sie im Passwort.
        match einheit {
            0x08 => return self.rueckschritt(),
            0x0D => return Wirkung::Bestaetigt,
            0x1B => return Wirkung::Abgebrochen,
            // Alles unter dem Leerzeichen ist eine Steuertaste: Tabulator,
            // Zeilenvorschub, Klingel. Nichts davon gehoert in ein
            // Passwort, und stillschweigend aufzunehmen waere schlimmer
            // als zu uebergehen -- der Nutzer saehe einen Punkt mehr und
            // wuesste nicht, wofuer.
            0..=0x1F | 0x7F => {
                self.schwebend = None;
                return Wirkung::Nichts;
            }
            _ => {}
        }

        // Ersatzzeichen: Ein hohes wartet auf sein niedriges.
        if (0xD800..=0xDBFF).contains(&einheit) {
            self.schwebend = Some(einheit);
            // NOCH KEINE Aenderung: Es ist ein halbes Zeichen, und der
            // Punkt gehoert erst gezeichnet, wenn es ganz ist.
            return Wirkung::Nichts;
        }

        let paar = self.schwebend.take();
        if (0xDC00..=0xDFFF).contains(&einheit) {
            let Some(hoch) = paar else {
                // Ein niedriges Ersatzzeichen ohne sein hohes. Das schickt
                // Windows nicht von sich aus -- es kaeme von einem fremden
                // Programm, das Tastendruecke einspeist. Uebergehen statt
                // raten.
                return Wirkung::Nichts;
            };
            return self.einfuegen(&[hoch, einheit]);
        }

        self.einfuegen(&[einheit])
    }

    /// Nimmt eine vollständige UTF-16-Folge auf.
    fn einfuegen(&mut self, einheiten: &[u16]) -> Wirkung {
        let mut text = String::new();
        for teil in char::decode_utf16(einheiten.iter().copied()) {
            match teil {
                Ok(c) => text.push(c),
                // Kann nach der Behandlung oben nicht mehr vorkommen. Wenn
                // doch, wird nichts geraten: Ein Ersatzzeichen als „?" in
                // ein Passwort zu schreiben ergaebe ein anderes Passwort
                // als das getippte, und das faellt erst beim naechsten
                // Entsperren auf.
                Err(_) => return Wirkung::Nichts,
            }
        }
        if text.is_empty() {
            return Wirkung::Nichts;
        }

        match self.puffer.anhaengen(&text) {
            Ok(()) => {
                self.zeichen = self.zeichen.saturating_add(text.chars().count());
                Wirkung::Geaendert
            }
            Err(Voll) => Wirkung::Voll,
        }
    }

    /// Nimmt das letzte Zeichen zurück.
    fn rueckschritt(&mut self) -> Wirkung {
        // Ein halbes Zeichen zaehlt nicht. Wer nach einem hohen
        // Ersatzzeichen die Ruecktaste drueckt, hat noch nichts getippt --
        // und darf nicht das ZEICHEN DAVOR verlieren.
        if self.schwebend.take().is_some() {
            return Wirkung::Nichts;
        }
        if self.ist_leer() {
            return Wirkung::Nichts;
        }
        self.puffer.letztes_zeichen_loeschen();
        self.zeichen = self.zeichen.saturating_sub(1);
        Wirkung::Geaendert
    }
}

impl core::fmt::Debug for Eingabe {
    /// Von Hand: Das abgeleitete druckte den Puffer mit aus.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Eingabe")
            .field("punkte", &self.zeichen)
            .field("schwebend", &self.schwebend.is_some())
            .finish()
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod pruefungen {
    use super::{Eingabe, Wirkung};

    /// Tippt eine Zeichenkette, wie Windows sie schicken würde.
    fn tippen(e: &mut Eingabe, text: &str) {
        for einheit in text.encode_utf16() {
            e.zeichen(einheit);
        }
    }

    #[test]
    fn getipptes_landet_im_puffer() {
        let mut e = Eingabe::neu(256);
        tippen(&mut e, "geheim");

        assert_eq!(e.als_bytes(), b"geheim");
        assert_eq!(e.punkte(), 6);
        assert!(!e.ist_leer());
    }

    #[test]
    fn steuerzeichen_landen_nicht_im_passwort() {
        // DIE ERSTE FALLE. Windows schickt Ruecktaste, Eingabe und Escape
        // als gewoehnliche Zeichennachricht. Wer sie durchlaesst, hat sie
        // im Passwort -- und niemand sieht es, denn die Punkte sehen
        // genauso aus.
        let mut e = Eingabe::neu(256);

        assert_eq!(e.zeichen(0x0D), Wirkung::Bestaetigt);
        assert_eq!(e.zeichen(0x1B), Wirkung::Abgebrochen);
        assert_eq!(e.zeichen(0x09), Wirkung::Nichts, "Tabulator");
        assert_eq!(e.zeichen(0x0A), Wirkung::Nichts, "Zeilenvorschub");
        assert_eq!(e.zeichen(0x07), Wirkung::Nichts, "Klingel");
        assert_eq!(e.zeichen(0x7F), Wirkung::Nichts, "Entfernen");

        assert!(e.ist_leer(), "eine Steuertaste ist im Puffer gelandet");
        assert_eq!(e.punkte(), 0);
    }

    #[test]
    fn ein_emoji_ergibt_einen_punkt_und_nicht_zwei() {
        // DIE ZWEITE FALLE. `WM_CHAR` liefert UTF-16-Einheiten; ein Emoji
        // kommt als ZWEI Nachrichten. Wer jede einzeln nimmt, schreibt
        // zwei ungueltige Haelften in den Puffer -- das Passwort ist dann
        // ein anderes als das getippte, und das faellt erst beim naechsten
        // Entsperren auf.
        let mut e = Eingabe::neu(256);
        tippen(&mut e, "🔑");

        assert_eq!(e.punkte(), 1, "ein Zeichen, ein Punkt");
        assert_eq!(e.als_bytes(), "🔑".as_bytes());
    }

    #[test]
    fn ein_halbes_zeichen_erscheint_noch_nicht() {
        // Zwischen den beiden Nachrichten darf kein Punkt dastehen: Der
        // Nutzer hat noch kein Zeichen getippt.
        let mut e = Eingabe::neu(256);
        let einheiten: Vec<u16> = "🔑".encode_utf16().collect();

        assert_eq!(e.zeichen(einheiten[0]), Wirkung::Nichts);
        assert_eq!(e.punkte(), 0, "ein halbes Zeichen ergibt keinen Punkt");
        assert!(e.als_bytes().is_empty(), "eine Haelfte steht im Puffer");

        assert_eq!(e.zeichen(einheiten[1]), Wirkung::Geaendert);
        assert_eq!(e.punkte(), 1);
    }

    #[test]
    fn die_ruecktaste_nach_einem_halben_zeichen_frisst_nicht_das_vorige() {
        // Der unangenehmste der Ersatzzeichen-Faelle: Wer nach der ersten
        // Haelfte abbricht, hat noch nichts getippt -- und darf nicht das
        // Zeichen DAVOR verlieren.
        let mut e = Eingabe::neu(256);
        tippen(&mut e, "ab");
        let hoch = "🔑".encode_utf16().next().unwrap();

        e.zeichen(hoch);
        assert_eq!(e.zeichen(0x08), Wirkung::Nichts);

        assert_eq!(e.als_bytes(), b"ab", "das Zeichen davor ist verschwunden");
        assert_eq!(e.punkte(), 2);
    }

    #[test]
    fn ein_niedriges_ersatzzeichen_allein_wird_uebergangen() {
        // Windows schickt das nicht von sich aus -- es kaeme von einem
        // fremden Programm, das Tastendruecke einspeist. Uebergehen statt
        // raten: Ein „?" im Passwort ergaebe ein anderes als das getippte.
        let mut e = Eingabe::neu(256);
        let niedrig = "🔑".encode_utf16().nth(1).unwrap();

        assert_eq!(e.zeichen(niedrig), Wirkung::Nichts);
        assert!(e.ist_leer());
    }

    #[test]
    fn die_ruecktaste_nimmt_ganze_zeichen() {
        let mut e = Eingabe::neu(256);
        tippen(&mut e, "aä🔑");
        assert_eq!(e.punkte(), 3);

        assert_eq!(e.zeichen(0x08), Wirkung::Geaendert);
        assert_eq!(e.als_bytes(), "aä".as_bytes(), "Emoji nur halb entfernt");
        assert_eq!(e.punkte(), 2);

        e.zeichen(0x08);
        assert_eq!(e.als_bytes(), b"a", "Umlaut nur halb entfernt");
    }

    #[test]
    fn auf_dem_leeren_feld_richtet_die_ruecktaste_nichts_an() {
        let mut e = Eingabe::neu(256);
        assert_eq!(e.zeichen(0x08), Wirkung::Nichts);
        assert!(e.ist_leer());
        assert_eq!(e.punkte(), 0);
    }

    #[test]
    fn ein_volles_feld_sagt_es_statt_stillzuhalten() {
        // Wer weitertippt, ohne dass sich etwas ruehrt, muss es merken.
        // Ein stilles Verschlucken waere die schlechtere Antwort: Er
        // glaubte, sein Passwort sei laenger als es ist.
        let mut e = Eingabe::neu(4);
        tippen(&mut e, "abcd");
        assert_eq!(e.punkte(), 4);

        assert_eq!(e.zeichen(u16::from(b'e')), Wirkung::Voll);
        assert_eq!(e.punkte(), 4, "es wurde doch etwas aufgenommen");
        assert_eq!(e.als_bytes(), b"abcd");
    }

    #[test]
    fn die_punkte_zaehlen_zeichen_und_nicht_bytes() {
        // Sonst waeren drei Emoji zwoelf Punkte, und ein Umlaut zwei.
        let mut e = Eingabe::neu(256);
        tippen(&mut e, "äöü🔑");

        assert_eq!(e.punkte(), 4);
        assert_eq!(e.als_bytes().len(), 10, "vier Zeichen, zehn Bytes");
    }

    #[test]
    fn debug_zeigt_das_passwort_nicht() {
        let mut e = Eingabe::neu(256);
        tippen(&mut e, "strenggeheim");

        let text = format!("{e:?}");
        assert!(
            !text.contains("geheim"),
            "Debug hat das Passwort ausgedruckt: {text}"
        );
    }
}
