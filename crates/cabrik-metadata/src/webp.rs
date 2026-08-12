//! WebP (`spec/metadata.md` §4).
//!
//! Ein RIFF-Behälter: `RIFF` ‖ Größe ‖ `WEBP`, danach Chunks aus
//! Kennung(4) ‖ Größe(4, little-endian) ‖ Nutzdaten ‖ Füllbyte bei ungerader
//! Größe.
//!
//! Das Verfahren ist dasselbe wie bei PNG: Chunks mit Metadaten fallen weg,
//! alle übrigen gehen **byteweise unverändert** durch. Das Bild wird nicht neu
//! kodiert — der v1-Fehler, der bei Palette-PNGs die Farben zerstörte, kann
//! hier gar nicht erst entstehen.
//!
//! # Die Falle: das Merkmalsbyte in `VP8X`
//!
//! Ein erweitertes WebP beginnt mit einem `VP8X`-Chunk, dessen erstes Byte
//! ankreuzt, **welche** optionalen Chunks folgen — darunter ICC, EXIF und XMP.
//!
//! Wer die Chunks entfernt und die Ankreuzung stehen lässt, hinterlässt eine
//! Datei, die Metadaten ankündigt, die es nicht mehr gibt. Strenge Leser
//! halten sie für beschädigt. Die Merkmale werden deshalb mitgelöscht.

use crate::model::{Finding, FindingKind, Inspection, Severity, StripResult};

use cabrik_core::{Error, Result};

/// Länge von `RIFF` ‖ Größe ‖ `WEBP`.
const KOPF_LEN: usize = 12;
/// Länge von Kennung ‖ Größe.
const CHUNK_KOPF: usize = 8;

/// Höchstgröße einer Datei, die wir anfassen.
const MAX_DATEI: usize = 256 * 1024 * 1024;

/// Merkmalsbits im ersten Byte von `VP8X`.
const MERKMAL_ICC: u8 = 0x20;
const MERKMAL_EXIF: u8 = 0x08;
const MERKMAL_XMP: u8 = 0x04;

/// Ob die Bytes wie ein WebP aussehen.
#[must_use]
pub fn looks_like_webp(daten: &[u8]) -> bool {
    daten.len() >= KOPF_LEN && daten.starts_with(b"RIFF") && daten.get(8..12) == Some(b"WEBP")
}

/// Ein gelesener Chunk.
struct Chunk<'a> {
    kennung: [u8; 4],
    nutzdaten: &'a [u8],
}

impl Chunk<'_> {
    fn name(&self) -> String {
        String::from_utf8_lossy(&self.kennung).trim_end().to_owned()
    }

    /// Ob der Chunk Metadaten trägt und damit wegfällt.
    fn ist_metadaten(&self) -> bool {
        matches!(&self.kennung, b"EXIF" | b"XMP " | b"ICCP")
    }
}

/// Zerlegt die Datei in Chunks.
fn zerlege(daten: &[u8]) -> Result<Vec<Chunk<'_>>> {
    if !looks_like_webp(daten) {
        return Err(Error::Malformed("webp: kein RIFF/WEBP-Kopf"));
    }
    if daten.len() > MAX_DATEI {
        return Err(Error::Malformed("webp: Datei zu gross"));
    }

    let mut chunks = Vec::new();
    let mut pos = KOPF_LEN;

    while pos < daten.len() {
        let Some(kopf) = daten.get(pos..pos.saturating_add(CHUNK_KOPF)) else {
            // Ein angebrochener Chunk-Kopf am Ende: abbrechen statt raten.
            break;
        };
        let kennung: [u8; 4] = kopf
            .get(..4)
            .and_then(|s| s.try_into().ok())
            .ok_or(Error::Malformed("webp: Kennung unlesbar"))?;
        let laenge = u32::from_le_bytes(
            kopf.get(4..8)
                .and_then(|s| s.try_into().ok())
                .ok_or(Error::Malformed("webp: Laenge unlesbar"))?,
        );
        let laenge =
            usize::try_from(laenge).map_err(|_| Error::Malformed("webp: Laenge zu gross"))?;

        let start = pos.saturating_add(CHUNK_KOPF);
        let ende = start
            .checked_add(laenge)
            .ok_or(Error::Malformed("webp: Laengenueberlauf"))?;
        let nutzdaten = daten
            .get(start..ende)
            .ok_or(Error::Malformed("webp: Chunk reicht ueber das Dateiende"))?;

        chunks.push(Chunk { kennung, nutzdaten });

        // RIFF füllt ungerade Längen auf ein gerades Vielfaches auf.
        let schritt = laenge.saturating_add(laenge % 2);
        pos = start
            .checked_add(schritt)
            .ok_or(Error::Malformed("webp: Positionsueberlauf"))?;
    }

    Ok(chunks)
}

