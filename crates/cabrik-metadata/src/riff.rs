//! RIFF — der gemeinsame Unterbau von AVI und WAV.
//!
//! Beide Formate sind dasselbe Blockformat mit unterschiedlicher Füllung, und
//! beide legen ihre Angaben in eine `LIST INFO` mit denselben vierbuchstabigen
//! Kennungen. Diesen Läufer zweimal zu schreiben hieße, **zweimal Gelegenheit
//! für denselben Fehler** zu schaffen — und ein Parser ist die teuerste Art
//! von Code, die man doppelt haben kann.
//!
//! Auch der Platzhalter ist derselbe: der **`JUNK`-Block**, den jeder Leser
//! überspringt. Er ist keine Erfindung dieses Programms; ffmpeg legt von sich
//! aus welche an, um Blöcke auf gerade Adressen auszurichten.

use crate::model::{FindingKind, Severity};

use cabrik_core::{Error, Result};

/// Höchstzahl der Blöcke, die verfolgt werden.
const MAX_BLOECKE: usize = 100_000;
/// Höchste Schachtelungstiefe.
const MAX_TIEFE: usize = 8;

/// Ein gefundener Block.
#[derive(Debug, Clone, Copy)]
pub struct Block {
    /// Die vierbuchstabige Kennung.
    pub typ: [u8; 4],
    /// Bei `RIFF` und `LIST`: die Art der Liste.
    pub art: Option<[u8; 4]>,
    /// Anfang des Blocks, einschließlich Kopf.
    pub anfang: usize,
    /// Anfang des Inhalts.
    pub inhalt: usize,
    /// Ende des Blocks.
    pub ende: usize,
}

impl Block {
    /// Ob es sich um eine Liste handelt.
    #[must_use]
    pub fn ist_liste(&self) -> bool {
        self.typ == *b"LIST" || self.typ == *b"RIFF"
    }

    /// Länge des Inhalts.
    #[must_use]
    pub const fn inhalt_laenge(&self) -> usize {
        self.ende.saturating_sub(self.inhalt)
    }
}

fn vier(daten: &[u8], p: usize) -> Option<[u8; 4]> {
    daten.get(p..p.checked_add(4)?)?.try_into().ok()
}

fn u32_le(daten: &[u8], p: usize) -> Option<u32> {
    Some(u32::from_le_bytes(vier(daten, p)?))
}

/// Ob die Bytes ein RIFF der angegebenen Art sind.
///
/// AVI und WAV unterscheiden sich allein in diesen vier Bytes ab Versatz acht
/// — und WebP ist ebenfalls RIFF, weshalb hier nie auf `RIFF` allein geprüft
/// werden darf.
#[must_use]
pub fn ist_riff(daten: &[u8], art: &[u8; 4]) -> bool {
    daten.starts_with(b"RIFF") && daten.get(8..12) == Some(art.as_slice())
}

/// Läuft die Blockstruktur ab.
///
/// In die in `ausgespart` genannten Listenarten wird **nicht** abgestiegen —
/// dort liegen die Nutzdaten (`movi` bei AVI), und ein Modul, das dort sucht,
/// durchläuft jedes einzelne Bild.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputtem Aufbau.
pub fn sammle(daten: &[u8], ausgespart: &[&[u8; 4]]) -> Result<Vec<Block>> {
    let mut aus = Vec::new();
    lauf(daten, 0, daten.len(), 0, ausgespart, &mut aus)?;
    Ok(aus)
}

