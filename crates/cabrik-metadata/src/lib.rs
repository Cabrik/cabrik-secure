//! Erkennen und Entfernen von Metadaten in Nutzdateien.
//!
//! Setzt `spec/metadata.md` um.
//!
//! # Zwei getrennte Probleme
//!
//! | | Problem | Wo gelöst |
//! |---|---|---|
//! | **A** | Der **Envelope** verrät Dateiname, Größe, Zeitpunkt, Absender | `cabrik-core`, vollständig |
//! | **B** | Die **Nutzdatei** trägt eingebettete Metadaten | hier, nur teilweise |
//!
//! Problem A war in v1 gravierender und blieb unbemerkt: Der Dateiname stand
//! im Klartext in der `.enc`-Datei, lesbar ohne jeden Schlüssel. Kein noch so
//! gründliches EXIF-Strippen hätte das aufgewogen.
//!
//! Problem B lässt sich **grundsätzlich nicht vollständig** lösen — Metadaten
//! aus einem Format zu entfernen, das man nicht versteht, ist unmöglich.
//! Entscheidend ist daher nicht die Abdeckung, sondern der ehrliche Umgang
//! mit Lücken. Siehe [`model::StripResult`].
//!
//! # Warum eigenständig
//!
//! Metadaten-Bereinigung heißt Parser für viele Dateiformate. Parser sind
//! Angriffsfläche, und die Abhängigkeiten haben im auditierten Krypto-Kern
//! nichts verloren — der später per UniFFI nach iOS und Android geht.
//!
//! # Stand
//!
//! Behandelt werden Bilder (PNG, JPEG, WebP, GIF, BMP, TIFF, HEIC/AVIF, SVG),
//! Dokumente (PDF, OOXML, ODF, ZIP), Video (MP4/MOV, Matroska/WebM, AVI) und
//! Ton (MP3, FLAC, Ogg/Opus, WAV, M4A).
//!
//! Alle übrigen Formate melden [`model::StripResult::Unknown`] — **korrektes
//! Verhalten, keine Lücke**. v1 kopierte sie stillschweigend durch und
//! suggerierte damit Sauberkeit.
//!
//! # Die eine Regel, die über allen Formaten steht
//!
//! **Nichts verschieben, worauf etwas zeigt.**
//!
//! Sie klingt wie „nichts verschieben", ist es aber nicht — und der
//! Unterschied entscheidet über das Vorgehen:
//!
//! | Verweist die Datei auf Byte-Positionen? | Formate | Vorgehen |
//! |---|---|---|
//! | ja | TIFF, HEIC, MP4, Matroska, AVI, WAV | Platzhalter an Ort und Stelle |
//! | nein | MP3, FLAC | wirklich entfernen, die Datei wird kleiner |
//! | nein, aber je Seite eine Prüfsumme | Ogg, Opus | neu schreiben und neu rechnen |
//!
//! Für den ersten Fall bringt **jedes** dieser Formate seinen eigenen
//! Platzhalter mit: `free` in ISO-BMFF, `Void` in EBML, `JUNK` in RIFF,
//! `PADDING` in FLAC. Ihn zu benutzen ist der vorgesehene Weg, kein
//! Kunstgriff — eine gewöhnliche ffmpeg-Datei enthält von sich aus welche.

pub mod avi;
pub mod bmff;
pub mod bmp;
pub mod container;
pub mod flac;
pub mod gif;
pub mod jpeg;
pub mod matroska;
pub mod model;
pub mod mp3;
pub mod odf;
pub mod ogg;
pub mod ooxml;
pub mod pdf;
pub mod png;
pub mod riff;
pub mod svg;
pub mod tiff;
pub mod video;
pub mod vorbis;
pub mod wav;
pub mod webp;
pub mod xml;
pub mod zip_archiv;

pub use model::{Finding, FindingKind, Inspection, Severity, StripOptions, StripResult};

use cabrik_core::Result;

