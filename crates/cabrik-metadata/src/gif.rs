//! GIF (`spec/metadata.md` §4).
//!
//! Aufbau: Kopf ‖ Bildschirmbeschreibung ‖ optionale Farbtabelle ‖ Blöcke ‖
//! `0x3B`. Jeder Block beginnt mit einem Kennbyte:
//!
//! | Byte | Bedeutung |
//! |---|---|
//! | `0x21` | Erweiterung, danach folgt ihr Kennzeichen |
//! | `0x2C` | Bildbeschreibung samt Bilddaten |
//! | `0x3B` | Ende |
//!
//! # Was hier drinsteckt
//!
//! - **Kommentar-Erweiterung** (`0xFE`) — freier Text. Bildbearbeitungs-
//!   programme schreiben dort ihren Namen hinein, Scanner ihre Modellnummer.
//! - **Anwendungs-Erweiterung** (`0xFF`) — trägt eine achtstellige Kennung.
//!   `NETSCAPE` steuert die Wiederholung einer Animation und **muss bleiben**;
//!   `XMP Data` dagegen ist ein vollständiger XMP-Block mit Verfassernamen.
//!
//! Die Unterscheidung ist der ganze Witz dieses Moduls: Wer alle
//! Anwendungs-Erweiterungen entfernt, nimmt einer Animation die Wiederholung.
//! Wer keine entfernt, lässt den XMP-Block stehen.

use crate::model::{Finding, FindingKind, Inspection, Severity, StripResult};

use cabrik_core::{Error, Result};

/// Kennzeichen der Erweiterungen.
const EXT_EINLEITUNG: u8 = 0x21;
const EXT_ANZEIGE: u8 = 0xF9;
const EXT_KOMMENTAR: u8 = 0xFE;
const EXT_TEXT: u8 = 0x01;
const EXT_ANWENDUNG: u8 = 0xFF;
const BILD_EINLEITUNG: u8 = 0x2C;
const ENDE: u8 = 0x3B;

/// Anwendungs-Erweiterungen, die zur Darstellung gehören und bleiben.
const ANWENDUNG_BEHALTEN: [&[u8]; 2] = [b"NETSCAPE", b"ANIMEXTS"];

/// Ob die Bytes wie ein GIF aussehen.
#[must_use]
pub fn looks_like_gif(daten: &[u8]) -> bool {
    daten.starts_with(b"GIF87a") || daten.starts_with(b"GIF89a")
}

/// Ein Block der Datei, wie er in der Ausgabe wieder erscheinen soll.
struct Block {
    /// Die Rohbytes, unverändert.
    bytes: Vec<u8>,
    /// Was daran gefunden wurde, sofern es Metadaten sind.
    fund: Option<Finding>,
}

/// Liest die Länge der Farbtabelle aus dem Packfeld.
fn farbtabelle_len(packfeld: u8) -> usize {
    if packfeld & 0x80 == 0 {
        return 0;
    }
    // Die unteren drei Bits geben die Größe als Zweierpotenz an.
    let n = u32::from(packfeld & 0x07).saturating_add(1);
    3usize.saturating_mul(1usize << n.min(8))
}

/// Überliest eine Kette von Unterblöcken und gibt die Position dahinter.
///
/// Unterblöcke sind Länge(1) ‖ Daten, abgeschlossen durch eine Länge von null.
fn ueberlies_unterbloecke(daten: &[u8], mut pos: usize) -> Result<usize> {
    loop {
        let laenge = *daten
            .get(pos)
            .ok_or(Error::Malformed("gif: Unterblock reicht ueber das Ende"))?;
        pos = pos.saturating_add(1);
        if laenge == 0 {
            return Ok(pos);
        }
        pos = pos.saturating_add(usize::from(laenge));
        if pos > daten.len() {
            return Err(Error::Malformed("gif: Unterblock reicht ueber das Ende"));
        }
    }
}

