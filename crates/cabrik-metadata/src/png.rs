//! PNG auf Chunk-Ebene (`spec/metadata.md` §4, §7.3).
//!
//! # Der Palette-Bug aus v1
//!
//! ```python
//! im2 = Image.new(mode, size)
//! im2.putdata(data)
//! ```
//!
//! Bei Palette-PNGs (Mode `P`) erzeugte das ein Bild **ohne Palette**: Die
//! Indexwerte wurden übernommen, die Farbtabelle nicht — das Ergebnis hatte
//! falsche Farben. Obendrein kodierte v1 das Bild neu.
//!
//! Hier wird stattdessen die **Chunk-Struktur** bearbeitet: Metadaten-Chunks
//! fallen weg, alle übrigen bleiben Byte für Byte erhalten. Das ist
//! verlustfrei und lässt `PLTE` unangetastet.
//!
//! # Aufbau
//!
//! ```text
//! Signatur: 89 50 4E 47 0D 0A 1A 0A
//! je Chunk: length(u32 BE) ‖ type(4) ‖ data(length) ‖ crc(u32 BE)
//! ```

use crate::model::{Finding, FindingKind, Inspection, Severity, StripResult};
use cabrik_core::{Error, Result};

/// PNG-Signatur.
pub const SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// Chunks, die für die Darstellung gebraucht werden und **bleiben**.
///
/// `PLTE` steht ausdrücklich darin — sein Verlust war der v1-Bug.
const KEEP: [&[u8; 4]; 8] = [
    b"IHDR", b"PLTE", b"IDAT", b"IEND", b"tRNS", b"gAMA", b"cHRM", b"sRGB",
];

/// Höchstgröße eines Chunks, den wir annehmen.
///
/// Ohne Grenze könnte eine präparierte Datei 4 GiB anfordern.
const MAX_CHUNK: usize = 64 * 1024 * 1024;

/// Ob die Bytes wie ein PNG aussehen.
#[must_use]
pub fn looks_like_png(data: &[u8]) -> bool {
    data.starts_with(&SIGNATURE)
}

struct Chunk<'a> {
    typ: [u8; 4],
    data: &'a [u8],
    /// Der vollständige Chunk inklusive Länge und CRC.
    roh: &'a [u8],
}

fn parse(data: &[u8]) -> Result<Vec<Chunk<'_>>> {
    if !looks_like_png(data) {
        return Err(Error::Malformed("png: bad signature"));
    }
    let mut chunks = Vec::new();
    let mut pos = SIGNATURE.len();

    loop {
        if pos == data.len() {
            break;
        }
        let kopf_ende = pos
            .checked_add(8)
            .ok_or(Error::Malformed("png: offset overflow"))?;
        let kopf = data
            .get(pos..kopf_ende)
            .ok_or(Error::Malformed("png: truncated chunk header"))?;

        let len = usize::try_from(u32::from_be_bytes(
            kopf.get(0..4)
                .and_then(|s| s.try_into().ok())
                .ok_or(Error::Malformed("png: bad length"))?,
        ))
        .map_err(|_| Error::Malformed("png: length overflow"))?;

        if len > MAX_CHUNK {
            return Err(Error::Malformed("png: chunk exceeds size limit"));
        }

        let typ: [u8; 4] = kopf
            .get(4..8)
            .and_then(|s| s.try_into().ok())
            .ok_or(Error::Malformed("png: bad type"))?;

        let daten_ende = kopf_ende
            .checked_add(len)
            .ok_or(Error::Malformed("png: offset overflow"))?;
        let crc_ende = daten_ende
            .checked_add(4)
            .ok_or(Error::Malformed("png: offset overflow"))?;

        let chunk_daten = data
            .get(kopf_ende..daten_ende)
            .ok_or(Error::Malformed("png: truncated chunk data"))?;
        let roh = data
            .get(pos..crc_ende)
            .ok_or(Error::Malformed("png: truncated chunk crc"))?;

        let ist_ende = &typ == b"IEND";
        chunks.push(Chunk {
            typ,
            data: chunk_daten,
            roh,
        });
        pos = crc_ende;

        if ist_ende {
            break;
        }
    }

    if pos != data.len() {
        return Err(Error::Malformed("png: trailing bytes after IEND"));
    }
    if chunks.is_empty() {
        return Err(Error::Malformed("png: no chunks"));
    }
    Ok(chunks)
}

