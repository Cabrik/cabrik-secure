//! JPEG auf Segment-Ebene (`spec/metadata.md` §4, §7.1).
//!
//! # Das eingebettete Vorschaubild
//!
//! EXIF trägt eine verkleinerte Vorschau. Viele Programme aktualisieren beim
//! Zuschneiden **das Hauptbild, aber nicht die Vorschau**. Wer ein Foto
//! beschneidet, um ein Gesicht, ein Kennzeichen oder ein Dokument im
//! Hintergrund zu entfernen, trägt das Entfernte in der Vorschau weiter mit
//! sich.
//!
//! Der bekannteste Fall ist der von Cat Schwartz (2003): ein für einen Blog
//! zugeschnittenes Porträt, dessen EXIF-Thumbnail die unbeschnittene Aufnahme
//! enthielt. Für die Zielgruppe dieses Programms ist die ernstere Variante
//! die Schwärzung von Dokumentfotos.
//!
//! Deshalb ist ein Vorschaubild [`Severity::Critical`] — es ist keine
//! Metadatenart im engeren Sinn, sondern eine **zweite Kopie des Inhalts**.
//!
//! # Aufbau
//!
//! ```text
//! SOI: FF D8
//! je Segment: FF <marker> <length u16 BE, einschliesslich der 2 Laengenbytes> <daten>
//! SOS (FF DA) leitet die Bilddaten ein; danach folgt der Entropie-Strom.
//! ```

use crate::model::{Finding, FindingKind, Inspection, Severity, StripResult};
use cabrik_core::{Error, Result};

/// Start of Image.
const SOI: [u8; 2] = [0xFF, 0xD8];
/// Start of Scan — danach beginnen die Bilddaten.
const SOS: u8 = 0xDA;
/// End of Image.
const EOI: u8 = 0xD9;

/// Ob die Bytes wie ein JPEG aussehen.
#[must_use]
pub fn looks_like_jpeg(data: &[u8]) -> bool {
    data.starts_with(&SOI)
}

/// Marker, die für das Bild gebraucht werden und **bleiben**.
///
/// Alles andere ist Anwendungs- oder Kommentarsegment und fliegt raus.
const fn ist_bildsegment(marker: u8) -> bool {
    matches!(
        marker,
        // SOF0..SOF15 ohne die Marker, die keine Rahmen sind
        0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF
        // DHT, DQT, DRI, SOS
        | 0xC4 | 0xDB | 0xDD | SOS
    )
}

struct Segment<'a> {
    marker: u8,
    data: &'a [u8],
    /// Vollständiges Segment inklusive Marker und Länge.
    roh: &'a [u8],
}

fn parse(data: &[u8]) -> Result<(Vec<Segment<'_>>, &[u8])> {
    if !looks_like_jpeg(data) {
        return Err(Error::Malformed("jpeg: bad signature"));
    }
    let mut segmente = Vec::new();
    let mut pos = 2usize;

    loop {
        let Some(&0xFF) = data.get(pos) else {
            return Err(Error::Malformed("jpeg: expected marker"));
        };

        // Auffuellbytes 0xFF ueberspringen.
        let mut m_pos = pos
            .checked_add(1)
            .ok_or(Error::Malformed("jpeg: overflow"))?;
        while data.get(m_pos) == Some(&0xFF) {
            m_pos = m_pos
                .checked_add(1)
                .ok_or(Error::Malformed("jpeg: overflow"))?;
        }
        let marker = *data
            .get(m_pos)
            .ok_or(Error::Malformed("jpeg: truncated marker"))?;

        if marker == EOI {
            return Err(Error::Malformed("jpeg: EOI before image data"));
        }

        let len_pos = m_pos
            .checked_add(1)
            .ok_or(Error::Malformed("jpeg: overflow"))?;
        let len = usize::from(u16::from_be_bytes(
            data.get(len_pos..len_pos.saturating_add(2))
                .and_then(|s| s.try_into().ok())
                .ok_or(Error::Malformed("jpeg: truncated length"))?,
        ));
        if len < 2 {
            return Err(Error::Malformed("jpeg: segment length below minimum"));
        }

        let daten_start = len_pos
            .checked_add(2)
            .ok_or(Error::Malformed("jpeg: overflow"))?;
        let daten_ende = len_pos
            .checked_add(len)
            .ok_or(Error::Malformed("jpeg: overflow"))?;
        let seg_daten = data
            .get(daten_start..daten_ende)
            .ok_or(Error::Malformed("jpeg: truncated segment"))?;
        let roh = data
            .get(pos..daten_ende)
            .ok_or(Error::Malformed("jpeg: truncated segment"))?;

        segmente.push(Segment {
            marker,
            data: seg_daten,
            roh,
        });
        pos = daten_ende;

        // Nach SOS folgt der Entropie-Strom bis zum Dateiende.
        if marker == SOS {
            let rest = data.get(pos..).unwrap_or(&[]);
            return Ok((segmente, rest));
        }
    }
}