/// Erkanntes Dateiformat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Format {
    /// PNG — auf Chunk-Ebene vollständig behandelbar.
    Png,
    /// JPEG — auf Segment-Ebene vollständig behandelbar.
    Jpeg,
    /// WebP — RIFF-Chunks wie bei PNG.
    Webp,
    /// GIF — Blockstruktur.
    Gif,
    /// BMP — trägt kaum Metadaten, wird aber geprüft statt durchgewinkt.
    Bmp,
    /// TIFF — die Metadatenstruktur **ist** das Dateiformat.
    Tiff,
    /// HEIC, HEIF und AVIF — ISO-BMFF mit Items.
    Bmff,
    /// SVG — beliebiges XML, bleibt immer `Partial`.
    Svg,
    /// PDF — Objektgraph mit Änderungshistorie, bleibt immer `Partial`.
    Pdf,
    /// MP4, MOV, M4V — ISO-BMFF mit Spuren und Benutzerdaten.
    Video,
    /// Matroska und WebM — EBML mit `Void` als eigenem Platzhalter.
    Matroska,
    /// AVI — RIFF mit `JUNK` als eigenem Platzhalter.
    Avi,
    /// MP3 — ID3v2, ID3v1 und APEv2. Wird als einziges wirklich gekürzt.
    Mp3,
    /// FLAC — Metadatenblöcke mit `PADDING` als eigenem Platzhalter.
    Flac,
    /// Ogg mit Vorbis, Opus oder Speex — Seiten mit eigener Prüfsumme.
    Ogg,
    /// WAV — RIFF wie AVI. Der `bext`-Block ist der eigentliche Fund.
    Wav,
    /// OOXML: `docx`, `xlsx`, `pptx`.
    Ooxml(ooxml::Art),
    /// ODF: `odt`, `ods`, `odp`.
    Odf(odf::Art),
    /// Ein gewöhnliches ZIP-Archiv.
    Zip,
}

impl Format {
    /// Erkennt das Format an seinen Kennbytes.
    ///
    /// Bewusst **nicht** an der Dateiendung: Die sagt nichts darüber aus, was
    /// wirklich in der Datei steht. Eine `.docx`, die in Wahrheit ein JPEG
    /// ist, wird als JPEG behandelt — und umgekehrt.
    #[must_use]
    pub fn detect(data: &[u8]) -> Option<Self> {
        if png::looks_like_png(data) {
            return Some(Self::Png);
        }
        if jpeg::looks_like_jpeg(data) {
            return Some(Self::Jpeg);
        }
        if webp::looks_like_webp(data) {
            return Some(Self::Webp);
        }
        if gif::looks_like_gif(data) {
            return Some(Self::Gif);
        }
        if bmp::looks_like_bmp(data) {
            return Some(Self::Bmp);
        }
        if tiff::looks_like_tiff(data) {
            return Some(Self::Tiff);
        }
        // Video vor Bild: Beides ist ISO-BMFF, die Marke entscheidet.
        if video::looks_like_video(data) {
            return Some(Self::Video);
        }
        if bmff::looks_like_bmff(data) {
            return Some(Self::Bmff);
        }
        if avi::looks_like_avi(data) {
            return Some(Self::Avi);
        }
        if wav::looks_like_wav(data) {
            return Some(Self::Wav);
        }
        if matroska::looks_like_matroska(data) {
            return Some(Self::Matroska);
        }
        // FLAC vor MP3: Steht ein ID3-Tag vor der fLaC-Kennung, hielte
        // die MP3-Erkennung die Datei sonst für ein MP3.
        if flac::looks_like_flac(data) {
            return Some(Self::Flac);
        }
        if mp3::looks_like_mp3(data) {
            return Some(Self::Mp3);
        }
        if ogg::looks_like_ogg(data) {
            return Some(Self::Ogg);
        }
        if svg::looks_like_svg(data) {
            return Some(Self::Svg);
        }
        if pdf::looks_like_pdf(data) {
            return Some(Self::Pdf);
        }
        if container::sieht_aus_wie_zip(data) {
            // Ein ZIP allein sagt noch nichts. Erst der Inhalt entscheidet,
            // ob ein Office-Dokument darin steckt — und wenn nicht, bleibt es
            // ein Archiv, das immerhin bekannt ist.
            let eintraege = container::lies(data).ok()?;
            if let Some(a) = ooxml::erkenne(&eintraege) {
                return Some(Self::Ooxml(a));
            }
            if let Some(a) = odf::erkenne(&eintraege) {
                return Some(Self::Odf(a));
            }
            return Some(Self::Zip);
        }
        None
    }