/// Beschreibt einen Metadaten-Chunk.
fn befund(typ: &[u8; 4], data: &[u8]) -> Option<Finding> {
    let name = core::str::from_utf8(typ).unwrap_or("????");
    let ort = format!("PNG:{name}");

    match typ {
        // Textuelle Metadaten. `tEXt` ist "Schlüssel\0Wert".
        b"tEXt" | b"zTXt" | b"iTXt" => {
            let text = data
                .split(|&b| b == 0)
                .next()
                .and_then(|k| core::str::from_utf8(k).ok())
                .map(str::to_owned);
            Some(Finding::new(
                FindingKind::Comment,
                ort,
                text,
                Severity::Notable,
            ))
        }
        // Eingebettetes EXIF — kann GPS enthalten.
        b"eXIf" => Some(Finding::new(
            FindingKind::Gps,
            ort,
            Some(format!("{} Bytes EXIF", data.len())),
            Severity::Critical,
        )),
        b"tIME" => Some(Finding::new(
            FindingKind::Timestamp,
            ort,
            None,
            Severity::Notable,
        )),
        b"iCCP" => Some(Finding::new(
            FindingKind::ColorProfile,
            ort,
            None,
            Severity::Minor,
        )),
        _ => Some(Finding::new(
            FindingKind::UnknownExtension,
            ort,
            Some(format!("{} Bytes", data.len())),
            Severity::Minor,
        )),
    }
}

/// Zeigt die Metadaten einer PNG-Datei an, ohne sie zu verändern.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Struktur.
pub fn inspect(data: &[u8]) -> Result<Inspection> {
    let chunks = parse(data)?;
    let findings = chunks
        .iter()
        .filter(|c| !KEEP.contains(&&c.typ))
        .filter_map(|c| befund(&c.typ, c.data))
        .collect();

    Ok(Inspection {
        format: Some("PNG".to_owned()),
        findings,
        understood: true,
    })
}

