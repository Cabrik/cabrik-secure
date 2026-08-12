//! FLAC (`spec/metadata.md` §4).
//!
//! Das dritte Format, das seinen Platzhalter mitbringt — und das einzige, bei
//! dem er nicht nachträglich zweckentfremdet, sondern **wörtlich so genannt**
//! wird: `PADDING`. Die Norm sieht ihn ausdrücklich dafür vor, dass Marken
//! später wachsen dürfen, ohne die Datei neu zu schreiben. Genau das nutzen
//! wir in der Gegenrichtung.
//!
//! Eine gewöhnliche FLAC-Datei enthält ihn von sich aus: Die Vorlage dieses
//! Projekts hat hinter dem Kommentarblock **8192 Bytes `PADDING`**, von ffmpeg
//! angelegt. Einen Block in `PADDING` zu verwandeln fällt damit nicht einmal
//! auf.
//!
//! # Was bleibt und warum
//!
//! - **`STREAMINFO`** ist Pflicht und enthält unter anderem eine MD5-Summe
//!   der Tonspur. Das ist ein Fingerabdruck des **Inhalts**, nicht der
//!   Person, und ohne den Block ist die Datei ungültig. Er bleibt.
//! - **`SEEKTABLE`** führt Sprungmarken. Ihre Versätze zählen **ab dem ersten
//!   Tonrahmen**, nicht ab dem Dateianfang — deshalb bleiben sie richtig,
//!   auch wenn vorne ein ID3-Tag wegfällt.
//! - **`CUESHEET`** beschreibt Titelgrenzen. Das ist Navigation, also Inhalt,
//!   dieselbe Grenze wie bei Kapiteln in Matroska. Wird gemeldet, nicht
//!   entfernt — das Ergebnis ist dann `Partial`.
//!
//! # Der ID3-Tag, der hier nichts zu suchen hat
//!
//! FLAC kennt kein ID3. Trotzdem schreiben manche Programme einen davor —
//! und dann steht der Verfassername in einer Datei, die ihn nach ihrer
//! eigenen Norm gar nicht tragen kann. Ein Werkzeug, das nur die
//! FLAC-Blöcke säubert, übersieht ihn vollständig. Er wird hier abgetragen
//! wie bei MP3.

use crate::model::{Finding, FindingKind, Inspection, Severity, StripResult};
use crate::{mp3, vorbis};

use cabrik_core::{Error, Result};

/// Höchstzahl der Blöcke, die verfolgt werden.
const MAX_BLOECKE: usize = 10_000;

const STREAMINFO: u8 = 0;
const PADDING: u8 = 1;
const APPLICATION: u8 = 2;
const SEEKTABLE: u8 = 3;
const VORBIS_COMMENT: u8 = 4;
const CUESHEET: u8 = 5;
const PICTURE: u8 = 6;

/// Wo die Kennung `fLaC` steht — hinter einem etwaigen ID3-Tag.
fn vorspann(daten: &[u8]) -> Option<usize> {
    if daten.starts_with(b"fLaC") {
        return Some(0);
    }
    let versatz = mp3::id3v2_laenge(daten)?;
    if daten.get(versatz..versatz.saturating_add(4)) == Some(b"fLaC") {
        Some(versatz)
    } else {
        None
    }
}

/// Ob die Bytes wie eine FLAC-Datei aussehen.
///
/// Auch dann, wenn ein ID3-Tag davorsteht — sonst hielte die Erkennung sie
/// für ein MP3 und suchte vergeblich nach Tonrahmen.
#[must_use]
pub fn looks_like_flac(daten: &[u8]) -> bool {
    vorspann(daten).is_some()
}

/// Ein Metadatenblock mit seiner Lage.
#[derive(Debug, Clone, Copy)]
struct Block {
    art: u8,
    letzter: bool,
    /// Anfang des vier Byte langen Kopfes.
    anfang: usize,
    /// Anfang des Inhalts.
    inhalt: usize,
    ende: usize,
}

