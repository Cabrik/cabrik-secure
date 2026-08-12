//! WAV (`spec/metadata.md` §4).
//!
//! Derselbe Aufbau wie AVI, deshalb derselbe Läufer aus [`crate::riff`] und
//! derselbe Platzhalter: der **`JUNK`-Block**.
//!
//! # Der Block, um den es wirklich geht
//!
//! Eine WAV-Datei gilt als „nackte" Tondatei ohne Metadaten. Das stimmt für
//! die Datei, die ein Schnittprogramm ausgibt — und **nicht** für die, die
//! aus einem Aufnahmegerät kommt.
//!
//! Feldrekorder schreiben einen **`bext`-Block** hinein, die *Broadcast Wave
//! Extension*. Darin stehen:
//!
//! - **`Originator`** — Gerät oder Person, oft der Name des Aufnehmenden
//! - **`OriginatorReference`** — eine Kennung, die das einzelne Gerät benennt
//! - **`OriginationDate`** und **`OriginationTime`** — Datum und **Uhrzeit
//!   der Aufnahme**, auf die Sekunde
//! - **`Description`** — was der Aufnehmende ins Feld getippt hat
//! - **`CodingHistory`** — die Kette aller Bearbeitungsschritte
//! - **`UMID`** — eine weltweit eindeutige Materialkennung
//!
//! Für ein Interview, das anonym bleiben soll, ist das der schwerwiegendste
//! Fund des ganzen Formats. Ein Werkzeug, das WAV für metadatenfrei hält,
//! übersieht ihn vollständig.
//!
//! Daneben: `LIST INFO` wie bei AVI, ein eingebetteter `id3 `-Block, `iXML`
//! und `_PMX` (XMP) aus der Rundfunktechnik.

use crate::model::{Finding, FindingKind, Inspection, Severity, StripResult};
use crate::riff::{self, Block};
use crate::{mp3, xml};

use cabrik_core::Result;

/// In `data` liegen die Abtastwerte — dort wird nichts gesucht.
const AUSGESPART: [&[u8; 4]; 1] = [b"data"];

/// Ob die Bytes wie eine WAV-Datei aussehen.
#[must_use]
pub fn looks_like_wav(daten: &[u8]) -> bool {
    riff::ist_riff(daten, b"WAVE")
}

/// Die Felder des `bext`-Blocks: Name, Versatz, Länge, Einordnung.
///
/// Die Längen stehen so in EBU Tech 3285 und sind **feste Breiten**, nicht
/// nullterminiert — ein Feld endet, wo das nächste beginnt.
const BEXT_FELDER: [(&str, usize, usize, FindingKind, Severity); 6] = [
    (
        "Beschreibung",
        0,
        256,
        FindingKind::Comment,
        Severity::Critical,
    ),
    (
        "Aufnehmender",
        256,
        32,
        FindingKind::Author,
        Severity::Critical,
    ),
    (
        "Gerätekennung",
        288,
        32,
        FindingKind::Device,
        Severity::Critical,
    ),
    (
        "Aufnahmedatum",
        320,
        10,
        FindingKind::Timestamp,
        Severity::Notable,
    ),
    (
        "Aufnahmezeit",
        330,
        8,
        FindingKind::Timestamp,
        Severity::Notable,
    ),
    // Nach UMID und den Pegelangaben folgt die Bearbeitungskette bis zum
    // Blockende.
    (
        "Bearbeitungskette",
        602,
        usize::MAX,
        FindingKind::Software,
        Severity::Notable,
    ),
];

fn bext_funde(daten: &[u8], b: &Block) -> Vec<Finding> {
    let mut aus = Vec::new();

    for (name, versatz, laenge, art, schwere) in BEXT_FELDER {
        let von = b.inhalt.saturating_add(versatz);
        let bis = if laenge == usize::MAX {
            b.ende
        } else {
            von.saturating_add(laenge).min(b.ende)
        };
        if von >= bis {
            continue;
        }
        let wert = riff::teiltext(daten, von, bis);
        if wert.is_empty() {
            continue;
        }
        aus.push(Finding {
            kind: art,
            location: format!("WAV:bext/{name}"),
            value: Some(wert),
            severity: schwere,
        });
    }

    // Die UMID ist eine weltweit eindeutige Kennung des Materials. Sie ist
    // binär, deshalb wird nur ihr Vorhandensein gemeldet.
    let umid_von = b.inhalt.saturating_add(338);
    let umid_bis = umid_von.saturating_add(64).min(b.ende);
    if umid_von < umid_bis
        && daten
            .get(umid_von..umid_bis)
            .is_some_and(|s| s.iter().any(|x| *x != 0))
    {
        aus.push(Finding {
            kind: FindingKind::Device,
            location: "WAV:bext/UMID".to_owned(),
            value: Some("weltweit eindeutige Materialkennung".to_owned()),
            severity: Severity::Critical,
        });
    }
    aus
}

