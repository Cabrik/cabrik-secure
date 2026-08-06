//! Ciphersuites nach `spec/envelope-v2.md` §4.
//!
//! Eine Implementierung **muss** jede unbekannte `suite_id` ablehnen — auch
//! dann, wenn der Rest des Envelopes lesbar erscheint. Ein Format, das
//! Unbekanntes überliest, lässt sich herabstufen.

use crate::error::{Error, Result};

/// Kennung der klassischen Suite: DHKEM(X25519, HKDF-SHA256) +
/// HKDF-SHA256 + ChaCha20-Poly1305.
pub const SUITE_CLASSICAL: u16 = 0x0001;

/// Kennung der Post-Quantum-Suite: X-Wing (X25519 + ML-KEM-768).
pub const SUITE_HYBRID: u16 = 0x0002;

/// Unterstützte Ciphersuite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Suite {
    /// `0x0001` — klassisch, Voreinstellung.
    Classical,
    /// `0x0002` — Post-Quantum-Hybrid (X-Wing).
    ///
    /// Wehrt „heute mitschneiden, später entschlüsseln" ab. Nicht
    /// voreingestellt, weil ein X-Wing-Public-Key rund 1 620 Base64-Zeichen
    /// ergibt und den Austausch per Zwischenablage beendet
    /// (`spec/envelope-v2.md` §4.2).
    Hybrid,
}

impl Suite {
    /// Kennung im Prolog.
    #[must_use]
    pub const fn id(self) -> u16 {
        match self {
            Self::Classical => SUITE_CLASSICAL,
            Self::Hybrid => SUITE_HYBRID,
        }
    }

    /// Länge der KEM-Kapsel `enc` in Bytes.
    #[must_use]
    pub const fn enc_len(self) -> usize {
        match self {
            Self::Classical => 32,
            Self::Hybrid => 1120,
        }
    }

    /// Länge eines Empfänger-Public-Keys in Bytes.
    #[must_use]
    pub const fn pk_len(self) -> usize {
        match self {
            Self::Classical => 32,
            Self::Hybrid => 1216,
        }
    }

    /// Zufallsbedarf einer Kapselung in Bytes (`spec/envelope-v2.md` §11).
    #[must_use]
    pub const fn kem_randomness_len(self) -> usize {
        match self {
            // HPKE-`ikmE` für DHKEM(X25519).
            Self::Classical => 32,
            // X-Wing `eseed`: vordere 32 Bytes ML-KEM, hintere 32 X25519.
            Self::Hybrid => 64,
        }
    }

    /// Länge einer vollständigen HPKE-Kapsel: `enc` + gewrappter CEK.
    ///
    /// Der gewrappte CEK ist 48 Bytes lang — 32 Bytes Schlüssel plus
    /// 16 Bytes AEAD-Tag.
    #[must_use]
    pub const fn stanza_len(self) -> usize {
        match self {
            Self::Classical => 80,
            Self::Hybrid => 1168,
        }
    }

    /// Liest eine Suite-Kennung.
    ///
    /// # Fehler
    ///
    /// [`Error::UnsupportedSuite`] bei jeder unbekannten Kennung. Eine Datei,
    /// die man nicht sicher verarbeiten kann, wird abgelehnt und nicht halb
    /// gelesen.
    pub const fn from_id(id: u16) -> Result<Self> {
        match id {
            SUITE_CLASSICAL => Ok(Self::Classical),
            SUITE_HYBRID => Ok(Self::Hybrid),
            _ => Err(Error::UnsupportedSuite),
        }
    }

    /// `info`-Parameter für HPKE nach `spec/envelope-v2.md` §5.1.
    ///
    /// `"cabrik-envelope-v2" ‖ suite_id` — bindet Kontext und Suite in die
    /// Schlüsselableitung, sodass eine Kapsel nicht in einen anderen
    /// Verwendungszusammenhang übertragen werden kann.
    #[must_use]
    pub fn hpke_info(self) -> Vec<u8> {
        let mut info = Vec::with_capacity(20);
        info.extend_from_slice(b"cabrik-envelope-v2");
        info.extend_from_slice(&self.id().to_be_bytes());
        info
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;

    #[test]
    fn kennungen_entsprechen_der_spezifikation() {
        assert_eq!(Suite::Classical.id(), 0x0001);
        assert_eq!(Suite::Classical.enc_len(), 32);
        assert_eq!(Suite::Classical.pk_len(), 32);
        assert_eq!(Suite::Classical.kem_randomness_len(), 32);
        assert_eq!(Suite::Classical.stanza_len(), 80);
        // enc + 32 Bytes CEK + 16 Bytes Tag
        assert_eq!(Suite::Classical.stanza_len(), 32 + 32 + 16);

        assert_eq!(Suite::Hybrid.id(), 0x0002);
        assert_eq!(Suite::Hybrid.enc_len(), 1120);
        assert_eq!(Suite::Hybrid.pk_len(), 1216);
        assert_eq!(Suite::Hybrid.kem_randomness_len(), 64);
        assert_eq!(Suite::Hybrid.stanza_len(), 1168);
        assert_eq!(Suite::Hybrid.stanza_len(), 1120 + 32 + 16);
    }

    #[test]
    fn unbekannte_kennungen_werden_abgelehnt() {
        for id in [0x0000, 0x0003, 0x00FF, 0xFFFF] {
            assert_eq!(
                Suite::from_id(id).unwrap_err().code(),
                "UNSUPPORTED_SUITE",
                "Kennung {id:#06x} haette abgelehnt werden muessen"
            );
        }
    }

    #[test]
    fn beide_suiten_werden_erkannt() {
        assert_eq!(Suite::from_id(SUITE_CLASSICAL).unwrap(), Suite::Classical);
        assert_eq!(Suite::from_id(SUITE_HYBRID).unwrap(), Suite::Hybrid);
    }

    #[test]
    fn info_unterscheidet_die_suiten() {
        // Sonst waere eine Kapsel zwischen den Suiten uebertragbar.
        assert_ne!(Suite::Classical.hpke_info(), Suite::Hybrid.hpke_info());
    }

    #[test]
    fn info_bindet_suite_und_kontext() {
        let info = Suite::Classical.hpke_info();
        assert_eq!(&info[..18], b"cabrik-envelope-v2");
        assert_eq!(&info[18..], &[0x00, 0x01]);
        assert_eq!(info.len(), 20);
    }
}