/// Untersucht ein WebP.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Struktur.
pub fn inspect(daten: &[u8]) -> Result<Inspection> {
    let chunks = zerlege(daten)?;
    Ok(Inspection {
        format: Some("WebP".to_owned()),
        findings: sammle(&chunks),
        understood: true,
    })
}

fn sammle(chunks: &[Chunk<'_>]) -> Vec<Finding> {
    let mut funde = Vec::new();
    for c in chunks {
        let (art, schwere, was) = match &c.kennung {
            b"EXIF" => (
                FindingKind::Device,
                Severity::Notable,
                "EXIF-Block — kann Kamera, Zeitpunkt und Ort enthalten",
            ),
            b"XMP " => (
                FindingKind::Author,
                Severity::Critical,
                "XMP-Block — trägt häufig Verfasser und Bearbeitungsverlauf",
            ),
            b"ICCP" => (
                FindingKind::ColorProfile,
                Severity::Minor,
                "eingebettetes Farbprofil",
            ),
            _ => continue,
        };
        funde.push(Finding::new(
            art,
            format!("WebP:{}", c.name()),
            Some(format!("{was} ({} Bytes)", c.nutzdaten.len())),
            schwere,
        ));
    }
    funde
}

/// Entfernt die Metadaten-Chunks.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Struktur.
pub fn strip(daten: &[u8]) -> Result<(Vec<u8>, StripResult)> {
    let chunks = zerlege(daten)?;
    let entfernt = sammle(&chunks);

    let mut koerper: Vec<u8> = Vec::with_capacity(daten.len());
    for c in &chunks {
        if c.ist_metadaten() {
            continue;
        }

        koerper.extend_from_slice(&c.kennung);
        let laenge = u32::try_from(c.nutzdaten.len())
            .map_err(|_| Error::Malformed("webp: Chunk zu gross"))?;
        koerper.extend_from_slice(&laenge.to_le_bytes());

        if c.kennung == *b"VP8X" {
            // Die Ankreuzung mitlöschen — siehe Modulkopf.
            let mut kopie = c.nutzdaten.to_vec();
            if let Some(b) = kopie.first_mut() {
                *b &= !(MERKMAL_ICC | MERKMAL_EXIF | MERKMAL_XMP);
            }
            koerper.extend_from_slice(&kopie);
        } else {
            koerper.extend_from_slice(c.nutzdaten);
        }

        if c.nutzdaten.len() % 2 == 1 {
            koerper.push(0);
        }
    }

    let mut aus = Vec::with_capacity(KOPF_LEN.saturating_add(koerper.len()));
    aus.extend_from_slice(b"RIFF");
    // Die RIFF-Größe zählt ab `WEBP`, also vier Bytes plus Körper.
    let groesse = u32::try_from(koerper.len().saturating_add(4))
        .map_err(|_| Error::Malformed("webp: Datei zu gross"))?;
    aus.extend_from_slice(&groesse.to_le_bytes());
    aus.extend_from_slice(b"WEBP");
    aus.extend_from_slice(&koerper);

    Ok((aus, StripResult::Complete { removed: entfernt }))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;

    fn chunk(kennung: &[u8; 4], nutzdaten: &[u8]) -> Vec<u8> {
        let mut v = kennung.to_vec();
        v.extend_from_slice(&u32::try_from(nutzdaten.len()).unwrap().to_le_bytes());
        v.extend_from_slice(nutzdaten);
        if nutzdaten.len() % 2 == 1 {
            v.push(0);
        }
        v
    }

    /// Ein erweitertes WebP mit allen drei Metadatenarten.
    fn bild() -> Vec<u8> {
        // VP8X mit gesetzten Merkmalen fuer ICC, EXIF und XMP.
        let mut vp8x = vec![MERKMAL_ICC | MERKMAL_EXIF | MERKMAL_XMP | 0x10];
        vp8x.extend_from_slice(&[0, 0, 0]); // reserviert
        vp8x.extend_from_slice(&[9, 0, 0, 9, 0, 0]); // Breite/Hoehe minus eins

        let mut koerper = Vec::new();
        koerper.extend_from_slice(&chunk(b"VP8X", &vp8x));
        koerper.extend_from_slice(&chunk(b"ICCP", b"ICC-Profil-Daten"));
        koerper.extend_from_slice(&chunk(b"EXIF", b"Exif\0\0GPS-Koordinaten"));
        koerper.extend_from_slice(&chunk(b"XMP ", b"<x>Dr. Anna Beispiel</x>"));
        koerper.extend_from_slice(&chunk(b"VP8 ", b"BILDDATEN-UNVERAENDERT"));

        let mut aus = b"RIFF".to_vec();
        aus.extend_from_slice(&u32::try_from(koerper.len() + 4).unwrap().to_le_bytes());
        aus.extend_from_slice(b"WEBP");
        aus.extend_from_slice(&koerper);
        aus
    }

    #[test]
    fn webp_wird_an_den_kennbytes_erkannt() {
        assert!(looks_like_webp(&bild()));
        assert!(!looks_like_webp(b"RIFF\0\0\0\0WAVE"));
        assert!(!looks_like_webp(b"kurz"));
    }

    #[test]
    fn alle_drei_metadatenarten_werden_gefunden() {
        let i = inspect(&bild()).unwrap();
        assert_eq!(i.format.as_deref(), Some("WebP"));
        for erwartet in ["WebP:EXIF", "WebP:XMP", "WebP:ICCP"] {
            assert!(
                i.findings.iter().any(|f| f.location == erwartet),
                "{erwartet} fehlt: {:?}",
                i.findings.iter().map(|f| &f.location).collect::<Vec<_>>()
            );
        }
    }

    /// XMP traegt haeufig den Verfassernamen und wiegt deshalb schwerer als
    /// ein Farbprofil.
    #[test]
    fn xmp_wiegt_schwerer_als_das_farbprofil() {
        let i = inspect(&bild()).unwrap();
        let xmp = i
            .findings
            .iter()
            .find(|f| f.location == "WebP:XMP")
            .unwrap();
        let icc = i
            .findings
            .iter()
            .find(|f| f.location == "WebP:ICCP")
            .unwrap();
        assert_eq!(xmp.severity, Severity::Critical);
        assert_eq!(icc.severity, Severity::Minor);
    }

    #[test]
    fn die_bilddaten_bleiben_unveraendert() {
        let (sauber, ergebnis) = strip(&bild()).unwrap();
        assert!(ergebnis.may_show_clean());

        assert!(looks_like_webp(&sauber));
        let chunks = zerlege(&sauber).unwrap();
        let bilddaten = chunks
            .iter()
            .find(|c| c.kennung == *b"VP8 ")
            .expect("Bilddaten verschwunden");
        assert_eq!(bilddaten.nutzdaten, b"BILDDATEN-UNVERAENDERT");
    }

    #[test]
    fn die_metadaten_chunks_verschwinden() {
        let (sauber, _) = strip(&bild()).unwrap();
        let chunks = zerlege(&sauber).unwrap();

        assert!(
            !chunks.iter().any(Chunk::ist_metadaten),
            "Metadaten blieben"
        );
        assert!(
            !sauber.windows(15).any(|f| f == b"GPS-Koordinaten"),
            "EXIF-Inhalt blieb in den Bytes"
        );
        assert!(
            !sauber.windows(17).any(|f| f == b"Dr. Anna Beispiel"),
            "der Name blieb in den Bytes"
        );
    }

    /// **Die Falle aus dem Modulkopf.** Bleibt die Ankreuzung stehen,
    /// kuendigt die Datei Metadaten an, die es nicht mehr gibt.
    #[test]
    fn die_merkmale_in_vp8x_werden_mitgeloescht() {
        let (sauber, _) = strip(&bild()).unwrap();
        let chunks = zerlege(&sauber).unwrap();
        let vp8x = chunks
            .iter()
            .find(|c| c.kennung == *b"VP8X")
            .expect("VP8X verschwunden");

        let merkmale = vp8x.nutzdaten[0];
        assert_eq!(merkmale & MERKMAL_ICC, 0, "ICC bleibt angekuendigt");
        assert_eq!(merkmale & MERKMAL_EXIF, 0, "EXIF bleibt angekuendigt");
        assert_eq!(merkmale & MERKMAL_XMP, 0, "XMP bleibt angekuendigt");
        // Das Alpha-Merkmal darf **nicht** mitgeloescht werden.
        assert_eq!(merkmale & 0x10, 0x10, "das Alpha-Merkmal wurde zerstoert");
    }

    /// Die RIFF-Groesse im Kopf muss zur neuen Laenge passen.
    #[test]
    fn die_riff_groesse_wird_neu_berechnet() {
        let (sauber, _) = strip(&bild()).unwrap();
        let angegeben = u32::from_le_bytes(sauber[4..8].try_into().unwrap()) as usize;
        assert_eq!(
            angegeben,
            sauber.len() - 8,
            "die angegebene Groesse passt nicht zur Datei"
        );
    }

    /// Ungerade Chunk-Laengen brauchen ein Fuellbyte.
    #[test]
    fn ungerade_laengen_werden_aufgefuellt() {
        let mut koerper = chunk(b"VP8 ", b"ungerade");
        koerper.extend_from_slice(&chunk(b"EXIF", b"x"));
        let mut roh = b"RIFF".to_vec();
        roh.extend_from_slice(&u32::try_from(koerper.len() + 4).unwrap().to_le_bytes());
        roh.extend_from_slice(b"WEBP");
        roh.extend_from_slice(&koerper);

        let (sauber, _) = strip(&roh).unwrap();
        assert_eq!(sauber.len() % 2, 0, "die Datei ist nicht mehr ausgerichtet");
        assert!(zerlege(&sauber).is_ok());
    }

    #[test]
    fn ein_bild_ohne_metadaten_bleibt_gleich() {
        let mut koerper = chunk(b"VP8 ", b"nur Bilddaten!!");
        koerper.truncate(koerper.len());
        let mut roh = b"RIFF".to_vec();
        roh.extend_from_slice(&u32::try_from(koerper.len() + 4).unwrap().to_le_bytes());
        roh.extend_from_slice(b"WEBP");
        roh.extend_from_slice(&koerper);

        let (sauber, ergebnis) = strip(&roh).unwrap();
        assert_eq!(sauber, roh);
        assert!(ergebnis.may_show_clean());
        match ergebnis {
            StripResult::Complete { removed } => assert!(removed.is_empty()),
            other => panic!("erwartete Complete, bekam {other:?}"),
        }
    }

    #[test]
    fn kaputte_daten_ergeben_einen_fehler_keinen_absturz() {
        assert!(inspect(b"RIFF\xff\xff\xff\xffWEBPVP8 \xff\xff\xff\xff").is_err());
        assert!(inspect(b"RIFF").is_err());
        assert!(inspect(b"").is_err());
    }
}
