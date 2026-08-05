//! Padmé-Längenauffüllung nach `spec/envelope-v2.md` §10.2.
//!
//! AEAD verbirgt den Inhalt, nicht die Länge. Bei kurzen Texten ist die Länge
//! oft schon die Nachricht: `Ja` (2 Bytes) und `Nein` (4 Bytes) lassen sich
//! ohne jeden Schlüssel unterscheiden.
//!
//! Padmé rundet auf Zahlen mit wenigen signifikanten Bits auf. Dadurch gibt es
//! bei kleinen Längen viele Größenklassen (wenig Verschnitt) und bei großen
//! wenige (starke Verschleierung), bei beschränktem relativem Verschnitt über
//! den gesamten Bereich.
//!
//! Quelle: Nikitin et al., *Reducing Metadata Leakage from Encrypted Files and
//! Communication with PURBs*, PETS 2019.

use crate::error::{Error, Result};

/// Untergrenze der Auffüllung in Bytes.
///
/// Ohne sie liefert die Formel für sehr kurze Eingaben die Identität —
/// `PADME(2) == 2` — und würde genau dort nicht schützen, wo Längenanalyse am
/// wirksamsten ist.
pub const PAD_MIN: u64 = 256;

/// Rundet `len` auf die nächste Padmé-Größenklasse auf.
///
/// # Fehler
///
/// [`Error::Malformed`] bei Überlauf, also für Längen nahe `u64::MAX`.
///
/// # Beispiele
///
/// ```
/// use cabrik_core::padme::padme;
/// assert_eq!(padme(100).unwrap(),   256);      // Untergrenze
/// assert_eq!(padme(1_000).unwrap(), 1_024);
/// assert_eq!(padme(1_025).unwrap(), 1_088);
/// ```
pub fn padme(len: u64) -> Result<u64> {
    if len <= PAD_MIN {
        return Ok(PAD_MIN);
    }

    // floor(log2(x)) als reine Ganzzahloperation.
    //
    // Die Spezifikation schreibt das ausdrücklich vor: Gleitkomma-log2 liefert
    // an Zweierpotenzen je nach Plattform und Optimierungsstufe unterschiedliche
    // Ergebnisse. Ein einziger Grenzfall, der auf Desktop und iOS verschieden
    // ausfällt, würde bit-genaue Envelopes unmöglich machen.
    let e = u64::from(len.ilog2());

    // floor(log2(e)) + 1 — die Zahl der Bits, um e darzustellen.
    // e >= 8, weil len > PAD_MIN == 2^8; ilog2 ist damit definiert.
    let s = u64::from(e.ilog2())
        .checked_add(1)
        .ok_or(Error::Malformed("padme: s overflow"))?;

    // Für len > PAD_MIN gilt e >= 8 > s, die Subtraktion ist also sicher.
    // checked_sub statt Kommentar, damit die Annahme geprüft und nicht geglaubt wird.
    let z = e
        .checked_sub(s)
        .ok_or(Error::Malformed("padme: z underflow"))?;

    let mask = 1u64
        .checked_shl(u32::try_from(z).map_err(|_| Error::Malformed("padme: z too large"))?)
        .ok_or(Error::Malformed("padme: shift overflow"))?
        .wrapping_sub(1);

    let rounded = len
        .checked_add(mask)
        .ok_or(Error::Malformed("padme: length overflow"))?
        & !mask;

    Ok(rounded)
}

/// Zahl der anzuhängenden Füllbytes für `len`.
///
/// # Fehler
///
/// Wie [`padme`].
pub fn padding_len(len: u64) -> Result<u64> {
    let padded = padme(len)?;
    padded
        .checked_sub(len)
        .ok_or(Error::Malformed("padme: negative padding"))
}

#[cfg(test)]
// In Tests ist `unwrap` erwünscht: ein Fehlschlag *soll* den Test abbrechen.
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    #[test]
    fn beispiele_aus_der_spezifikation() {
        // spec/envelope-v2.md §10.2
        assert_eq!(padme(100).unwrap(), 256);
        assert_eq!(padme(1_000).unwrap(), 1_024);
        assert_eq!(padme(1_025).unwrap(), 1_088);
        assert_eq!(padme(10_000).unwrap(), 10_240);
        assert_eq!(padme(1_000_000).unwrap(), 1_015_808);
        assert_eq!(padme(10_000_000).unwrap(), 10_223_616);
    }

    #[test]
    fn ergebnis_ist_nie_kleiner_als_die_eingabe() {
        for len in (0..100_000).step_by(97) {
            assert!(padme(len).unwrap() >= len, "fehlgeschlagen bei {len}");
        }
    }

    #[test]
    fn verschnitt_bleibt_unter_6_25_prozent() {
        // spec/envelope-v2.md §10.2: wegen PAD_MIN = 256 ist E >= 8 und
        // damit S >= 4, der Verschnitt also hoechstens 2^-4.
        for len in PAD_MIN.checked_add(1).unwrap()..400_000 {
            let padded = padme(len).unwrap();
            let overhead = padded.checked_sub(len).unwrap();
            assert!(
                overhead.checked_mul(16).unwrap() <= len,
                "Verschnitt {overhead} bei L={len} ueberschreitet 6,25 %"
            );
        }
    }

    #[test]
    fn ist_idempotent() {
        // Ein bereits ausgerichteter Wert darf nicht weiter wachsen.
        for len in (PAD_MIN..200_000).step_by(89) {
            let once = padme(len).unwrap();
            assert_eq!(padme(once).unwrap(), once, "nicht idempotent bei {len}");
        }
    }

    #[test]
    fn ist_monoton() {
        let mut prev = 0;
        for len in (0..200_000).step_by(13) {
            let p = padme(len).unwrap();
            assert!(p >= prev, "nicht monoton bei {len}");
            prev = p;
        }
    }

    #[test]
    fn ueberlauf_wird_erkannt_statt_zu_paniken() {
        let r = padme(u64::MAX);
        assert!(r.is_err());
        assert_eq!(r.unwrap_err().code(), "MALFORMED");
    }

    #[test]
    fn padding_len_ergaenzt_zur_klassengroesse() {
        for len in (0..50_000).step_by(211) {
            let pad = padding_len(len).unwrap();
            assert_eq!(len.checked_add(pad).unwrap(), padme(len).unwrap());
        }
    }
}
