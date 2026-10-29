//! Die Erkennung am Anfang gegen die Erkennung über die ganze Datei
//! (`spec/grosse-dateien.md` §13).
//!
//! Die Zusage: **nie „unbekannt“, wo die ganze Datei ein Format ergäbe, und
//! nie ein anderes Format.** Sonst ginge eine Datei unbereinigt hinaus, ohne
//! dass jemand das gewählt hätte. „Braucht die ganze Datei“ ist immer
//! erlaubt — es ist nur langsamer.
//!
//! Geprüft an jeder Testdatei, an verlängerten Fassungen davon (damit der
//! Anfang wirklich ein Anfang ist) und an Zufall hinter jeder Kennung, die
//! es gibt. Das Fuzz-Ziel `metadata_anfang` setzt es nachts fort.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use cabrik_metadata::{ANFANG, Anfang, Format, am_anfang};

fn ordner() -> Vec<PathBuf> {
    let wurzel = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testvectors");
    vec![wurzel.join("metadata"), wurzel.join("fuzz/metadata")]
}

fn alle_dateien() -> Vec<(String, Vec<u8>)> {
    let mut aus = Vec::new();
    for o in ordner() {
        for e in std::fs::read_dir(&o).unwrap_or_else(|e| panic!("{}: {e}", o.display())) {
            let p = e.unwrap().path();
            if p.is_file() && p.extension().is_none_or(|x| x != "json") {
                aus.push((p.display().to_string(), std::fs::read(&p).unwrap()));
            }
        }
    }
    assert!(
        aus.len() > 40,
        "zu wenige Testdateien gefunden: {}",
        aus.len()
    );
    aus
}

/// Ein einfacher, fester Zufall.
fn zufall(n: usize, saat: u64) -> Vec<u8> {
    let mut x = saat.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 24) as u8
        })
        .collect()
}

/// Die Zusage für eine Datei, an mehreren Anfängen.
fn haelt(daten: &[u8], fall: &str) -> Anfang {
    let ganz = Format::detect(daten);
    let mut letzte = Anfang::GanzeDatei;
    for n in [ANFANG, ANFANG + 1, ANFANG + 4097, daten.len()] {
        if n > daten.len() {
            continue;
        }
        let a = am_anfang(&daten[..n], daten.len() as u64);
        match &a {
            Anfang::Bekannt(f) => assert_eq!(
                Some(*f),
                ganz,
                "{fall}: am Anfang ({n} Bytes) {f:?}, über die ganze Datei {ganz:?}"
            ),
            Anfang::Unbekannt { .. } => assert_eq!(
                ganz, None,
                "{fall}: am Anfang ({n} Bytes) unbekannt, über die ganze Datei {ganz:?}"
            ),
            Anfang::GanzeDatei => {}
        }
        letzte = a;
    }
    letzte
}

#[test]
fn an_jeder_testdatei_haelt_die_zusage() {
    for (name, daten) in alle_dateien() {
        haelt(&daten, &name);

        // Verlängert — mit Nullen und mit Zufall —, damit es einen echten
        // Anfang gibt. Bei manchen Formaten ändert das die Erkennung über
        // die ganze Datei; genau dann muss der Anfang vorsichtig sein.
        let mut nullen = daten.clone();
        nullen.resize(daten.len() + 3 * ANFANG, 0);
        haelt(&nullen, &format!("{name} + Nullen"));

        let mut zufaellig = daten.clone();
        zufaellig.extend_from_slice(&zufall(3 * ANFANG, daten.len() as u64));
        haelt(&zufaellig, &format!("{name} + Zufall"));
    }
}

#[test]
fn hinter_jeder_kennung_haelt_die_zusage() {
    // Jede Kennung, die einer der Erkenner kennt — mit Zufall dahinter.
    let kennungen: [&[u8]; 22] = [
        b"\x89PNG\r\n\x1a\n",
        b"\xFF\xD8\xFF",
        b"RIFF\x00\x00\x10\x00WEBP",
        b"GIF89a",
        b"BM",
        b"II*\x00",
        b"MM\x00*",
        b"\x00\x00\x00\x18ftypisom",
        b"\x00\x00\x00\x18ftypheic",
        b"\x00\x10\x00\x00ftypmp42",
        b"RIFF\x00\x00\x10\x00AVI ",
        b"RIFF\x00\x00\x10\x00WAVE",
        b"\x1A\x45\xDF\xA3",
        b"fLaC",
        b"ID3\x03\x00\x00\x00\x00\x00\x10fLaC",
        b"ID3\x03\x00\x00\x00\x7F\x7F\x7F",
        b"\xFF\xFB\x90\x00",
        b"OggS\x00",
        b"<?xml version=\"1.0\"?><svg",
        b"  <svg xmlns=\"http://www.w3.org/2000/svg\">",
        b"%PDF-1.7",
        b"PK\x03\x04",
    ];
    for (i, k) in kennungen.iter().enumerate() {
        for saat in 0..8u64 {
            let mut d = k.to_vec();
            d.extend_from_slice(&zufall(2 * ANFANG + 3, saat * 31 + i as u64));
            haelt(&d, &format!("Kennung {i}, Saat {saat}"));
        }
    }
}