fn lauf(
    daten: &[u8],
    von: usize,
    bis: usize,
    tiefe: usize,
    ausgespart: &[&[u8; 4]],
    aus: &mut Vec<Block>,
) -> Result<()> {
    if tiefe > MAX_TIEFE {
        return Ok(());
    }
    let mut p = von;

    while p.checked_add(8).is_some_and(|e| e <= bis) {
        if aus.len() >= MAX_BLOECKE {
            return Err(Error::Malformed("riff: zu viele Bloecke"));
        }
        let typ = vier(daten, p).ok_or(Error::Malformed("riff: Blocktyp unlesbar"))?;
        let groesse = u32_le(daten, p.saturating_add(4))
            .and_then(|g| usize::try_from(g).ok())
            .ok_or(Error::Malformed("riff: Blockgroesse unlesbar"))?;

        let inhalt = p.saturating_add(8);
        let ende = inhalt
            .checked_add(groesse)
            .ok_or(Error::Malformed("riff: Blockende ueberlaeuft"))?;
        if ende > bis {
            return Err(Error::Malformed("riff: Block reicht ueber seinen Bereich"));
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

        let ueberspringen = art.is_some_and(|a| ausgespart.iter().any(|x| a == **x));
        if ist_liste && !ueberspringen {
            lauf(
                daten,
                inhalt.saturating_add(4),
                ende,
                tiefe.saturating_add(1),
                ausgespart,
                aus,
            )?;
        }

        // RIFF richtet jeden Block auf gerade Adressen aus.
        let weiter = ende.saturating_add(groesse & 1);
        if weiter <= p {
            return Err(Error::Malformed("riff: Block ohne Fortschritt"));
        }
        p = weiter;
    }
    Ok(())
}

/// Inhalt eines Blocks als Zeichenkette, bis zum ersten Nullbyte.
#[must_use]
pub fn text(daten: &[u8], b: &Block) -> String {
    teiltext(daten, b.inhalt, b.ende)
}

/// Wie [`text`], aber für einen Ausschnitt innerhalb eines Blocks.
#[must_use]
pub fn teiltext(daten: &[u8], von: usize, bis: usize) -> String {
    let roh = daten.get(von..bis).unwrap_or(&[]);
    let ohne_null = roh.split(|x| *x == 0).next().unwrap_or(&[]);
    String::from_utf8_lossy(ohne_null).trim().to_owned()
}

/// Macht aus einem Block einen `JUNK`-Block gleicher Größe.
///
/// Der Kopf bleibt stehen — nur die Kennung wird zu `JUNK` und der Inhalt
/// genullt. Die Längenangabe stimmt weiterhin, also verschiebt sich nichts.
pub fn zu_junk(aus: &mut [u8], b: &Block) {
    if let Some(name) = aus.get_mut(b.anfang..b.anfang.saturating_add(4)) {
        name.copy_from_slice(b"JUNK");
    }
    if let Some(inhalt) = aus.get_mut(b.inhalt..b.ende) {
        inhalt.fill(0);
    }
}

/// Ordnet einen Block aus einer `LIST INFO` ein.
///
/// Die Liste stammt aus der RIFF-Spezifikation und gilt für AVI wie für WAV.
/// Alles Unbekannte, das mit `I` beginnt, gilt als Kommentar — lieber einmal
/// zu viel gemeldet als eine Angabe stillschweigend übergangen.
#[must_use]
pub fn info_einordnung(typ: &[u8; 4]) -> Option<(&'static str, FindingKind, Severity)> {
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

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "Tests duerfen laut werden"
)]
mod tests {
    use super::*;

    pub(crate) fn block(typ: &[u8; 4], inhalt: &[u8]) -> Vec<u8> {
        let mut v = typ.to_vec();
        v.extend_from_slice(&u32::try_from(inhalt.len()).unwrap().to_le_bytes());
        v.extend_from_slice(inhalt);
        if inhalt.len() % 2 == 1 {
            v.push(0);
        }
        v
    }

    pub(crate) fn liste(art: &[u8; 4], inhalt: &[u8]) -> Vec<u8> {
        let mut v = art.to_vec();
        v.extend_from_slice(inhalt);
        block(b"LIST", &v)
    }

