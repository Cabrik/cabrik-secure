//! PDF (`spec/metadata.md` §4.1).
//!
//! Das folgenreichste Format dieses ganzen Moduls — nicht wegen seiner
//! Metadaten, sondern wegen seiner **Änderungshistorie**.
//!
//! # Der Fall, der Schaden anrichtet
//!
//! PDF speichert Änderungen, indem es sie **anhängt**, statt zu ersetzen. Wer
//! eine Stelle unkenntlich macht und speichert, erzeugt eine Datei, die beides
//! enthält:
//!
//! ```text
//! Was jeder Leser anzeigt:   Interne Marge: XXXXXXXXXXX
//! Was in der Datei steht:    Interne Marge: 38 Prozent.
//! ```
//!
//! Ein Firmenname in den Dokumenteigenschaften ist peinlich. Eine lesbare
//! Schwärzung kann existenzbedrohend sein.
//!
//! # Warum das Neuschreiben hilft
//!
//! Beim Laden wird für jedes Objekt nur die **jüngste** Fassung aufgelöst. Wer
//! das Ergebnis frisch schreibt, hat die Historie schlicht nicht mehr dabei —
//! zusammen mit den Dokumenteigenschaften, dem XMP-Block und der Dateikennung
//! `/ID`.
//!
//! # Jede Fassung ist ein vollständiges PDF
//!
//! Jede inkrementelle Änderung endet mit `%%EOF`. Schneidet man dort ab, hat
//! man ein **gültiges** früheres PDF — so ist das Format definiert. Deshalb
//! lassen sich die Fassungen einzeln anzeigen und einzeln einflachen, ohne
//! irgendetwas nachzubauen.
//!
//! # Zwei Fälle, in denen nicht neu geschrieben wird
//!
//! - **Signiert.** Eine Signatur deckt einen Byte-Bereich ab; jede Änderung
//!   macht sie ungültig. Aus einem beweiskräftigen Dokument würde ein
//!   wertloses. Wird erkannt und abgelehnt — die Funde werden trotzdem
//!   gemeldet.
//! - **Verschlüsselt mit Öffnungspasswort.** Ohne Passwort geht nichts. Ein
//!   Passwort zu **raten** kommt nicht in Frage: Das wäre ein Knacker, für ein
//!   Sicherheitswerkzeug das falsche Signal. Ein PDF, das nur Rechte
//!   einschränkt (leeres Benutzerpasswort), wird dagegen ohne Nachfrage
//!   geöffnet — das ist der häufige Fall.

use crate::model::{Finding, FindingKind, Inspection, Severity, StripResult};

use cabrik_core::{Error, Result};
use lopdf::{Document, Object};

/// Höchstgröße einer Datei, die wir anfassen.
const MAX_DATEI: usize = 256 * 1024 * 1024;
/// Höchstmenge entpackter Daten bei der Textvorschau — Schutz gegen
/// Dekomprimierungsbomben.
const MAX_TEXT: usize = 8 * 1024 * 1024;
/// Höchstzahl der Fassungen, die verfolgt werden.
const MAX_FASSUNGEN: usize = 64;
/// Länge des Textauszugs je Fassung.
const AUSZUG_MAX: usize = 400;

/// Ob die Bytes wie ein PDF aussehen.
///
/// Die Kennung darf laut Norm bis zu 1024 Bytes weit hinten stehen; in der
/// Praxis steht sie am Anfang. Beides wird abgedeckt.
#[must_use]
pub fn looks_like_pdf(daten: &[u8]) -> bool {
    daten
        .get(..1024.min(daten.len()))
        .is_some_and(|k| k.windows(5).any(|f| f == b"%PDF-"))
}

// ---------------------------------------------------------------------------
// Fassungen
// ---------------------------------------------------------------------------