    /// Name für die Anzeige.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Webp => "WebP",
            Self::Gif => "GIF",
            Self::Bmp => "BMP",
            Self::Tiff => "TIFF",
            Self::Bmff => "HEIC/AVIF",
            Self::Svg => "SVG",
            Self::Pdf => "PDF",
            Self::Video => "MP4/MOV",
            Self::Matroska => "Matroska/WebM",
            Self::Avi => "AVI",
            Self::Mp3 => "MP3",
            Self::Flac => "FLAC",
            Self::Ogg => "Ogg",
            Self::Wav => "WAV",
            Self::Ooxml(a) => a.name(),
            Self::Odf(a) => a.name(),
            Self::Zip => "ZIP-Archiv",
        }
    }
}

/// Zeigt die Metadaten einer Datei an, ohne sie zu verändern.
///
/// Bei unbekanntem Format ist [`Inspection::understood`] falsch — eine leere
/// Fundliste sagt dann **nichts** über die Sauberkeit aus.
///
/// # Fehler
///
/// [`cabrik_core::Error::Malformed`] bei kaputter Struktur eines *erkannten*
/// Formats. Ein unbekanntes Format ist kein Fehler.
pub fn inspect(data: &[u8]) -> Result<Inspection> {
    match Format::detect(data) {
        Some(Format::Png) => png::inspect(data),
        Some(Format::Jpeg) => jpeg::inspect(data),
        Some(Format::Webp) => webp::inspect(data),
        Some(Format::Gif) => gif::inspect(data),
        Some(Format::Bmp) => bmp::inspect(data),
        Some(Format::Tiff) => tiff::inspect(data),
        Some(Format::Bmff) => bmff::inspect(data),
        Some(Format::Svg) => svg::inspect(data),
        Some(Format::Pdf) => pdf::inspect(data),
        Some(Format::Video) => video::inspect(data),
        Some(Format::Matroska) => matroska::inspect(data),
        Some(Format::Avi) => avi::inspect(data),
        Some(Format::Mp3) => mp3::inspect(data),
        Some(Format::Flac) => flac::inspect(data),
        Some(Format::Ogg) => ogg::inspect(data),
        Some(Format::Wav) => wav::inspect(data),
        Some(Format::Ooxml(_)) => ooxml::inspect(data),
        Some(Format::Odf(_)) => odf::inspect(data),
        Some(Format::Zip) => zip_archiv::inspect(data),
        None => Ok(Inspection::not_understood(hinweis(data))),
    }
}

/// Entfernt Metadaten, soweit das Format verstanden wird.
///
/// Gibt die bereinigten Bytes und das Ergebnis zurück. Bei unbekanntem
/// Format bleiben die Bytes **unverändert** und das Ergebnis ist
/// [`StripResult::Unknown`].
///
/// Anders als v1 wird dabei nichts stillschweigend kopiert und keine
/// Sauberkeit behauptet.
///
/// # Fehler
///
/// [`cabrik_core::Error::Malformed`] bei kaputter Struktur eines erkannten
/// Formats.
pub fn strip(data: &[u8]) -> Result<(Vec<u8>, StripResult)> {
    strip_with(data, StripOptions::nur_metadaten())
}

/// Entfernt Metadaten mit ausdrücklichen Optionen.
///
/// Die Voreinstellung von [`strip`] rührt den **Inhalt** nicht an. Erst
/// [`StripOptions`] erlaubt, Kommentare zu entfernen und nachverfolgte
/// Änderungen anzunehmen — beides verändert das Dokument und ist deshalb eine
/// gesonderte Entscheidung des Nutzers (`spec/metadata.md` §4.2.2).
///
/// Für Formate ohne solche Bestandteile ist der Unterschied wirkungslos.
///
/// # Fehler
///
/// [`cabrik_core::Error::Malformed`] bei kaputter Struktur eines erkannten
/// Formats.
pub fn strip_with(data: &[u8], opts: StripOptions) -> Result<(Vec<u8>, StripResult)> {
    match Format::detect(data) {
        Some(Format::Png) => png::strip(data),
        Some(Format::Jpeg) => jpeg::strip(data),
        Some(Format::Webp) => webp::strip(data),
        Some(Format::Gif) => gif::strip(data),
        Some(Format::Bmp) => bmp::strip(data),
        Some(Format::Tiff) => tiff::strip(data),
        Some(Format::Bmff) => bmff::strip(data),
        Some(Format::Svg) => svg::strip(data),
        Some(Format::Pdf) => pdf::strip(data),
        Some(Format::Video) => video::strip(data),
        Some(Format::Matroska) => matroska::strip(data),
        Some(Format::Avi) => avi::strip(data),
        Some(Format::Mp3) => mp3::strip(data),
        Some(Format::Flac) => flac::strip(data),
        Some(Format::Ogg) => ogg::strip(data),
        Some(Format::Wav) => wav::strip(data),
        Some(Format::Ooxml(_)) => ooxml::strip_with(data, opts),
        Some(Format::Odf(_)) => odf::strip_with(data, opts),
        Some(Format::Zip) => zip_archiv::strip_with(data, opts),
        None => Ok((
            data.to_vec(),
            StripResult::Unknown {
                format_hint: hinweis(data),
            },
        )),
    }
}

