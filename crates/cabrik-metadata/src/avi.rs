//! AVI (`spec/metadata.md` §4).
//!
//! Der dritte Videobehälter, und der einfachste. RIFF besteht aus Blöcken mit
//! Namen und Länge, und wie ISO-BMFF mit `free` und EBML mit `Void` hat es
//! einen eigenen Platzhalter: den **`JUNK`-Block**. Jeder Leser überspringt
//! ihn. Dass das der vorgesehene Weg ist und kein Kunstgriff, zeigt schon
//! eine gewöhnliche von ffmpeg erzeugte Datei — sie enthält von sich aus
//! zwei `JUNK`-Blöcke als Ausrichtungsfüllung.
//!
//! # Warum auch hier nichts verschoben wird
//!
//! Der `idx1`-Block am Dateiende ist ein Verzeichnis aller Bilder mit ihren
//! Versätzen. Bei AVI 2.0 kommt `indx` mit absoluten Positionen hinzu. Ein
//! Block, der nach vorn rückt, macht jeden Eintrag falsch.
//!
//! # Was drinsteht
//!
//! - **`LIST INFO`** — die eigentlichen Metadaten: `IART` Verfasser, `ICMT`
//!   Kommentar, `INAM` Titel, `ISFT` erzeugendes Programm, `ICRD`
//!   Erstellungsdatum. Der ganze Block wird zu `JUNK`.
//! - **`strn`** — der Name der Spur, den ein Schnittprogramm vergibt.
//! - **`IDIT`** — der Zeitpunkt der Digitalisierung, oft außerhalb von `INFO`.

use crate::model::{Finding, FindingKind, Inspection, Severity, StripResult};

use cabrik_core::{Error, Result};

/// Höchstzahl der Blöcke, die verfolgt werden.
const MAX_BLOECKE: usize = 100_000;
/// Höchste Schachtelungstiefe.
const MAX_TIEFE: usize = 8;

/// Ob die Bytes wie ein AVI aussehen.
#[must_use]
pub fn looks_like_avi(daten: &[u8]) -> bool {
    daten.starts_with(b"RIFF") && daten.get(8..12) == Some(b"AVI ")
}

/// Ein gefundener Block.
#[derive(Debug, Clone, Copy)]
struct Block {
    typ: [u8; 4],
    /// Bei `RIFF` und `LIST`: die Art der Liste.
    art: Option<[u8; 4]>,
    anfang: usize,
    inhalt: usize,
    ende: usize,
}

fn vier(daten: &[u8], p: usize) -> Option<[u8; 4]> {
    daten.get(p..p.checked_add(4)?)?.try_into().ok()
}

fn u32_le(daten: &[u8], p: usize) -> Option<u32> {
    Some(u32::from_le_bytes(vier(daten, p)?))
}

fn sammle(daten: &[u8]) -> Result<Vec<Block>> {
    let mut aus = Vec::new();
    lauf(daten, 0, daten.len(), 0, &mut aus)?;
    Ok(aus)
}

fn lauf(daten: &[u8], von: usize, bis: usize, tiefe: usize, aus: &mut Vec<Block>) -> Result<()> {
    if tiefe > MAX_TIEFE {
        return Ok(());
    }
    let mut p = von;

    while p.checked_add(8).is_some_and(|e| e <= bis) {
        if aus.len() >= MAX_BLOECKE {
            return Err(Error::Malformed("avi: zu viele Bloecke"));
        }
        let typ = vier(daten, p).ok_or(Error::Malformed("avi: Blocktyp unlesbar"))?;
        let groesse = u32_le(daten, p.saturating_add(4))
            .and_then(|g| usize::try_from(g).ok())
            .ok_or(Error::Malformed("avi: Blockgroesse unlesbar"))?;

        let inhalt = p.saturating_add(8);
        let ende = inhalt
            .checked_add(groesse)
            .ok_or(Error::Malformed("avi: Blockende ueberlaeuft"))?;
        if ende > bis {
            return Err(Error::Malformed("avi: Block reicht ueber seinen Bereich"));
        }

        let ist_liste = typ == *b"LIST" || typ == *b"RIFF";
        let art = if ist_liste { vier(daten, inhalt) } else { None };

        aus.push(Block {
            typ,
            art,
            anfang: p,
            inhalt,
            ende,
        });

        // In `movi` liegen die Bilder. Dort wird nicht gesucht.
        if ist_liste && art != Some(*b"movi") {
            lauf(
                daten,
                inhalt.saturating_add(4),
                ende,
                tiefe.saturating_add(1),
                aus,
            )?;
        }

        // RIFF richtet jeden Block auf gerade Adressen aus.
        let weiter = ende.saturating_add(groesse & 1);
        if weiter <= p {
            return Err(Error::Malformed("avi: Block ohne Fortschritt"));
        }
        p = weiter;
    }
    Ok(())
}

fn text(daten: &[u8], b: &Block) -> String {
    let roh = daten.get(b.inhalt..b.ende).unwrap_or(&[]);
    let ohne_null = roh.split(|x| *x == 0).next().unwrap_or(&[]);
    String::from_utf8_lossy(ohne_null).trim().to_owned()
}

