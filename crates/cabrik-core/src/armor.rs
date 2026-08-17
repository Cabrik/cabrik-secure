//! Armor: der Envelope als Text zum Kopieren (`spec/envelope-v2.md` §14).
//!
//! # Wofür
//!
//! Für Kanäle, die keine Dateien nehmen — ein Chatfenster, eine E-Mail, ein
//! Ticketsystem. Wer dort etwas verschlüsselt hinschicken will, braucht
//! Zeichen, keine Bytes.
//!
//! # Der Preis, und dass er bewusst gezahlt wird
//!
//! Ein Drittel mehr Umfang, und die Rahmenzeilen **nennen das Produkt**.
//! Das steht gegen `spec/threat-model.md` §6.3, wonach ein Envelope nicht
//! verraten soll, womit er gemacht wurde. Die Spezifikation entscheidet das
//! so und begründet es: Wer diesen Schutz braucht, nimmt den Binärmodus.
//! Wer Armor nutzt, fügt den Text ohnehin in einen Kanal ein, der den
//! Zusammenhang längst preisgibt.
//!
//! # Warum Base64 hier steht und nicht als Abhängigkeit kommt
//!
//! Aus demselben Grund wie [`crate::base32`]: Es sind sechzig Zeilen ohne
//! Zustand, ohne Zuteilung im Zweifel und ohne Geheimnisse. Eine
//! Abhängigkeit dafür wäre mehr Prüfaufwand als Code.

use crate::error::{Error, Result};

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Die Kopfzeile.
pub const KOPF: &str = "-----BEGIN CABRIK ENVELOPE-----";
/// Die Fußzeile.
pub const FUSS: &str = "-----END CABRIK ENVELOPE-----";

/// Zeilenlänge nach `spec/envelope-v2.md` §14.
const ZEILE: usize = 64;

/// Wandelt einen Envelope in Text.
///
/// Zeilen zu 64 Zeichen, mit Kopf- und Fußzeile. Die Zeilenumbrüche sind
/// `\n`: Wer den Text in ein Windows-Programm einfügt, bekommt sie dort
/// umgesetzt — umgekehrt wäre `\r\n` in einem Terminal störend.
#[must_use]
pub fn encode(daten: &[u8]) -> String {
    let roh = base64(daten);
    let platz = roh
        .len()
        .saturating_add(roh.len() / ZEILE)
        .saturating_add(KOPF.len())
        .saturating_add(FUSS.len())
        .saturating_add(4);
    let mut aus = String::with_capacity(platz);
    aus.push_str(KOPF);
    aus.push('\n');
    for stueck in roh.as_bytes().chunks(ZEILE) {
        // `chunks` liefert nur gültige ASCII-Teile: Base64 ist einbytig.
        aus.push_str(&String::from_utf8_lossy(stueck));
        aus.push('\n');
    }
    aus.push_str(FUSS);
    aus
}

/// Liest einen Envelope aus Text.
///
/// # Was großzügig behandelt wird
///
/// Alles, was beim Kopieren passiert: führende und folgende Leerzeichen,
/// eingerückte Zeilen, `\r\n`, Zeilen von beliebiger Länge, ein
/// Zitatzeichen am Zeilenanfang. Wer einen Envelope aus einer E-Mail
/// herauskopiert, hat selten saubere Zeilen — und eine Zurückweisung
/// deshalb wäre eine Schikane ohne Sicherheitsgewinn.
///
/// # Was nicht großzügig behandelt wird
///
/// Fehlende Rahmenzeilen und fremde Zeichen im Inhalt. Beides deutet auf
/// etwas anderes als einen Envelope hin, und darüber zu raten hilft
/// niemandem.
///
/// # Fehler
///
/// [`Error::Malformed`], wenn Kopf oder Fuß fehlen oder der Inhalt kein
/// gültiges Base64 ist.
pub fn decode(text: &str) -> Result<Vec<u8>> {
    let beginn = text
        .find(KOPF)
        .ok_or(Error::Malformed("armor: missing header"))?;
    let rest = text
        .get(beginn.saturating_add(KOPF.len())..)
        .ok_or(Error::Malformed("armor: truncated"))?;
    let ende = rest
        .find(FUSS)
        .ok_or(Error::Malformed("armor: missing footer"))?;
    let inhalt = rest
        .get(..ende)
        .ok_or(Error::Malformed("armor: truncated"))?;

    // Zitatzeichen und Einrückung fallen mit weg: Was nicht zum Alphabet
    // gehört, gehört nicht zum Inhalt.
    let sauber: Vec<u8> = inhalt
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'>')
        .collect();
    unbase64(&sauber)
}