/// Läuft die Blockkette ab und gibt zusätzlich zurück, wo der Ton beginnt.
fn sammle(daten: &[u8], von: usize) -> Result<(Vec<Block>, usize)> {
    let mut aus = Vec::new();
    let mut p = von
        .checked_add(4)
        .ok_or(Error::Malformed("flac: Versatz ueberlaeuft"))?;

    loop {
        if aus.len() >= MAX_BLOECKE {
            return Err(Error::Malformed("flac: zu viele Bloecke"));
        }
        let kopf = *daten
            .get(p)
            .ok_or(Error::Malformed("flac: Blockkopf fehlt"))?;
        let letzter = kopf & 0x80 != 0;
        let art = kopf & 0x7F;

        let laenge = daten
            .get(p.saturating_add(1)..p.saturating_add(4))
            .and_then(|b| {
                Some(
                    usize::from(*b.first()?) << 16
                        | usize::from(*b.get(1)?) << 8
                        | usize::from(*b.get(2)?),
                )
            })
            .ok_or(Error::Malformed("flac: Blocklaenge unlesbar"))?;

        let inhalt = p.saturating_add(4);
        let ende = inhalt
            .checked_add(laenge)
            .ok_or(Error::Malformed("flac: Blockende ueberlaeuft"))?;
        if ende > daten.len() {
            return Err(Error::Malformed("flac: Block reicht ueber das Dateiende"));
        }

        aus.push(Block {
            art,
            letzter,
            anfang: p,
            inhalt,
            ende,
        });
        p = ende;
        if letzter {
            break;
        }
    }
    Ok((aus, p))
}

fn funde(daten: &[u8], bloecke: &[Block], id3: usize) -> Vec<Finding> {
    let mut aus = Vec::new();

    if id3 > 0 {
        aus.push(Finding {
            kind: FindingKind::UnknownExtension,
            location: "FLAC:ID3v2".to_owned(),
            value: Some(format!(
                "ein ID3-Tag vor der FLAC-Kennung ({id3} Bytes) — FLAC kennt kein ID3, \
                 ein reiner FLAC-Reiniger übersieht ihn"
            )),
            severity: Severity::Critical,
        });
        aus.extend(mp3::id3v2_funde(daten, id3));
    }

    for b in bloecke {
        match b.art {
            VORBIS_COMMENT => {
                let roh = daten.get(b.inhalt..b.ende).unwrap_or(&[]);
                if let Some(k) = vorbis::lies(roh) {
                    aus.extend(vorbis::funde(&k, "FLAC:VORBIS_COMMENT"));
                } else {
                    aus.push(Finding {
                        kind: FindingKind::UnknownExtension,
                        location: "FLAC:VORBIS_COMMENT".to_owned(),
                        value: Some("Kommentarblock nicht lesbar".to_owned()),
                        severity: Severity::Notable,
                    });
                }
            }
            PICTURE => aus.push(Finding {
                kind: FindingKind::EmbeddedPreview,
                location: "FLAC:PICTURE".to_owned(),
                value: Some(format!(
                    "eingebettetes Bild ({} Bytes) — es trägt eigene Metadaten",
                    b.ende.saturating_sub(b.inhalt)
                )),
                severity: Severity::Critical,
            }),
            APPLICATION => {
                let kennung = daten
                    .get(b.inhalt..b.inhalt.saturating_add(4))
                    .map(|k| String::from_utf8_lossy(k).into_owned())
                    .unwrap_or_default();
                aus.push(Finding {
                    kind: FindingKind::UnknownExtension,
                    location: "FLAC:APPLICATION".to_owned(),
                    value: Some(format!(
                        "Daten eines fremden Programms „{kennung}“ ({} Bytes) — \
                         der Inhalt ist von außen nicht zu beurteilen",
                        b.ende.saturating_sub(b.inhalt)
                    )),
                    severity: Severity::Critical,
                });
            }
            // Diese vier tragen keine Angaben über eine Person:
            // `STREAMINFO` beschreibt die Tonspur und ist Pflicht,
            // `SEEKTABLE` führt Sprungmarken, `PADDING` ist leer, und
            // `CUESHEET` wird weiter unten als Rest gemeldet.
            STREAMINFO | SEEKTABLE | PADDING | CUESHEET => {}
            _ => {}
        }
    }
    aus
}

