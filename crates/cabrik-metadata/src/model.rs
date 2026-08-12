//! Fähigkeitsmodell nach `spec/metadata.md` §3.
//!
//! # Der Kernfehler in v1
//!
//! ```python
//! else:
//!     import shutil
//!     shutil.copy2(path, out_path)
//! ```
//!
//! Für jedes nicht unterstützte Format kopierte v1 die Datei — und meldete
//! keinen Fehler. Der Nutzer klickte „Metadaten strippen", bekam eine
//! `.clean`-Datei und schloss daraus, sie sei bereinigt. `shutil.copy2`
//! **erhielt** obendrein die Zeitstempel: die einzige Metadatenart, die auch
//! bei unbekannten Formaten entfernbar gewesen wäre, wurde aktiv mitgenommen.
//!
//! Deshalb gibt es hier **keinen Wahrheitswert**, sondern drei Zustände. Für
//! ein Format, das nicht verstanden wird, wird Sauberkeit **niemals**
//! behauptet.

use core::fmt;

/// Wie schwer ein Fund wiegt (`spec/metadata.md` §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Farbprofil, Auflösung, Orientierung.
    Minor,
    /// Kameramodell, Software, Bearbeitungszeit, Vorlagenname.
    Notable,
    /// GPS, Klarname, Gerätenummer, Firmenname — und **Zweitkopien des
    /// Inhalts** (`spec/metadata.md` §7).
    Critical,
}

/// Art eines Fundes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FindingKind {
    /// Ortsangabe.
    Gps,
    /// Personenname.
    Author,
    /// Gerät, Seriennummer.
    Device,
    /// Erzeugende Software.
    Software,
    /// Zeitangabe.
    Timestamp,
    /// Firmen- oder Organisationsname.
    Organization,
    /// **Eingebettetes Vorschaubild.**
    ///
    /// Keine Metadatenart im engeren Sinn, sondern eine **zweite Kopie des
    /// Inhalts** — oft in einem Zustand, den der Nutzer gerade beseitigen
    /// wollte. Siehe [`Finding`] und `spec/metadata.md` §7.1.
    EmbeddedPreview,
    /// **Zugeschnittenes Bild in einem Office-Dokument.**
    ///
    /// Wer ein Bild in Word oder PowerPoint einfügt und dort zuschneidet,
    /// verschickt das **vollständige Original**. Der Zuschnitt ist nur ein
    /// Anzeigerechteck in der XML-Beschreibung; die Bilddatei unter
    /// `word/media/` bleibt unverändert. Empfänger machen ihn mit zwei Klicks
    /// rückgängig.
    ///
    /// Wird gemeldet, aber **nicht** automatisch behoben: Den
    /// weggeschnittenen Bereich wirklich zu beseitigen hieße, das Bild neu zu
    /// kodieren und im Dokument zu ersetzen — das verändert das Dokument
    /// sichtbar (`spec/metadata.md` §7.2).
    CroppedImage,
    /// **Nachverfolgte Änderung.**
    ///
    /// Trägt Name und Zeitpunkt jeder Bearbeitung, und der gelöschte Text
    /// steht weiterhin vollständig im Dokument.
    TrackedChange,
    /// Farbprofil.
    ColorProfile,
    /// Freier Kommentar.
    Comment,
    /// Bearbeitungssitzung (`rsid`).
    ///
    /// Word vergibt je Sitzung eine Kennung und schreibt sie an jeden
    /// bearbeiteten Absatz. Zwei Dokumente mit gemeinsamen `rsid`-Werten
    /// stammen nachweislich aus derselben Sitzung — das verkettet Dokumente
    /// über Empfänger hinweg.
    EditingSession,
    /// **Ursprünglicher Dateiname.**
    ///
    /// Der Name, unter dem die Datei beim Verfasser lag — oft aussagekräftiger
    /// als ihr Inhalt. Genau dieses Leck hatte v1 im Umschlag: Der Dateiname
    /// stand dort im Klartext, lesbar ohne jeden Schlüssel.
    FileName,
    /// Etwas Unbekanntes, das entfernt wurde.
    UnknownExtension,
}