/// Eine Fassung des Dokuments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fassung {
    /// Zählung ab eins, älteste zuerst.
    pub nummer: usize,
    /// Länge der Datei bis zum Ende dieser Fassung.
    pub bytes: usize,
    /// Zahl der Seiten.
    pub seiten: usize,
    /// Ob dies die Fassung ist, die ein Leser anzeigt.
    pub ist_aktuell: bool,
    /// Anfang des Textes, gekürzt.
    pub auszug: String,
    /// Zeilen, die es **nur hier** gibt — also später entfernt wurden.
    ///
    /// Das ist die eigentliche Auskunft: nicht „wie sah diese Fassung aus",
    /// sondern „was wurde herausgenommen".
    pub nur_hier: Vec<String>,
}

/// Findet die Byte-Grenzen aller Fassungen.
///
/// Jede inkrementelle Änderung endet mit `%%EOF`.
fn grenzen(daten: &[u8]) -> Vec<usize> {
    let mut aus = Vec::new();
    let mut p = 0usize;
    while let Some(i) = daten.get(p..).and_then(|r| finde_muster(r, b"%%EOF")) {
        let ende = p.saturating_add(i).saturating_add(5);
        // Ein folgender Zeilenumbruch gehört noch dazu.
        let mut ende = ende;
        if daten.get(ende) == Some(&b'\r') {
            ende = ende.saturating_add(1);
        }
        if daten.get(ende) == Some(&b'\n') {
            ende = ende.saturating_add(1);
        }
        aus.push(ende);
        p = ende;
        if aus.len() >= MAX_FASSUNGEN {
            break;
        }
    }
    aus
}

fn finde_muster(heu: &[u8], nadel: &[u8]) -> Option<usize> {
    if nadel.is_empty() || heu.len() < nadel.len() {
        return None;
    }
    heu.windows(nadel.len()).position(|f| f == nadel)
}

/// Lädt ein Dokument und entschlüsselt es, soweit das ohne Passwort geht.
///
/// # Fehler
///
/// [`Error::Malformed`], wenn die Datei nicht lesbar ist oder ein
/// Öffnungspasswort braucht.
fn lade(daten: &[u8], passwort: Option<&str>) -> Result<Document> {
    let mut doc = Document::load_mem(daten).map_err(|_| Error::Malformed("pdf: nicht lesbar"))?;

    if doc.is_encrypted() {
        // Zuerst das leere Passwort. Das deckt den häufigen Fall ab: ein PDF,
        // das nur Drucken oder Kopieren einschränkt, aber ohne Passwort
        // aufgeht. Erst danach das angegebene.
        let versuche: [&str; 2] = [passwort.unwrap_or(""), ""];
        let mut geoeffnet = false;
        for pw in versuche {
            if doc.decrypt(pw).is_ok() {
                geoeffnet = true;
                break;
            }
        }
        if !geoeffnet {
            return Err(Error::Malformed("pdf: braucht ein Oeffnungspasswort"));
        }
    }
    Ok(doc)
}

/// Textauszug und Zeilenmenge einer Fassung.
fn text_von(doc: &Document) -> (String, Vec<String>) {
    let seiten: Vec<u32> = doc.get_pages().keys().copied().collect();
    let text = doc
        .extract_text_with_limit(&seiten, MAX_TEXT)
        .unwrap_or_default();

    let zeilen: Vec<String> = text
        .lines()
        .map(|z| z.trim().to_owned())
        .filter(|z| !z.is_empty())
        .collect();

    let auszug = kuerze(&zeilen.join(" · "));
    (auszug, zeilen)
}

fn kuerze(s: &str) -> String {
    if s.chars().count() <= AUSZUG_MAX {
        return s.to_owned();
    }
    let gekuerzt: String = s.chars().take(AUSZUG_MAX).collect();
    format!("{gekuerzt}…")
}

