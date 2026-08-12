//! AVI (`spec/metadata.md` §4).
//!
//! Der dritte Videobehälter, und der einfachste. Er baut auf [`crate::riff`]
//! auf — demselben Läufer, den auch WAV benutzt, denn beide sind dasselbe
//! Blockformat mit unterschiedlicher Füllung.
//!
//! # Warum auch hier nichts verschoben wird
//!
//! Der `idx1`-Block am Dateiende ist ein Verzeichnis aller Bilder mit ihren
//! Versätzen. Bei AVI 2.0 kommt `indx` mit absoluten Positionen hinzu. Ein
//! Block, der nach vorn rückt, macht jeden Eintrag falsch.
//!
//! RIFF sieht dafür den **`JUNK`-Block** vor, den jeder Leser überspringt.
//! Dass das der vorgesehene Weg ist und kein Kunstgriff, zeigt schon eine
//! gewöhnliche von ffmpeg erzeugte Datei — sie enthält von sich aus zwei
//! `JUNK`-Blöcke als Ausrichtungsfüllung.
//!
//! # Was drinsteht
//!
//! - **`LIST INFO`** — die eigentlichen Metadaten: `IART` Verfasser, `ICMT`
//!   Kommentar, `INAM` Titel, `ISFT` erzeugendes Programm, `ICRD`
//!   Erstellungsdatum. Der ganze Block wird zu `JUNK`.
//! - **`strn`** — der Name der Spur, den ein Schnittprogramm vergibt.
//! - **`IDIT`** — der Zeitpunkt der Digitalisierung, oft außerhalb von `INFO`.

use crate::model::{Finding, FindingKind, Inspection, Severity, StripResult};
use crate::riff::{self, Block};

use cabrik_core::Result;

/// In `movi` liegen die Bilder. Dort wird nicht gesucht.
const AUSGESPART: [&[u8; 4]; 1] = [b"movi"];

/// Ob die Bytes wie ein AVI aussehen.
#[must_use]
pub fn looks_like_avi(daten: &[u8]) -> bool {
    riff::ist_riff(daten, b"AVI ")
}

fn funde(daten: &[u8], bloecke: &[Block]) -> Vec<Finding> {
    let mut aus = Vec::new();

    for b in bloecke {
        if b.typ == *b"strn" {
            aus.push(Finding {
                kind: FindingKind::Comment,
                location: "AVI:strn".to_owned(),
                value: Some(format!("Spurname „{}“", riff::text(daten, b))),
                severity: Severity::Notable,
            });
            continue;
        }
        let Some((name, art, schwere)) = riff::info_einordnung(&b.typ) else {
            continue;
        };
        let kennung = String::from_utf8_lossy(&b.typ).into_owned();
        aus.push(Finding {
            kind: art,
            location: format!("AVI:INFO/{kennung}"),
            value: Some(format!("{name}: {}", riff::text(daten, b))),
            severity: schwere,
        });
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
        format: Some("AVI".to_owned()),
        findings: funde(daten, &bloecke),
        understood: true,
    })
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

    // Ganze INFO-Listen zuerst: Damit sind ihre Unterblöcke miterledigt.
    let mut erledigt: Vec<(usize, usize)> = Vec::new();
    for b in &bloecke {
        if b.typ == *b"LIST" && b.art == Some(*b"INFO") {
            riff::zu_junk(&mut aus, b);
            erledigt.push((b.anfang, b.ende));
        }
    }

    // Was außerhalb einer INFO-Liste steht — `IDIT` etwa steht oft für sich
    // allein, und `strn` gehört zur Spurbeschreibung.
    for b in &bloecke {
        if b.ist_liste() {
            continue;
        }
        if erledigt.iter().any(|(a, e)| b.anfang >= *a && b.ende <= *e) {
            continue;
        }
        if b.typ == *b"strn" || riff::info_einordnung(&b.typ).is_some() {
            riff::zu_junk(&mut aus, b);
        }
    }

    debug_assert_eq!(
        aus.len(),
        daten.len(),
        "in AVI darf sich nichts verschieben"
    );

    Ok((aus, StripResult::Complete { removed: entfernt }))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
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

    fn beispiel() -> Vec<u8> {
        let strl = liste(b"strl", &block(b"strn", b"Kameraspur A\0"));
        let hdrl = liste(b"hdrl", &[block(b"avih", &[0u8; 56]), strl].concat());
        let info = liste(
            b"INFO",
            &[
                block(b"IART", b"Dr. Anna Beispiel\0"),
                block(b"ICMT", b"Nicht an den Kunden geben\0"),
                block(b"ISFT", b"Bearbeitungsprogramm 3.1\0"),
            ]
            .concat(),
        );
        let idit = block(b"IDIT", b"Mon Mar 01 09:12:00 2026\0");
        let movi = liste(b"movi", &block(b"00dc", &[0x42u8; 64]));

        let inneres = [b"AVI ".to_vec(), hdrl, info, idit, movi].concat();
        block(b"RIFF", &inneres)
    }

    #[test]
    fn avi_wird_erkannt() {
        assert!(looks_like_avi(&beispiel()));
        // WAV ist auch RIFF und darf hier nicht beansprucht werden.
        assert!(!looks_like_avi(b"RIFF\x24\x00\x00\x00WAVEfmt "));
        assert!(!looks_like_avi(b"RIFF"));
    }

    #[test]
    fn die_info_liste_wird_gelesen() {
        let i = inspect(&beispiel()).unwrap();
        assert!(i.understood);
        assert_eq!(i.format.as_deref(), Some("AVI"));

        let verfasser = i
            .findings
            .iter()
            .find(|f| f.location == "AVI:INFO/IART")
            .expect("IART fehlt");
        assert_eq!(verfasser.kind, FindingKind::Author);
        assert_eq!(verfasser.severity, Severity::Critical);
        assert!(verfasser.value.as_deref().unwrap().contains("Dr. Anna"));

        assert!(i.findings.iter().any(|f| f.location == "AVI:strn"));
        assert!(i.findings.iter().any(|f| f.location == "AVI:INFO/IDIT"));
    }

    /// `idx1` führt Versätze. Verschiebt sich ein Block, ist der Index falsch.
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
            b"Nicht an den Kunden",
            b"Bearbeitungsprogramm",
            b"Kameraspur",
            b"Mon Mar 01",
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

    /// Die Bilddaten in `movi` bleiben Byte für Byte gleich.
    #[test]
    fn die_bilddaten_bleiben_unberuehrt() {
        let vorher = beispiel();
        let (nachher, _) = strip(&vorher).unwrap();
        let movi = vorher
            .windows(4)
            .position(|f| f == b"movi")
            .expect("kein movi");
        assert_eq!(nachher.get(movi..), vorher.get(movi..));
    }

    #[test]
    fn ein_zweiter_durchlauf_aendert_nichts() {
        let (einmal, _) = strip(&beispiel()).unwrap();
        let (zweimal, _) = strip(&einmal).unwrap();
        assert_eq!(einmal, zweimal);
    }

    #[test]
    fn ein_block_ueber_seinen_bereich_hinaus_ist_ein_fehler() {
        let mut kaputt = beispiel();
        // Die Länge des ersten Unterblocks weit über das Dateiende setzen.
        kaputt[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(inspect(&kaputt).is_err());
    }
}