/// Wie weit die Bereinigung gehen darf.
///
/// # Warum das eine Entscheidung des Nutzers ist
///
/// Die Voreinstellung entfernt ausschließlich **Metadaten** — Angaben *über*
/// das Dokument. Kommentare und nachverfolgte Änderungen sind etwas anderes:
/// Sie sind Bestandteil des Dokuments. Eine nachverfolgte Löschung zu
/// beseitigen heißt, sie anzunehmen oder zu verwerfen, und das verändert den
/// sichtbaren Text.
///
/// Ein Werkzeug, das das ungefragt tut, überrascht seinen Nutzer im
/// schlechtesten Moment. Deshalb wird zuerst **gemeldet**, und der Eingriff
/// erfolgt nur auf ausdrückliche Anweisung (`spec/metadata.md` §4.2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StripOptions {
    /// Kommentare entfernen.
    ///
    /// Betrifft nur die Anmerkungen, nicht den Text: Was im Dokument steht,
    /// bleibt Zeichen für Zeichen erhalten.
    pub remove_comments: bool,

    /// Nachverfolgte Änderungen annehmen.
    ///
    /// Entspricht „Alle Änderungen annehmen" in Word: Einfügungen bleiben,
    /// Löschungen verschwinden **samt ihrem Text**, Formatierungsvermerke
    /// entfallen. Danach steht im Dokument genau das, was der Verfasser
    /// zuletzt gesehen hat.
    ///
    /// **Das verändert den Inhalt.** Wer die Änderungen noch braucht, sollte
    /// vorher eine Kopie behalten.
    pub accept_changes: bool,
}

impl StripOptions {
    /// Nur Metadaten entfernen — die Voreinstellung.
    #[must_use]
    pub const fn nur_metadaten() -> Self {
        Self {
            remove_comments: false,
            accept_changes: false,
        }
    }

    /// Zusätzlich Kommentare entfernen und Änderungen annehmen.
    #[must_use]
    pub const fn auch_inhaltliche_reste() -> Self {
        Self {
            remove_comments: true,
            accept_changes: true,
        }
    }

    /// Ob überhaupt in den Inhalt eingegriffen wird.
    #[must_use]
    pub const fn greift_in_den_inhalt_ein(self) -> bool {
        self.remove_comments || self.accept_changes
    }
}

/// Ein einzelner Metadaten-Fund.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Art des Fundes.
    pub kind: FindingKind,
    /// Wo er saß, etwa `"EXIF:GPSLatitude"` oder `"PNG:tEXt"`.
    pub location: String,
    /// Der Wert, auf 200 Zeichen gekürzt. `None`, wenn nicht darstellbar.
    ///
    /// v1 gab rohe EXIF-Tag-Nummern aus (`0th:271`, `GPS:2`) — für den
    /// Nutzer unlesbar.
    pub value: Option<String>,
    /// Schweregrad.
    pub severity: Severity,
}

impl Finding {
    /// Neuer Fund.
    #[must_use]
    pub fn new(
        kind: FindingKind,
        location: impl Into<String>,
        value: Option<String>,
        severity: Severity,
    ) -> Self {
        Self {
            kind,
            location: location.into(),
            value: value.map(|v| kuerzen(&v, 200)),
            severity,
        }
    }
}

fn kuerzen(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

/// Ergebnis einer Bereinigung (`spec/metadata.md` §3).
///
/// **Bewusst kein Wahrheitswert.** Siehe Modul-Dokumentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StripResult {
    /// Alle **bekannten** Metadatenträger des Formats wurden behandelt.
    ///
    /// Heißt ausdrücklich *nicht* „garantiert metadatenfrei". Die
    /// Oberfläche muss diesen Unterschied im Hilfetext benennen.
    Complete {
        /// Was entfernt wurde.
        removed: Vec<Finding>,
    },
    /// Bereinigt, aber Reste sind benannt.
    Partial {
        /// Was entfernt wurde.
        removed: Vec<Finding>,
        /// Was bleiben musste.
        remaining: Vec<Finding>,
        /// Warum.
        reason: String,
    },
    /// Format nicht verstanden. **Keine Aussage über Sauberkeit.**
    Unknown {
        /// Was das Format vermutlich war, soweit erkennbar.
        format_hint: Option<String>,
    },
}

impl StripResult {
    /// Ob die Anzeige grün sein darf.
    ///
    /// Nur bei [`StripResult::Complete`]. Ein unverstandenes Format ist
    /// **nie** grün — das war der Fehler in v1.
    #[must_use]
    pub const fn may_show_clean(&self) -> bool {
        matches!(self, Self::Complete { .. })
    }

    /// Was entfernt wurde.
    #[must_use]
    pub fn removed(&self) -> &[Finding] {
        match self {
            Self::Complete { removed } | Self::Partial { removed, .. } => removed,
            Self::Unknown { .. } => &[],
        }
    }

    /// Was bleiben musste.
    #[must_use]
    pub fn remaining(&self) -> &[Finding] {
        match self {
            Self::Partial { remaining, .. } => remaining,
            Self::Complete { .. } | Self::Unknown { .. } => &[],
        }
    }