/// Ob ein APP1-Segment EXIF enthält.
fn ist_exif(data: &[u8]) -> bool {
    data.starts_with(b"Exif\0\0")
}

/// Liest, was im EXIF steht — Eintrag für Eintrag.
///
/// # Warum der TIFF-Leser und keine eigene Suche
///
/// Hier stand eine grobe Bytesuche, begründet damit, das Segment werde
/// ohnehin vollständig entfernt; es gehe allein darum, dem Nutzer sagen zu
/// können, *was* drinstand. Genau das tat sie aber nicht: Sie meldete
/// „3780 Bytes EXIF-Block“ und verschwieg Kameramodell, Seriennummer,
/// Aufnahmezeit und Software.
///
/// Ein EXIF-Segment **ist** ein TIFF-Strom hinter sechs Bytes Vorspann,
/// und `tiff.rs` liest ihn seit jeher vollständig, mit Tag-Namen und
/// lesbaren Werten. Ihn nicht zu benutzen hieße, denselben Leser ein
/// zweites Mal zu schreiben — schlechter und ungeprüft.
///
/// # Was die grobe Suche trotzdem beiträgt
///
/// Zwei Dinge, die der Leser nicht sieht, weil er nur der Kette der
/// Hauptverzeichnisse folgt und nicht in Unterverzeichnisse absteigt: das
/// eingebettete Vorschaubild und den GPS-Zeiger. Beide sind zu wichtig, um
/// sie fallenzulassen — ein zweites, womöglich unbeschnittenes Bild und
/// eine Ortsangabe.
fn exif_befunde(data: &[u8], out: &mut Vec<Finding>) {
    // Ein zweites SOI im EXIF-Block ist das Vorschaubild.
    let hat_vorschau = data.windows(2).skip(1).any(|w| w == SOI);
    if hat_vorschau {
        out.push(Finding::new(
            FindingKind::EmbeddedPreview,
            "EXIF:Thumbnail",
            Some("eingebettetes Vorschaubild".to_owned()),
            // Zweite Kopie des Inhalts, moeglicherweise unbeschnitten.
            Severity::Critical,
        ));
    }

    // GPS-IFD-Zeiger, Tag 0x8825. Beide Bytefolgen, weil EXIF in beiden
    // Byte-Reihenfolgen vorkommt.
    if data
        .windows(2)
        .any(|w| w == [0x88, 0x25] || w == [0x25, 0x88])
    {
        out.push(Finding::new(
            FindingKind::Gps,
            "EXIF:GPSInfo",
            Some("Ortsangabe".to_owned()),
            Severity::Critical,
        ));
    }

    // `Exif` plus zwei Nullbytes weg -- dahinter beginnt der TIFF-Strom.
    let einzeln = data
        .get(6..)
        .and_then(|tiff| crate::tiff::inspect(tiff).ok())
        .map(|i| i.findings)
        .unwrap_or_default();

    if einzeln.is_empty() {
        // Der Leser kam nicht durch. Dann ist die grobe Aussage besser als
        // gar keine: Es steht etwas drin, und wir sagen wenigstens wie viel.
        out.push(Finding::new(
            FindingKind::Device,
            "EXIF",
            Some(format!("{} Bytes EXIF-Block", data.len())),
            Severity::Notable,
        ));
        return;
    }

    for f in einzeln {
        // Die Fundstelle bekommt den Vorspann, damit im Bericht steht, wo
        // sie herkommt: `EXIF:Model` statt `TIFF:Model`.
        let ort = f
            .location
            .strip_prefix("TIFF:")
            .map_or_else(|| f.location.clone(), |rest| format!("EXIF:{rest}"));
        out.push(Finding::new(f.kind, ort, f.value, f.severity));
    }
}