/// Rät das Format anhand bekannter Kennbytes — nur für die Meldung.
fn hinweis(data: &[u8]) -> Option<String> {
    // Kennungen von Formaten, die dieses Programm **nicht** behandelt — und
    // von behandelten, deren Datei so beschädigt ist, dass die Erkennung sie
    // nicht mehr beansprucht. Beides führt zur selben ehrlichen Auskunft:
    // „so etwas ist das wohl — beurteilen kann ich es nicht".
    const KENNUNGEN: [(&[u8], &str); 9] = [
        (b"%PDF-", "PDF"),
        (b"PK\x03\x04", "ZIP-Container (OOXML, ODF)"),
        (b"GIF87a", "GIF"),
        (b"GIF89a", "GIF"),
        (b"BM", "BMP"),
        (b"FLV\x01", "Flash Video"),
        (b"\x00\x00\x01\xBA", "MPEG-Programmstrom"),
        (
            b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1",
            "Office-Altformat (.doc, .xls, .ppt)",
        ),
        (b"<?xml", "XML (evtl. SVG)"),
    ];
    for (magic, name) in KENNUNGEN {
        if data.starts_with(magic) {
            return Some((*name).to_owned());
        }
    }
    // ISO-BMFF: die Kennung steht ab Offset 4.
    if data.get(4..8) == Some(b"ftyp") {
        return Some("ISO-BMFF (MP4, HEIC, AVIF)".to_owned());
    }
    None
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;

    #[test]
    fn unbekanntes_format_wird_nicht_still_durchkopiert() {
        // Der Kernfehler aus v1: shutil.copy2 ohne Fehlermeldung.
        // Das Office-Altformat ist als Kennung bekannt, als Format aber
        // nicht behandelt.
        let daten = b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1irgendwas".to_vec();
        let (out, ergebnis) = strip(&daten).unwrap();

        assert_eq!(out, daten, "Inhalt bleibt unangetastet");
        assert!(
            !ergebnis.may_show_clean(),
            "fuer ein unverstandenes Format darf keine Sauberkeit behauptet werden"
        );
        match ergebnis {
            StripResult::Unknown { format_hint } => {
                assert_eq!(
                    format_hint.as_deref(),
                    Some("Office-Altformat (.doc, .xls, .ppt)")
                );
            }
            other => panic!("erwartete Unknown, bekam {other:?}"),
        }
    }

    /// Ein **kaputtes** PDF ist etwas anderes als ein unbekanntes Format: Wir
    /// erkennen es und melden einen Fehler, statt zu behaupten, wir kennten
    /// das Format nicht. So halten es alle erkannten Formate.
    #[test]
    fn ein_kaputtes_pdf_ergibt_einen_fehler_kein_unknown() {
        assert_eq!(Format::detect(b"%PDF-1.7\nirgendwas"), Some(Format::Pdf));
        assert!(strip(b"%PDF-1.7\nirgendwas").is_err());
    }

    #[test]
    fn formathinweise_helfen_dem_nutzer() {
        for (daten, erwartet) in [
            (&b"%PDF-1.4"[..], "PDF"),
            (&b"PK\x03\x04..."[..], "ZIP-Container (OOXML, ODF)"),
            (&b"GIF89a"[..], "GIF"),
            (
                &b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1"[..],
                "Office-Altformat (.doc, .xls, .ppt)",
            ),
        ] {
            assert_eq!(hinweis(daten).as_deref(), Some(erwartet));
        }
        assert_eq!(
            hinweis(b"\x00\x00\x00\x18ftypavif").as_deref(),
            Some("ISO-BMFF (MP4, HEIC, AVIF)")
        );
        assert_eq!(hinweis(b"\xDE\xAD\xBE\xEF"), None);
    }

    #[test]
    fn format_wird_an_kennbytes_erkannt_nicht_an_der_endung() {
        assert_eq!(
            Format::detect(&png::SIGNATURE),
            Some(Format::Png),
            "PNG an der Signatur"
        );
        assert_eq!(Format::detect(b"\xFF\xD8\xFF\xE0"), Some(Format::Jpeg));
        assert_eq!(Format::detect(b"beliebiger Text"), None);
    }

    #[test]
    fn inspektion_eines_unbekannten_formats_behauptet_nichts() {
        // Ein Word-Dokument im Altformat: als Kennung bekannt, als Format
        // nicht behandelt (`spec/metadata.md` §9). Eine leere Fundliste sagt
        // hier **nichts** über die Sauberkeit aus.
        let i = inspect(b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1 irgendwas").unwrap();
        assert!(!i.understood);
        assert!(i.findings.is_empty());
        assert_eq!(
            i.format.as_deref(),
            Some("Office-Altformat (.doc, .xls, .ppt)")
        );
    }

    /// Die drei Videobehälter benutzen jeweils den Platzhalter, den ihr
    /// eigenes Format dafür vorsieht: `free`, `Void` und `JUNK`. Dass sie
    /// dabei auseinandergehalten werden, hält dieser Test fest — RIFF trägt
    /// sowohl AVI als auch WAV, und nur das erste wird beansprucht.
    #[test]
    fn die_videobehaelter_werden_auseinandergehalten() {
        let mut mp4 = vec![0, 0, 0, 24];
        mp4.extend_from_slice(b"ftypisomisom");
        mp4.extend_from_slice(&[0; 8]);
        assert_eq!(Format::detect(&mp4), Some(Format::Video));

        assert_eq!(
            Format::detect(b"\x1A\x45\xDF\xA3 irgendwas"),
            Some(Format::Matroska)
        );

        let mut avi = b"RIFF".to_vec();
        avi.extend_from_slice(&64u32.to_le_bytes());
        avi.extend_from_slice(b"AVI LIST");
        assert_eq!(Format::detect(&avi), Some(Format::Avi));

        // RIFF trägt drei Formate, die sich allein in diesen vier Bytes
        // unterscheiden. Alle drei landen in verschiedenen Modulen.
        assert_eq!(
            Format::detect(b"RIFF\x24\x00\x00\x00WAVEfmt "),
            Some(Format::Wav)
        );
        assert_eq!(
            Format::detect(b"RIFF\x24\x00\x00\x00WEBPVP8 "),
            Some(Format::Webp)
        );
    }

    /// Video und HEIC sind **dasselbe Behälterformat**. Nur die Marke in
    /// `ftyp` unterscheidet sie, und die Reihenfolge der Prüfungen in
    /// [`Format::detect`] hängt davon ab. Beide Richtungen werden festgehalten:
    /// Ein Vertauschen ließe eine der beiden Dateiarten im falschen Modul
    /// landen, wo sie nach Boxen sucht, die es dort nicht gibt.
    #[test]
    fn die_marke_entscheidet_zwischen_video_und_bild() {
        let baue = |marke: &[u8; 4]| {
            let mut d = vec![0, 0, 0, 24];
            d.extend_from_slice(b"ftyp");
            d.extend_from_slice(marke);
            d.extend_from_slice(marke);
            d.extend_from_slice(&[0; 8]);
            d
        };

        assert_eq!(Format::detect(&baue(b"isom")), Some(Format::Video));
        assert_eq!(Format::detect(&baue(b"mp42")), Some(Format::Video));
        assert_eq!(Format::detect(&baue(b"qt  ")), Some(Format::Video));
        assert_eq!(Format::detect(&baue(b"heic")), Some(Format::Bmff));
        assert_eq!(Format::detect(&baue(b"avif")), Some(Format::Bmff));
    }
}
