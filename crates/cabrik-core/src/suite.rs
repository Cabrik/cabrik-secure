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
///
/// Reserviert. Wird in Schritt 2.6 implementiert; bis dahin lehnt
/// [`Suite::from_id`] sie ab.
pub const SUITE_HYBRID: u16 = 0x0002;

/// Unterstützte Ciphersuite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Suite {
    /// `0x0001` — klassisch, Voreinstellung.
    Classical,
}

impl Suite {
    /// Kennung im Prolog.
    #[must_use]
    pub const fn id(self) -> u16 {
        match self {
            Self::Classical => SUITE_CLASSICAL,
        }
    }

    /// Länge der KEM-Kapsel `enc` in Bytes.
    #[must_use]
    pub const fn enc_len(self) -> usize {
        match self {
            Self::Classical => 32,
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
        }
    }

    /// Liest eine Suite-Kennung.
    ///
    /// # Fehler
    ///
    /// [`Error::UnsupportedSuite`] bei jeder Kennung, die dieser Build nicht
    /// beherrscht — einschließlich [`SUITE_HYBRID`], solange Schritt 2.6
    /// aussteht. Eine Datei, die man nicht sicher verarbeiten kann, wird
    /// abgelehnt und nicht halb gelesen.
    pub const fn from_id(id: u16) -> Result<Self> {
        match id {
            SUITE_CLASSICAL => Ok(Self::Classical),
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
        assert_eq!(Suite::Classical.stanza_len(), 80);
        // 32 Bytes enc + 32 Bytes CEK + 16 Bytes Tag
        assert_eq!(Suite::Classical.stanza_len(), 32 + 32 + 16);
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
    fn hybrid_ist_reserviert_aber_noch_nicht_verfuegbar() {
        // Schritt 2.6. Bis dahin ist Ablehnung das richtige Verhalten --
        // eine halb unterstuetzte Suite waere schlimmer als keine.
        assert_eq!(
            Suite::from_id(SUITE_HYBRID).unwrap_err().code(),
            "UNSUPPORTED_SUITE"
        );
    }

    #[test]
    fn info_bindet_suite_und_kontext() {
        let info = Suite::Classical.hpke_info();
        assert_eq!(&info[..18], b"cabrik-envelope-v2");
        assert_eq!(&info[18..], &[0x00, 0x01]);
        assert_eq!(info.len(), 20);
    }
}