fn funde(daten: &[u8], bloecke: &[Block]) -> Vec<Finding> {
    let mut aus = Vec::new();

    for b in bloecke {
        match &b.typ {
            b"bext" => aus.extend(bext_funde(daten, b)),
            b"id3 " | b"ID3 " => {
                // Ein ID3-Tag mitten in einer WAV-Datei. Der Blockinhalt ist
                // genau das, was auch am Anfang eines MP3 stünde.
                let roh = daten.get(b.inhalt..b.ende).unwrap_or(&[]);
                let laenge = mp3::id3v2_laenge(roh).unwrap_or(roh.len());
                aus.push(Finding {
                    kind: FindingKind::UnknownExtension,
                    location: "WAV:id3".to_owned(),
                    value: Some(format!("ein ID3-Tag in der WAV-Datei ({laenge} Bytes)")),
                    severity: Severity::Notable,
                });
                aus.extend(mp3::id3v2_funde(roh, laenge));
            }
            b"iXML" | b"_PMX" | b"aXML" => {
                let roh = String::from_utf8_lossy(daten.get(b.inhalt..b.ende).unwrap_or(&[]));
                let kennung = String::from_utf8_lossy(&b.typ).trim().to_owned();
                if xml::hat_textinhalt(&roh) {
                    aus.push(Finding {
                        kind: FindingKind::Comment,
                        location: format!("WAV:{kennung}"),
                        value: Some(format!(
                            "XML-Beschreibung aus der Rundfunktechnik ({} Bytes)",
                            b.inhalt_laenge()
                        )),
                        severity: Severity::Critical,
                    });
                }
            }
            b"strn" => {}
            _ => {
                let Some((name, art, schwere)) = riff::info_einordnung(&b.typ) else {
                    continue;
                };
                let kennung = String::from_utf8_lossy(&b.typ).into_owned();
                aus.push(Finding {
                    kind: art,
                    location: format!("WAV:INFO/{kennung}"),
                    value: Some(format!("{name}: {}", riff::text(daten, b))),
                    severity: schwere,
                });
            }
        }
    }
    aus
}

/// Zeigt die Metadaten an, ohne die Datei zu verändern.
///
/// # Fehler
///
/// [`cabrik_core::Error::Malformed`] bei kaputtem RIFF-Aufbau.
pub fn inspect(daten: &[u8]) -> Result<Inspection> {
    let bloecke = riff::sammle(daten, &AUSGESPART)?;
    Ok(Inspection {
        format: Some("WAV".to_owned()),
        findings: funde(daten, &bloecke),
        understood: true,
    })
}

/// Ob dieser Block zu entfernen ist.
fn ist_marke(typ: &[u8; 4]) -> bool {
    matches!(
        typ,
        b"bext" | b"id3 " | b"ID3 " | b"iXML" | b"_PMX" | b"aXML" | b"cset"
    ) || riff::info_einordnung(typ).is_some()
}

/// Entfernt die Metadaten.
///
/// # Fehler
///
/// [`cabrik_core::Error::Malformed`] bei kaputtem RIFF-Aufbau.
pub fn strip(daten: &[u8]) -> Result<(Vec<u8>, StripResult)> {
    let bloecke = riff::sammle(daten, &AUSGESPART)?;
    let entfernt = funde(daten, &bloecke);

    let mut aus = daten.to_vec();

    let mut erledigt: Vec<(usize, usize)> = Vec::new();
    for b in &bloecke {
        if b.typ == *b"LIST" && b.art == Some(*b"INFO") {
            riff::zu_junk(&mut aus, b);
            erledigt.push((b.anfang, b.ende));
        }
    }

    for b in &bloecke {
        if b.ist_liste() {
            continue;
        }
        if erledigt.iter().any(|(a, e)| b.anfang >= *a && b.ende <= *e) {
            continue;
        }
        if ist_marke(&b.typ) {
            riff::zu_junk(&mut aus, b);
        }
    }

    debug_assert_eq!(
        aus.len(),
        daten.len(),
        "in WAV darf sich nichts verschieben"
    );

    Ok((aus, StripResult::Complete { removed: entfernt }))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "Tests duerfen laut werden"
)]
mod tests {
    use super::*;

