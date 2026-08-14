//! Was im EXIF steht, muss dastehen — nicht bloß, wie viele Bytes es sind.
//!
//! # Der Bericht, der dazu führte
//!
//! Ein Nutzer sah für ein Foto: „Gerät oder Seriennummer — 3780 Bytes
//! EXIF-Block". Das ist keine Auskunft, sondern eine Mengenangabe. Wer
//! entscheiden soll, ob er eine Datei verschickt, muss lesen können, was
//! über ihn drinsteht.
//!
//! Der TIFF-Leser der Kiste konnte das die ganze Zeit. Der JPEG-Pfad hat
//! ihn nur nicht benutzt.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "Fehlschlag soll den Test abbrechen; die Groessen sind hier von Hand gesetzt"
)]

use cabrik_metadata::{Severity, inspect};

/// Baut ein JPEG mit einem EXIF-Segment, das echte Einträge trägt.
fn jpeg_mit_exif(eintraege: &[(u16, &str)]) -> Vec<u8> {
    jpeg_mit_exif_und_vorschau(eintraege, false)
}

/// Wie oben, wahlweise mit einem zweiten SOI im EXIF-Block.
fn jpeg_mit_exif_und_vorschau(eintraege: &[(u16, &str)], vorschau: bool) -> Vec<u8> {
    // --- TIFF-Kopf: Little Endian, Magie 42, IFD0 bei Versatz 8 ---------
    let mut tiff: Vec<u8> = b"II".to_vec();
    tiff.extend_from_slice(&42_u16.to_le_bytes());
    tiff.extend_from_slice(&8_u32.to_le_bytes());

    // Die Werte liegen hinter dem Verzeichnis.
    let anzahl = u16::try_from(eintraege.len()).expect("Anzahl");
    let ifd_bytes = 2 + eintraege.len() * 12 + 4;
    let mut werte_versatz = 8 + ifd_bytes;

    tiff.extend_from_slice(&anzahl.to_le_bytes());
    let mut werte: Vec<u8> = Vec::new();
    for (tag, text) in eintraege {
        let roh = format!("{text}\0");
        let len = u32::try_from(roh.len()).expect("Laenge");
        tiff.extend_from_slice(&tag.to_le_bytes());
        tiff.extend_from_slice(&2_u16.to_le_bytes()); // ASCII
        tiff.extend_from_slice(&len.to_le_bytes());
        if len <= 4 {
            let mut vier = [0_u8; 4];
            vier[..roh.len()].copy_from_slice(roh.as_bytes());
            tiff.extend_from_slice(&vier);
        } else {
            tiff.extend_from_slice(&u32::try_from(werte_versatz).expect("Versatz").to_le_bytes());
            werte.extend_from_slice(roh.as_bytes());
            werte_versatz += roh.len();
        }
    }
    tiff.extend_from_slice(&0_u32.to_le_bytes()); // kein weiteres IFD
    tiff.extend_from_slice(&werte);

    // --- JPEG drumherum -------------------------------------------------
    let mut app1: Vec<u8> = b"Exif\0\0".to_vec();
    app1.extend_from_slice(&tiff);
    if vorschau {
        // Ein zweites SOI -- so sieht ein eingebettetes Vorschaubild aus.
        app1.extend_from_slice(&[0xFF, 0xD8]);
    }

    let mut d: Vec<u8> = vec![0xFF, 0xD8]; // SOI
    d.extend_from_slice(&[0xFF, 0xE1]);
    d.extend_from_slice(&u16::try_from(app1.len() + 2).expect("Laenge").to_be_bytes());
    d.extend_from_slice(&app1);
    // Ein minimales SOS, damit es nach einer Bilddatei aussieht.
    d.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x02, 0xFF, 0xD9]);
    d
}

#[test]
fn das_kameramodell_steht_im_klartext_da() {
    let d = jpeg_mit_exif(&[(0x0110, "Pixel 8 Pro")]);

    let i = inspect(&d).expect("lesbar");

    let werte: Vec<_> = i.findings.iter().filter_map(|f| f.value.as_deref()).collect();
    assert!(
        werte.iter().any(|w| w.contains("Pixel 8 Pro")),
        "das Modell fehlt: {werte:?}"
    );
}

