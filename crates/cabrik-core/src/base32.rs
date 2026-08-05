//! Crockford-Base32 nach `spec/trust-store.md` §2.2.
//!
//! Für Fingerprints, die Menschen vergleichen und abtippen. Gegenüber
//! Standard-Base32 (RFC 4648) fehlen `I`, `L`, `O` und `U`:
//!
//! - `0`/`O` und `1`/`I`/`l` sind nicht mehr verwechselbar,
//! - `U` entfällt, damit keine anstößigen Zeichenfolgen entstehen.
//!
//! Beim Dekodieren wird großzügig ausgelegt: Kleinschreibung ist erlaubt,
//! `I`/`l` werden zu `1`, `O` zu `0`, und Trennzeichen werden übergangen.
//! Beim Kodieren dagegen entsteht immer dieselbe kanonische Form.

use crate::error::{Error, Result};

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Kodiert `data` als Crockford-Base32 ohne Trennzeichen.
///
/// Es wird nicht aufgefüllt: Die Ausgabe ist `ceil(bits / 5)` Zeichen lang.
#[must_use]
pub fn encode(data: &[u8]) -> String {
    let mut out = String::new();
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;

    for &byte in data {
        // Vor diesem Schritt trägt `acc` höchstens 4 gültige Bits — die
        // Schleife unten leert, bis `bits < 5`. Nach dem Schieben sind es
        // höchstens 12, `u32` reicht also mit großem Abstand. Die
        // `wrapping_*`-Form macht diese Zusicherung explizit, statt sie dem
        // Leser (und dem Lint) zu überlassen.
        acc = acc.wrapping_shl(8) | u32::from(byte);
        bits = bits.wrapping_add(8);

        while bits >= 5 {
            bits = bits.wrapping_sub(5);
            let idx = (acc.wrapping_shr(bits) & 0x1F) as usize;
            if let Some(&c) = ALPHABET.get(idx) {
                out.push(char::from(c));
            }
        }
    }

    if bits > 0 {
        // `bits` liegt hier in 1..=4, `shift` also in 1..=4.
        let shift = 5_u32.wrapping_sub(bits);
        let idx = (acc.wrapping_shl(shift) & 0x1F) as usize;
        if let Some(&c) = ALPHABET.get(idx) {
            out.push(char::from(c));
        }
    }
    out
}

/// Kodiert `data` und gruppiert die Ausgabe in Blöcke zu `group` Zeichen.
///
/// Gruppierung dient allein der Lesbarkeit; die Bindestriche gehören nicht
/// zum Wert und werden von [`decode`] übergangen.
///
/// # Beispiele
///
/// ```
/// use cabrik_core::base32::encode_grouped;
/// // 3 Bytes = 24 Bit = 5 Zeichen (ceil(24/5)), in Vierergruppen.
/// assert_eq!(encode_grouped(&[0xFF, 0xFF, 0xFF], 4), "ZZZZ-Y");
/// ```
#[must_use]
pub fn encode_grouped(data: &[u8], group: usize) -> String {
    let raw = encode(data);
    if group == 0 {
        return raw;
    }
    // Über Chunks statt über einen Modulo-Zähler: kürzer und ohne Arithmetik,
    // die auf Überlauf geprüft werden müsste.
    let chars: Vec<char> = raw.chars().collect();
    chars
        .chunks(group)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-")
}

/// Wert eines einzelnen Zeichens, oder `None` bei einem Trennzeichen.
fn value_of(ch: char) -> Result<Option<u8>> {
    // Bindestrich und Leerzeichen sind reine Lesehilfen.
    if ch == '-' || ch == ' ' {
        return Ok(None);
    }

    let up = u8::try_from(u32::from(ch.to_ascii_uppercase()))
        .map_err(|_| Error::Malformed("base32: non-ascii character"))?;

    // Crockfords Nachsicht gegenüber verwechselbaren Zeichen.
    let normalized = match up {
        b'O' => b'0',
        b'I' | b'L' => b'1',
        other => other,
    };

    // Die Position im Alphabet *ist* der Wert — auch für Ziffern. Damit
    // entfällt eine gesonderte Behandlung von '0'..='9' und die zugehörige
    // Subtraktion.
    let pos = ALPHABET
        .iter()
        .position(|&c| c == normalized)
        .ok_or(Error::Malformed("base32: invalid character"))?;

    u8::try_from(pos)
        .map(Some)
        .map_err(|_| Error::Malformed("base32: index out of range"))
}

