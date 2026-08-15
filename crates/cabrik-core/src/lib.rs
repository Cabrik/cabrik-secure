//! Kryptographischer Kern von Cabrik Secure.
//!
//! Diese Bibliothek setzt die Dokumente unter `spec/` um. Sie ist die einzige
//! Stelle, an der Schlüsselmaterial verarbeitet wird — Oberfläche und CLI
//! greifen ausschließlich hierauf zu.
//!
//! # Stand
//!
//! Schritt 2.4 der Roadmap: Hilfsfunktionen, TLV-Kodierung, Keyfile v2,
//! HPKE-Schlüsselverpackung und der Chunk-Stream. Die Zusammensetzung zum
//! vollständigen Envelope folgt in 2.5.
//!
//! # Grundsätze
//!
//! - Kein `unsafe`. Erzwungen über `unsafe_code = "forbid"`.
//! - Arithmetik, die überlaufen kann, wird geprüft statt gehofft:
//!   `clippy::arithmetic_side_effects` ist auf `deny`.
//! - Kein `unwrap`, kein `panic` außerhalb von Tests.
//! - Fehler benennen ihren Kode aus `spec/test-vectors.md` §7, damit alle
//!   Implementierungen dieselben Unterscheidungen treffen.
//! - Zufall wird **injiziert**, nie direkt vom Betriebssystem geholt — sonst
//!   sind bit-genaue Testvektoren später nicht nachrüstbar. Siehe [`rng`].
//! - Schlüsselmaterial wird zeroisiert und gibt sich in `Debug` nicht preis.
//! - Kein `std::io` in den Krypto-Pfaden. Der Kern soll später per UniFFI
//!   nach Swift und Kotlin; Dateizugriff gehört in die aufrufende Schicht.

pub mod armor;
pub mod base32;
pub mod envelope;
pub mod error;
pub mod fingerprint;
pub mod kem;
pub mod keyfile;
pub mod padme;
pub mod passwort;
pub mod rng;
pub mod stream;
pub mod suite;
pub mod tlv;
pub mod trust;
pub mod xwing;

pub use envelope::{ContentType, Opened, Opener, SealOptions, Signer};
pub use error::{Error, Result};
pub use fingerprint::{Fingerprint, safety_number};
pub use kem::Cek;
pub use keyfile::{Identity, KdfParams};
pub use padme::{PAD_MIN, padding_len, padme};
pub use rng::{OsRandom, Randomness};
pub use stream::{CHUNK_SIZE, StreamKey};
pub use suite::Suite;
pub use trust::{Authenticity, Contact, TrustState, TrustStore};

/// Version des Envelope-Formats, das dieser Kern schreibt.
pub const ENVELOPE_VERSION: u8 = 2;

/// Magic-Bytes eines v2-Envelopes (`spec/envelope-v2.md` §3).
pub const ENVELOPE_MAGIC: [u8; 2] = [0xCA, 0x02];

/// Magic-Bytes eines v2-Keyfiles (`spec/keyfile-v2.md` §2).
pub const KEYFILE_MAGIC: [u8; 2] = [0xCA, 0x4B];

#[cfg(test)]
mod zeroize_abdeckung {
    //! **Wer ein Geheimnis hält, überschreibt es beim Freigeben.**
    //!
    //! Diese Prüfung findet zur Übersetzungszeit statt: `belegt` nimmt nur
    //! Typen an, die [`zeroize::ZeroizeOnDrop`] erfüllen. Entfernt jemand
    //! später eine Ableitung, lässt sich der Test nicht mehr übersetzen.
    //!
    //! Das ist keine Vollständigkeitsgarantie — ein **neuer** Typ ohne
    //! Ableitung fällt hier nicht auf. Aber es hält fest, was einmal
    //! entschieden wurde, und genau daran scheitern Projekte sonst.

    use zeroize::ZeroizeOnDrop;

    const fn belegt<T: ZeroizeOnDrop>() {}

    #[test]
    fn alle_oeffentlichen_schluesseltypen_sind_abgedeckt() {
        belegt::<crate::kem::Cek>();
        belegt::<crate::stream::StreamKey>();
        belegt::<crate::trust::ContactsKey>();
        belegt::<crate::xwing::PrivateKey>();
        belegt::<crate::keyfile::Identity>();
    }

    /// Der Klartext ist kein Schlüssel, aber der eigentliche Gegenstand des
    /// Schutzes. Er steckt in [`zeroize::Zeroizing`], damit der Schutz auch
    /// dann mitwandert, wenn der Aufrufer den Puffer herausnimmt.
    #[test]
    fn der_klartext_traegt_seinen_schutz_mit_sich() {
        belegt::<zeroize::Zeroizing<Vec<u8>>>();

        // Ein Typwechsel am Feld würde hier auffallen.
        fn _pruefe(auf: &crate::Opened) -> &zeroize::Zeroizing<Vec<u8>> {
            &auf.plaintext
        }
    }
}
