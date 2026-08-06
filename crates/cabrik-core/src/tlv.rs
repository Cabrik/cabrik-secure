//! TLV-Kodierung nach `spec/envelope-v2.md` §7.2.
//!
//! ```text
//! type   : u8
//! length : u16 BE
//! value  : length Bytes
//! ```
//!
//! Verwendet vom verschlüsselten Header, vom Keyfile und vom Trust Store.
//!
//! # Strenge ist hier Sicherheitseigenschaft, nicht Pedanterie
//!
//! - Felder **müssen** in aufsteigender Typreihenfolge stehen.
//! - Jeder Typ darf **höchstens einmal** vorkommen.
//! - Ein unbekannter Typ führt zu [`Error::Malformed`] — es gibt kein
//!   Überlesen.
//!
//! Die ersten beiden Regeln machen die Kodierung **kanonisch**: Zu jedem
//! Feldsatz gibt es genau eine gültige Bytefolge. Ohne sie könnte ein
//! Angreifer denselben Inhalt unterschiedlich kodieren — und da diese Bytes
//! in Signaturen und AEAD-AAD eingehen, wäre das ein Weg, an einer Prüfung
//! vorbeizukommen.
//!
//! Die dritte Regel verhindert, dass eine Implementierung Felder ignoriert,
//! die eine andere auswertet. Neue Felder erfordern eine neue Formatversion.

use crate::error::{Error, Result};

/// Schreibt TLV-Felder in aufsteigender Typreihenfolge.
#[derive(Debug, Default)]
pub struct TlvWriter {
    buf: Vec<u8>,
    last_type: Option<u8>,
}

impl TlvWriter {
    /// Neuer, leerer Schreiber.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Hängt ein Feld an.
    ///
    /// # Fehler
    ///
    /// - [`Error::Malformed`], wenn `ty` nicht größer als der zuletzt
    ///   geschriebene Typ ist. Das fängt Programmierfehler an der Stelle ab,
    ///   an der sie entstehen, statt beim Leser.
    /// - [`Error::Malformed`], wenn `value` länger als 65 535 Bytes ist.
    pub fn push(&mut self, ty: u8, value: &[u8]) -> Result<()> {
        if let Some(last) = self.last_type
            && ty <= last
        {
            return Err(Error::Malformed("tlv: types must be strictly ascending"));
        }
        let len = u16::try_from(value.len())
            .map_err(|_| Error::Malformed("tlv: value exceeds 65535 bytes"))?;

        self.buf.push(ty);
        self.buf.extend_from_slice(&len.to_be_bytes());
        self.buf.extend_from_slice(value);
        self.last_type = Some(ty);
        Ok(())
    }

    /// Gibt die fertige Bytefolge zurück.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.buf
    }

    /// Bisher geschriebene Länge in Bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Ob noch kein Feld geschrieben wurde.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
}

/// Liest TLV-Felder und erzwingt dabei die Kanonizitätsregeln.
#[derive(Debug)]
pub struct TlvReader<'a> {
    data: &'a [u8],
    pos: usize,
    last_type: Option<u8>,
}

impl<'a> TlvReader<'a> {
    /// Neuer Leser über `data`.
    #[must_use]
    pub const fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            last_type: None,
        }
    }

    /// Liest das nächste Feld, oder `None` am Ende.
    ///
    /// # Fehler
    ///
    /// [`Error::Malformed`] bei abgeschnittenen Feldern, absteigender oder
    /// gleicher Typreihenfolge.
    pub fn next_field(&mut self) -> Result<Option<(u8, &'a [u8])>> {
        if self.pos >= self.data.len() {
            return Ok(None);
        }

        // Kopf: 1 Byte Typ + 2 Bytes Länge.
        let head_end = self
            .pos
            .checked_add(3)
            .ok_or(Error::Malformed("tlv: position overflow"))?;
        let head = self
            .data
            .get(self.pos..head_end)
            .ok_or(Error::Malformed("tlv: truncated header"))?;

        let ty = *head
            .first()
            .ok_or(Error::Malformed("tlv: truncated type"))?;
        let len_bytes: [u8; 2] = head
            .get(1..3)
            .and_then(|s| s.try_into().ok())
            .ok_or(Error::Malformed("tlv: truncated length"))?;
        let len = usize::from(u16::from_be_bytes(len_bytes));

        if let Some(last) = self.last_type
            && ty <= last
        {
            // Deckt beides ab: Duplikate und absteigende Reihenfolge.
            return Err(Error::Malformed("tlv: types must be strictly ascending"));
        }

        let value_end = head_end
            .checked_add(len)
            .ok_or(Error::Malformed("tlv: length overflow"))?;
        let value = self
            .data
            .get(head_end..value_end)
            .ok_or(Error::Malformed("tlv: truncated value"))?;

        self.pos = value_end;
        self.last_type = Some(ty);
        Ok(Some((ty, value)))
    }
}