/// Ordnet einen `INFO`-Block ein.
///
/// Die Liste stammt aus der RIFF-Spezifikation. Alles Unbekannte, das mit `I`
/// beginnt, gilt als Kommentar — lieber einmal zu viel gemeldet.
fn info_einordnung(typ: &[u8; 4]) -> Option<(&'static str, FindingKind, Severity)> {
    Some(match typ {
        b"IART" => ("Verfasser", FindingKind::Author, Severity::Critical),
        b"IENG" => ("Techniker", FindingKind::Author, Severity::Critical),
        b"ITCH" => ("Bearbeiter", FindingKind::Author, Severity::Critical),
        b"ICMT" => ("Kommentar", FindingKind::Comment, Severity::Critical),
        b"ISBJ" => ("Thema", FindingKind::Comment, Severity::Notable),
        b"INAM" => ("Titel", FindingKind::Comment, Severity::Notable),
        b"IKEY" => ("Schlagwörter", FindingKind::Comment, Severity::Notable),
        b"IPRD" => ("Produkt", FindingKind::Comment, Severity::Notable),
        b"IGNR" => ("Gattung", FindingKind::Comment, Severity::Notable),
        b"ISFT" => (
            "erzeugendes Programm",
            FindingKind::Software,
            Severity::Notable,
        ),
        b"ICRD" | b"IDIT" => ("Zeitpunkt", FindingKind::Timestamp, Severity::Notable),
        b"ICOP" => ("Urheberrecht", FindingKind::Organization, Severity::Notable),
        b"ICMS" => (
            "Auftraggeber",
            FindingKind::Organization,
            Severity::Critical,
        ),
        b"ISRC" => ("Quelle", FindingKind::Organization, Severity::Notable),
        b"IMED" | b"ISRF" => ("Aufnahmemittel", FindingKind::Device, Severity::Notable),
        andere if andere.starts_with(b"I") => ("Angabe", FindingKind::Comment, Severity::Notable),
        _ => return None,
    })
}

fn funde(daten: &[u8], bloecke: &[Block]) -> Vec<Finding> {
    let mut aus = Vec::new();

    for b in bloecke {
        if b.typ == *b"strn" {
            aus.push(Finding {
                kind: FindingKind::Comment,
                location: "AVI:strn".to_owned(),
                value: Some(format!("Spurname „{}“", text(daten, b))),
                severity: Severity::Notable,
            });
            continue;
        }
        let Some((name, art, schwere)) = info_einordnung(&b.typ) else {
            continue;
        };
        let kennung = String::from_utf8_lossy(&b.typ).into_owned();
        aus.push(Finding {
            kind: art,
            location: format!("AVI:INFO/{kennung}"),
            value: Some(format!("{name}: {}", text(daten, b))),
            severity: schwere,
        });
    }
    aus
}

/// Zeigt die Metadaten an, ohne die Datei zu verändern.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputtem RIFF-Aufbau.
pub fn inspect(daten: &[u8]) -> Result<Inspection> {
    let bloecke = sammle(daten)?;
    Ok(Inspection {
        format: Some("AVI".to_owned()),
        findings: funde(daten, &bloecke),
        understood: true,
    })
}

/// Macht aus einem Block einen `JUNK`-Block gleicher Größe.
///
/// Der Kopf bleibt stehen — nur der Name wird zu `JUNK` und der Inhalt
/// genullt. Die Längenangabe stimmt weiterhin, also verschiebt sich nichts.
fn zu_junk(aus: &mut [u8], b: &Block) {
    if let Some(name) = aus.get_mut(b.anfang..b.anfang.saturating_add(4)) {
        name.copy_from_slice(b"JUNK");
    }
    if let Some(inhalt) = aus.get_mut(b.inhalt..b.ende) {
        inhalt.fill(0);
    }
}

/// Entfernt die Metadaten.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputtem RIFF-Aufbau.
pub fn strip(daten: &[u8]) -> Result<(Vec<u8>, StripResult)> {
    let bloecke = sammle(daten)?;
    let entfernt = funde(daten, &bloecke);

    let mut aus = daten.to_vec();

    // Ganze INFO-Listen zuerst: Damit sind ihre Unterblöcke miterledigt.
    let mut erledigt: Vec<(usize, usize)> = Vec::new();
    for b in &bloecke {
        if b.typ == *b"LIST" && b.art == Some(*b"INFO") {
            zu_junk(&mut aus, b);
            erledigt.push((b.anfang, b.ende));
        }
    }

    // Was außerhalb einer INFO-Liste steht — `IDIT` etwa steht oft für sich
    // allein, und `strn` gehört zur Spurbeschreibung.
    for b in &bloecke {
        if b.typ == *b"LIST" || b.typ == *b"RIFF" {
            continue;
        }
        let drin = erledigt.iter().any(|(a, e)| b.anfang >= *a && b.ende <= *e);
        if drin {
            continue;
        }
        if b.typ == *b"strn" || info_einordnung(&b.typ).is_some() {
            zu_junk(&mut aus, b);
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
        // WAV ist auch RIFF und darf nicht beansprucht werden.
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