/// Zerlegt die Datei in Kopf und Blöcke.
fn zerlege(daten: &[u8]) -> Result<(Vec<u8>, Vec<Block>)> {
    if !looks_like_gif(daten) {
        return Err(Error::Malformed("gif: kein GIF-Kopf"));
    }

    // Kopf(6) + Bildschirmbeschreibung(7) + optionale Farbtabelle.
    let packfeld = *daten
        .get(10)
        .ok_or(Error::Malformed("gif: Bildschirmbeschreibung fehlt"))?;
    let kopf_ende = 13usize.saturating_add(farbtabelle_len(packfeld));
    let kopf = daten
        .get(..kopf_ende)
        .ok_or(Error::Malformed("gif: Farbtabelle reicht ueber das Ende"))?
        .to_vec();

    let mut bloecke = Vec::new();
    let mut pos = kopf_ende;

    while pos < daten.len() {
        let kennbyte = *daten
            .get(pos)
            .ok_or(Error::Malformed("gif: Blockende fehlt"))?;

        match kennbyte {
            ENDE => {
                bloecke.push(Block {
                    bytes: vec![ENDE],
                    fund: None,
                });
                break;
            }
            EXT_EINLEITUNG => {
                let kennzeichen = *daten
                    .get(pos.saturating_add(1))
                    .ok_or(Error::Malformed("gif: Erweiterungskennzeichen fehlt"))?;

                let (ende, fund) = erweiterung(daten, pos, kennzeichen)?;
                bloecke.push(Block {
                    bytes: daten
                        .get(pos..ende)
                        .ok_or(Error::Malformed("gif: Erweiterung unlesbar"))?
                        .to_vec(),
                    fund,
                });
                pos = ende;
            }
            BILD_EINLEITUNG => {
                let ende = bildblock(daten, pos)?;
                bloecke.push(Block {
                    bytes: daten
                        .get(pos..ende)
                        .ok_or(Error::Malformed("gif: Bildblock unlesbar"))?
                        .to_vec(),
                    fund: None,
                });
                pos = ende;
            }
            _ => return Err(Error::Malformed("gif: unbekanntes Blockkennbyte")),
        }
    }

    Ok((kopf, bloecke))
}

/// Liest eine Erweiterung und beurteilt sie.
fn erweiterung(daten: &[u8], pos: usize, kennzeichen: u8) -> Result<(usize, Option<Finding>)> {
    // Einleitung(1) + Kennzeichen(1), danach die Unterblöcke — bei der
    // Anwendungs-Erweiterung steht davor noch ein Block fester Länge.
    let nach_kopf = pos.saturating_add(2);

    match kennzeichen {
        EXT_KOMMENTAR => {
            let ende = ueberlies_unterbloecke(daten, nach_kopf)?;
            let text = sammle_unterbloecke(daten, nach_kopf);
            Ok((
                ende,
                Some(Finding::new(
                    FindingKind::Comment,
                    "GIF:Kommentar".to_owned(),
                    Some(String::from_utf8_lossy(&text).trim().to_owned()),
                    Severity::Notable,
                )),
            ))
        }
        EXT_TEXT => {
            // Nach dem Kennzeichen folgt ein 12 Byte langer Block.
            let ende = ueberlies_unterbloecke(daten, nach_kopf.saturating_add(13))?;
            Ok((
                ende,
                Some(Finding::new(
                    FindingKind::Comment,
                    "GIF:Klartext-Erweiterung".to_owned(),
                    Some("eingebetteter Text — von kaum einem Betrachter angezeigt".to_owned()),
                    Severity::Notable,
                )),
            ))
        }
        EXT_ANWENDUNG => {
            // Blocklänge(1) ‖ Kennung(8) ‖ Kennzeichen(3), danach Unterblöcke.
            let laenge = usize::from(
                *daten
                    .get(nach_kopf)
                    .ok_or(Error::Malformed("gif: Anwendungsblock unlesbar"))?,
            );
            let kennung_start = nach_kopf.saturating_add(1);
            let kennung = daten
                .get(kennung_start..kennung_start.saturating_add(8.min(laenge)))
                .unwrap_or(&[]);
            let ende = ueberlies_unterbloecke(daten, kennung_start.saturating_add(laenge))?;

            let name = String::from_utf8_lossy(kennung).trim().to_owned();
            if ANWENDUNG_BEHALTEN.iter().any(|k| kennung.starts_with(k)) {
                // Steuert die Wiederholung einer Animation — kein Fund.
                return Ok((ende, None));
            }
            Ok((
                ende,
                Some(Finding::new(
                    if kennung.starts_with(b"XMP ") {
                        FindingKind::Author
                    } else {
                        FindingKind::UnknownExtension
                    },
                    format!("GIF:Anwendung/{name}"),
                    Some(if kennung.starts_with(b"XMP ") {
                        "XMP-Block — trägt häufig Verfasser und Bearbeitungsverlauf".to_owned()
                    } else {
                        format!("Anwendungs-Erweiterung „{name}\"")
                    }),
                    if kennung.starts_with(b"XMP ") {
                        Severity::Critical
                    } else {
                        Severity::Notable
                    },
                )),
            ))
        }
        EXT_ANZEIGE => {
            // Steuert Anzeigedauer und Transparenz — gehört zum Bild.
            let ende = ueberlies_unterbloecke(daten, nach_kopf)?;
            Ok((ende, None))
        }
        _ => {
            let ende = ueberlies_unterbloecke(daten, nach_kopf)?;
            Ok((
                ende,
                Some(Finding::new(
                    FindingKind::UnknownExtension,
                    format!("GIF:Erweiterung 0x{kennzeichen:02X}"),
                    Some("unbekannte Erweiterung".to_owned()),
                    Severity::Minor,
                )),
            ))
        }
    }
}