    fn block(typ: &[u8; 4], inhalt: &[u8]) -> Vec<u8> {
        let mut v = typ.to_vec();
        v.extend_from_slice(&u32::try_from(inhalt.len()).unwrap().to_le_bytes());
        v.extend_from_slice(inhalt);
        if inhalt.len() % 2 == 1 {
            v.push(0);
        }
        v
    }

    fn liste(art: &[u8; 4], inhalt: &[u8]) -> Vec<u8> {
        let mut v = art.to_vec();
        v.extend_from_slice(inhalt);
        block(b"LIST", &v)
    }

    /// Ein `bext`-Block, wie ihn ein Feldrekorder schreibt.
    fn bext() -> Vec<u8> {
        let mut v = vec![0u8; 602];
        let setze = |v: &mut Vec<u8>, versatz: usize, text: &str| {
            let b = text.as_bytes();
            v[versatz..versatz + b.len()].copy_from_slice(b);
        };
        setze(&mut v, 0, "Interview Hinterhof, Quelle will anonym bleiben");
        setze(&mut v, 256, "Dr. Anna Beispiel");
        setze(&mut v, 288, "ZOOM-F8N-00473829");
        setze(&mut v, 320, "2026-03-01");
        setze(&mut v, 330, "09:12:00");
        // UMID
        v[338..402].copy_from_slice(&[0x5Au8; 64]);
        v.extend_from_slice(b"A=PCM,F=48000,W=24,M=stereo,T=ZOOM F8n\0");
        block(b"bext", &v)
    }

    fn beispiel() -> Vec<u8> {
        let info = liste(
            b"INFO",
            &[
                block(b"IART", b"Dr. Anna Beispiel\0"),
                block(b"ICMT", b"Nicht an den Kunden geben\0"),
                block(b"ISFT", b"Bearbeitungsprogramm 3.1\0"),
            ]
            .concat(),
        );
        let inneres = [
            b"WAVE".to_vec(),
            block(b"fmt ", &[0u8; 16]),
            bext(),
            info,
            block(b"data", &[0x42u8; 256]),
        ]
        .concat();
        block(b"RIFF", &inneres)
    }

    #[test]
    fn wav_wird_erkannt() {
        assert!(looks_like_wav(&beispiel()));
        // AVI ist auch RIFF und darf hier nicht beansprucht werden.
        assert!(!looks_like_wav(b"RIFF\x24\x00\x00\x00AVI LIST"));
        assert!(!looks_like_wav(b"RIFF"));
    }

    /// **Der Fund, um den es geht.** Eine WAV aus einem Feldrekorder ist
    /// alles andere als nackt.
    #[test]
    fn der_bext_block_verraet_aufnehmenden_geraet_und_uhrzeit() {
        let i = inspect(&beispiel()).unwrap();
        assert!(i.understood);
        assert_eq!(i.format.as_deref(), Some("WAV"));

        let hole = |ort: &str| {
            i.findings
                .iter()
                .find(|f| f.location == ort)
                .unwrap_or_else(|| panic!("{ort} fehlt: {:?}", i.findings))
                .clone()
        };

        let a = hole("WAV:bext/Aufnehmender");
        assert_eq!(a.kind, FindingKind::Author);
        assert_eq!(a.severity, Severity::Critical);
        assert_eq!(a.value.as_deref(), Some("Dr. Anna Beispiel"));

        assert_eq!(
            hole("WAV:bext/Gerätekennung").value.as_deref(),
            Some("ZOOM-F8N-00473829")
        );
        assert_eq!(
            hole("WAV:bext/Aufnahmedatum").value.as_deref(),
            Some("2026-03-01")
        );
        assert_eq!(
            hole("WAV:bext/Aufnahmezeit").value.as_deref(),
            Some("09:12:00")
        );
        assert!(
            hole("WAV:bext/Beschreibung")
                .value
                .unwrap()
                .contains("anonym bleiben")
        );
        assert!(
            hole("WAV:bext/Bearbeitungskette")
                .value
                .unwrap()
                .contains("ZOOM F8n")
        );
        assert_eq!(hole("WAV:bext/UMID").severity, Severity::Critical);
    }

    #[test]
    fn die_dateilaenge_bleibt_unveraendert() {
        let vorher = beispiel();
        let (nachher, _) = strip(&vorher).unwrap();
        assert_eq!(nachher.len(), vorher.len());
    }

