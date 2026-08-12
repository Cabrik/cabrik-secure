//! ODF: `odt`, `ods`, `odp` (`spec/metadata.md` §4).
//!
//! Derselbe Behälter wie OOXML, andere Aufteilung. Die Metadaten stehen
//! gesammelt in `meta.xml` statt verteilt auf drei Teile — das macht die
//! Bereinigung übersichtlicher, ändert aber nichts an der Sache.
//!
//! # Was ODF zusätzlich verrät
//!
//! Zwei Angaben gibt es so nur hier, und beide sind aussagekräftiger, als sie
//! aussehen:
//!
//! - `meta:editing-duration` — die **Gesamtbearbeitungszeit**, etwa
//!   `PT4H12M30S`. Wer ein Schreiben als schnell hingeworfen darstellen will,
//!   verrät damit vier Stunden Arbeit.
//! - `meta:editing-cycles` — wie oft gespeichert wurde.
//!
//! Dazu nennt `meta:generator` nicht nur das Programm, sondern die
//! **Betriebssystemvariante**: `LibreOffice/7.4.2$Windows_X86_64`.
//!
//! # Die Reihenfolge im Archiv ist Teil des Formats
//!
//! ODF verlangt, dass der Eintrag `mimetype` **als erster** und
//! **unkomprimiert** im Archiv steht. Wird das verletzt, erkennen manche
//! Programme die Datei nicht mehr als ODF — sie sähe für den Nutzer kaputt
//! aus, obwohl jeder Teil für sich in Ordnung ist.
//!
//! [`crate::container`] erhält Reihenfolge und Kompressionsverfahren jedes
//! Eintrags. Ein Test hält das fest.

use crate::container::{self, Eintrag};
use crate::model::{Finding, FindingKind, Inspection, Severity, StripOptions, StripResult};
use crate::xml;

use cabrik_core::Result;

/// Welche Art von ODF-Dokument vorliegt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Art {
    /// Textdokument (`odt`).
    Text,
    /// Tabelle (`ods`).
    Tabelle,
    /// Präsentation (`odp`).
    Praesentation,
    /// Zeichnung, Formel und weitere.
    Sonstiges,
}

impl Art {
    /// Name für die Anzeige.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Text => "ODF (Text)",
            Self::Tabelle => "ODF (Tabelle)",
            Self::Praesentation => "ODF (Präsentation)",
            Self::Sonstiges => "ODF",
        }
    }
}

/// Erkennt ein ODF-Dokument an seinem `mimetype`-Eintrag.
#[must_use]
pub fn erkenne(eintraege: &[Eintrag]) -> Option<Art> {
    let typ = container::finde(eintraege, "mimetype")?.text()?.trim();
    if !typ.starts_with("application/vnd.oasis.opendocument.") {
        return None;
    }
    Some(match typ.rsplit('.').next().unwrap_or("") {
        "text" => Art::Text,
        "spreadsheet" => Art::Tabelle,
        "presentation" => Art::Praesentation,
        _ => Art::Sonstiges,
    })
}

/// Felder aus `meta.xml`.
const META_FELDER: [(&str, FindingKind, Severity); 13] = [
    ("initial-creator", FindingKind::Author, Severity::Critical),
    ("creator", FindingKind::Author, Severity::Critical),
    ("printed-by", FindingKind::Author, Severity::Critical),
    ("description", FindingKind::Comment, Severity::Critical),
    ("title", FindingKind::Comment, Severity::Notable),
    ("subject", FindingKind::Comment, Severity::Notable),
    ("keyword", FindingKind::Comment, Severity::Notable),
    ("date", FindingKind::Timestamp, Severity::Notable),
    ("creation-date", FindingKind::Timestamp, Severity::Notable),
    ("print-date", FindingKind::Timestamp, Severity::Notable),
    ("generator", FindingKind::Software, Severity::Notable),
    // Die beiden gibt es so nur in ODF — siehe Modulkopf.
    (
        "editing-duration",
        FindingKind::EditingSession,
        Severity::Notable,
    ),
    (
        "editing-cycles",
        FindingKind::EditingSession,
        Severity::Notable,
    ),
];