/// Was gemeldet wird, aber stehen bleibt.
fn reste(bloecke: &[Block]) -> Vec<Finding> {
    let mut aus = Vec::new();
    for b in bloecke {
        let groesse = b.ende.saturating_sub(b.inhalt);
        match b.art {
            // Navigation, also Inhalt — wie Kapitel in Matroska.
            CUESHEET => aus.push(Finding {
                kind: FindingKind::Comment,
                location: "FLAC:CUESHEET".to_owned(),
                value: Some(format!(
                    "Titelgrenzen ({groesse} Bytes); enthält Katalognummern"
                )),
                severity: Severity::Notable,
            }),
            // Ein Blocktyp, den die Norm zur Zeit dieses Codes nicht kannte.
            // Ihn zu `PADDING` zu machen wäre eine Wette darauf, dass er
            // entbehrlich ist — und genau solche Wetten hat v1 verloren.
            art if art > PICTURE && art < 127 => aus.push(Finding {
                kind: FindingKind::UnknownExtension,
                location: format!("FLAC:Block {art}"),
                value: Some(format!(
                    "Blocktyp {art} ist diesem Programm unbekannt ({groesse} Bytes) — \
                     er bleibt unangetastet, weil sein Inhalt nicht zu beurteilen ist"
                )),
                severity: Severity::Notable,
            }),
            _ => {}
        }
    }
    aus
}

/// Zeigt die Marken an, ohne die Datei zu verändern.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Blockkette.
pub fn inspect(daten: &[u8]) -> Result<Inspection> {
    let von = vorspann(daten).ok_or(Error::Malformed("flac: keine fLaC-Kennung"))?;
    let (bloecke, _) = sammle(daten, von)?;

    let mut alle = funde(daten, &bloecke, von);
    alle.extend(reste(&bloecke));

    Ok(Inspection {
        format: Some("FLAC".to_owned()),
        findings: alle,
        understood: true,
    })
}

/// Macht aus einem Block einen `PADDING`-Block gleicher Größe.
///
/// Das Merkbit für den letzten Block bleibt erhalten — ginge es verloren,
/// suchte ein Leser hinter den Tonrahmen nach weiteren Blöcken.
fn zu_padding(aus: &mut [u8], b: &Block) {
    if let Some(kopf) = aus.get_mut(b.anfang) {
        *kopf = if b.letzter { 0x80 | PADDING } else { PADDING };
    }
    if let Some(inhalt) = aus.get_mut(b.inhalt..b.ende) {
        inhalt.fill(0);
    }
}

/// Entfernt die Marken.
///
/// Die FLAC-Blöcke werden zu `PADDING` **gleicher Größe**; ein etwaiger
/// ID3-Tag davor fällt ganz weg. Die Datei wird also höchstens um diesen Tag
/// kürzer, im Inneren verschiebt sich nichts.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Blockkette.
pub fn strip(daten: &[u8]) -> Result<(Vec<u8>, StripResult)> {
    let von = vorspann(daten).ok_or(Error::Malformed("flac: keine fLaC-Kennung"))?;
    let (bloecke, _) = sammle(daten, von)?;
    let entfernt = funde(daten, &bloecke, von);
    let uebrig = reste(&bloecke);

    // Ohne den ID3-Tag davor. Die Versätze der Blöcke verschieben sich damit
    // um `von` nach vorn.
    let mut aus = daten
        .get(von..)
        .ok_or(Error::Malformed("flac: Versatz jenseits der Datei"))?
        .to_vec();

    for b in &bloecke {
        if !matches!(b.art, VORBIS_COMMENT | PICTURE | APPLICATION) {
            continue;
        }
        let verschoben = Block {
            anfang: b.anfang.saturating_sub(von),
            inhalt: b.inhalt.saturating_sub(von),
            ende: b.ende.saturating_sub(von),
            ..*b
        };
        zu_padding(&mut aus, &verschoben);
    }

    debug_assert_eq!(
        aus.len().saturating_add(von),
        daten.len(),
        "es wurde mehr als der ID3-Tag entfernt"
    );

    if uebrig.is_empty() {
        Ok((aus, StripResult::Complete { removed: entfernt }))
    } else {
        Ok((
            aus,
            StripResult::Partial {
                removed: entfernt,
                remaining: uebrig,
                reason: "es bleiben Blöcke stehen, die Inhalt tragen oder deren Inhalt \
                         dieses Programm nicht beurteilen kann"
                    .to_owned(),
            },
        ))
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Tests duerfen laut werden"
)]
mod tests {
    use super::*;

    fn block(art: u8, letzter: bool, inhalt: &[u8]) -> Vec<u8> {
        let mut v = vec![if letzter { 0x80 | art } else { art }];
        let n = inhalt.len();
        v.extend_from_slice(&[(n >> 16) as u8, (n >> 8) as u8, n as u8]);
        v.extend_from_slice(inhalt);
        v
    }