fn sammle_unterbloecke(daten: &[u8], mut pos: usize) -> Vec<u8> {
    let mut aus = Vec::new();
    loop {
        let Some(&laenge) = daten.get(pos) else {
            return aus;
        };
        pos = pos.saturating_add(1);
        if laenge == 0 {
            return aus;
        }
        let ende = pos.saturating_add(usize::from(laenge));
        match daten.get(pos..ende) {
            Some(s) => aus.extend_from_slice(s),
            None => return aus,
        }
        pos = ende;
    }
}

/// Liest einen Bildblock: Beschreibung ‖ optionale Farbtabelle ‖ Bilddaten.
fn bildblock(daten: &[u8], pos: usize) -> Result<usize> {
    let packfeld = *daten
        .get(pos.saturating_add(9))
        .ok_or(Error::Malformed("gif: Bildbeschreibung unlesbar"))?;
    let nach_beschreibung = pos
        .saturating_add(10)
        .saturating_add(farbtabelle_len(packfeld));

    // Ein Byte Mindest-Codelänge, danach die Bilddaten als Unterblöcke.
    ueberlies_unterbloecke(daten, nach_beschreibung.saturating_add(1))
}

/// Untersucht ein GIF.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Struktur.
pub fn inspect(daten: &[u8]) -> Result<Inspection> {
    let (_, bloecke) = zerlege(daten)?;
    Ok(Inspection {
        format: Some("GIF".to_owned()),
        findings: bloecke.into_iter().filter_map(|b| b.fund).collect(),
        understood: true,
    })
}