/// Eine leere `meta.xml`. Der Teil bleibt bestehen, damit das Manifest
/// nicht ins Leere zeigt — siehe `ooxml`, dort gilt dasselbe.
const META_HUELLE: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    "\n",
    r#"<office:document-meta "#,
    r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
    r#"xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0" "#,
    r#"xmlns:dc="http://purl.org/dc/elements/1.1/" "#,
    r#"office:version="1.3"><office:meta/></office:document-meta>"#
);

/// Eine leere `settings.xml`.
///
/// LibreOffice legt hier Ansichtseinstellungen ab — darunter den **Namen des
/// zuletzt verwendeten Druckers** und Pfade zuletzt geöffneter Dateien. Beides
/// hat mit dem Dokument nichts zu tun und wird vollständig ersetzt.
const SETTINGS_HUELLE: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    "\n",
    r#"<office:document-settings "#,
    r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
    r#"office:version="1.3"><office:settings/></office:document-settings>"#
);

// ---------------------------------------------------------------------------
// Inspektion
// ---------------------------------------------------------------------------

/// Untersucht ein ODF-Dokument.
///
/// # Fehler
///
/// [`cabrik_core::Error::Malformed`] bei kaputtem Container.
pub fn inspect(daten: &[u8]) -> Result<Inspection> {
    let eintraege = container::lies(daten)?;
    Ok(Inspection {
        format: Some(erkenne(&eintraege).map_or_else(|| "ODF".to_owned(), |a| a.name().to_owned())),
        findings: sammle(&eintraege),
        understood: true,
    })
}

fn sammle(eintraege: &[Eintrag]) -> Vec<Finding> {
    let mut funde = Vec::new();

    for e in eintraege {
        match e.name.as_str() {
            "meta.xml" => {
                meta_felder(&mut funde, e);
                benutzerfelder(&mut funde, e);
                vorlage(&mut funde, e);
            }
            "settings.xml" => einstellungen(&mut funde, e),
            "content.xml" => {
                aenderungen(&mut funde, e);
                anmerkungen(&mut funde, e);
            }
            _ => {}
        }

        if e.name.starts_with("Thumbnails/") && !e.verzeichnis && !e.inhalt.is_empty() {
            funde.push(Finding::new(
                FindingKind::EmbeddedPreview,
                format!("ODF:{}", e.name),
                Some(format!(
                    "Vorschaubild, {} Bytes — zeigt den Dokumentinhalt",
                    e.inhalt.len()
                )),
                Severity::Critical,
            ));
        }

        if ist_medium(&e.name) {
            eingebettete_medien(&mut funde, e);
        }
    }
    funde
}

fn meta_felder(funde: &mut Vec<Finding>, e: &Eintrag) {
    let Some(text) = e.text() else { return };
    for (name, art, schwere) in &META_FELDER {
        for wert in xml::element_texte(text, name) {
            if wert.trim().is_empty() {
                continue;
            }
            let erlaeutert = match *name {
                "editing-duration" => format!("{wert} — Gesamtbearbeitungszeit"),
                "editing-cycles" => format!("{wert} Speichervorgänge"),
                _ => wert,
            };
            funde.push(Finding::new(
                *art,
                format!("ODF:meta.xml/{name}"),
                Some(erlaeutert),
                *schwere,
            ));
        }
    }
}

/// `meta:user-defined` — frei belegbare Felder.
fn benutzerfelder(funde: &mut Vec<Finding>, e: &Eintrag) {
    let Some(text) = e.text() else { return };
    for name in xml::attribut_werte(text, "user-defined", "name") {
        funde.push(Finding::new(
            FindingKind::Comment,
            "ODF:meta.xml/user-defined".to_owned(),
            Some(format!("benutzerdefiniertes Feld „{name}\"")),
            Severity::Critical,
        ));
    }
}

/// `meta:template` verweist per `xlink:href` auf die Vorlage — nicht selten
/// mit einem **lokalen Pfad**, der den Benutzernamen enthält.
fn vorlage(funde: &mut Vec<Finding>, e: &Eintrag) {
    let Some(text) = e.text() else { return };
    for href in xml::attribut_werte(text, "template", "href") {
        if href.trim().is_empty() {
            continue;
        }
        let schwere = if href.contains(":\\") || href.contains("/home/") || href.contains("/Users/")
        {
            // Ein Pfad enthält fast immer den Benutzernamen.
            Severity::Critical
        } else {
            Severity::Notable
        };
        funde.push(Finding::new(
            FindingKind::Comment,
            "ODF:meta.xml/template".to_owned(),
            Some(format!("Vorlage: {href}")),
            schwere,
        ));
    }
}