    #[test]
    fn nach_dem_bereinigen_ist_nichts_mehr_lesbar() {
        let (sauber, ergebnis) = strip(&beispiel()).unwrap();

        for spur in [
            &b"Dr. Anna Beispiel"[..],
            b"ZOOM-F8N-00473829",
            b"anonym bleiben",
            b"2026-03-01",
            b"Nicht an den Kunden",
            b"Bearbeitungsprogramm",
        ] {
            assert!(
                !sauber.windows(spur.len()).any(|f| f == spur),
                "noch lesbar: {}",
                String::from_utf8_lossy(spur)
            );
        }
        assert!(matches!(ergebnis, StripResult::Complete { .. }));
        assert!(inspect(&sauber).unwrap().findings.is_empty());
    }

    /// Die Abtastwerte bleiben Byte für Byte gleich.
    #[test]
    fn die_tondaten_bleiben_unberuehrt() {
        let vorher = beispiel();
        let (nachher, _) = strip(&vorher).unwrap();
        let data = vorher
            .windows(4)
            .position(|f| f == b"data")
            .expect("kein data");
        assert_eq!(nachher.get(data..), vorher.get(data..));
    }

    /// Ein ID3-Tag mitten in einer WAV-Datei — es kommt vor, und ein reiner
    /// RIFF-Reiniger übersähe seinen Inhalt.
    #[test]
    fn ein_id3_block_wird_gelesen_und_entfernt() {
        let rahmen = {
            let inhalt = b"\x00Dr. Anna Beispiel";
            let mut r = b"TPE1".to_vec();
            r.extend_from_slice(&u32::try_from(inhalt.len()).unwrap().to_be_bytes());
            r.extend_from_slice(&[0, 0]);
            r.extend_from_slice(inhalt);
            r
        };
        let mut tag = b"ID3".to_vec();
        tag.extend_from_slice(&[3, 0, 0]);
        let g = rahmen.len();
        tag.extend_from_slice(&[0, 0, ((g >> 7) & 0x7F) as u8, (g & 0x7F) as u8]);
        tag.extend_from_slice(&rahmen);

        let inneres = [
            b"WAVE".to_vec(),
            block(b"fmt ", &[0u8; 16]),
            block(b"id3 ", &tag),
            block(b"data", &[0x42u8; 64]),
        ]
        .concat();
        let datei = block(b"RIFF", &inneres);

        let i = inspect(&datei).unwrap();
        assert!(i.findings.iter().any(|f| f.location == "WAV:id3"));
        assert!(
            i.findings.iter().any(|f| f.location == "MP3:ID3v2/TPE1"),
            "der Inhalt des ID3-Tags wurde nicht gelesen: {:?}",
            i.findings
        );

        let (sauber, _) = strip(&datei).unwrap();
        assert_eq!(sauber.len(), datei.len());
        assert!(!sauber.windows(8).any(|f| f == b"Dr. Anna"));
    }

    #[test]
    fn ein_zweiter_durchlauf_aendert_nichts() {
        let (einmal, _) = strip(&beispiel()).unwrap();
        let (zweimal, _) = strip(&einmal).unwrap();
        assert_eq!(einmal, zweimal);
    }

    /// Ein `bext`-Block ohne Bearbeitungskette ist kürzer als 602 Bytes.
    /// Er darf nicht dazu führen, dass über das Blockende hinaus gelesen wird.
    #[test]
    fn ein_verkuerzter_bext_block_wird_vertragen() {
        let mut kurz = vec![0u8; 340];
        kurz[256..273].copy_from_slice(b"Dr. Anna Beispiel");
        let inneres = [
            b"WAVE".to_vec(),
            block(b"bext", &kurz),
            block(b"data", &[0u8; 16]),
        ]
        .concat();
        let datei = block(b"RIFF", &inneres);

        let i = inspect(&datei).unwrap();
        assert_eq!(
            i.findings
                .iter()
                .find(|f| f.location == "WAV:bext/Aufnehmender")
                .and_then(|f| f.value.as_deref()),
            Some("Dr. Anna Beispiel")
        );
        assert!(
            !i.findings
                .iter()
                .any(|f| f.location == "WAV:bext/Bearbeitungskette"),
            "es wurde ueber das Blockende hinaus gelesen"
        );

        let (sauber, _) = strip(&datei).unwrap();
        assert_eq!(sauber.len(), datei.len());
        assert!(!sauber.windows(8).any(|f| f == b"Dr. Anna"));
    }
}