/// Listet alle Fassungen mit Vorschau.
///
/// # Fehler
///
/// [`Error::Malformed`] bei unlesbarer Datei.
pub fn fassungen(daten: &[u8], passwort: Option<&str>) -> Result<Vec<Fassung>> {
    if daten.len() > MAX_DATEI {
        return Err(Error::Malformed("pdf: Datei zu gross"));
    }
    let mut enden = grenzen(daten);
    // Manche Erzeuger lassen das abschließende `%%EOF` weg.
    if enden.last().copied() != Some(daten.len()) {
        enden.push(daten.len());
    }

    // Zuerst die aktuelle Fassung — sie ist der Vergleichsmaßstab.
    let aktuell = lade(daten, passwort)?;
    let (_, aktuelle_zeilen) = text_von(&aktuell);

    let mut aus = Vec::with_capacity(enden.len());
    for (i, ende) in enden.iter().enumerate() {
        let ist_aktuell = i.saturating_add(1) == enden.len();
        let teil = daten.get(..*ende).unwrap_or(daten);

        let Ok(doc) = lade(teil, passwort) else {
            // Eine Fassung, die sich nicht laden lässt, wird gemeldet statt
            // verschwiegen — sonst fehlte sie in der Zählung.
            aus.push(Fassung {
                nummer: i.saturating_add(1),
                bytes: *ende,
                seiten: 0,
                ist_aktuell,
                auszug: "(diese Fassung ließ sich nicht lesen)".to_owned(),
                nur_hier: Vec::new(),
            });
            continue;
        };

        let (auszug, zeilen) = text_von(&doc);
        let nur_hier: Vec<String> = if ist_aktuell {
            Vec::new()
        } else {
            zeilen
                .iter()
                .filter(|z| !aktuelle_zeilen.contains(z))
                .cloned()
                .collect()
        };

        aus.push(Fassung {
            nummer: i.saturating_add(1),
            bytes: *ende,
            seiten: doc.get_pages().len(),
            ist_aktuell,
            auszug,
            nur_hier,
        });
    }
    Ok(aus)
}

// ---------------------------------------------------------------------------
// Untersuchen
// ---------------------------------------------------------------------------

/// Ob das Dokument eine digitale Signatur trägt.
fn ist_signiert(daten: &[u8]) -> bool {
    // `/ByteRange` kommt praktisch nur in Signaturen vor und steht im
    // Klartext, auch wenn die Objekte komprimiert sind.
    finde_muster(daten, b"/ByteRange").is_some() || finde_muster(daten, b"/Sig").is_some()
}

/// Felder des Dokumenteigenschaften-Wörterbuchs.
const INFO_FELDER: [(&str, FindingKind, Severity); 8] = [
    ("Author", FindingKind::Author, Severity::Critical),
    ("Creator", FindingKind::Software, Severity::Notable),
    ("Producer", FindingKind::Software, Severity::Notable),
    ("Title", FindingKind::Comment, Severity::Notable),
    ("Subject", FindingKind::Comment, Severity::Notable),
    ("Keywords", FindingKind::Comment, Severity::Notable),
    ("CreationDate", FindingKind::Timestamp, Severity::Notable),
    ("ModDate", FindingKind::Timestamp, Severity::Notable),
];

fn als_text(o: &Object) -> Option<String> {
    match o {
        Object::String(b, _) => {
            let s = String::from_utf8_lossy(b).trim().to_owned();
            (!s.is_empty()).then_some(s)
        }
        Object::Name(b) => Some(String::from_utf8_lossy(b).into_owned()),
        _ => None,
    }
}

