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

pub mod base32;
pub mod error;
pub mod fingerprint;
pub mod kem;
pub mod keyfile;
pub mod padme;
pub mod rng;
pub mod stream;
pub mod suite;
pub mod tlv;

pub use error::{Error, Result};
pub use fingerprint::{Fingerprint, safety_number};
pub use kem::Cek;
pub use keyfile::{Identity, KdfParams};
pub use padme::{PAD_MIN, padding_len, padme};
pub use rng::{OsRandom, Randomness};
pub use stream::{CHUNK_SIZE, StreamKey};
pub use suite::Suite;

/// Version des Envelope-Formats, das dieser Kern schreibt.
pub const ENVELOPE_VERSION: u8 = 2;

/// Magic-Bytes eines v2-Envelopes (`spec/envelope-v2.md` §3).
pub const ENVELOPE_MAGIC: [u8; 2] = [0xCA, 0x02];

/// Magic-Bytes eines v2-Keyfiles (`spec/keyfile-v2.md` §2).
pub const KEYFILE_MAGIC: [u8; 2] = [0xCA, 0x4B];
