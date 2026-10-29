//! Das Gerüst gegen die ganze Datei (`spec/grosse-dateien.md` §5.4).
//!
//! Die Pflicht: Die Flicken aus dem Gerüst, auf die Originaldatei
//! angewandt, ergeben **Byte für Byte** das, was `strip` auf der ganzen
//! Datei liefert — samt demselben Ergebnis und demselben Formatnamen.
//! Oder der Gerüst-Weg lehnt ab. Ein Drittes gibt es nicht.
//!
//! Geprüft an den echten Videovorlagen und an tausenden gestörten
//! Fassungen davon: gekippte Bits, abgeschnittene Enden. Das Fuzz-Ziel
//! `metadata_geruest` setzt das nachts an zufälligen Eingaben fort.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::PathBuf;

use cabrik_metadata::{Flicken, Format, flicken_anwenden, inspect, strip, ueber_geruest};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testvectors/metadata")
}

fn lade(name: &str) -> Vec<u8> {
    std::fs::read(dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

const VORLAGEN: [&str; 5] = [
    "video_mit_ortsangabe.mp4",
    "live_photo.mov",
    "video_mit_marken.mkv",
    "video_mit_marken.webm",
    "video_mit_marken.avi",
];

const VIDEO: [Format; 3] = [Format::Video, Format::Matroska, Format::Avi];

/// Die Pflicht für eine Eingabe. Gibt zurück, ob das Gerüst angenommen hat.
fn gleich_wie_ganz(daten: &[u8], fall: &str) -> bool {
    let Some(format) = Format::detect(daten).filter(|f| VIDEO.contains(f)) else {
        return false;
    };
    let befund = ueber_geruest(&mut Cursor::new(daten), daten.len() as u64, format)
        .expect("aus dem Speicher liest es sich immer");
    let Some(b) = befund else {
        return false;
    };

    let (sauber, ergebnis) = strip(daten)
        .unwrap_or_else(|e| panic!("{fall}: das Gerüst nahm an, die ganze Datei scheitert: {e}"));
    let mut geflickt = daten.to_vec();
    flicken_anwenden(0, &mut geflickt, &b.flicken);
    assert!(
        geflickt == sauber,
        "{fall}: andere Bytes als strip über die ganze Datei"
    );
    assert_eq!(b.ergebnis, ergebnis, "{fall}: anderes Ergebnis");
    assert_eq!(
        b.format,
        inspect(daten).ok().and_then(|i| i.format),
        "{fall}: anderer Formatname"
    );
    true
}

#[test]
fn an_den_echten_vorlagen_ist_das_geruest_dasselbe_wie_die_ganze_datei() {
    for name in VORLAGEN {
        let daten = lade(name);
        assert!(
            gleich_wie_ganz(&daten, name),
            "{name}: das Gerüst wurde abgelehnt — dann nützt der Weg bei dieser Art Datei nichts"
        );
    }
}

#[test]
fn die_vorlagen_werden_tatsaechlich_bereinigt() {
    // Sonst prüfte der Vergleich oben nur, dass zweimal nichts geschieht.
    for name in VORLAGEN {
        let daten = lade(name);
        let format = Format::detect(&daten).unwrap();
        let b = ueber_geruest(&mut Cursor::new(&daten), daten.len() as u64, format)
            .unwrap()
            .unwrap();
        assert!(!b.flicken.is_empty(), "{name}: keine Flicken");
    }
}

#[test]
fn gestoerte_fassungen_ergeben_dasselbe_oder_eine_ablehnung() {
    let mut angenommen = 0usize;
    let mut geprueft = 0usize;
    for name in VORLAGEN {
        let daten = lade(name);
        let schritt = (daten.len() / 1500).max(1);

        // Gekippte Bits an vielen Stellen — auch in Köpfen und Größen.
        for (n, stelle) in (0..daten.len()).step_by(schritt).enumerate() {
            let mut v = daten.clone();
            v[stelle] ^= 1 << (n % 8);
            geprueft += 1;
            angenommen += usize::from(gleich_wie_ganz(&v, &format!("{name}, Bit bei {stelle}")));
        }
        // Abgeschnittene Enden.
        for laenge in (0..daten.len()).step_by(schritt * 3) {
            geprueft += 1;
            angenommen += usize::from(gleich_wie_ganz(
                &daten[..laenge],
                &format!("{name}, gekürzt auf {laenge}"),
            ));
        }
    }
    // Ohne angenommene Fälle hätte der Test nichts verglichen.
    assert!(
        angenommen > geprueft / 4,
        "nur {angenommen} von {geprueft} Fassungen angenommen"
    );
}

/// Ein Leser, der mitzählt, wie viel gelesen wurde.
struct Zaehler<'a> {
    innen: Cursor<&'a [u8]>,
    gelesen: u64,
}

impl Read for Zaehler<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.innen.read(buf)?;
        self.gelesen += n as u64;
        Ok(n)
    }
}

impl Seek for Zaehler<'_> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.innen.seek(pos)
    }
}

#[test]
fn die_bilddaten_werden_nicht_gelesen() {
    // Der Zweck des Ganzen: Ein Video aus zwanzig MiB Bilddaten wird
    // bereinigt, ohne dass die Bilddaten gelesen werden.
    let mut daten = lade("video_mit_ortsangabe.mp4");
    let vorher = daten.len();
    let gross = 20u32 * 1024 * 1024;
    daten.extend_from_slice(&gross.to_be_bytes());
    daten.extend_from_slice(b"mdat");
    daten.resize(vorher + gross as usize, 0x5A);

    assert!(gleich_wie_ganz(&daten, "mit 20 MiB mdat"));

    let mut z = Zaehler {
        innen: Cursor::new(&daten),
        gelesen: 0,
    };
    ueber_geruest(&mut z, daten.len() as u64, Format::Video)
        .unwrap()
        .unwrap();
    assert!(
        z.gelesen < 2 * vorher as u64 + 64,
        "{} Bytes gelesen bei einer Vorlage von {vorher}",
        z.gelesen
    );
}

#[test]
fn andere_formate_kennen_den_weg_nicht() {
    let daten = lade("foto_mit_exif.jpg");
    let b = ueber_geruest(&mut Cursor::new(&daten), daten.len() as u64, Format::Jpeg).unwrap();
    assert!(b.is_none());
}

// ---------------------------------------------------------------------------
// Flicken einsetzen
// ---------------------------------------------------------------------------

#[test]
fn flicken_werden_auch_ueber_blockgrenzen_eingesetzt() {
    let flicken = vec![
        Flicken {
            versatz: 2,
            bytes: b"AB".to_vec(),
        },
        Flicken {
            versatz: 7,
            bytes: b"CDEF".to_vec(),
        },
    ];
    let original = vec![b'.'; 14];
    let mut ganz = original.clone();
    flicken_anwenden(0, &mut ganz, &flicken);
    assert_eq!(ganz, b"..AB...CDEF...");

    // In Blöcken jeder Größe dasselbe.
    for block in 1..=14 {
        let mut stueckweise = Vec::new();
        for (i, teil) in original.chunks(block).enumerate() {
            let mut t = teil.to_vec();
            flicken_anwenden((i * block) as u64, &mut t, &flicken);
            stueckweise.extend_from_slice(&t);
        }
        assert_eq!(stueckweise, ganz, "Blöcke zu {block}");
    }
}
