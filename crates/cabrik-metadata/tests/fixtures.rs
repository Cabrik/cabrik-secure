//! Prüft die Bereinigung an **echten** Bilddateien.
//!
//! Die Modultests bauen JPEGs und PNGs von Hand und prüfen damit die
//! Struktur. Hier kommen Dateien zum Einsatz, die Pillow und piexif erzeugt
//! haben — mit echtem EXIF, echtem GPS und einem echten Vorschaubild.
//!
//! Die bereinigten Ergebnisse werden neben den Vorlagen abgelegt, damit
//! `testvectors/tools/verify_metadata_stripped.py` sie **unabhängig** wieder
//! öffnen und nachmessen kann: keine Metadaten mehr, unveränderte Pixel,
//! Palette intakt. Der v1-Palette-Bug erzeugte eine gültige Datei mit
//! falschen Farben — das fällt nur auf, wenn jemand das Bild wirklich öffnet.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use cabrik_metadata::{FindingKind, Severity, StripResult, inspect, strip};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testvectors/metadata")
}

fn lade(name: &str) -> Vec<u8> {
    let p = dir().join(name);
    std::fs::read(&p).unwrap_or_else(|e| {
        panic!(
            "{} nicht lesbar: {e}\nVorlagen erzeugen mit: \
             python testvectors/tools/gen_metadata_fixtures.py",
            p.display()
        )
    })
}

/// Legt das Ergebnis für die unabhängige Python-Prüfung ab.
fn schreibe_ergebnis(name: &str, daten: &[u8]) {
    let p = dir().join(format!("{name}.stripped"));
    std::fs::write(&p, daten).unwrap_or_else(|e| panic!("{} nicht schreibbar: {e}", p.display()));
}

#[test]
fn echtes_foto_gps_und_vorschaubild_werden_erkannt() {
    let daten = lade("foto_mit_exif.jpg");
    let i = inspect(&daten).unwrap();

    assert!(i.understood);
    assert_eq!(i.format.as_deref(), Some("JPEG"));

    let gps = i.findings.iter().find(|f| f.kind == FindingKind::Gps);
    assert!(gps.is_some(), "GPS im echten EXIF nicht gefunden");
    assert_eq!(gps.unwrap().severity, Severity::Critical);

    let vorschau = i
        .findings
        .iter()
        .find(|f| f.kind == FindingKind::EmbeddedPreview);
    assert!(
        vorschau.is_some(),
        "eingebettetes Vorschaubild nicht gefunden -- \
         genau die Zweitkopie, die beim Zuschneiden ueberlebt"
    );
    assert_eq!(vorschau.unwrap().severity, Severity::Critical);
}

#[test]
fn echtes_foto_wird_bereinigt() {
    let daten = lade("foto_mit_exif.jpg");
    let (sauber, ergebnis) = strip(&daten).unwrap();

    assert!(ergebnis.may_show_clean());
    assert!(ergebnis.has_critical());

    // Keine Spur der Metadaten mehr in den Bytes.
    for spur in [
        &b"Exif\0\0"[..],
        b"Canon",
        b"EOS 5D Mark IV",
        b"Max Mustermann",
        b"Cabrik Testaufbau",
    ] {
        assert!(
            !sauber.windows(spur.len()).any(|w| w == spur),
            "{:?} blieb in der Datei",
            String::from_utf8_lossy(spur)
        );
    }

    assert!(sauber.len() < daten.len(), "die Datei wurde nicht kleiner");
    schreibe_ergebnis("foto_mit_exif.jpg", &sauber);
}

#[test]
fn foto_ohne_metadaten_wird_kaum_angefasst() {
    let daten = lade("foto_ohne_exif.jpg");
    let (sauber, ergebnis) = strip(&daten).unwrap();

    // Pillow schreibt ein JFIF-APP0-Segment; das ist ein Anwendungssegment
    // und faellt weg. Die Bilddaten bleiben.
    assert!(ergebnis.may_show_clean());
    assert!(
        !ergebnis.has_critical(),
        "eine saubere Datei darf keine kritischen Funde ergeben"
    );
    schreibe_ergebnis("foto_ohne_exif.jpg", &sauber);
}

