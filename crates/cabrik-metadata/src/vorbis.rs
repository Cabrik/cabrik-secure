//! Vorbis-Kommentare — die Marken von FLAC, Ogg Vorbis und Opus.
//!
//! Drei Formate, ein Aufbau. Diese Marken einmal zu lesen statt dreimal ist
//! nicht nur weniger Arbeit: **Jeder Parser ist Angriffsfläche**, und derselbe
//! Aufbau dreimal geschrieben heißt dreimal Gelegenheit für denselben Fehler.
//!
//! # Aufbau
//!
//! Anders als fast alles andere in diesen Formaten zählen die Längen hier in
//! **kleiner** Byte-Reihenfolge — ein Erbe aus Vorbis:
//!
//! ```text
//! u32  Länge der Herstellerangabe
//! …    "reference libFLAC 1.4.3" oder ähnlich
//! u32  Anzahl der Einträge
//! je Eintrag:
//!   u32  Länge
//!   …    "ARTIST=Dr. Anna Beispiel"
//! [1]  Rahmenbit — nur in Ogg, nicht in FLAC
//! ```
//!
//! Die Schlüssel sind **nicht festgelegt**. Es gibt gebräuchliche, aber jedes
//! Programm darf eigene erfinden. Deshalb gilt hier: Was nicht erkannt wird,
//! ist trotzdem ein Fund — nur eben ein unbenannter.

use crate::model::{Finding, FindingKind, Severity};

/// Höchstzahl der Einträge, die gelesen werden.
const MAX_EINTRAEGE: usize = 100_000;

/// Ein gelesener Kommentarblock.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Kommentare {
    /// Das Programm, das die Datei geschrieben hat.
    pub hersteller: String,
    /// Schlüssel und Wert je Eintrag. Der Schlüssel steht so da, wie er in
    /// der Datei steht — verglichen wird ohne Rücksicht auf Groß- und
    /// Kleinschreibung.
    pub eintraege: Vec<(String, String)>,
}

fn u32_le(daten: &[u8], p: usize) -> Option<usize> {
    let b = daten.get(p..p.checked_add(4)?)?;
    Some(u32::from_le_bytes(b.try_into().ok()?) as usize)
}

/// Liest einen Kommentarblock.
///
/// Gibt `None` zurück, wenn die Längen nicht zum Puffer passen — das ist
/// keine Ausnahme, sondern der Regelfall bei beschädigten Dateien, und der
/// Aufrufer entscheidet, was daraus folgt.
#[must_use]
pub fn lies(roh: &[u8]) -> Option<Kommentare> {
    let hersteller_len = u32_le(roh, 0)?;
    let hersteller_bis = 4usize.checked_add(hersteller_len)?;
    let hersteller = String::from_utf8_lossy(roh.get(4..hersteller_bis)?).into_owned();

    let anzahl = u32_le(roh, hersteller_bis)?;
    if anzahl > MAX_EINTRAEGE {
        return None;
    }
    let mut p = hersteller_bis.checked_add(4)?;
    let mut eintraege = Vec::with_capacity(anzahl.min(64));

    for _ in 0..anzahl {
        let len = u32_le(roh, p)?;
        let von = p.checked_add(4)?;
        let bis = von.checked_add(len)?;
        let text = String::from_utf8_lossy(roh.get(von..bis)?).into_owned();
        // Ohne Gleichheitszeichen ist der Eintrag formal fehlerhaft. Er wird
        // trotzdem behalten, sonst verschwiege die Anzeige ihn.
        let (schluessel, wert) = text.split_once('=').map_or_else(
            || (String::new(), text.clone()),
            |(k, v)| (k.to_owned(), v.to_owned()),
        );
        eintraege.push((schluessel, wert));
        p = bis;
    }

    Some(Kommentare {
        hersteller,
        eintraege,
    })
}

/// Ein leerer Kommentarblock: keine Herstellerangabe, keine Einträge.
///
/// `rahmenbit` setzt das abschließende Bit, das **Ogg** verlangt und FLAC
/// nicht kennt. Es zu vergessen macht eine Ogg-Datei unlesbar.
#[must_use]
pub fn leer(rahmenbit: bool) -> Vec<u8> {
    let mut v = Vec::with_capacity(9);
    v.extend_from_slice(&0u32.to_le_bytes()); // Herstellerangabe: leer
    v.extend_from_slice(&0u32.to_le_bytes()); // keine Einträge
    if rahmenbit {
        v.push(0x01);
    }
    v
}

