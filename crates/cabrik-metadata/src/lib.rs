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
//! Schritt 2.9a: Fähigkeitsmodell, PNG und JPEG. Alle übrigen Formate melden
//! [`model::StripResult::Unknown`] — **korrektes Verhalten, keine Lücke**.
//! v1 kopierte sie stillschweigend durch und suggerierte damit Sauberkeit.

pub mod container;
pub mod jpeg;
pub mod model;
pub mod ooxml;
pub mod png;
pub mod xml;

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
    /// OOXML: `docx`, `xlsx`, `pptx`.
    Ooxml(ooxml::Art),
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
        if container::sieht_aus_wie_zip(data) {
            // Ein ZIP allein sagt noch nichts. Erst der Inhalt entscheidet,
            // ob es ein Office-Dokument ist.
            let eintraege = container::lies(data).ok()?;
            return ooxml::erkenne(&eintraege).map(Self::Ooxml);
        }
        None
    }

    /// Name für die Anzeige.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Ooxml(a) => a.name(),
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
        Some(Format::Ooxml(_)) => ooxml::inspect(data),
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
        Some(Format::Ooxml(_)) => ooxml::strip_with(data, opts),
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
    const KENNUNGEN: [(&[u8], &str); 8] = [
        (b"%PDF-", "PDF"),
        (b"PK\x03\x04", "ZIP-Container (OOXML, ODF)"),
        (b"GIF87a", "GIF"),
        (b"GIF89a", "GIF"),
        (b"BM", "BMP"),
        (b"\x1A\x45\xDF\xA3", "Matroska"),
        (b"ID3", "MP3"),
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
        let daten = b"%PDF-1.7\nirgendwas".to_vec();
        let (out, ergebnis) = strip(&daten).unwrap();

        assert_eq!(out, daten, "Inhalt bleibt unangetastet");
        assert!(
            !ergebnis.may_show_clean(),
            "fuer ein unverstandenes Format darf keine Sauberkeit behauptet werden"
        );
        match ergebnis {
            StripResult::Unknown { format_hint } => {
                assert_eq!(format_hint.as_deref(), Some("PDF"));
            }
            other => panic!("erwartete Unknown, bekam {other:?}"),
        }
    }

    #[test]
    fn formathinweise_helfen_dem_nutzer() {
        for (daten, erwartet) in [
            (&b"%PDF-1.4"[..], "PDF"),
            (&b"PK\x03\x04..."[..], "ZIP-Container (OOXML, ODF)"),
            (&b"GIF89a"[..], "GIF"),
            (&b"ID3\x03"[..], "MP3"),
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
        let i = inspect(b"\x00\x00\x00\x18ftypheic").unwrap();
        assert!(!i.understood);
        assert!(i.findings.is_empty());
        assert_eq!(i.format.as_deref(), Some("ISO-BMFF (MP4, HEIC, AVIF)"));
    }
}
