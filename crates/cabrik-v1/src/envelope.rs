//! Lesen von v1-Envelopes (`spec/envelope-v2.md` §13).
//!
//! Ein v1-Envelope ist Base64 über JSON:
//!
//! ```json
//! { "header": { … }, "nonce": "…", "ciphertext": "…", "signature": "…" }
//! ```
//!
//! # Was dabei sichtbar wird
//!
//! Der Header liegt **im Klartext**. Wer die Datei besitzt, liest ohne jeden
//! Schlüssel: Dateiname, Klartextgröße, Empfänger-Fingerprint, Zeitstempel,
//! verwendetes Programm — und bei signierten Nachrichten den dauerhaften
//! Signatur-Public-Key des Absenders.
//!
//! Genau das ist der Konstruktionsfehler, wegen dem v2 entstanden ist.
//! [`Warnings`] macht ihn dem Aufrufer zugänglich, damit die Oberfläche ihn
//! benennen kann.

use crate::canonical_json;
use crate::{b64_decode, json_str};
use cabrik_core::{Error, Result};

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use ed25519_dalek::{Signature, Verifier as _, VerifyingKey};
use hkdf::Hkdf;
use serde_json::Value;
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

/// Was ein v1-Envelope unverschlüsselt preisgibt.
///
/// Wird beim Lesen mitgeliefert, damit die Oberfläche dem Nutzer sagen kann,
/// was an dieser Nachricht offen lag.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Warnings {
    /// Der Dateiname stand im Klartext.
    pub filename_exposed: Option<String>,
    /// Die Klartextgröße stand im Klartext.
    pub size_exposed: Option<String>,
    /// Der Sendezeitpunkt stand im Klartext.
    pub timestamp_exposed: Option<u64>,
    /// Der dauerhafte Absenderschlüssel stand im Klartext.
    ///
    /// Das ist der schwerwiegendste Fall: Der ephemere Schlüsselaustausch
    /// macht den Absender unsichtbar, dieser Eintrag hebt das sofort wieder
    /// auf.
    pub sender_key_exposed: bool,
    /// Das verwendete Programm war erkennbar (`"branding"`).
    pub product_named: Option<String>,
}

/// Ergebnis des Lesens eines v1-Envelopes.
#[derive(Debug)]
pub struct OpenedV1 {
    /// Die Nutzdaten.
    pub plaintext: Vec<u8>,
    /// `"text"`, `"file"` oder `"msg"`.
    pub purpose: String,
    /// Gültige Signatur des Inhabers dieses Schlüssels, sofern vorhanden.
    ///
    /// Wie in v2 **kein Wahrheitswert**: Wer der Inhaber ist, entscheidet
    /// der Trust Store. Eine gültige v1-Signatur eines unbekannten
    /// Schlüssels ergibt „Unbekannt", nicht „Verifiziert"
    /// (`spec/envelope-v2.md` §13).
    pub signer: Option<[u8; 32]>,
    /// Was der Envelope offen preisgab.
    pub warnings: Warnings,
}

/// Ob die Bytes wie ein v1-Envelope aussehen.
///
/// v1-Envelopes sind Base64 über JSON und beginnen daher mit `eyJ` — der
/// Base64-Darstellung von `{"`. v2-Envelopes beginnen mit `0xCA 0x02`.
#[must_use]
pub fn looks_like_v1(data: &[u8]) -> bool {
    data.iter()
        .position(|b| !b.is_ascii_whitespace())
        .and_then(|i| data.get(i..i.saturating_add(3)))
        .is_some_and(|s| s == b"eyJ")
}

