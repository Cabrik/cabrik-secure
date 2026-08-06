//! Lesezugriff auf das alte Cabrik-Secure-Format v1.
//!
//! Diese Crate liest v1-Keyfiles und v1-Envelopes und migriert Identitäten
//! nach v2. Sie **schreibt niemals** v1 — `spec/envelope-v2.md` §13 und
//! `spec/keyfile-v2.md` §5.
//!
//! # Warum eigenständig
//!
//! v1 ist JSON über Base64. Beides wird sonst nirgends gebraucht, und
//! `cabrik-core` soll auditierbar bleiben und per UniFFI nach iOS und
//! Android gehen. Altlast gehört nicht in dieses Gepäck.
//!
//! Der Inhalt hier ist **eingefroren**: einmal geschrieben, gegen die
//! Referenzimplementierung geprüft, danach unverändert.
//!
//! # Was v1 preisgab
//!
//! Der Envelope-Header lag im Klartext. Wer die Datei besaß, las ohne jeden
//! Schlüssel Dateiname, Größe, Zeitstempel und — bei signierten Nachrichten —
//! die dauerhafte Absenderkennung. [`envelope::Warnings`] macht das für die
//! Oberfläche zugänglich, damit sie es benennen kann statt zu verschweigen.

pub mod canonical_json;
pub mod envelope;
pub mod keyfile;

pub use envelope::{OpenedV1, Warnings};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use cabrik_core::{Error, Result};
use serde_json::Value;

/// Dekodiert Base64 im Standardalphabet mit Auffüllung, wie Python es schreibt.
fn b64_decode(s: &str) -> Result<Vec<u8>> {
    STANDARD
        .decode(s)
        .map_err(|_| Error::Malformed("v1: invalid base64"))
}

/// Liest ein Zeichenkettenfeld, oder `None` bei `null` oder Abwesenheit.
fn json_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

/// Ob die Daten überhaupt nach v1 aussehen — Keyfile oder Envelope.
#[must_use]
pub fn is_v1(data: &[u8]) -> bool {
    keyfile::looks_like_v1(data) || envelope::looks_like_v1(data)
}