/// `settings.xml` — Ansichtseinstellungen, darunter der Druckername.
fn einstellungen(funde: &mut Vec<Finding>, e: &Eintrag) {
    let Some(text) = e.text() else { return };
    let anzahl = xml::zaehle_elemente(text, "config-item");
    if anzahl == 0 {
        return;
    }
    let drucker = xml::element_texte(text, "config-item")
        .into_iter()
        .any(|w| w.contains("\\\\") || w.contains(":\\"));

    funde.push(Finding::new(
        FindingKind::UnknownExtension,
        "ODF:settings.xml".to_owned(),
        Some(if drucker {
            format!("{anzahl} Einstellungen, darunter Pfade oder Druckernamen")
        } else {
            format!("{anzahl} Ansichtseinstellungen")
        }),
        if drucker {
            Severity::Critical
        } else {
            Severity::Notable
        },
    ));
}

/// `text:tracked-changes` — dasselbe Problem wie in OOXML.
fn aenderungen(funde: &mut Vec<Finding>, e: &Eintrag) {
    let Some(text) = e.text() else { return };
    let anzahl = xml::zaehle_elemente(text, "changed-region");
    if anzahl == 0 {
        return;
    }
    let mut autoren = xml::element_texte(text, "creator");
    autoren.sort_unstable();
    autoren.dedup();
    let wer = if autoren.is_empty() {
        String::new()
    } else {
        format!(" von {}", autoren.join(", "))
    };

    funde.push(Finding::new(
        FindingKind::TrackedChange,
        "ODF:content.xml".to_owned(),
        Some(format!(
            "{anzahl} nachverfolgte Änderung(en){wer} — gelöschter Text steht \
             weiterhin vollständig im Dokument"
        )),
        Severity::Critical,
    ));
}

/// `office:annotation` — Kommentare.
fn anmerkungen(funde: &mut Vec<Finding>, e: &Eintrag) {
    let Some(text) = e.text() else { return };
    let anzahl = xml::zaehle_elemente(text, "annotation");
    if anzahl > 0 {
        funde.push(Finding::new(
            FindingKind::Comment,
            "ODF:content.xml/annotation".to_owned(),
            Some(format!("{anzahl} Kommentar(e) mit Namen und Zeitpunkten")),
            Severity::Critical,
        ));
    }
}

fn eingebettete_medien(funde: &mut Vec<Finding>, e: &Eintrag) {
    let Ok(inner) = crate::inspect(&e.inhalt) else {
        return;
    };
    for f in inner.findings {
        funde.push(Finding::new(
            f.kind,
            format!("ODF:{} → {}", e.name, f.location),
            Some(f.value.unwrap_or_else(|| "eingebettetes Medium".to_owned())),
            f.severity,
        ));
    }
}

fn ist_medium(name: &str) -> bool {
    let unten = name.to_ascii_lowercase();
    unten.starts_with("pictures/")
        && (unten.ends_with(".png") || unten.ends_with(".jpg") || unten.ends_with(".jpeg"))
}

// ---------------------------------------------------------------------------
// Bereinigung
// ---------------------------------------------------------------------------

/// Elemente einer nachverfolgten Änderung in ODF.
///
/// `text:tracked-changes` sammelt die Änderungsbeschreibungen; `text:deletion`
/// darin enthält den gelöschten Text.
const AENDERUNG_VERWERFEN: [&str; 2] = ["tracked-changes", "annotation-end"];

/// `text:change-start`, `text:change-end` und `text:change` sind leere Marken
/// im Fließtext; sie verschwinden ersatzlos.
const AENDERUNG_MARKEN: [&str; 3] = ["change-start", "change-end", "change"];

/// Bereinigt ein ODF-Dokument — nur Metadaten.
///
/// # Fehler
///
/// [`cabrik_core::Error::Malformed`] bei kaputtem Container.
pub fn strip(daten: &[u8]) -> Result<(Vec<u8>, StripResult)> {
    strip_with(daten, StripOptions::nur_metadaten())
}