/// Entfernt Kommentare und fremde Anwendungs-Erweiterungen.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Struktur.
pub fn strip(daten: &[u8]) -> Result<(Vec<u8>, StripResult)> {
    let (kopf, bloecke) = zerlege(daten)?;

    let mut aus = kopf;
    let mut entfernt = Vec::new();
    for b in bloecke {
        match b.fund {
            Some(f) => entfernt.push(f),
            None => aus.extend_from_slice(&b.bytes),
        }
    }

    // Ein GIF ohne Abschlussbyte wäre unvollständig.
    if aus.last() != Some(&ENDE) {
        aus.push(ENDE);
    }

    Ok((aus, StripResult::Complete { removed: entfernt }))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;

    /// Unterblöcke aus einem Datenstück bauen.
    fn unterbloecke(daten: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        for stueck in daten.chunks(255) {
            v.push(u8::try_from(stueck.len()).unwrap());
            v.extend_from_slice(stueck);
        }
        v.push(0);
        v
    }

    fn bild() -> Vec<u8> {
        let mut v = b"GIF89a".to_vec();
        // Bildschirmbeschreibung: 4x4, globale Farbtabelle mit 2 Eintraegen.
        v.extend_from_slice(&[4, 0, 4, 0, 0x80, 0, 0]);
        v.extend_from_slice(&[0, 0, 0, 255, 255, 255]); // 2 * 3 Bytes

        // Anzeige-Erweiterung — gehoert zum Bild.
        v.extend_from_slice(&[EXT_EINLEITUNG, EXT_ANZEIGE]);
        v.extend_from_slice(&unterbloecke(&[0, 10, 0, 0]));

        // NETSCAPE — steuert die Wiederholung, muss bleiben.
        v.extend_from_slice(&[EXT_EINLEITUNG, EXT_ANWENDUNG, 11]);
        v.extend_from_slice(b"NETSCAPE2.0");
        v.extend_from_slice(&unterbloecke(&[1, 0, 0]));

        // XMP — traegt den Verfassernamen.
        v.extend_from_slice(&[EXT_EINLEITUNG, EXT_ANWENDUNG, 11]);
        v.extend_from_slice(b"XMP DataXMP");
        v.extend_from_slice(&unterbloecke(b"<x>Dr. Anna Beispiel</x>"));

        // Kommentar.
        v.extend_from_slice(&[EXT_EINLEITUNG, EXT_KOMMENTAR]);
        v.extend_from_slice(&unterbloecke(b"Erstellt mit Scanner XY-2000"));

        // Bildblock.
        v.push(BILD_EINLEITUNG);
        v.extend_from_slice(&[0, 0, 0, 0, 4, 0, 4, 0, 0]); // ohne lokale Tabelle
        v.push(2); // Mindest-Codelaenge
        v.extend_from_slice(&unterbloecke(b"BILDDATEN"));

        v.push(ENDE);
        v
    }

    #[test]
    fn gif_wird_an_den_kennbytes_erkannt() {
        assert!(looks_like_gif(b"GIF89a..."));
        assert!(looks_like_gif(b"GIF87a..."));
        assert!(!looks_like_gif(b"GIF88a..."));
    }

    #[test]
    fn kommentar_und_xmp_werden_gefunden() {
        let i = inspect(&bild()).unwrap();
        assert_eq!(i.format.as_deref(), Some("GIF"));

        let kommentar = i
            .findings
            .iter()
            .find(|f| f.location == "GIF:Kommentar")
            .expect("Kommentar nicht gefunden");
        assert_eq!(
            kommentar.value.as_deref(),
            Some("Erstellt mit Scanner XY-2000")
        );

        let xmp = i
            .findings
            .iter()
            .find(|f| f.location.contains("XMP"))
            .expect("XMP nicht gefunden");
        assert_eq!(xmp.severity, Severity::Critical);
    }

    /// **Der Witz des Moduls.** `NETSCAPE` steuert die Wiederholung einer
    /// Animation. Wer alle Anwendungs-Erweiterungen entfernt, nimmt ihr die
    /// Schleife -- ein sichtbarer Schaden.
    #[test]
    fn die_wiederholungssteuerung_bleibt() {
        let i = inspect(&bild()).unwrap();
        assert!(
            !i.findings.iter().any(|f| f.location.contains("NETSCAPE")),
            "NETSCAPE wurde als Metadatum gemeldet"
        );

        let (sauber, _) = strip(&bild()).unwrap();
        assert!(
            sauber.windows(8).any(|f| f == b"NETSCAPE"),
            "die Wiederholungssteuerung wurde entfernt"
        );
    }

    #[test]
    fn kommentar_und_xmp_verschwinden_die_bilddaten_bleiben() {
        let (sauber, ergebnis) = strip(&bild()).unwrap();
        assert!(ergebnis.may_show_clean());

        assert!(
            !sauber.windows(17).any(|f| f == b"Dr. Anna Beispiel"),
            "der Name blieb"
        );
        assert!(
            !sauber.windows(7).any(|f| f == b"Scanner"),
            "der Kommentar blieb"
        );
        assert!(
            sauber.windows(9).any(|f| f == b"BILDDATEN"),
            "die Bilddaten gingen verloren"
        );
        assert_eq!(sauber.last(), Some(&ENDE));
        assert!(looks_like_gif(&sauber));
    }

    /// Nach der Bereinigung muss die Datei wieder lesbar sein.
    #[test]
    fn das_ergebnis_ist_wieder_ein_gueltiges_gif() {
        let (sauber, _) = strip(&bild()).unwrap();
        let i = inspect(&sauber).unwrap();
        assert!(i.findings.is_empty(), "es blieb etwas: {:?}", i.findings);
        assert_eq!(strip(&sauber).unwrap().0, sauber, "nicht stabil");
    }

    #[test]
    fn die_farbtabellengroesse_wird_richtig_berechnet() {
        assert_eq!(farbtabelle_len(0x00), 0, "kein Merkmalsbit gesetzt");
        assert_eq!(farbtabelle_len(0x80), 6, "2 Eintraege");
        assert_eq!(farbtabelle_len(0x87), 768, "256 Eintraege");
    }

    #[test]
    fn kaputte_daten_ergeben_einen_fehler_keinen_absturz() {
        assert!(inspect(b"GIF89a").is_err());
        assert!(inspect(b"GIF89a\x04\x00\x04\x00\x80\x00\x00").is_err());
        assert!(inspect(b"").is_err());
    }
}