/// Öffnet einen v1-Envelope mit dem privaten X25519-Schlüssel des Empfängers.
///
/// # Fehler
///
/// - [`Error::Malformed`] bei kaputter Struktur
/// - [`Error::AuthFailed`] bei falschem Schlüssel oder Manipulation
/// - [`Error::SignatureInvalid`] bei ungültiger Signatur
/// - [`Error::SignatureMissing`], wenn `require_signature` gesetzt ist und
///   keine Signatur vorliegt
pub fn open(data: &[u8], enc_sk: &[u8; 32], require_signature: bool) -> Result<OpenedV1> {
    let text =
        core::str::from_utf8(data).map_err(|_| Error::Malformed("v1 envelope: not valid UTF-8"))?;
    let raw = b64_decode(text.trim())?;
    let doc: Value = serde_json::from_slice(&raw)
        .map_err(|_| Error::Malformed("v1 envelope: not valid JSON"))?;

    let header = doc
        .get("header")
        .ok_or(Error::Malformed("v1 envelope: header missing"))?;

    let version = header
        .get("version")
        .and_then(Value::as_u64)
        .ok_or(Error::Malformed("v1 envelope: version missing"))?;
    if version != 1 {
        return Err(Error::UnsupportedVersion);
    }

    let purpose = json_str(header, "purpose").unwrap_or("msg").to_owned();
    let eph_pub = b64_decode(
        json_str(header, "eph_pub").ok_or(Error::Malformed("v1 envelope: eph_pub missing"))?,
    )?;
    let hkdf_salt = b64_decode(
        json_str(header, "hkdf_salt").ok_or(Error::Malformed("v1 envelope: hkdf_salt missing"))?,
    )?;
    let nonce: [u8; 24] =
        b64_decode(json_str(&doc, "nonce").ok_or(Error::Malformed("v1 envelope: nonce missing"))?)?
            .as_slice()
            .try_into()
            .map_err(|_| Error::Malformed("v1 envelope: nonce is not 24 bytes"))?;
    let ciphertext = b64_decode(
        json_str(&doc, "ciphertext").ok_or(Error::Malformed("v1 envelope: ciphertext missing"))?,
    )?;

    let eph: [u8; 32] = eph_pub
        .as_slice()
        .try_into()
        .map_err(|_| Error::Malformed("v1 envelope: eph_pub is not 32 bytes"))?;

    // --- Sitzungsschlüssel -------------------------------------------------
    //
    // v1 bildete `info` aus der Version des Headers, aber den Algorithmus-
    // namen aus seinen eigenen Konstanten — nicht aus dem Header. Das wird
    // hier bewusst nachgebildet.
    let mut shared = x25519_dalek::x25519(*enc_sk, eph);
    if shared == [0u8; 32] {
        // libsodiums crypto_scalarmult lehnt das ab; x25519-dalek nicht.
        shared.zeroize();
        return Err(Error::AuthFailed);
    }

    let info = format!("cabrik|v{version}|X25519|XChaCha20-Poly1305|{purpose}");
    let mut sess_key = [0u8; 32];
    let hk = Hkdf::<Sha256>::new(Some(&hkdf_salt), &shared);
    shared.zeroize();
    hk.expand(info.as_bytes(), &mut sess_key)
        .map_err(|_| Error::Malformed("v1 envelope: hkdf failed"))?;

    // --- AAD ---------------------------------------------------------------
    //
    // Die AAD ist die KANONISCHE Serialisierung des Headers (sortierte
    // Schlüssel), während der Envelope selbst unsortiert serialisiert wurde.
    // Sie lässt sich daher nicht aus der Datei übernehmen.
    let aad = canonical_json::dumps(header);

    let plaintext = XChaCha20Poly1305::new_from_slice(&sess_key)
        .map_err(|_| Error::Malformed("v1 envelope: bad key length"))?
        .decrypt(
            &XNonce::from(nonce),
            Payload {
                msg: &ciphertext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| Error::AuthFailed);
    sess_key.zeroize();
    let plaintext = plaintext?;

    // --- Signatur ----------------------------------------------------------
    let signer = match (
        json_str(&doc, "signature"),
        json_str(header, "sender_sig_pub"),
    ) {
        (Some(sig_b64), Some(pk_b64)) => {
            let sig_bytes = b64_decode(sig_b64)?;
            let pk_bytes = b64_decode(pk_b64)?;
            let sig: [u8; 64] = sig_bytes
                .as_slice()
                .try_into()
                .map_err(|_| Error::Malformed("v1 envelope: signature is not 64 bytes"))?;
            let pk: [u8; 32] = pk_bytes
                .as_slice()
                .try_into()
                .map_err(|_| Error::Malformed("v1 envelope: sig key is not 32 bytes"))?;

            // v1 signierte über nonce ‖ ciphertext ‖ SHA-256(aad).
            let mut to_verify = Vec::new();
            to_verify.extend_from_slice(&nonce);
            to_verify.extend_from_slice(&ciphertext);
            to_verify.extend_from_slice(&Sha256::digest(aad.as_bytes()));

            VerifyingKey::from_bytes(&pk)
                .map_err(|_| Error::SignatureInvalid)?
                .verify(&to_verify, &Signature::from_bytes(&sig))
                .map_err(|_| Error::SignatureInvalid)?;
            Some(pk)
        }
        (Some(_), None) => {
            return Err(Error::Malformed(
                "v1 envelope: signature without sender_sig_pub",
            ));
        }
        _ => None,
    };

    if require_signature && signer.is_none() {
        return Err(Error::SignatureMissing);
    }

    // --- Was offen lag -----------------------------------------------------
    let meta = header.get("meta");
    let warnings = Warnings {
        filename_exposed: meta
            .and_then(|m| json_str(m, "filename"))
            .map(str::to_owned),
        size_exposed: meta.and_then(|m| json_str(m, "size")).map(str::to_owned),
        timestamp_exposed: header.get("ts").and_then(Value::as_u64),
        sender_key_exposed: json_str(header, "sender_sig_pub").is_some(),
        product_named: json_str(header, "branding").map(str::to_owned),
    };

    Ok(OpenedV1 {
        plaintext,
        purpose,
        signer,
        warnings,
    })
}