#[test]
fn ein_grosses_video_wird_am_anfang_erkannt() {
    // Der Fall, für den es die Erkennung gibt: ein großes Video, von dem
    // nur der Anfang gelesen wird.
    let mut daten = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../testvectors/metadata/video_mit_ortsangabe.mp4"),
    )
    .unwrap();
    daten.resize(daten.len() + 4 * ANFANG, 0);
    assert_eq!(
        am_anfang(&daten[..ANFANG], daten.len() as u64),
        Anfang::Bekannt(Format::Video)
    );
}

#[test]
fn ein_grosser_unbekannter_block_ist_unbekannt() {
    // Und der andere Fall: etwas, das keines der Formate ist — etwa ein
    // Disk-Abbild. Es soll strömen dürfen, ohne ganz gelesen zu werden.
    let daten = zufall(5 * ANFANG, 4711);
    assert!(matches!(
        am_anfang(&daten[..ANFANG], daten.len() as u64),
        Anfang::Unbekannt { .. }
    ));
}

#[test]
fn office_braucht_die_ganze_datei() {
    // Die Falle, die zu §13 führte: Eine `.docx` beginnt mit `PK`, und ihr
    // Verzeichnis steht am Ende.
    let mut daten = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../testvectors/metadata/dokument_mit_metadaten.docx"),
    )
    .unwrap();
    daten.resize(daten.len().max(2 * ANFANG), 0);
    assert_eq!(
        am_anfang(&daten[..ANFANG], daten.len() as u64),
        Anfang::GanzeDatei
    );
}

#[test]
fn ein_zu_kurzer_anfang_entscheidet_nichts() {
    let daten = zufall(2 * ANFANG, 1);
    assert_eq!(
        am_anfang(&daten[..100], daten.len() as u64),
        Anfang::GanzeDatei
    );
}

#[test]
fn ein_svg_das_mitten_in_einem_umlaut_abgeschnitten_wird() {
    // SVG verlangt gültiges UTF-8 über die ganze Datei. Endet der Anfang
    // mitten in einem Zeichen, ist er für sich ungültig — die ganze Datei
    // aber nicht. Ohne Vorsicht hiesse das „unbekannt“ für ein SVG.
    let mut d = b"<svg xmlns=\"http://www.w3.org/2000/svg\"><desc>".to_vec();
    while d.len() < 3 * ANFANG {
        d.extend_from_slice("Grüße ".as_bytes());
    }
    d.extend_from_slice(b"</desc></svg>");
    assert_eq!(Format::detect(&d), Some(Format::Svg));
    for n in ANFANG..ANFANG + 8 {
        let a = am_anfang(&d[..n], d.len() as u64);
        assert!(
            !matches!(a, Anfang::Unbekannt { .. }),
            "bei {n} Bytes für unbekannt gehalten"
        );
    }
}

#[test]
fn ein_grosses_bmp_wird_nicht_fuer_unbekannt_gehalten() {
    // Die Erkennung vergleicht die angegebene Größe mit der Länge. Am
    // Anfang ist die angegebene größer — ohne Vorsicht hiesse das
    // „unbekannt“ für ein gewöhnliches Bild.
    let mut d = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../testvectors/metadata/bild_schlicht.bmp"),
    )
    .unwrap();
    let laenge = 3 * ANFANG;
    d.resize(laenge, 0);
    d[2..6].copy_from_slice(&(laenge as u32).to_le_bytes());
    assert_eq!(Format::detect(&d), Some(Format::Bmp));
    haelt(&d, "großes BMP");
}

#[test]
fn ein_flac_mit_langem_id3_tag_wird_nicht_fuer_mp3_gehalten() {
    // Ein ID3-Tag, länger als der Anfang, dahinter `fLaC`. Am Anfang sieht
    // nur der Tag aus wie MP3; erst dahinter entscheidet sich, was es ist.
    let tag = 2 * ANFANG;
    let syncsafe = |n: usize| {
        [
            ((n >> 21) & 0x7F) as u8,
            ((n >> 14) & 0x7F) as u8,
            ((n >> 7) & 0x7F) as u8,
            (n & 0x7F) as u8,
        ]
    };
    let mut d = b"ID3  ".to_vec();
    d.extend_from_slice(&syncsafe(tag));
    d.resize(10 + tag, 0);
    d.extend_from_slice(b"fLaC");
    d.extend_from_slice(&zufall(ANFANG, 9));
    assert_eq!(Format::detect(&d), Some(Format::Flac));
    haelt(&d, "FLAC mit langem ID3-Tag");
}

#[test]
fn eine_marke_jenseits_des_anfangs_wird_nicht_uebersehen() {
    // Die Markenliste in `ftyp` darf länger sein als der Anfang. Steht die
    // einzige bekannte Marke erst dahinter, sähe der Anfang nichts davon.
    let groesse = 2 * ANFANG;
    let mut d = (groesse as u32).to_be_bytes().to_vec();
    d.extend_from_slice(b"ftypxxxx    ");
    while d.len() < groesse - 4 {
        d.extend_from_slice(b"zzzz");
    }
    d.extend_from_slice(b"isom");
    d.extend_from_slice(&zufall(ANFANG, 11));
    assert_eq!(Format::detect(&d), Some(Format::Video));
    haelt(&d, "Marke jenseits des Anfangs");
}