    /// Ob mindestens ein Fund als [`Severity::Critical`] eingestuft wurde.
    #[must_use]
    pub fn has_critical(&self) -> bool {
        self.removed()
            .iter()
            .chain(self.remaining())
            .any(|f| f.severity == Severity::Critical)
    }
}

impl fmt::Display for StripResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Complete { removed } => {
                write!(f, "Bereinigt ({} Fund(e) entfernt)", removed.len())
            }
            Self::Partial {
                remaining, reason, ..
            } => write!(
                f,
                "Teilweise bereinigt — {} Rest(e): {reason}",
                remaining.len()
            ),
            Self::Unknown { format_hint } => match format_hint {
                Some(h) => write!(f, "Unbekanntes Format ({h}) — keine Aussage möglich"),
                None => f.write_str("Unbekanntes Format — keine Aussage möglich"),
            },
        }
    }
}

/// Ergebnis einer Inspektion, ohne die Datei zu verändern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspection {
    /// Erkanntes Format, soweit möglich.
    pub format: Option<String>,
    /// Gefundene Metadaten.
    pub findings: Vec<Finding>,
    /// Ob das Format überhaupt verstanden wurde.
    ///
    /// Ist das falsch, sagt eine leere Fundliste **nichts** über die
    /// Sauberkeit der Datei aus.
    pub understood: bool,
}

impl Inspection {
    /// Für ein nicht verstandenes Format.
    #[must_use]
    pub const fn not_understood(format: Option<String>) -> Self {
        Self {
            format,
            findings: Vec::new(),
            understood: false,
        }
    }

    /// Ob mindestens ein kritischer Fund vorliegt.
    #[must_use]
    pub fn has_critical(&self) -> bool {
        self.findings
            .iter()
            .any(|f| f.severity == Severity::Critical)
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Fehlschlag soll den Test abbrechen")]
mod tests {
    use super::*;

    fn fund(sev: Severity) -> Finding {
        Finding::new(FindingKind::Gps, "EXIF:GPS", Some("47.0, 8.0".into()), sev)
    }

    #[test]
    fn nur_complete_darf_gruen_sein() {
        assert!(StripResult::Complete { removed: vec![] }.may_show_clean());

        assert!(
            !StripResult::Partial {
                removed: vec![],
                remaining: vec![fund(Severity::Notable)],
                reason: "eingebettete Schriften".into(),
            }
            .may_show_clean()
        );

        assert!(
            !StripResult::Unknown { format_hint: None }.may_show_clean(),
            "das war der Fehler in v1"
        );
    }

    #[test]
    fn unbekanntes_format_behauptet_nichts() {
        let r = StripResult::Unknown {
            format_hint: Some("MP4".into()),
        };
        assert!(r.removed().is_empty());
        assert!(r.remaining().is_empty());
        assert!(!r.has_critical());
        assert!(r.to_string().contains("keine Aussage"));
    }

    #[test]
    fn kritische_funde_werden_gemeldet() {
        let r = StripResult::Complete {
            removed: vec![fund(Severity::Minor), fund(Severity::Critical)],
        };
        assert!(r.has_critical());

        let ohne = StripResult::Complete {
            removed: vec![fund(Severity::Minor)],
        };
        assert!(!ohne.has_critical());
    }

    #[test]
    fn kritisches_zaehlt_auch_wenn_es_bleiben_musste() {
        let r = StripResult::Partial {
            removed: vec![],
            remaining: vec![fund(Severity::Critical)],
            reason: "kann nicht entfernt werden".into(),
        };
        assert!(r.has_critical(), "Reste duerfen nicht untergehen");
    }

    #[test]
    fn lange_werte_werden_gekuerzt() {
        let lang = "x".repeat(500);
        let f = Finding::new(FindingKind::Comment, "test", Some(lang), Severity::Minor);
        let v = f.value.unwrap();
        assert_eq!(v.chars().count(), 201);
        assert!(v.ends_with('…'));
    }

    #[test]
    fn schweregrade_sind_geordnet() {
        assert!(Severity::Critical > Severity::Notable);
        assert!(Severity::Notable > Severity::Minor);
    }

    #[test]
    fn inspektion_ohne_verstaendnis_sagt_nichts_aus() {
        let i = Inspection::not_understood(Some("RAW".into()));
        assert!(!i.understood);
        assert!(i.findings.is_empty());
        assert!(
            !i.has_critical(),
            "aber das heisst nicht, dass die Datei sauber ist"
        );
    }
}
