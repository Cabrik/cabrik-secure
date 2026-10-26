//! Lesen und Migrieren von v1-Keyfiles (`spec/keyfile-v2.md` §5).
//!
//! v1-Keyfiles sind JSON:
//!
//! ```json
//! {
//!   "version": 1, "branding": "Cabrik Secure",
//!   "salt": "…16 Bytes…",
//!   "enc_pub": "…", "sig_pub": "…" | null,
//!   "enc_priv": { "nonce": "…24 Bytes…", "ciphertext": "…" },
//!   "sig_priv": { "nonce": … | null, "ciphertext": … | null }
//! }
//! ```
//!
//! Die öffentlichen Schlüssel stehen darin **im Klartext** — genau der Grund,
//! warum v2 sie nicht mehr speichert (`spec/keyfile-v2.md` §1).

use crate::{b64_decode, json_str};
use cabrik_core::keyfile::{self, Identity, KdfParams, Speichergrenze};
use cabrik_core::rng::Randomness;
use cabrik_core::{Error, Result};

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde_json::Value;
use zeroize::Zeroize;

/// Argon2id-Parameter, die libsodiums `MODERATE` entsprechen.
///
/// v1 rief `argon2id.kdf(..., opslimit=OPSLIMIT_MODERATE,
/// memlimit=MEMLIMIT_MODERATE)`. Das sind 3 Durchgänge und 256 MiB, mit
/// Parallelität 1 — libsodium führt Argon2id ausschließlich einspurig aus.
///
/// Zufällig liegen die Werte innerhalb der v2-Grenzen. Deshalb kann v1
/// denselben Ableitungsweg nehmen wie der Kern — mit fehlbar angelegtem
/// und hinterher überschriebenem Speicher (`cabrik_core::keyfile::argon2id`).
const V1_PARAMS: KdfParams = KdfParams {
    m_cost: 262_144,
    t_cost: 3,
    p_cost: 1,
};

/// Ob die Bytes wie ein v1-Keyfile aussehen.
///
/// v1-Keyfiles sind JSON und beginnen mit `{`; v2-Keyfiles mit den
/// Magic-Bytes `0xCA 0x4B`.
#[must_use]
pub fn looks_like_v1(data: &[u8]) -> bool {
    data.iter()
        .find(|b| !b.is_ascii_whitespace())
        .is_some_and(|&b| b == b'{')
}

/// Öffnet ein v1-Keyfile und gibt die enthaltenen privaten Schlüssel zurück.
///
/// Ergebnis: `(enc_sk, sig_sk)`. `sig_sk` fehlt bei Anonymitäts-Keyfiles.
///
/// # Fehler
///
/// - [`Error::Malformed`] bei kaputter Struktur
/// - [`Error::KeyfileAuthFailed`] bei falschem Passwort oder Manipulation
pub fn read_keys(data: &[u8], password: &[u8]) -> Result<([u8; 32], Option<[u8; 32]>)> {
    let doc: Value =
        serde_json::from_slice(data).map_err(|_| Error::Malformed("v1 keyfile: not valid JSON"))?;

    let version = doc
        .get("version")
        .and_then(Value::as_u64)
        .ok_or(Error::Malformed("v1 keyfile: version missing"))?;
    if version != 1 {
        return Err(Error::UnsupportedVersion);
    }
    let branding = json_str(&doc, "branding").unwrap_or("Cabrik Secure");

    let salt = b64_decode(json_str(&doc, "salt").ok_or(Error::Malformed("v1 keyfile: salt"))?)?;

    // v1 baute die AAD aus Version und Branding des Keyfiles selbst.
    let aad = format!("cabrik-keyfile|v{version}|{branding}").into_bytes();

    // Ohne Grenze: Die Werte stehen im Code, nicht in der Datei. Es gibt
    // keinen Absender, der sie hochtreiben koennte.
    let mut kek = [0u8; 32];
    keyfile::argon2id(
        password,
        &salt,
        &V1_PARAMS,
        Speichergrenze::UNBEKANNT,
        &mut kek,
    )?;

    let enc_sk = open_part(&doc, "enc_priv", &kek, &aad)?
        .ok_or(Error::Malformed("v1 keyfile: enc_priv missing"))?;
    let sig_sk = open_part(&doc, "sig_priv", &kek, &aad)?;
    kek.zeroize();

    Ok((enc_sk, sig_sk))
}

/// Entschlüsselt einen `{nonce, ciphertext}`-Block, falls vorhanden.
fn open_part(doc: &Value, feld: &str, kek: &[u8; 32], aad: &[u8]) -> Result<Option<[u8; 32]>> {
    let Some(teil) = doc.get(feld) else {
        return Ok(None);
    };
    let (Some(nonce_b64), Some(ct_b64)) = (json_str(teil, "nonce"), json_str(teil, "ciphertext"))
    else {
        // Bei Anonymitäts-Keyfiles stehen hier null-Werte.
        return Ok(None);
    };

    let nonce: [u8; 24] = b64_decode(nonce_b64)?
        .as_slice()
        .try_into()
        .map_err(|_| Error::Malformed("v1 keyfile: nonce is not 24 bytes"))?;
    let ct = b64_decode(ct_b64)?;

    let mut plain = XChaCha20Poly1305::new_from_slice(kek)
        .map_err(|_| Error::Malformed("v1 keyfile: bad key length"))?
        .decrypt(&XNonce::from(nonce), Payload { msg: &ct, aad })
        .map_err(|_| Error::KeyfileAuthFailed)?;

    let key: Result<[u8; 32]> = plain
        .as_slice()
        .try_into()
        .map_err(|_| Error::Malformed("v1 keyfile: private key is not 32 bytes"));
    plain.zeroize();
    key.map(Some)
}

/// Migriert ein v1-Keyfile zu einer v2-Identität.
///
/// Die X25519- und Ed25519-Schlüssel bleiben **unverändert**, damit
/// bestehende Kontaktbeziehungen und alte Envelopes gültig bleiben. Neu
/// erzeugt wird ausschließlich der Post-Quantum-Seed.
///
/// **Der Fingerprint ändert sich dadurch** (`spec/trust-store.md` §2.4).
/// Die Oberfläche muss darauf hinweisen und empfehlen, bestehende Gegenüber
/// einmalig neu zu verifizieren.
///
/// Das v1-Keyfile wird **nicht** gelöscht — ein fehlgeschlagener
/// Schreibvorgang darf nicht die einzige Kopie der Identität vernichten
/// (`spec/keyfile-v2.md` §5).
///
/// # Fehler
///
/// Wie [`read_keys`], plus Fehler der Zufallsquelle.
pub fn migrate<R: Randomness>(data: &[u8], password: &[u8], rng: &mut R) -> Result<Identity> {
    let (enc_sk, sig_sk) = read_keys(data, password)?;

    let doc: Value =
        serde_json::from_slice(data).map_err(|_| Error::Malformed("v1 keyfile: not valid JSON"))?;
    let created = doc.get("created").and_then(Value::as_u64).unwrap_or(0);

    let mut pq_seed = [0u8; 32];
    rng.fill(&mut pq_seed)?;

    Ok(Identity {
        enc_sk,
        sig_sk,
        pq_seed,
        created,
        label: None,
    })
}