/// Fasst mehrfach vorkommende Fundstellen zu einer zusammen.
///
/// # Warum das nötig ist
///
/// Ein großes ICC-Farbprofil passt nicht in ein Segment: JPEG begrenzt sie
/// auf 64 KiB, also wird es über mehrere APP2-Segmente verteilt. Jedes
/// ergibt einen eigenen Fund an derselben Stelle — bei einem
/// Kameraprofil waren es elf.
///
/// Elf gleichlautende Zeilen sind zwar wahr, aber unbrauchbar: Sie
/// verdrängen die Funde, auf die es ankommt, aus dem Blick. Eine Zeile mit
/// der Zahl daneben sagt dasselbe und lässt den Rest lesbar.
///
/// **Die Reihenfolge bleibt.** Der zusammengefasste Fund steht dort, wo
/// der erste stand — sonst wanderte er unerwartet in der Liste.
fn zusammenfassen(funde: Vec<Finding>) -> Vec<Finding> {
    let mut aus: Vec<Finding> = Vec::with_capacity(funde.len());
    let mut anzahl: Vec<usize> = Vec::with_capacity(funde.len());

    for f in funde {
        if let Some(i) = aus
            .iter()
            .position(|a| a.kind == f.kind && a.location == f.location)
        {
            if let Some(n) = anzahl.get_mut(i) {
                *n = n.saturating_add(1);
            }
        } else {
            aus.push(f);
            anzahl.push(1);
        }
    }

    for (f, n) in aus.iter_mut().zip(anzahl) {
        if n > 1 {
            f.value = Some(match f.value.take() {
                Some(v) => format!("{n} Segmente, je {v}"),
                None => format!("{n} Segmente"),
            });
        }
    }
    aus
}

fn befund(marker: u8, data: &[u8]) -> Vec<Finding> {
    let mut out = Vec::new();
    match marker {
        // APP1: EXIF oder XMP.
        0xE1 => {
            if ist_exif(data) {
                exif_befunde(data, &mut out);
            } else if data.starts_with(b"http://ns.adobe.com/xap/") {
                out.push(Finding::new(
                    FindingKind::Software,
                    "APP1:XMP",
                    Some("XMP-Metadaten".to_owned()),
                    Severity::Notable,
                ));
            } else {
                out.push(Finding::new(
                    FindingKind::UnknownExtension,
                    "APP1",
                    None,
                    Severity::Minor,
                ));
            }
        }
        // APP13: Photoshop/IPTC, enthaelt oft Autorenangaben.
        0xED => out.push(Finding::new(
            FindingKind::Author,
            "APP13:IPTC",
            Some("IPTC-Block".to_owned()),
            Severity::Critical,
        )),
        // APP2: haeufig ICC-Farbprofil.
        0xE2 => out.push(Finding::new(
            FindingKind::ColorProfile,
            "APP2:ICC",
            None,
            Severity::Minor,
        )),
        // COM: Kommentar.
        0xFE => out.push(Finding::new(
            FindingKind::Comment,
            "COM",
            core::str::from_utf8(data).ok().map(str::to_owned),
            Severity::Notable,
        )),
        // Uebrige APPn.
        0xE0..=0xEF => out.push(Finding::new(
            FindingKind::UnknownExtension,
            format!("APP{}", marker.saturating_sub(0xE0)),
            Some(format!("{} Bytes", data.len())),
            Severity::Minor,
        )),
        _ => out.push(Finding::new(
            FindingKind::UnknownExtension,
            format!("Marker {marker:#04X}"),
            None,
            Severity::Minor,
        )),
    }
    out
}