/// Ob ein Text wie ein Armor-Envelope aussieht.
///
/// Für die Oberfläche: Sie soll unterscheiden können, ob jemand einen
/// Envelope eingefügt hat oder etwas anderes — **ohne** zu entschlüsseln.
#[must_use]
pub fn looks_like_armor(text: &str) -> bool {
    text.contains(KOPF) && text.contains(FUSS)
}

// ---------------------------------------------------------------------------
// Base64
// ---------------------------------------------------------------------------

fn base64(daten: &[u8]) -> String {
    let mut aus = String::with_capacity(daten.len().div_ceil(3).saturating_mul(4));
    for block in daten.chunks(3) {
        let b0 = u32::from(block.first().copied().unwrap_or(0));
        let b1 = u32::from(block.get(1).copied().unwrap_or(0));
        let b2 = u32::from(block.get(2).copied().unwrap_or(0));
        let drei = (b0 << 16_u32) | (b1 << 8_u32) | b2;

        for i in 0..4_u32 {
            // Die letzten ein oder zwei Zeichen entfallen, wenn der Block
            // kürzer als drei Bytes war -- dafür steht das Füllzeichen.
            if usize::try_from(i).unwrap_or(4) <= block.len() {
                let schub = 18_u32.saturating_sub(i.saturating_mul(6));
                let sechs = (drei >> schub) & 0x3F;
                let z = ALPHABET
                    .get(usize::try_from(sechs).unwrap_or(0))
                    .copied()
                    .unwrap_or(b'A');
                aus.push(char::from(z));
            } else {
                aus.push('=');
            }
        }
    }
    aus
}

fn wert(z: u8) -> Option<u32> {
    ALPHABET
        .iter()
        .position(|a| *a == z)
        .and_then(|p| u32::try_from(p).ok())
}

fn unbase64(daten: &[u8]) -> Result<Vec<u8>> {
    let ohne_fuell: Vec<u8> = daten.iter().copied().take_while(|b| *b != b'=').collect();
    // Alles nach dem ersten Füllzeichen darf nur Füllzeichen sein.
    if daten
        .get(ohne_fuell.len()..)
        .is_some_and(|rest| rest.iter().any(|b| *b != b'='))
    {
        return Err(Error::Malformed("armor: data after padding"));
    }
    if ohne_fuell.len() % 4 == 1 {
        return Err(Error::Malformed("armor: bad base64 length"));
    }

    let mut aus = Vec::with_capacity((ohne_fuell.len() / 4).saturating_mul(3));
    for block in ohne_fuell.chunks(4) {
        let mut vier = 0_u32;
        for (i, z) in block.iter().enumerate() {
            let w = wert(*z).ok_or(Error::Malformed("armor: bad base64 character"))?;
            let schub = u32::try_from(18_usize.saturating_sub(i.saturating_mul(6))).unwrap_or(0);
            vier |= w << schub;
        }
        // Aus n Zeichen entstehen n-1 Bytes.
        for i in 0..block.len().saturating_sub(1) {
            let schub = u32::try_from(16_usize.saturating_sub(i.saturating_mul(8))).unwrap_or(0);
            let b = u8::try_from((vier >> schub) & 0xFF).unwrap_or(0);
            aus.push(b);
        }
    }
    Ok(aus)
}