fn sammle(daten: &[u8], doc: &Document, fassungen: &[Fassung]) -> Vec<Finding> {
    let mut funde = Vec::new();

    // --- Dokumenteigenschaften ---
    if let Ok(Object::Dictionary(info)) = doc
        .trailer
        .get(b"Info")
        .and_then(|o| doc.dereference(o).map(|(_, x)| x.clone()))
    {
        for (name, art, schwere) in &INFO_FELDER {
            if let Ok(wert) = info.get(name.as_bytes())
                && let Some(text) = als_text(wert)
            {
                funde.push(Finding::new(
                    *art,
                    format!("PDF:Info/{name}"),
                    Some(text),
                    *schwere,
                ));
            }
        }
    }

    // --- XMP ---
    if let Ok(katalog) = doc.catalog()
        && katalog.get(b"Metadata").is_ok()
    {
        funde.push(Finding::new(
            FindingKind::Author,
            "PDF:XMP".to_owned(),
            Some(
                "XMP-Block — trägt Verfasser, erzeugendes Programm und oft einen \
                 Bearbeitungsverlauf"
                    .to_owned(),
            ),
            Severity::Critical,
        ));
    }

    // --- Dateikennung ---
    if doc.trailer.get(b"ID").is_ok() {
        funde.push(Finding::new(
            FindingKind::UnknownExtension,
            "PDF:ID".to_owned(),
            Some(
                "Dateikennung — bleibt über Speichervorgänge hinweg gleich und \
                 verknüpft Fassungen desselben Dokuments"
                    .to_owned(),
            ),
            Severity::Notable,
        ));
    }

    // --- Änderungshistorie: der wichtigste Fund ---
    let frueher: Vec<&Fassung> = fassungen.iter().filter(|f| !f.ist_aktuell).collect();
    if !frueher.is_empty() {
        let entfernte: Vec<String> = frueher
            .iter()
            .flat_map(|f| f.nur_hier.iter().cloned())
            .collect();

        let wert = if entfernte.is_empty() {
            format!(
                "{} frühere Fassung(en) — kein Leser zeigt sie an, die Daten stehen \
                 aber vollständig in der Datei",
                frueher.len()
            )
        } else {
            format!(
                "{} frühere Fassung(en). Nur dort steht: „{}\" — das zeigt kein \
                 Leser an, es ist aber lesbar",
                frueher.len(),
                kuerze(&entfernte.join(" · "))
            )
        };

        funde.push(Finding::new(
            FindingKind::TrackedChange,
            "PDF:Änderungshistorie".to_owned(),
            Some(wert),
            Severity::Critical,
        ));
    }

    // --- Was bleiben muss ---
    if finde_muster(daten, b"/FontFile").is_some() {
        funde.push(Finding::new(
            FindingKind::Software,
            "PDF:Schriften".to_owned(),
            Some(
                "eingebettete Schriften — tragen Hersteller- und Lizenzangaben. Sie \
                 zu entfernen zerstört die Darstellung"
                    .to_owned(),
            ),
            Severity::Minor,
        ));
    }
    if finde_muster(daten, b"/EmbeddedFile").is_some() {
        funde.push(Finding::new(
            FindingKind::UnknownExtension,
            "PDF:Anhänge".to_owned(),
            Some("angehängte Dateien — Inhalt und Herkunft nicht überschaubar".to_owned()),
            Severity::Critical,
        ));
    }
    if finde_muster(daten, b"/JavaScript").is_some() || finde_muster(daten, b"/JS").is_some() {
        funde.push(Finding::new(
            FindingKind::UnknownExtension,
            "PDF:JavaScript".to_owned(),
            Some("ausführbarer Code im Dokument".to_owned()),
            Severity::Critical,
        ));
    }
    if ist_signiert(daten) {
        funde.push(Finding::new(
            FindingKind::UnknownExtension,
            "PDF:Signatur".to_owned(),
            Some(
                "digitale Signatur — das Dokument wird deshalb nicht neu geschrieben, \
                 weil jede Änderung sie ungültig macht"
                    .to_owned(),
            ),
            Severity::Notable,
        ));
    }

    funde
}

/// Untersucht ein PDF.
///
/// # Fehler
///
/// [`Error::Malformed`] bei unlesbarer Datei oder fehlendem Passwort.
pub fn inspect(daten: &[u8]) -> Result<Inspection> {
    inspect_mit(daten, None)
}

/// Untersucht ein PDF mit Passwort.
///
/// # Fehler
///
/// [`Error::Malformed`] bei unlesbarer Datei oder falschem Passwort.
pub fn inspect_mit(daten: &[u8], passwort: Option<&str>) -> Result<Inspection> {
    let doc = lade(daten, passwort)?;
    let f = fassungen(daten, passwort)?;
    let anzahl = f.len();

    Ok(Inspection {
        format: Some(if anzahl > 1 {
            format!("PDF ({anzahl} Fassungen)")
        } else {
            "PDF".to_owned()
        }),
        findings: sammle(daten, &doc, &f),
        understood: true,
    })
}