    fn kommentare(eintraege: &[&str]) -> Vec<u8> {
        let hersteller = b"reference libFLAC 1.4.3";
        let mut v = u32::try_from(hersteller.len())
            .unwrap()
            .to_le_bytes()
            .to_vec();
        v.extend_from_slice(hersteller);
        v.extend_from_slice(&u32::try_from(eintraege.len()).unwrap().to_le_bytes());
        for e in eintraege {
            v.extend_from_slice(&u32::try_from(e.len()).unwrap().to_le_bytes());
            v.extend_from_slice(e.as_bytes());
        }
        v
    }

    /// Ein Tonrahmen beginnt mit vierzehn gesetzten Bits.
    fn ton() -> Vec<u8> {
        let mut v = vec![0xFFu8, 0xF8, 0x59, 0x18];
        v.extend_from_slice(&[0x11u8; 64]);
        v
    }

    fn beispiel() -> Vec<u8> {
        [
            b"fLaC".to_vec(),
            block(STREAMINFO, false, &[0x22u8; 34]),
            block(
                VORBIS_COMMENT,
                false,
                &kommentare(&[
                    "ARTIST=Dr. Anna Beispiel",
                    "TITLE=Angebot Nordstern",
                    "DESCRIPTION=Nicht an den Kunden geben",
                ]),
            ),
            block(PADDING, true, &[0u8; 128]),
            ton(),
        ]
        .concat()
    }

    #[test]
    fn flac_wird_erkannt() {
        assert!(looks_like_flac(&beispiel()));
        assert!(!looks_like_flac(b"nicht FLAC"));
        assert!(!looks_like_flac(b""));
    }

    #[test]
    fn die_kommentare_werden_gelesen() {
        let i = inspect(&beispiel()).unwrap();
        assert!(i.understood);
        assert_eq!(i.format.as_deref(), Some("FLAC"));

        let f = i
            .findings
            .iter()
            .find(|f| f.location == "FLAC:VORBIS_COMMENT/ARTIST")
            .expect("ARTIST fehlt");
        assert_eq!(f.kind, FindingKind::Author);
        assert_eq!(f.severity, Severity::Critical);
        assert_eq!(f.value.as_deref(), Some("Dr. Anna Beispiel"));

        assert!(
            i.findings
                .iter()
                .any(|f| f.location == "FLAC:VORBIS_COMMENT/Hersteller"),
            "die Herstellerangabe ist auch eine Angabe"
        );
    }

    /// **Der Kern des Verfahrens.** `PADDING` ist der Platzhalter, den FLAC
    /// selbst dafür vorsieht — die Länge bleibt auf das Byte gleich.
    #[test]
    fn der_kommentarblock_wird_zu_padding_gleicher_groesse() {
        let vorher = beispiel();
        let (nachher, ergebnis) = strip(&vorher).unwrap();

        assert_eq!(nachher.len(), vorher.len());
        assert!(matches!(ergebnis, StripResult::Complete { .. }));

        let (bloecke, _) = sammle(&nachher, 0).unwrap();
        assert!(
            !bloecke.iter().any(|b| b.art == VORBIS_COMMENT),
            "der Kommentarblock steht noch da"
        );
        assert_eq!(bloecke.len(), 3, "die Kette hat ihre Länge geändert");

        for spur in [
            &b"Dr. Anna Beispiel"[..],
            b"Angebot Nordstern",
            b"Nicht an den Kunden",
            b"libFLAC",
        ] {
            assert!(
                !nachher.windows(spur.len()).any(|f| f == spur),
                "noch lesbar: {}",
                String::from_utf8_lossy(spur)
            );
        }
        assert!(inspect(&nachher).unwrap().findings.is_empty());
    }

    /// Das Merkbit für den letzten Block muss überleben. Ginge es verloren,
    /// suchte ein Leser hinter dem Ton nach weiteren Blöcken.
    #[test]
    fn das_merkbit_des_letzten_blocks_bleibt() {
        let datei = [
            b"fLaC".to_vec(),
            block(STREAMINFO, false, &[0x22u8; 34]),
            // Diesmal ist der Kommentarblock selbst der letzte.
            block(VORBIS_COMMENT, true, &kommentare(&["ARTIST=Anna"])),
            ton(),
        ]
        .concat();

        let (sauber, _) = strip(&datei).unwrap();
        let (bloecke, ton_von) = sammle(&sauber, 0).unwrap();
        assert_eq!(bloecke.len(), 2);
        assert!(bloecke.last().unwrap().letzter, "das Merkbit ging verloren");
        assert_eq!(
            sauber.get(ton_von..ton_von + 2),
            Some(&[0xFF, 0xF8][..]),
            "der Ton beginnt nicht mehr an der erwarteten Stelle"
        );
    }