#[cfg(test)]
#[expect(clippy::expect_used, reason = "Fehlschlag soll den Test abbrechen")]
mod tests {
    use super::{FUSS, KOPF, decode, encode, looks_like_armor};

    #[test]
    fn was_hineingeht_kommt_heraus() {
        for laenge in [0_usize, 1, 2, 3, 4, 5, 63, 64, 65, 1000] {
            let daten: Vec<u8> = (0..laenge)
                .map(|i| u8::try_from(i % 251).unwrap_or(0))
                .collect();
            let text = encode(&daten);
            assert_eq!(decode(&text).expect("lesbar"), daten, "Länge {laenge}");
        }
    }

    #[test]
    fn die_rahmenzeilen_stehen_da() {
        let text = encode(b"etwas");
        assert!(text.starts_with(KOPF));
        assert!(text.ends_with(FUSS));
    }

    #[test]
    fn keine_zeile_ist_laenger_als_vierundsechzig() {
        // Sonst bricht mancher Mailversand sie selbst um -- an einer
        // Stelle, die er sich aussucht.
        let text = encode(&vec![0x5A; 500]);
        for zeile in text.lines() {
            assert!(zeile.len() <= 64, "zu lang: {}", zeile.len());
        }
    }

    #[test]
    fn kopiertes_mit_zitatzeichen_und_einrueckung_geht_auch() {
        // Der Normalfall beim Herauskopieren aus einer E-Mail. Eine
        // Zurückweisung wäre Schikane ohne Sicherheitsgewinn.
        let daten = vec![0x11_u8; 200];
        let sauber = encode(&daten);
        let zerzaust: String = sauber.lines().map(|z| format!("  > {z}  \r\n")).collect();

        assert_eq!(decode(&zerzaust).expect("lesbar"), daten);
    }

    #[test]
    fn text_um_den_envelope_herum_stoert_nicht() {
        // „Hallo, hier ist die Datei: ..." -- so kommt es an.
        let daten = b"geheim".to_vec();
        let text = format!("Guten Tag,\n\n{}\n\nViele Gruesse", encode(&daten));

        assert_eq!(decode(&text).expect("lesbar"), daten);
    }

    #[test]
    fn ohne_rahmen_gibt_es_nichts() {
        assert!(decode("nur ein Satz").is_err());
        assert!(decode(&format!("{KOPF}\nAAAA")).is_err());
    }

    #[test]
    fn fremde_zeichen_im_inhalt_werden_abgelehnt() {
        // Nicht stillschweigend uebergehen: Ein Envelope mit einem
        // verschluckten Zeichen waere kaputt, und der Fehler faellt dann
        // erst beim Entschluesseln auf.
        let text = format!("{KOPF}\nAAAA§AAA\n{FUSS}");
        assert!(decode(&text).is_err());
    }

    #[test]
    fn erkannt_wird_ohne_zu_entschluesseln() {
        assert!(looks_like_armor(&encode(b"x")));
        assert!(!looks_like_armor("ein gewoehnlicher Satz"));
        assert!(!looks_like_armor(KOPF), "ohne Fuss ist es keiner");
    }

    #[test]
    fn das_ergebnis_stimmt_mit_bekannten_werten_ueberein() {
        // Gegen eine fremde Umsetzung gehalten, nicht gegen die eigene:
        // Ein Rundweg allein bewiese nur, dass wir zu uns selbst passen.
        let hole = |s: &str| {
            let t = encode(s.as_bytes());
            t.lines().nth(1).unwrap_or("").to_owned()
        };
        assert_eq!(hole("f"), "Zg==");
        assert_eq!(hole("fo"), "Zm8=");
        assert_eq!(hole("foo"), "Zm9v");
        assert_eq!(hole("foob"), "Zm9vYg==");
        assert_eq!(hole("fooba"), "Zm9vYmE=");
        assert_eq!(hole("foobar"), "Zm9vYmFy");
    }
}