/// Bereinigt ein ODF-Dokument mit ausdrücklichen Optionen.
///
/// # Fehler
///
/// [`cabrik_core::Error::Malformed`] bei kaputtem Container.
pub fn strip_with(daten: &[u8], opts: StripOptions) -> Result<(Vec<u8>, StripResult)> {
    let eintraege = container::lies(daten)?;
    let alle = sammle(&eintraege);

    let mut entfernt = Vec::new();
    let mut geblieben = Vec::new();
    for f in alle {
        if bleibt_erhalten(&f, opts) {
            geblieben.push(f);
        } else {
            entfernt.push(f);
        }
    }

    let mut neu: Vec<Eintrag> = Vec::with_capacity(eintraege.len());
    for e in eintraege {
        // Das Vorschaubild verschwindet samt seinem Manifest-Eintrag.
        if e.name.starts_with("Thumbnails/") {
            continue;
        }

        let inhalt = match e.name.as_str() {
            "meta.xml" => META_HUELLE.as_bytes().to_vec(),
            "settings.xml" => SETTINGS_HUELLE.as_bytes().to_vec(),
            "META-INF/manifest.xml" => e.text().map_or_else(
                || e.inhalt.clone(),
                |t| xml::entferne_manifest_eintraege(t, "Thumbnails/").into_bytes(),
            ),
            "content.xml" if opts.greift_in_den_inhalt_ein() => e.text().map_or_else(
                || e.inhalt.clone(),
                |t| behandle_inhalt(t, opts).into_bytes(),
            ),
            _ if ist_medium(&e.name) => {
                crate::strip(&e.inhalt).map_or_else(|_| e.inhalt.clone(), |(sauber, _)| sauber)
            }
            _ => e.inhalt.clone(),
        };

        neu.push(Eintrag { inhalt, ..e });
    }

    let aus = container::schreib(&neu)?;
    let ergebnis = if geblieben.is_empty() {
        StripResult::Complete { removed: entfernt }
    } else {
        StripResult::Partial {
            removed: entfernt,
            remaining: geblieben,
            reason: "Kommentare und nachverfolgte Änderungen sind Bestandteil des \
                     Dokuments, nicht Beiwerk. Sie lassen sich auf ausdrückliche \
                     Anweisung auflösen."
                .to_owned(),
        }
    };
    Ok((aus, ergebnis))
}

fn behandle_inhalt(text: &str, opts: StripOptions) -> String {
    let mut verwerfen: Vec<&str> = Vec::new();
    let mut entpacken: Vec<&str> = Vec::new();

    if opts.accept_changes {
        verwerfen.extend(AENDERUNG_VERWERFEN);
        verwerfen.extend(AENDERUNG_MARKEN);
    }
    if opts.remove_comments {
        verwerfen.push("annotation");
        entpacken.push("annotation-end");
    }

    if verwerfen.is_empty() && entpacken.is_empty() {
        return text.to_owned();
    }
    xml::forme_um(text, &verwerfen, &entpacken)
}

