//! Kryptographischer Kern von Cabrik Secure.
//!
//! Diese Bibliothek setzt die Dokumente unter `spec/` um. Sie ist die einzige
//! Stelle, an der Schlüsselmaterial verarbeitet wird — Oberfläche und CLI
//! greifen ausschließlich hierauf zu.
//!
//! # Stand
//!
//! Schritt 2.1 der Roadmap: Hilfsfunktionen ohne Kryptographie im engeren
//! Sinn. Envelope, Keyfile und Trust Store folgen in 2.2 bis 2.8.
//!
//! # Grundsätze
//!
//! - Kein `unsafe`. Erzwungen über `unsafe_code = "forbid"`.
//! - Arithmetik, die überlaufen kann, wird geprüft statt gehofft:
//!   `clippy::arithmetic_side_effects` ist auf `deny`.
//! - Kein `unwrap`, kein `panic` außerhalb von Tests.
//! - Fehler benennen ihren Kode aus `spec/test-vectors.md` §7, damit alle
//!   Implementierungen dieselben Unterscheidungen treffen.

pub mod base32;
pub mod error;
pub mod fingerprint;
pub mod padme;

pub use error::{Error, Result};
pub use fingerprint::{Fingerprint, safety_number};
pub use padme::{PAD_MIN, padding_len, padme};

/// Version des Envelope-Formats, das dieser Kern schreibt.
pub const ENVELOPE_VERSION: u8 = 2;

/// Magic-Bytes eines v2-Envelopes (`spec/envelope-v2.md` §3).
pub const ENVELOPE_MAGIC: [u8; 2] = [0xCA, 0x02];

/// Magic-Bytes eines v2-Keyfiles (`spec/keyfile-v2.md` §2).
pub const KEYFILE_MAGIC: [u8; 2] = [0xCA, 0x4B];

/// Klartextgröße eines Chunks in Bytes (`spec/envelope-v2.md` §8).
pub const CHUNK_SIZE: usize = 65_536;