    #[test]
    fn die_art_entscheidet_nicht_die_kennung_riff() {
        // AVI, WAV und WebP sind alle RIFF. Nur die Art trennt sie.
        let avi = [
            b"RIFF".to_vec(),
            4u32.to_le_bytes().to_vec(),
            b"AVI ".to_vec(),
        ]
        .concat();
        assert!(ist_riff(&avi, b"AVI "));
        assert!(!ist_riff(&avi, b"WAVE"));
        assert!(!ist_riff(b"RIFF", b"AVI "));
    }

    #[test]
    fn ein_ausgesparter_bereich_wird_nicht_durchlaufen() {
        let inneres = [
            b"WAVE".to_vec(),
            liste(b"INFO", &block(b"IART", b"Anna\0")),
            liste(b"movi", &block(b"XXXX", b"soll unsichtbar bleiben\0")),
        ]
        .concat();
        let datei = block(b"RIFF", &inneres);

        let mit = sammle(&datei, &[]).unwrap();
        assert!(mit.iter().any(|b| b.typ == *b"XXXX"));

        let ohne = sammle(&datei, &[b"movi"]).unwrap();
        assert!(!ohne.iter().any(|b| b.typ == *b"XXXX"));
        // Die INFO-Liste wird trotzdem gefunden.
        assert!(ohne.iter().any(|b| b.typ == *b"IART"));
    }

    /// Blöcke ungerader Länge tragen ein Füllbyte, das **nicht** zur
    /// Längenangabe zählt. Wer es übersieht, verliert die Ausrichtung und
    /// liest ab dem nächsten Block Unsinn.
    #[test]
    fn das_fuellbyte_bei_ungerader_laenge_wird_beachtet() {
        let inneres = [
            b"WAVE".to_vec(),
            block(b"AAAA", b"drei"),    // 4, gerade
            block(b"BBBB", b"fuenff"),  // 6, gerade
            block(b"CCCC", b"sieben!"), // 7, ungerade -> Fuellbyte
            block(b"DDDD", b"acht"),
        ]
        .concat();
        let datei = block(b"RIFF", &inneres);
        let b = sammle(&datei, &[]).unwrap();

        let namen: Vec<String> = b
            .iter()
            .map(|x| String::from_utf8_lossy(&x.typ).into_owned())
            .collect();
        assert!(
            namen.contains(&"DDDD".to_owned()),
            "nach dem ungeraden Block ging die Ausrichtung verloren: {namen:?}"
        );
    }

    #[test]
    fn ein_block_ueber_seinen_bereich_hinaus_ist_ein_fehler() {
        let mut datei = block(b"RIFF", &[b"WAVE".to_vec(), block(b"AAAA", b"x")].concat());
        datei[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(sammle(&datei, &[]).is_err());
    }

    #[test]
    fn aus_einem_block_wird_ein_junk_gleicher_groesse() {
        let datei = block(
            b"RIFF",
            &[b"WAVE".to_vec(), block(b"IART", b"Anna\0")].concat(),
        );
        let b = sammle(&datei, &[]).unwrap();
        let iart = b.iter().find(|x| x.typ == *b"IART").unwrap();

        let mut aus = datei.clone();
        zu_junk(&mut aus, iart);
        assert_eq!(aus.len(), datei.len());
        assert!(!aus.windows(4).any(|f| f == b"Anna"));
        assert!(aus.windows(4).any(|f| f == b"JUNK"));

        // Der Aufbau bleibt lesbar.
        assert_eq!(sammle(&aus, &[]).unwrap().len(), b.len());
    }

    #[test]
    fn die_einordnung_deckt_die_wichtigen_kennungen_ab() {
        assert_eq!(info_einordnung(b"IART").unwrap().1, FindingKind::Author);
        assert_eq!(info_einordnung(b"ICMT").unwrap().2, Severity::Critical);
        // Unbekanntes mit I wird gemeldet ...
        assert!(info_einordnung(b"IZZZ").is_some());
        // ... alles andere nicht.
        assert!(info_einordnung(b"data").is_none());
        assert!(info_einordnung(b"fmt ").is_none());
    }
}