/// Ordnet einen Schlüssel ein.
///
/// Die Liste deckt ab, was gebräuchlich ist. Alles Übrige gilt als Kommentar
/// — lieber einmal zu viel gemeldet als eine Angabe stillschweigend übergehen.
#[must_use]
pub fn einordnung(schluessel: &str) -> (FindingKind, Severity) {
    match schluessel.to_ascii_uppercase().as_str() {
        "ARTIST" | "ALBUMARTIST" | "PERFORMER" | "COMPOSER" | "AUTHOR" | "CONDUCTOR"
        | "ENSEMBLE" | "ARRANGER" | "LYRICIST" | "WRITER" => {
            (FindingKind::Author, Severity::Critical)
        }
        "COMMENT" | "COMMENTS" | "DESCRIPTION" | "SYNOPSIS" | "LYRICS" => {
            (FindingKind::Comment, Severity::Critical)
        }
        // Ein eingebettetes Bild, in Base64 verpackt. Es bringt seine eigenen
        // Metadaten mit — und die sieht niemand.
        "METADATA_BLOCK_PICTURE" | "COVERART" => (FindingKind::EmbeddedPreview, Severity::Critical),
        "ENCODER" | "ENCODED_BY" | "ENCODING" | "VENDOR" => {
            (FindingKind::Software, Severity::Notable)
        }
        "DATE" | "YEAR" | "ORIGINALDATE" | "ENCODING_TIME" => {
            (FindingKind::Timestamp, Severity::Notable)
        }
        "ORGANIZATION" | "LABEL" | "PUBLISHER" | "COPYRIGHT" | "LICENSE" | "CONTACT" => {
            (FindingKind::Organization, Severity::Notable)
        }
        // Kennungen, mit denen sich Kopien einander zuordnen lassen.
        "MUSICBRAINZ_TRACKID" | "MUSICBRAINZ_ALBUMID" | "ACOUSTID_ID" | "ISRC" => {
            (FindingKind::Device, Severity::Notable)
        }
        _ => (FindingKind::Comment, Severity::Notable),
    }
}

/// Macht aus einem Kommentarblock eine Fundliste.
///
/// `ort` ist die Vorsilbe der Fundstelle, etwa `"FLAC:VORBIS_COMMENT"`.
#[must_use]
pub fn funde(k: &Kommentare, ort: &str) -> Vec<Finding> {
    let mut aus = Vec::new();

    if !k.hersteller.is_empty() {
        aus.push(Finding {
            kind: FindingKind::Software,
            location: format!("{ort}/Hersteller"),
            value: Some(k.hersteller.clone()),
            severity: Severity::Notable,
        });
    }

    for (schluessel, wert) in &k.eintraege {
        let (art, schwere) = einordnung(schluessel);
        let name = if schluessel.is_empty() {
            "ohne Schlüssel"
        } else {
            schluessel
        };
        // Bei einem eingebetteten Bild steht im Wert der Rohinhalt in Base64.
        // Ihn anzuzeigen hilft niemandem; die Größe hilft.
        let anzeige = if art == FindingKind::EmbeddedPreview {
            format!("eingebettetes Bild ({} Zeichen Base64)", wert.len())
        } else {
            wert.clone()
        };
        aus.push(Finding {
            kind: art,
            location: format!("{ort}/{name}"),
            value: Some(anzeige),
            severity: schwere,
        });
    }
    aus
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "Tests duerfen laut werden"
)]
mod tests {
    use super::*;