/// Entfernt alle Metadaten-Chunks.
///
/// Verlustfrei: Bilddaten, Palette und Transparenz bleiben Byte für Byte
/// erhalten. Es wird **nicht** neu kodiert.
///
/// Unbekannte Chunks werden entfernt und namentlich gemeldet; das Ergebnis
/// bleibt [`StripResult::Complete`] (`spec/metadata.md` §7.3).
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Struktur.
pub fn strip(data: &[u8]) -> Result<(Vec<u8>, StripResult)> {
    let chunks = parse(data)?;

    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&SIGNATURE);
    let mut removed = Vec::new();

    for c in &chunks {
        if KEEP.contains(&&c.typ) {
            out.extend_from_slice(c.roh);
        } else if let Some(f) = befund(&c.typ, c.data) {
            removed.push(f);
        }
    }

    Ok((out, StripResult::Complete { removed }))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;

    /// CRC32 nach PNG-Spezifikation — nur für den Testaufbau.
    fn crc32(data: &[u8]) -> u32 {
        let mut c: u32 = 0xFFFF_FFFF;
        for &b in data {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 == 1 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        c ^ 0xFFFF_FFFF
    }

    fn chunk(typ: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(typ);
        out.extend_from_slice(data);
        let mut crc_input = typ.to_vec();
        crc_input.extend_from_slice(data);
        out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
        out
    }

    /// Ein Palette-PNG mit Metadaten — genau der Fall, an dem v1 scheiterte.
    fn palette_png_mit_metadaten() -> Vec<u8> {
        let mut d = SIGNATURE.to_vec();
        d.extend(chunk(b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 3, 0, 0, 0]));
        d.extend(chunk(b"PLTE", &[255, 0, 0, 0, 255, 0, 0, 0, 255]));
        d.extend(chunk(b"tRNS", &[255, 128]));
        d.extend(chunk(b"tEXt", b"Author\0Max Mustermann"));
        d.extend(chunk(b"tIME", &[0x07, 0xE8, 1, 1, 0, 0, 0]));
        d.extend(chunk(b"eXIf", &[0x49, 0x49, 0x2A, 0x00]));
        d.extend(chunk(b"IDAT", &[0x78, 0x9C, 0x63, 0x00, 0x00, 0x00, 0x02]));
        d.extend(chunk(b"IEND", &[]));
        d
    }

    #[test]
    fn erkennt_png() {
        assert!(looks_like_png(&palette_png_mit_metadaten()));
        assert!(!looks_like_png(b"\xFF\xD8\xFF"));
        assert!(!looks_like_png(b""));
    }

    #[test]
    fn palette_bleibt_erhalten() {
        // Der v1-Bug: Image.new + putdata verlor die Farbtabelle.
        let orig = palette_png_mit_metadaten();
        let (sauber, _) = strip(&orig).unwrap();

        let chunks = parse(&sauber).unwrap();
        let typen: Vec<&[u8; 4]> = chunks.iter().map(|c| &c.typ).collect();

        assert!(typen.contains(&b"PLTE"), "Palette ging verloren");
        assert!(typen.contains(&b"tRNS"), "Transparenz ging verloren");
        assert!(typen.contains(&b"IHDR"));
        assert!(typen.contains(&b"IDAT"));
        assert!(typen.contains(&b"IEND"));
    }

    #[test]
    fn bilddaten_bleiben_byte_fuer_byte_gleich() {
        let orig = palette_png_mit_metadaten();
        let (sauber, _) = strip(&orig).unwrap();

        let vorher = parse(&orig).unwrap();
        let nachher = parse(&sauber).unwrap();

        for typ in [b"IHDR", b"PLTE", b"IDAT"] {
            let a = vorher.iter().find(|c| &c.typ == typ).unwrap();
            let b = nachher.iter().find(|c| &c.typ == typ).unwrap();
            assert_eq!(
                a.roh,
                b.roh,
                "{} wurde veraendert",
                core::str::from_utf8(typ).unwrap()
            );
        }
    }

    #[test]
    fn metadaten_verschwinden() {
        let (sauber, ergebnis) = strip(&palette_png_mit_metadaten()).unwrap();

        let typen: Vec<[u8; 4]> = parse(&sauber).unwrap().iter().map(|c| c.typ).collect();
        for weg in [b"tEXt", b"tIME", b"eXIf"] {
            assert!(
                !typen.contains(weg),
                "{} blieb stehen",
                core::str::from_utf8(weg).unwrap()
            );
        }

        assert!(ergebnis.may_show_clean());
        assert_eq!(ergebnis.removed().len(), 3);
        assert!(ergebnis.has_critical(), "eXIf kann GPS enthalten");
    }

    #[test]
    fn der_name_des_autors_taucht_im_befund_auf() {
        let i = inspect(&palette_png_mit_metadaten()).unwrap();
        let text = i
            .findings
            .iter()
            .find(|f| f.location == "PNG:tEXt")
            .unwrap();
        assert_eq!(text.value.as_deref(), Some("Author"));
    }

    #[test]
    fn inspektion_veraendert_nichts() {
        let orig = palette_png_mit_metadaten();
        let kopie = orig.clone();
        let i = inspect(&orig).unwrap();
        assert!(i.understood);
        assert_eq!(orig, kopie);
    }

    #[test]
    fn png_ohne_metadaten_bleibt_unveraendert() {
        let mut d = SIGNATURE.to_vec();
        d.extend(chunk(b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 0, 0, 0, 0]));
        d.extend(chunk(b"IDAT", &[0x78, 0x9C, 0x63, 0x00]));
        d.extend(chunk(b"IEND", &[]));

        let (sauber, ergebnis) = strip(&d).unwrap();
        assert_eq!(sauber, d, "eine saubere Datei darf nicht angefasst werden");
        assert!(ergebnis.removed().is_empty());
    }

    #[test]
    fn unbekannte_chunks_werden_entfernt_und_gemeldet() {
        // spec/metadata.md §7.3: entfernen, benennen, Ergebnis bleibt Complete.
        let mut d = SIGNATURE.to_vec();
        d.extend(chunk(b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 0, 0, 0, 0]));
        d.extend(chunk(b"zzZz", b"irgendwas"));
        d.extend(chunk(b"IDAT", &[0x78, 0x9C]));
        d.extend(chunk(b"IEND", &[]));

        let (sauber, ergebnis) = strip(&d).unwrap();
        assert!(!parse(&sauber).unwrap().iter().any(|c| &c.typ == b"zzZz"));
        assert!(
            ergebnis.may_show_clean(),
            "unbekannte Chunks kippen das Ergebnis nicht"
        );
        let f = &ergebnis.removed()[0];
        assert_eq!(f.kind, FindingKind::UnknownExtension);
        assert_eq!(f.location, "PNG:zzZz");
    }

    #[test]
    fn kaputte_dateien_werden_abgelehnt() {
        let orig = palette_png_mit_metadaten();
        assert!(strip(b"nicht png").is_err());
        assert!(strip(&SIGNATURE).is_err(), "Signatur allein ist kein PNG");
        assert!(strip(&orig[..orig.len() - 3]).is_err(), "abgeschnitten");

        let mut mit_muell = orig.clone();
        mit_muell.push(0xFF);
        assert!(strip(&mit_muell).is_err(), "Bytes nach IEND");

        // Uebergrosse Laengenangabe.
        let mut riesig = SIGNATURE.to_vec();
        riesig.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes());
        riesig.extend_from_slice(b"IDAT");
        assert!(strip(&riesig).is_err());
    }

    #[test]
    fn strippen_ist_idempotent() {
        let (einmal, _) = strip(&palette_png_mit_metadaten()).unwrap();
        let (zweimal, ergebnis) = strip(&einmal).unwrap();
        assert_eq!(einmal, zweimal);
        assert!(ergebnis.removed().is_empty());
    }
}