    /// **Der Tag, der hier nichts zu suchen hat.** FLAC kennt kein ID3 —
    /// ein Werkzeug, das nur die FLAC-Blöcke säubert, übersieht ihn.
    #[test]
    fn ein_id3_tag_vor_der_kennung_wird_gefunden_und_abgetragen() {
        let mut tag = b"ID3".to_vec();
        tag.extend_from_slice(&[3, 0, 0]);
        let rahmen = {
            let inhalt = b"\x00Dr. Anna Beispiel";
            let mut r = b"TPE1".to_vec();
            r.extend_from_slice(&u32::try_from(inhalt.len()).unwrap().to_be_bytes());
            r.extend_from_slice(&[0, 0]);
            r.extend_from_slice(inhalt);
            r
        };
        let groesse = rahmen.len();
        tag.extend_from_slice(&[0, 0, ((groesse >> 7) & 0x7F) as u8, (groesse & 0x7F) as u8]);
        tag.extend_from_slice(&rahmen);

        let datei = [tag.clone(), beispiel()].concat();
        assert!(
            looks_like_flac(&datei),
            "der ID3-Tag verdeckt die Erkennung"
        );

        let i = inspect(&datei).unwrap();
        let f = i
            .findings
            .iter()
            .find(|f| f.location == "FLAC:ID3v2")
            .expect("der ID3-Tag wurde nicht gemeldet");
        assert_eq!(f.severity, Severity::Critical);
        assert!(
            i.findings.iter().any(|f| f.location == "MP3:ID3v2/TPE1"),
            "der Inhalt des ID3-Tags wurde nicht gelesen"
        );

        let (sauber, _) = strip(&datei).unwrap();
        assert!(sauber.starts_with(b"fLaC"), "der Tag blieb stehen");
        assert_eq!(sauber.len(), datei.len() - tag.len());
        assert!(!sauber.windows(8).any(|f| f == b"Dr. Anna"));
    }

    /// `CUESHEET` ist Navigation. Es bleibt und macht das Ergebnis `Partial`.
    #[test]
    fn ein_cuesheet_bleibt_und_macht_das_ergebnis_teilweise() {
        let datei = [
            b"fLaC".to_vec(),
            block(STREAMINFO, false, &[0x22u8; 34]),
            block(CUESHEET, true, &[0x33u8; 64]),
            ton(),
        ]
        .concat();

        let (sauber, ergebnis) = strip(&datei).unwrap();
        let StripResult::Partial { remaining, .. } = ergebnis else {
            panic!("CUESHEET muss als Rest gemeldet werden");
        };
        assert!(remaining.iter().any(|f| f.location == "FLAC:CUESHEET"));
        assert_eq!(sauber, datei, "das CUESHEET wurde doch angetastet");
    }

    #[test]
    fn ein_bild_und_fremde_daten_sind_kritische_funde() {
        let datei = [
            b"fLaC".to_vec(),
            block(STREAMINFO, false, &[0x22u8; 34]),
            block(PICTURE, false, &[0x44u8; 200]),
            block(APPLICATION, true, b"XYZ\x00geheime Nutzlast"),
            ton(),
        ]
        .concat();

        let i = inspect(&datei).unwrap();
        for ort in ["FLAC:PICTURE", "FLAC:APPLICATION"] {
            let f = i
                .findings
                .iter()
                .find(|f| f.location == ort)
                .unwrap_or_else(|| panic!("{ort} fehlt"));
            assert_eq!(f.severity, Severity::Critical);
        }

        let (sauber, _) = strip(&datei).unwrap();
        assert_eq!(sauber.len(), datei.len());
        assert!(!sauber.windows(7).any(|f| f == b"geheime"));
    }

    #[test]
    fn ein_zweiter_durchlauf_aendert_nichts() {
        let (einmal, _) = strip(&beispiel()).unwrap();
        let (zweimal, _) = strip(&einmal).unwrap();
        assert_eq!(einmal, zweimal);
    }

    #[test]
    fn ein_block_ueber_das_dateiende_hinaus_ist_ein_fehler() {
        let datei = [b"fLaC".to_vec(), vec![0x84, 0xFF, 0xFF, 0xFF]].concat();
        assert!(inspect(&datei).is_err());
    }
}