    fn baue(hersteller: &str, eintraege: &[&str], rahmenbit: bool) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&u32::try_from(hersteller.len()).unwrap().to_le_bytes());
        v.extend_from_slice(hersteller.as_bytes());
        v.extend_from_slice(&u32::try_from(eintraege.len()).unwrap().to_le_bytes());
        for e in eintraege {
            v.extend_from_slice(&u32::try_from(e.len()).unwrap().to_le_bytes());
            v.extend_from_slice(e.as_bytes());
        }
        if rahmenbit {
            v.push(0x01);
        }
        v
    }

    #[test]
    fn ein_kommentarblock_wird_gelesen() {
        let roh = baue(
            "reference libFLAC 1.4.3",
            &["ARTIST=Dr. Anna Beispiel", "TITLE=Angebot Nordstern"],
            false,
        );
        let k = lies(&roh).expect("nicht lesbar");
        assert_eq!(k.hersteller, "reference libFLAC 1.4.3");
        assert_eq!(k.eintraege.len(), 2);
        assert_eq!(
            k.eintraege.first().unwrap(),
            &("ARTIST".to_owned(), "Dr. Anna Beispiel".to_owned())
        );
    }

    /// Umlaute sind der Regelfall, nicht die Ausnahme.
    #[test]
    fn umlaute_ueberstehen_das_lesen() {
        let roh = baue("", &["ARTIST=Jürgen Groß", "TITLE=Größe"], false);
        let k = lies(&roh).unwrap();
        assert_eq!(k.eintraege[0].1, "Jürgen Groß");
        assert_eq!(k.eintraege[1].1, "Größe");
    }

    /// Ein Eintrag ohne Gleichheitszeichen ist formal fehlerhaft. Ihn zu
    /// verschweigen wäre schlimmer, als ihn unbenannt zu melden.
    #[test]
    fn ein_eintrag_ohne_gleichheitszeichen_verschwindet_nicht() {
        let roh = baue("", &["einfach nur Text"], false);
        let k = lies(&roh).unwrap();
        assert_eq!(k.eintraege.len(), 1);
        assert_eq!(k.eintraege[0].0, "");

        let f = funde(&k, "Test");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].location, "Test/ohne Schlüssel");
    }

    #[test]
    fn zu_grosse_laengen_werden_abgelehnt_statt_zu_stuerzen() {
        // Herstellerangabe länger als der Puffer.
        assert!(lies(&[0xFF, 0xFF, 0xFF, 0x7F, 0x00]).is_none());
        // Anzahl der Einträge jenseits jeder Vernunft.
        let mut roh = 0u32.to_le_bytes().to_vec();
        roh.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(lies(&roh).is_none());
        assert!(lies(&[]).is_none());
    }

    #[test]
    fn der_leere_block_ist_lesbar_und_wirklich_leer() {
        for rahmenbit in [false, true] {
            let roh = leer(rahmenbit);
            let k = lies(&roh).unwrap();
            assert!(k.hersteller.is_empty());
            assert!(k.eintraege.is_empty());
            assert_eq!(roh.len(), if rahmenbit { 9 } else { 8 });
        }
    }

    #[test]
    fn die_einordnung_trifft_die_wichtigen_faelle() {
        assert_eq!(einordnung("artist").0, FindingKind::Author);
        assert_eq!(einordnung("ARTIST").1, Severity::Critical);
        assert_eq!(einordnung("Description").1, Severity::Critical);
        assert_eq!(
            einordnung("METADATA_BLOCK_PICTURE").0,
            FindingKind::EmbeddedPreview
        );
        assert_eq!(einordnung("ENCODER").0, FindingKind::Software);
        // Unbekanntes wird gemeldet, nicht übergangen.
        assert_eq!(einordnung("VOELLIG_NEU").0, FindingKind::Comment);
    }

    /// Ein eingebettetes Bild darf nicht als Base64-Wüste in der Anzeige
    /// landen — die Größe sagt alles, was zählt.
    #[test]
    fn ein_eingebettetes_bild_wird_nicht_ausgeschrieben() {
        let gross = format!("METADATA_BLOCK_PICTURE={}", "A".repeat(5000));
        let roh = baue("", &[&gross], false);
        let f = funde(&lies(&roh).unwrap(), "FLAC");
        assert_eq!(f[0].kind, FindingKind::EmbeddedPreview);
        let text = f[0].value.as_deref().unwrap();
        assert!(text.len() < 80, "die Anzeige ist zu lang: {}", text.len());
        assert!(text.contains("5000"));
    }
}