/// Dekodiert Crockford-Base32 zu Bytes.
///
/// Groß- und Kleinschreibung sind gleichwertig, `-` und Leerzeichen werden
/// übergangen, `O`→`0` und `I`/`L`→`1` werden korrigiert.
///
/// # Fehler
///
/// [`Error::Malformed`] bei unbekannten Zeichen oder wenn die Zeichenzahl
/// keine ganze Byte-Folge ergibt.
pub fn decode(s: &str) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;

    for ch in s.chars() {
        let Some(v) = value_of(ch)? else { continue };
        // Vor diesem Schritt trägt `acc` höchstens 7 gültige Bits, danach
        // höchstens 12 — siehe die Begründung in `encode`.
        acc = acc.wrapping_shl(5) | u32::from(v);
        bits = bits.wrapping_add(5);

        if bits >= 8 {
            bits = bits.wrapping_sub(8);
            let byte = (acc.wrapping_shr(bits) & 0xFF) as u8;
            out.push(byte);
        }
    }

    // Übrig bleiben dürfen nur Füllbits, und die müssen null sein. Andernfalls
    // ergäben zwei verschiedene Zeichenketten dasselbe Ergebnis — bei einem
    // Fingerprint wäre das genau die Mehrdeutigkeit, die man nicht will.
    //
    // `bits` liegt hier in 0..=7, `wrapping_shl` ist also wohldefiniert.
    let rest_mask = 1_u32.wrapping_shl(bits).wrapping_sub(1);
    if bits >= 5 || (acc & rest_mask) != 0 {
        return Err(Error::Malformed("base32: non-canonical trailing bits"));
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn alphabet_entspricht_crockford() {
        assert_eq!(ALPHABET.len(), 32);
        for &verboten in b"ILOU" {
            assert!(
                !ALPHABET.contains(&verboten),
                "{} darf nicht im Alphabet stehen",
                char::from(verboten)
            );
        }
    }

    #[test]
    fn round_trip_fuer_zufaellige_laengen() {
        for len in 0..64_usize {
            let data: Vec<u8> = (0..len).map(|i| (i.wrapping_mul(37) % 256) as u8).collect();
            let encoded = encode(&data);
            assert_eq!(decode(&encoded).unwrap(), data, "Laenge {len}");
        }
    }

    #[test]
    fn zeichenzahl_fuer_256_bit_ist_52() {
        // spec/trust-store.md §2.2
        let fp = [0xABu8; 32];
        assert_eq!(encode(&fp).len(), 52);
    }

    #[test]
    fn mindestanzeige_von_32_zeichen_traegt_160_bit() {
        let fp = [0x5Au8; 32];
        let voll = encode(&fp);
        let anzeige: String = voll.chars().take(32).collect();
        assert_eq!(anzeige.len(), 32);
        assert_eq!(32 * 5, 160);
    }

    #[test]
    fn gruppierung_ist_nur_lesehilfe() {
        let data = [0x12, 0x34, 0x56, 0x78, 0x9A];
        let grouped = encode_grouped(&data, 4);
        assert!(grouped.contains('-'));
        assert_eq!(decode(&grouped).unwrap(), data);
        assert_eq!(grouped.replace('-', ""), encode(&data));
    }

    #[test]
    fn verwechselbare_zeichen_werden_korrigiert() {
        // O -> 0, I und L -> 1.
        //
        // Acht Zeichen ergeben genau 40 Bit = 5 Bytes. Kuerzere Eingaben
        // liessen Restbits uebrig und wuerden — zu Recht — als nicht
        // kanonisch abgelehnt.
        assert_eq!(decode("O0000000").unwrap(), decode("00000000").unwrap());
        assert_eq!(decode("I1111111").unwrap(), decode("11111111").unwrap());
        assert_eq!(decode("L1111111").unwrap(), decode("11111111").unwrap());
        assert_eq!(decode("oOiIlL00").unwrap(), decode("00111100").unwrap());
    }

    #[test]
    fn kleinschreibung_ist_gleichwertig() {
        let data = [0xDE, 0xAD, 0xBE, 0xEF];
        let up = encode(&data);
        assert_eq!(decode(&up.to_lowercase()).unwrap(), data);
    }

    #[test]
    fn ungueltige_zeichen_werden_abgelehnt() {
        for bad in ["U", "$", "ä", "Z!"] {
            assert!(
                decode(bad).is_err(),
                "{bad} haette abgelehnt werden muessen"
            );
        }
    }

    #[test]
    fn nicht_kanonische_restbits_werden_abgelehnt() {
        // "ZZ" = 10 Bits: 1 Byte plus 2 gesetzte Restbits — mehrdeutig.
        assert!(decode("ZZ").is_err());
    }
}