#[test]
fn mehrere_eintraege_stehen_einzeln_da() {
    // Genau der Punkt: nicht ein Sammelposten, sondern je ein Fund.
    let d = jpeg_mit_exif(&[
        (0x010F, "Canon"),
        (0x0110, "EOS R6"),
        (0x0131, "Adobe Lightroom 13.2"),
        (0x0132, "2026:08:14 21:03:11"),
    ]);

    let i = inspect(&d).expect("lesbar");
    let werte: Vec<_> = i.findings.iter().filter_map(|f| f.value.as_deref()).collect();

    for erwartet in ["Canon", "EOS R6", "Adobe Lightroom 13.2", "2026:08:14 21:03:11"] {
        assert!(
            werte.iter().any(|w| w.contains(erwartet)),
            "{erwartet} fehlt in {werte:?}"
        );
    }
}

#[test]
fn die_fundstelle_sagt_exif_und_nicht_tiff() {
    // Sonst stünde im Bericht einer JPEG-Datei „TIFF:Model“, und der
    // Nutzer suchte nach einer TIFF-Datei, die es nicht gibt.
    let d = jpeg_mit_exif(&[(0x0110, "EOS R6")]);

    let i = inspect(&d).expect("lesbar");

    assert!(
        i.findings.iter().any(|f| f.location.starts_with("EXIF:")),
        "Fundstellen: {:?}",
        i.findings.iter().map(|f| &f.location).collect::<Vec<_>>()
    );
    assert!(
        !i.findings.iter().any(|f| f.location.starts_with("TIFF:")),
        "in einer JPEG-Datei hat „TIFF:“ nichts zu suchen"
    );
}

#[test]
fn der_autor_wiegt_schwerer_als_das_geraet() {
    // Die Einstufung kommt aus derselben Tabelle wie bei TIFF -- ein
    // Personenname ist kritisch, ein Kameramodell beachtlich.
    let d = jpeg_mit_exif(&[(0x013B, "Dani Willberg"), (0x0110, "EOS R6")]);

    let i = inspect(&d).expect("lesbar");
    let autor = i
        .findings
        .iter()
        .find(|f| f.value.as_deref().is_some_and(|v| v.contains("Dani")))
        .expect("Autor gefunden");

    assert_eq!(autor.severity, Severity::Critical);
}

#[test]
fn ein_kaputtes_exif_meldet_wenigstens_seine_groesse() {
    // Die Rückfallebene. Sie ist schlechter als eine Auskunft, aber besser
    // als Schweigen: Es steht etwas drin, und wir sagen wie viel.
    let mut d: Vec<u8> = vec![0xFF, 0xD8, 0xFF, 0xE1];
    let app1: Vec<u8> = b"Exif\0\0IIIIIIIIIIII".to_vec();
    d.extend_from_slice(&u16::try_from(app1.len() + 2).expect("Laenge").to_be_bytes());
    d.extend_from_slice(&app1);
    d.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x02, 0xFF, 0xD9]);

    let i = inspect(&d).expect("lesbar");

    assert!(
        i.findings
            .iter()
            .any(|f| f.value.as_deref().is_some_and(|v| v.contains("Bytes EXIF-Block"))),
        "Funde: {:?}",
        i.findings
    );
}

#[test]
fn das_vorschaubild_geht_nicht_verloren() {
    // Der Leser folgt nur der Kette der Hauptverzeichnisse. Vorschaubild
    // und GPS-Zeiger kommen deshalb weiter aus der groben Suche -- beide
    // sind zu wichtig, um sie fallenzulassen.
    // Ein zweites SOI im EXIF-Block, wie es ein Thumbnail hinterlaesst --
    // eingebaut, bevor die Segmentlaenge berechnet wird.
    let d = jpeg_mit_exif_und_vorschau(&[(0x0110, "EOS R6")], true);

    let i = inspect(&d).expect("lesbar");

    assert!(
        i.findings
            .iter()
            .any(|f| f.location.contains("Thumbnail")),
        "Funde: {:?}",
        i.findings.iter().map(|f| &f.location).collect::<Vec<_>>()
    );
}