// ---------------------------------------------------------------------------
// Bereinigen
// ---------------------------------------------------------------------------

/// Wie mit den Fassungen umgegangen wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verlauf {
    /// Die angezeigte Fassung einflachen — die Voreinstellung.
    #[default]
    Aktuelle,
    /// Eine bestimmte Fassung einflachen, gezählt ab eins.
    Fassung(usize),
    /// Nichts verändern.
    ///
    /// Für Fälle, in denen das Dokument nicht verändert werden **darf** —
    /// Beweismittel, Archivierung. Dieselbe Kategorie wie eine Signatur.
    Behalten,
}

/// Bereinigt ein PDF.
///
/// # Fehler
///
/// [`Error::Malformed`] bei unlesbarer Datei, fehlendem Passwort oder
/// unbekannter Fassungsnummer.
pub fn strip(daten: &[u8]) -> Result<(Vec<u8>, StripResult)> {
    strip_mit(daten, Verlauf::Aktuelle, None)
}

/// Bereinigt ein PDF mit Wahl der Fassung und Passwort.
///
/// # Fehler
///
/// [`Error::Malformed`] bei unlesbarer Datei, fehlendem Passwort oder
/// unbekannter Fassungsnummer.
pub fn strip_mit(
    daten: &[u8],
    verlauf: Verlauf,
    passwort: Option<&str>,
) -> Result<(Vec<u8>, StripResult)> {
    let f = fassungen(daten, passwort)?;
    let doc = lade(daten, passwort)?;
    let alle = sammle(daten, &doc, &f);

    // Eine Signatur ist ein Abbruchgrund, kein Hindernis, das man umgeht.
    if ist_signiert(daten) {
        return Ok((
            daten.to_vec(),
            StripResult::Partial {
                removed: Vec::new(),
                remaining: alle,
                reason: "Das Dokument ist digital signiert. Eine Signatur deckt einen \
                         Byte-Bereich der Datei ab; jede Änderung macht sie ungültig. \
                         Aus einem beweiskräftigen Dokument würde ein wertloses — \
                         deshalb wurde nichts verändert. Wer die Bereinigung braucht, \
                         muss sich bewusst gegen die Signatur entscheiden."
                    .to_owned(),
            },
        ));
    }

    if verlauf == Verlauf::Behalten {
        return Ok((
            daten.to_vec(),
            StripResult::Partial {
                removed: Vec::new(),
                remaining: alle,
                reason: "Auf ausdrückliche Anweisung wurde nichts verändert. Die Datei \
                         enthält weiterhin alle früheren Fassungen — wer sie öffnet, \
                         sieht die jüngste, kann die älteren aber wiederherstellen."
                    .to_owned(),
            },
        ));
    }

    // Welche Fassung wird eingeflacht?
    let gewaehlt = match verlauf {
        Verlauf::Fassung(n) => f
            .iter()
            .find(|x| x.nummer == n)
            .ok_or(Error::Malformed("pdf: diese Fassung gibt es nicht"))?,
        _ => f
            .last()
            .ok_or(Error::Malformed("pdf: keine Fassung gefunden"))?,
    };

    let teil = daten.get(..gewaehlt.bytes).unwrap_or(daten);
    let mut doc = lade(teil, passwort)?;

    // Das eigentliche Entfernen. Die Reihenfolge ist wichtig:
    //
    // 1. Verweise lösen. Danach zeigt kein Leser die Angaben mehr an —
    //    **die Objekte stehen aber weiterhin in der Datei.** Genau hier hört
    //    ein oberflächliches Werkzeug auf, und genau das wäre der v1-Fehler
    //    in neuer Gestalt: Ein Prüfprogramm meldet „keine Metadaten", während
    //    der Verfassername byteweise noch drinsteht.
    // 2. Die verwaisten Objekte tatsächlich wegwerfen.
    let info_id = doc
        .trailer
        .get(b"Info")
        .ok()
        .and_then(|o| o.as_reference().ok());
    let xmp_id = doc
        .catalog()
        .ok()
        .and_then(|k| k.get(b"Metadata").ok())
        .and_then(|o| o.as_reference().ok());

    doc.trailer.remove(b"Info");
    doc.trailer.remove(b"ID");
    if let Ok(katalog) = doc.catalog_mut() {
        katalog.remove(b"Metadata");
    }
    for id in [info_id, xmp_id].into_iter().flatten() {
        doc.delete_object(id);
    }
    // Was jetzt von nirgends mehr erreichbar ist, hat in der Datei nichts
    // verloren.
    doc.prune_objects();
    doc.compress();

    let mut aus = Vec::new();
    doc.save_to(&mut aus)
        .map_err(|_| Error::Malformed("pdf: liess sich nicht neu schreiben"))?;

    // Eingeflacht wurde alles bis auf das, was in der Datei bleiben muss.
    let (geblieben, entfernt): (Vec<Finding>, Vec<Finding>) = alle
        .into_iter()
        .partition(|x| matches!(x.location.as_str(), "PDF:Schriften" | "PDF:Anhänge"));

    Ok((
        aus,
        StripResult::Partial {
            removed: entfernt,
            remaining: geblieben,
            reason: "PDF ist kein Dateiformat, sondern ein Objektgraph, der sich \
                     unbegrenzt erweitern lässt. Änderungshistorie, \
                     Dokumenteigenschaften, XMP und Dateikennung wurden entfernt; \
                     eingebettete Schriften und Anhänge bleiben, weil ihr Entfernen \
                     die Darstellung zerstörte. Eine Zusage auf Vollständigkeit wäre \
                     bei diesem Format nicht haltbar."
                .to_owned(),
        },
    ))
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;

    #[test]
    fn pdf_wird_an_der_kennung_erkannt() {
        assert!(looks_like_pdf(b"%PDF-1.7\n..."));
        // Manche Dateien haben Muell davor -- die Norm erlaubt das.
        assert!(looks_like_pdf(b"vorspann\n%PDF-1.4\n"));
        assert!(!looks_like_pdf(b"kein PDF"));
        assert!(!looks_like_pdf(b""));
    }

    #[test]
    fn die_fassungsgrenzen_werden_gefunden() {
        let d = b"%PDF-1.4\nInhalt\n%%EOF\nmehr\n%%EOF\n";
        let g = grenzen(d);
        assert_eq!(g.len(), 2, "zwei Fassungen erwartet: {g:?}");
        assert_eq!(&d[..g[0]], b"%PDF-1.4\nInhalt\n%%EOF\n");
        assert_eq!(g[1], d.len());
    }

    #[test]
    fn ein_pdf_ohne_abschluss_zaehlt_trotzdem() {
        assert_eq!(grenzen(b"%PDF-1.4\nohne Abschluss").len(), 0);
    }

    /// Eine Signatur ist ein Abbruchgrund. Wer sie uebergeht, macht aus einem
    /// beweiskraeftigen Dokument ein wertloses.
    #[test]
    fn eine_signatur_wird_erkannt() {
        assert!(ist_signiert(b"... /ByteRange [0 100 200 300] ..."));
        assert!(ist_signiert(b"... /Type /Sig ..."));
        assert!(!ist_signiert(b"... gewoehnliches PDF ..."));
    }

    #[test]
    fn lange_auszuege_werden_gekuerzt() {
        let lang = "x".repeat(1000);
        let k = kuerze(&lang);
        assert!(k.chars().count() <= AUSZUG_MAX + 1);
        assert!(k.ends_with('…'));
        assert_eq!(kuerze("kurz"), "kurz");
    }

    #[test]
    fn kaputte_daten_ergeben_einen_fehler_keinen_absturz() {
        assert!(inspect(b"").is_err());
        assert!(inspect(b"%PDF-1.4\nnur Muell").is_err());
    }
}