fn bleibt_erhalten(f: &Finding, opts: StripOptions) -> bool {
    match f.kind {
        FindingKind::TrackedChange => !opts.accept_changes,
        FindingKind::Comment => f.location.contains("annotation") && !opts.remove_comments,
        _ => false,
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;

    fn eintrag(name: &str, inhalt: &str, komprimiert: bool) -> Eintrag {
        Eintrag {
            name: name.to_owned(),
            inhalt: inhalt.as_bytes().to_vec(),
            komprimiert,
            verzeichnis: false,
        }
    }

    fn dokument() -> Vec<u8> {
        let eintraege = vec![
            // Muss der erste Eintrag sein und unkomprimiert.
            eintrag("mimetype", "application/vnd.oasis.opendocument.text", false),
            eintrag(
                "META-INF/manifest.xml",
                concat!(
                    r#"<?xml version="1.0"?><manifest:manifest xmlns:manifest="m">"#,
                    r#"<manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.text"/>"#,
                    r#"<manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>"#,
                    r#"<manifest:file-entry manifest:full-path="Thumbnails/thumbnail.png" manifest:media-type="image/png"/>"#,
                    r#"</manifest:manifest>"#
                ),
                true,
            ),
            eintrag(
                "meta.xml",
                concat!(
                    r#"<?xml version="1.0"?><office:document-meta xmlns:office="o" xmlns:meta="m" xmlns:dc="d" xmlns:xlink="x"><office:meta>"#,
                    r#"<meta:generator>LibreOffice/7.4.2$Windows_X86_64</meta:generator>"#,
                    r#"<meta:initial-creator>Dr. Anna Beispiel</meta:initial-creator>"#,
                    r#"<dc:creator>Prof. Carl Chef</dc:creator>"#,
                    r#"<dc:description>Nicht an den Kunden geben</dc:description>"#,
                    r#"<meta:editing-duration>PT4H12M30S</meta:editing-duration>"#,
                    r#"<meta:editing-cycles>23</meta:editing-cycles>"#,
                    r#"<meta:template xlink:href="C:\Users\daniw\Vorlagen\Kanzlei.ott"/>"#,
                    r#"<meta:user-defined meta:name="Aktenzeichen">2026-0815</meta:user-defined>"#,
                    r#"</office:meta></office:document-meta>"#
                ),
                true,
            ),
            eintrag(
                "settings.xml",
                concat!(
                    r#"<?xml version="1.0"?><office:document-settings xmlns:office="o" xmlns:config="c"><office:settings>"#,
                    r#"<config:config-item config:name="PrinterName">\\SERVER\Kanzlei-Drucker</config:config-item>"#,
                    r#"</office:settings></office:document-settings>"#
                ),
                true,
            ),
            eintrag(
                "content.xml",
                concat!(
                    r#"<?xml version="1.0"?><office:document-content xmlns:office="o" xmlns:text="t" xmlns:dc="d"><office:body><office:text>"#,
                    r#"<text:tracked-changes><text:changed-region text:id="ct1"><text:deletion>"#,
                    r#"<office:change-info><dc:creator>Prof. Carl Chef</dc:creator><dc:date>2026-03-02T11:00:00</dc:date></office:change-info>"#,
                    r#"<text:p>GEHEIM-GELOESCHT</text:p></text:deletion></text:changed-region></text:tracked-changes>"#,
                    r#"<text:p>Sehr geehrte Damen und Herren, </text:p>"#,
                    r#"<text:p>Preis<office:annotation><dc:creator>Prof. Carl Chef</dc:creator>"#,
                    r#"<text:p>zu hoch, aber nicht sagen</text:p></office:annotation> steht fest.</text:p>"#,
                    r#"</office:text></office:body></office:document-content>"#
                ),
                true,
            ),
            eintrag("Thumbnails/thumbnail.png", "\u{FFFD}PNG-Vorschau", false),
        ];
        container::schreib(&eintraege).unwrap()
    }

    fn hat(funde: &[Finding], teil: &str) -> bool {
        funde.iter().any(|f| f.location.contains(teil))
    }

    #[test]
    fn art_wird_am_mimetype_erkannt() {
        let e = container::lies(&dokument()).unwrap();
        assert_eq!(erkenne(&e), Some(Art::Text));
    }

    #[test]
    fn ein_gewoehnliches_zip_ist_kein_odf() {
        let e =
            container::lies(&container::schreib(&[eintrag("a.txt", "nur Text", false)]).unwrap())
                .unwrap();
        assert_eq!(erkenne(&e), None);
    }

    /// Die beiden Angaben, die es so nur in ODF gibt.
    #[test]
    fn bearbeitungsdauer_und_zyklen_werden_gemeldet() {
        let i = inspect(&dokument()).unwrap();
        let dauer = i
            .findings
            .iter()
            .find(|f| f.location.contains("editing-duration"))
            .expect("Bearbeitungsdauer nicht gefunden");
        assert!(
            dauer.value.as_deref().unwrap_or_default().contains("PT4H"),
            "{dauer:?}"
        );
        assert!(hat(&i.findings, "editing-cycles"));
    }

    /// Ein Vorlagenpfad enthaelt fast immer den Benutzernamen.
    #[test]
    fn ein_vorlagenpfad_ist_kritisch() {
        let i = inspect(&dokument()).unwrap();
        let f = i
            .findings
            .iter()
            .find(|f| f.location.contains("template"))
            .expect("Vorlage nicht gefunden");
        assert_eq!(f.severity, Severity::Critical);
        assert!(f.value.as_deref().unwrap_or_default().contains("daniw"));
    }

    /// Der Druckername gehoert nicht zum Dokument.
    #[test]
    fn der_druckername_wird_gefunden_und_entfernt() {
        let i = inspect(&dokument()).unwrap();
        let f = i
            .findings
            .iter()
            .find(|f| f.location.contains("settings.xml"))
            .expect("settings.xml nicht geprueft");
        assert_eq!(f.severity, Severity::Critical);

        let (sauber, _) = strip(&dokument()).unwrap();
        let e = container::lies(&sauber).unwrap();
        assert!(
            !container::finde(&e, "settings.xml")
                .unwrap()
                .text()
                .unwrap()
                .contains("Kanzlei-Drucker")
        );
    }

    /// **Die Formatregel.** `mimetype` muss der erste Eintrag und
    /// unkomprimiert bleiben, sonst erkennen manche Programme die Datei nicht
    /// mehr als ODF.
    #[test]
    fn mimetype_bleibt_erster_eintrag_und_unkomprimiert() {
        let (sauber, _) = strip(&dokument()).unwrap();
        let e = container::lies(&sauber).unwrap();

        let erster = e.first().expect("leeres Archiv");
        assert_eq!(erster.name, "mimetype", "mimetype ist nicht mehr der erste");
        assert!(
            !erster.komprimiert,
            "mimetype wurde komprimiert — ODF verbietet das"
        );
        assert_eq!(
            erster.text(),
            Some("application/vnd.oasis.opendocument.text")
        );
    }

    #[test]
    fn die_metadaten_verschwinden() {
        let (sauber, _) = strip(&dokument()).unwrap();
        let e = container::lies(&sauber).unwrap();
        let meta = container::finde(&e, "meta.xml").unwrap().text().unwrap();

        for spur in [
            "Anna Beispiel",
            "Carl Chef",
            "Nicht an den Kunden",
            "PT4H12M30S",
            "daniw",
            "Aktenzeichen",
            "Windows_X86_64",
        ] {
            assert!(!meta.contains(spur), "„{spur}\" blieb in meta.xml");
        }
    }

    #[test]
    fn das_vorschaubild_verschwindet_samt_manifest_eintrag() {
        let (sauber, _) = strip(&dokument()).unwrap();
        let e = container::lies(&sauber).unwrap();

        assert!(container::finde(&e, "Thumbnails/thumbnail.png").is_none());
        let manifest = container::finde(&e, "META-INF/manifest.xml")
            .unwrap()
            .text()
            .unwrap();
        assert!(
            !manifest.contains("Thumbnails"),
            "der Manifest-Eintrag zeigt ins Leere: {manifest}"
        );
        assert!(
            manifest.contains("content.xml"),
            "andere Eintraege wurden mitentfernt"
        );
    }

    /// Voreinstellung: Der Inhalt bleibt unangetastet, wird aber gemeldet.
    #[test]
    fn ohne_anweisung_bleiben_aenderungen_und_kommentare() {
        let (sauber, ergebnis) = strip(&dokument()).unwrap();
        let e = container::lies(&sauber).unwrap();
        let inhalt = container::finde(&e, "content.xml").unwrap().text().unwrap();

        assert!(inhalt.contains("GEHEIM-GELOESCHT"));
        assert!(inhalt.contains("zu hoch, aber nicht sagen"));
        assert!(!ergebnis.may_show_clean());
    }

    /// Mit Anweisung verschwinden beide — der gewoehnliche Text bleibt.
    #[test]
    fn mit_anweisung_verschwinden_aenderungen_und_kommentare() {
        let (sauber, ergebnis) =
            strip_with(&dokument(), StripOptions::auch_inhaltliche_reste()).unwrap();
        let e = container::lies(&sauber).unwrap();
        let inhalt = container::finde(&e, "content.xml").unwrap().text().unwrap();

        assert!(
            !inhalt.contains("GEHEIM-GELOESCHT"),
            "geloeschter Text blieb: {inhalt}"
        );
        assert!(
            !inhalt.contains("zu hoch, aber nicht sagen"),
            "Kommentar blieb: {inhalt}"
        );
        assert!(
            inhalt.contains("Sehr geehrte Damen und Herren, "),
            "der Text ging verloren: {inhalt}"
        );
        assert!(inhalt.contains("steht fest."), "{inhalt}");
        assert!(ergebnis.may_show_clean(), "{ergebnis:?}");
    }

    #[test]
    fn die_bereinigung_ist_wiederholbar() {
        assert_eq!(strip(&dokument()).unwrap().0, strip(&dokument()).unwrap().0);
    }
}