/// Der v1-Bug, an einer echten Datei.
#[test]
fn palette_png_behaelt_seine_farbtabelle() {
    let daten = lade("palette_mit_text.png");
    let i = inspect(&daten).unwrap();

    assert!(
        i.findings
            .iter()
            .any(|f| f.location.starts_with("PNG:tEXt")),
        "Text-Chunks nicht gefunden"
    );

    let (sauber, ergebnis) = strip(&daten).unwrap();
    assert!(ergebnis.may_show_clean());

    // Die Farbtabelle muss Byte fuer Byte erhalten sein.
    let plte_vorher = finde_chunk(&daten, b"PLTE").expect("Vorlage hat keine Palette");
    let plte_nachher = finde_chunk(&sauber, b"PLTE").expect("Palette ging verloren");
    assert_eq!(
        plte_vorher, plte_nachher,
        "die Farbtabelle wurde veraendert -- genau der v1-Bug"
    );

    // Und die Bilddaten ebenso.
    assert_eq!(
        finde_chunk(&daten, b"IDAT"),
        finde_chunk(&sauber, b"IDAT"),
        "Bilddaten wurden neu kodiert"
    );

    assert!(
        !sauber.windows(14).any(|w| w == b"Max Mustermann"),
        "Autorenname blieb stehen"
    );
    schreibe_ergebnis("palette_mit_text.png", &sauber);
}

#[test]
fn png_ohne_metadaten_bleibt_unveraendert() {
    let daten = lade("bild_ohne_text.png");
    let (sauber, ergebnis) = strip(&daten).unwrap();
    assert_eq!(sauber, daten, "eine saubere Datei wurde angefasst");
    assert!(ergebnis.removed().is_empty());
    schreibe_ergebnis("bild_ohne_text.png", &sauber);
}

#[test]
fn strippen_ist_auch_bei_echten_dateien_idempotent() {
    for name in [
        "foto_mit_exif.jpg",
        "palette_mit_text.png",
        "bild_ohne_text.png",
    ] {
        let (einmal, _) = strip(&lade(name)).unwrap();
        let (zweimal, ergebnis) = strip(&einmal).unwrap();
        assert_eq!(einmal, zweimal, "{name}: nicht idempotent");
        assert!(
            ergebnis.removed().is_empty(),
            "{name}: beim zweiten Durchgang wurde noch etwas gefunden"
        );
    }
}

#[test]
fn ein_zugeschnittenes_foto_traegt_die_vorschau_nicht_weiter() {
    // spec/metadata.md §7.1 -- der Fall Cat Schwartz.
    let daten = lade("foto_mit_exif.jpg");
    let (sauber, _) = strip(&daten).unwrap();

    // Im Original steckt hinter dem ersten SOI ein weiteres (das Thumbnail).
    let vorher = daten
        .windows(2)
        .skip(1)
        .filter(|w| *w == [0xFF, 0xD8])
        .count();
    let nachher = sauber
        .windows(2)
        .skip(1)
        .filter(|w| *w == [0xFF, 0xD8])
        .count();

    assert!(vorher > 0, "Vorlage enthaelt kein Vorschaubild");
    assert_eq!(nachher, 0, "das Vorschaubild ueberlebte die Bereinigung");
}

/// Sucht die Nutzdaten eines PNG-Chunks.
fn finde_chunk<'a>(data: &'a [u8], typ: &[u8; 4]) -> Option<&'a [u8]> {
    let mut pos = 8usize;
    while pos + 8 <= data.len() {
        let len = u32::from_be_bytes(data[pos..pos + 4].try_into().ok()?) as usize;
        let t = &data[pos + 4..pos + 8];
        if t == typ {
            return data.get(pos + 8..pos + 8 + len);
        }
        pos = pos + 12 + len;
    }
    None
}

#[test]
fn unbekannte_formate_werden_nicht_still_durchkopiert() {
    // Der Kernfehler aus v1, an einem realistischen Beispiel.
    //
    // Bewusst kein PDF mehr: Das wird seit Phase 2.9b verstanden. Ein
    // Musikstück mit ID3-Kennung ist der passende Fall — die Kennung ist
    // bekannt, das Format wird nicht behandelt.
    let mp3 = b"ID3\x04\x00\x00\x00\x00\x00\x00TPE1\x00\x00\x00\x0cMax Mustermann";
    let (out, ergebnis) = strip(mp3).unwrap();

    assert_eq!(out, mp3, "der Inhalt darf nicht angetastet werden");
    assert!(
        !ergebnis.may_show_clean(),
        "v1 haette hier stillschweigend kopiert und Sauberkeit suggeriert"
    );
    assert!(matches!(ergebnis, StripResult::Unknown { .. }));
    assert!(
        ergebnis.to_string().contains("keine Aussage"),
        "die Meldung muss die Lücke benennen"
    );
}

/// Ein **kaputtes** PDF ist etwas anderes als ein unbekanntes Format: Es wird
/// erkannt, und der Fehler wird gemeldet — statt zu behaupten, das Format sei
/// unbekannt.
#[test]
fn ein_kaputtes_pdf_wird_als_fehler_gemeldet() {
    let pdf = b"%PDF-1.7\n1 0 obj\n<< /Author (Max Mustermann) >>\n";
    assert!(
        strip(pdf).is_err(),
        "ein erkanntes, aber kaputtes Format ist ein Fehler, kein Unknown"
    );
}