/// Zeigt die Metadaten einer JPEG-Datei an, ohne sie zu verändern.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Struktur.
pub fn inspect(data: &[u8]) -> Result<Inspection> {
    let (segmente, _) = parse(data)?;
    let findings = zusammenfassen(
        segmente
            .iter()
            .filter(|s| !ist_bildsegment(s.marker))
            .flat_map(|s| befund(s.marker, s.data))
            .collect(),
    );

    Ok(Inspection {
        format: Some("JPEG".to_owned()),
        findings,
        understood: true,
    })
}

/// Entfernt alle Metadatensegmente.
///
/// Die Bilddaten bleiben Byte für Byte erhalten; es wird **nicht** neu
/// kodiert. Das eingebettete Vorschaubild verschwindet mit dem EXIF-Block.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Struktur.
pub fn strip(data: &[u8]) -> Result<(Vec<u8>, StripResult)> {
    let (segmente, entropie) = parse(data)?;

    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&SOI);
    let mut removed = Vec::new();

    for s in &segmente {
        if ist_bildsegment(s.marker) {
            out.extend_from_slice(s.roh);
        } else {
            removed.extend(befund(s.marker, s.data));
        }
    }
    out.extend_from_slice(entropie);

    Ok((out, StripResult::Complete { removed }))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;

    fn segment(marker: u8, data: &[u8]) -> Vec<u8> {
        let mut out = vec![0xFF, marker];
        out.extend_from_slice(&((data.len() + 2) as u16).to_be_bytes());
        out.extend_from_slice(data);
        out
    }

    /// EXIF-Block mit GPS-Zeiger und eingebettetem Vorschaubild.
    fn exif_block() -> Vec<u8> {
        let mut d = b"Exif\0\0".to_vec();
        d.extend_from_slice(&[0x49, 0x49, 0x2A, 0x00]); // TIFF-Kopf
        d.extend_from_slice(&[0x88, 0x25]); // GPS-IFD-Tag
        d.extend_from_slice(b"Canon EOS");
        d.extend_from_slice(&SOI); // Beginn des Vorschaubilds
        d.extend_from_slice(&[0xFF, 0xD9]);
        d
    }

    fn foto_mit_allem() -> Vec<u8> {
        let mut d = SOI.to_vec();
        d.extend(segment(0xE0, b"JFIF\0\x01\x02\0\0\x01\0\x01\0\0")); // APP0
        d.extend(segment(0xE1, &exif_block())); // APP1 EXIF
        d.extend(segment(0xED, b"Photoshop 3.0\0IPTC")); // APP13
        d.extend(segment(0xFE, b"Aufgenommen von Max")); // COM
        d.extend(segment(0xDB, &[0u8; 64])); // DQT
        d.extend(segment(0xC0, &[8, 0, 1, 0, 1, 1, 0x11, 0])); // SOF0
        d.extend(segment(0xC4, &[0u8; 20])); // DHT
        d.extend(segment(0xDA, &[1, 0, 0])); // SOS
        d.extend_from_slice(&[0x12, 0x34, 0x56, 0xFF, 0xD9]); // Bilddaten + EOI
        d
    }

    #[test]
    fn erkennt_jpeg() {
        assert!(looks_like_jpeg(&foto_mit_allem()));
        assert!(!looks_like_jpeg(b"\x89PNG"));
    }

    /// Der Fall aus `spec/metadata.md` §7.1.
    #[test]
    fn eingebettetes_vorschaubild_ist_kritisch() {
        let i = inspect(&foto_mit_allem()).unwrap();
        let vorschau = i
            .findings
            .iter()
            .find(|f| f.kind == FindingKind::EmbeddedPreview)
            .expect("Vorschaubild wurde nicht erkannt");

        assert_eq!(
            vorschau.severity,
            Severity::Critical,
            "eine zweite Kopie des Inhalts wiegt schwerer als ein Kameramodell"
        );
        assert!(i.has_critical());
    }

    #[test]
    fn gps_wird_als_kritisch_gemeldet() {
        let i = inspect(&foto_mit_allem()).unwrap();
        let gps = i.findings.iter().find(|f| f.kind == FindingKind::Gps);
        assert_eq!(gps.map(|f| f.severity), Some(Severity::Critical));
    }

    #[test]
    fn bilddaten_bleiben_erhalten() {
        let orig = foto_mit_allem();
        let (sauber, _) = strip(&orig).unwrap();

        let (vorher, e_vorher) = parse(&orig).unwrap();
        let (nachher, e_nachher) = parse(&sauber).unwrap();

        assert_eq!(e_vorher, e_nachher, "Entropie-Strom wurde veraendert");
        for marker in [0xDB, 0xC0, 0xC4, 0xDA] {
            let a = vorher.iter().find(|s| s.marker == marker).unwrap();
            let b = nachher.iter().find(|s| s.marker == marker).unwrap();
            assert_eq!(a.roh, b.roh, "Segment {marker:#04X} wurde veraendert");
        }
    }

    #[test]
    fn metadaten_verschwinden_vollstaendig() {
        let (sauber, ergebnis) = strip(&foto_mit_allem()).unwrap();

        // Kein EXIF, kein IPTC, kein Kommentar mehr in den Bytes.
        assert!(
            !sauber.windows(6).any(|w| w == b"Exif\0\0"),
            "EXIF-Block blieb stehen"
        );
        assert!(
            !sauber.windows(4).any(|w| w == b"IPTC"),
            "IPTC-Block blieb stehen"
        );
        assert!(
            !sauber.windows(3).any(|w| w == b"Max"),
            "Kommentar blieb stehen"
        );
        assert!(
            !sauber.windows(9).any(|w| w == b"Canon EOS"),
            "Kameramodell blieb stehen"
        );

        assert!(ergebnis.may_show_clean());
        assert!(ergebnis.has_critical());
    }

    #[test]
    fn das_vorschaubild_verschwindet_mit() {
        // Der eigentliche Zweck: Ein zugeschnittenes Foto darf die
        // unbeschnittene Fassung nicht weitertragen.
        let orig = foto_mit_allem();
        let (sauber, _) = strip(&orig).unwrap();

        // Im Original gibt es nach dem ersten SOI ein weiteres.
        let weitere_soi_vorher = orig.windows(2).skip(1).filter(|w| *w == SOI).count();
        let weitere_soi_nachher = sauber.windows(2).skip(1).filter(|w| *w == SOI).count();

        assert!(weitere_soi_vorher > 0, "Testaufbau enthaelt keine Vorschau");
        assert_eq!(
            weitere_soi_nachher, 0,
            "das eingebettete Vorschaubild blieb erhalten"
        );
    }

    #[test]
    fn jpeg_ohne_metadaten_bleibt_fast_unveraendert() {
        let mut d = SOI.to_vec();
        d.extend(segment(0xDB, &[0u8; 64]));
        d.extend(segment(0xC0, &[8, 0, 1, 0, 1, 1, 0x11, 0]));
        d.extend(segment(0xC4, &[0u8; 20]));
        d.extend(segment(0xDA, &[1, 0, 0]));
        d.extend_from_slice(&[0x12, 0x34, 0xFF, 0xD9]);

        let (sauber, ergebnis) = strip(&d).unwrap();
        assert_eq!(sauber, d);
        assert!(ergebnis.removed().is_empty());
    }

    #[test]
    fn strippen_ist_idempotent() {
        let (einmal, _) = strip(&foto_mit_allem()).unwrap();
        let (zweimal, ergebnis) = strip(&einmal).unwrap();
        assert_eq!(einmal, zweimal);
        assert!(ergebnis.removed().is_empty());
    }

    #[test]
    fn kaputte_dateien_werden_abgelehnt() {
        let orig = foto_mit_allem();
        assert!(strip(b"kein jpeg").is_err());
        assert!(strip(&SOI).is_err(), "Signatur allein ist kein JPEG");
        assert!(strip(&orig[..10]).is_err(), "abgeschnitten");

        // Laengenangabe kleiner als das Minimum.
        let kaputt = [0xFF, 0xD8, 0xFF, 0xE1, 0x00, 0x01];
        assert!(strip(&kaputt).is_err());
    }

    #[test]
    fn inspektion_veraendert_nichts() {
        let orig = foto_mit_allem();
        let kopie = orig.clone();
        assert!(inspect(&orig).unwrap().understood);
        assert_eq!(orig, kopie);
    }
}