/// Erwartet genau `N` Bytes und gibt sie als Array zurück.
///
/// # Fehler
///
/// [`Error::Malformed`] bei abweichender Länge.
pub fn expect_len<const N: usize>(value: &[u8], feld: &'static str) -> Result<[u8; N]> {
    value.try_into().map_err(|_| Error::Malformed(feld))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;

    fn schreibe(felder: &[(u8, &[u8])]) -> Vec<u8> {
        let mut w = TlvWriter::new();
        for (ty, v) in felder {
            w.push(*ty, v).unwrap();
        }
        w.finish()
    }

    fn lies(data: &[u8]) -> Result<Vec<(u8, Vec<u8>)>> {
        let mut r = TlvReader::new(data);
        let mut out = Vec::new();
        while let Some((ty, v)) = r.next_field()? {
            out.push((ty, v.to_vec()));
        }
        Ok(out)
    }

    #[test]
    fn round_trip() {
        let data = schreibe(&[(0x01, b"abc"), (0x02, b""), (0x05, b"laenger")]);
        let gelesen = lies(&data).unwrap();
        assert_eq!(gelesen.len(), 3);
        assert_eq!(gelesen[0], (0x01, b"abc".to_vec()));
        assert_eq!(gelesen[1], (0x02, Vec::new()));
        assert_eq!(gelesen[2], (0x05, b"laenger".to_vec()));
    }

    #[test]
    fn leere_eingabe_ergibt_keine_felder() {
        assert!(lies(&[]).unwrap().is_empty());
    }

    #[test]
    fn schreiber_lehnt_absteigende_reihenfolge_ab() {
        let mut w = TlvWriter::new();
        w.push(0x05, b"x").unwrap();
        assert!(w.push(0x02, b"y").is_err());
        assert!(
            w.push(0x05, b"y").is_err(),
            "Duplikat muss abgelehnt werden"
        );
    }

    #[test]
    fn leser_lehnt_duplikate_ab() {
        // Von Hand gebaut, weil der Schreiber das gar nicht erzeugen kann.
        let data = [0x01, 0x00, 0x01, 0xAA, 0x01, 0x00, 0x01, 0xBB];
        let e = lies(&data).unwrap_err();
        assert_eq!(e.code(), "MALFORMED");
    }

    #[test]
    fn leser_lehnt_absteigende_reihenfolge_ab() {
        let data = [0x05, 0x00, 0x01, 0xAA, 0x02, 0x00, 0x01, 0xBB];
        assert_eq!(lies(&data).unwrap_err().code(), "MALFORMED");
    }

    #[test]
    fn abgeschnittener_kopf_wird_erkannt() {
        for len in 1..3_usize {
            let data = vec![0x01u8; len];
            assert_eq!(
                lies(&data).unwrap_err().code(),
                "MALFORMED",
                "Kopf der Laenge {len} haette abgelehnt werden muessen"
            );
        }
    }

    #[test]
    fn abgeschnittener_wert_wird_erkannt() {
        // Kuendigt 4 Bytes an, liefert 2.
        let data = [0x01, 0x00, 0x04, 0xAA, 0xBB];
        assert_eq!(lies(&data).unwrap_err().code(), "MALFORMED");
    }

    #[test]
    fn uebergrosse_laengenangabe_reserviert_keinen_speicher() {
        // Kuendigt 65 535 Bytes an, liefert keine. Der Leser darf daran
        // nicht scheitern, sondern muss sauber MALFORMED melden -- und
        // vor allem nichts vorab reservieren.
        let data = [0x01, 0xFF, 0xFF];
        assert_eq!(lies(&data).unwrap_err().code(), "MALFORMED");
    }

    #[test]
    fn kodierung_ist_kanonisch() {
        // Derselbe Feldsatz ergibt immer dieselben Bytes.
        let a = schreibe(&[(0x01, b"x"), (0x03, b"y")]);
        let b = schreibe(&[(0x01, b"x"), (0x03, b"y")]);
        assert_eq!(a, b);
    }

    #[test]
    fn expect_len_prueft_genau() {
        assert!(expect_len::<3>(b"abc", "test").is_ok());
        assert!(expect_len::<3>(b"ab", "test").is_err());
        assert!(expect_len::<3>(b"abcd", "test").is_err());
    }

    #[test]
    fn maximale_feldlaenge_wird_durchgereicht() {
        let gross = vec![0x42u8; 65_535];
        let data = schreibe(&[(0x01, &gross)]);
        let gelesen = lies(&data).unwrap();
        assert_eq!(gelesen[0].1.len(), 65_535);
    }

    #[test]
    fn zu_langer_wert_wird_beim_schreiben_abgelehnt() {
        let zu_gross = vec![0u8; 65_536];
        let mut w = TlvWriter::new();
        assert!(w.push(0x01, &zu_gross).is_err());
    }
}
