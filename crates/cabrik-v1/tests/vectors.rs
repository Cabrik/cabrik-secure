//! Prüft den v1-Leser gegen Vektoren aus der Referenzimplementierung.
//!
//! Erzeugt von `testvectors/tools/gen_v1_compat.py` mit `legacy/python-v1`.
//!
//! Nach dem bewährten Muster **zweimal aus verschiedenen Richtungen**:
//! einmal die kanonische JSON-Serialisierung einzeln gegen die von CPython
//! erzeugte AAD, einmal über den vollständigen Envelope. Einzeln geprüft
//! zeigt ein Fehler sofort, *wo* er sitzt; über das Ganze geprüft fällt auf,
//! was einzeln übersehen wurde.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use cabrik_core::rng::OsRandom;
use cabrik_v1::{canonical_json, envelope, keyfile};
use std::collections::HashMap;
use std::path::PathBuf;

fn load() -> serde_json::Value {
    let pfad = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testvectors/v1-compat.json");
    let raw = std::fs::read_to_string(&pfad)
        .unwrap_or_else(|e| panic!("{} nicht lesbar: {e}", pfad.display()));
    serde_json::from_str(&raw).expect("kein gueltiges JSON")
}

fn b64(s: &str) -> Vec<u8> {
    STANDARD.decode(s).expect("kein gueltiges Base64")
}

#[test]
fn v1_keyfiles_werden_gelesen_und_migriert() {
    let doc = load();
    let vectors = doc["keyfiles"].as_array().expect("keyfiles fehlt");
    assert!(!vectors.is_empty());

    for v in vectors {
        let id = v["id"].as_str().unwrap();
        let data = b64(v["input"]["keyfile_b64"].as_str().unwrap());
        let pw = v["input"]["password"].as_str().unwrap().as_bytes();

        assert!(keyfile::looks_like_v1(&data), "{id}: nicht als v1 erkannt");

        let (enc_sk, sig_sk) = keyfile::read_keys(&data, pw)
            .unwrap_or_else(|e| panic!("{id}: read_keys schlug fehl: {e}"));

        assert_eq!(
            enc_sk.to_vec(),
            b64(v["expected"]["enc_sk_b64"].as_str().unwrap()),
            "{id}: enc_sk weicht ab"
        );
        assert_eq!(
            sig_sk.map(|k| k.to_vec()),
            v["expected"]["sig_sk_b64"].as_str().map(b64),
            "{id}: sig_sk weicht ab"
        );

        // Falsches Passwort.
        assert_eq!(
            keyfile::read_keys(&data, b"falsch").unwrap_err().code(),
            "KEYFILE_AUTH_FAILED",
            "{id}: falsches Passwort wurde nicht erkannt"
        );

        // Migration erhaelt die klassischen Schluessel und erzeugt einen
        // frischen Post-Quantum-Seed.
        let ident = keyfile::migrate(&data, pw, &mut OsRandom).unwrap();
        assert_eq!(ident.enc_sk, enc_sk, "{id}: Migration aenderte enc_sk");
        assert_eq!(ident.sig_sk, sig_sk, "{id}: Migration aenderte sig_sk");
        assert_ne!(ident.pq_seed, [0u8; 32], "{id}: kein pq_seed erzeugt");

        let zweite = keyfile::migrate(&data, pw, &mut OsRandom).unwrap();
        assert_ne!(
            ident.pq_seed, zweite.pq_seed,
            "{id}: pq_seed ist nicht zufaellig"
        );
    }
}

/// Die kanonische AAD einzeln — ohne Kryptographie drumherum.
///
/// Schlaegt dieser Test fehl, liegt es an der Serialisierung. Schlaegt nur
/// der Envelope-Test fehl, liegt es woanders.
#[test]
fn kanonische_aad_stimmt_mit_cpython_ueberein() {
    let doc = load();
    let mut geprueft = 0;

    for v in doc["envelopes"].as_array().unwrap() {
        let id = v["id"].as_str().unwrap();
        let env_b64 = v["input"]["envelope_b64"].as_str().unwrap();
        let erwartet = v["expected"]["aad_utf8"].as_str().unwrap();

        let roh = b64(env_b64);
        let parsed: serde_json::Value = serde_json::from_slice(&roh).unwrap();
        let unsere = canonical_json::dumps(&parsed["header"]);

        assert_eq!(
            unsere, erwartet,
            "{id}: kanonische Serialisierung weicht von CPython ab"
        );
        geprueft += 1;
    }
    assert!(geprueft >= 5, "nur {geprueft} AADs geprueft");
}

#[test]
fn v1_envelopes_werden_gelesen() {
    let doc = load();

    // Empfaengerschluessel aus den Keyfile-Vektoren.
    let mut schluessel: HashMap<&str, [u8; 32]> = HashMap::new();
    for v in doc["keyfiles"].as_array().unwrap() {
        let id = v["id"].as_str().unwrap();
        let sk: [u8; 32] = b64(v["expected"]["enc_sk_b64"].as_str().unwrap())
            .try_into()
            .unwrap();
        schluessel.insert(id, sk);
    }

    for v in doc["envelopes"].as_array().unwrap() {
        let id = v["id"].as_str().unwrap();
        let env = v["input"]["envelope_b64"].as_str().unwrap().as_bytes();
        let sk = schluessel[v["input"]["recipient_keyfile"].as_str().unwrap()];

        assert!(envelope::looks_like_v1(env), "{id}: nicht als v1 erkannt");

        let auf = envelope::open(env, &sk, false)
            .unwrap_or_else(|e| panic!("{id}: open schlug fehl: {e}"));

        assert_eq!(
            auf.plaintext,
            b64(v["expected"]["plaintext_b64"].as_str().unwrap()),
            "{id}: Klartext weicht ab"
        );
        assert_eq!(
            auf.purpose,
            v["expected"]["purpose"].as_str().unwrap(),
            "{id}: purpose weicht ab"
        );
        assert_eq!(
            auf.signer.is_some(),
            v["expected"]["signed"].as_bool().unwrap(),
            "{id}: Signaturlage weicht ab"
        );

        // Falscher Empfaenger.
        let fremd = [0x42u8; 32];
        assert!(
            envelope::open(env, &fremd, false).is_err(),
            "{id}: fremder Schluessel wurde akzeptiert"
        );

        // Jede Einzelbyte-Aenderung am Base64 muss auffallen. Stichprobe,
        // weil jeder Versuch eine vollstaendige Entschluesselung ausloest.
        for i in [0, env.len() / 3, env.len() / 2, env.len() - 2] {
            let mut kaputt = env.to_vec();
            kaputt[i] = if kaputt[i] == b'A' { b'B' } else { b'A' };
            assert!(
                envelope::open(&kaputt, &sk, false).is_err(),
                "{id}: Aenderung an Byte {i} blieb unbemerkt"
            );
        }
    }
}

/// Der Kernbefund aus v1, jetzt als pruefbare Aussage.
#[test]
fn v1_gab_den_dateinamen_und_den_absender_preis() {
    let doc = load();
    let mut schluessel = [0u8; 32];
    for v in doc["keyfiles"].as_array().unwrap() {
        if v["id"] == "kf-v1-signing" {
            schluessel = b64(v["expected"]["enc_sk_b64"].as_str().unwrap())
                .try_into()
                .unwrap();
        }
    }

    for v in doc["envelopes"].as_array().unwrap() {
        let id = v["id"].as_str().unwrap();
        if id != "env-v1-file" {
            continue;
        }
        let env = v["input"]["envelope_b64"].as_str().unwrap().as_bytes();
        let auf = envelope::open(env, &schluessel, false).unwrap();

        assert_eq!(
            auf.warnings.filename_exposed.as_deref(),
            Some("Kuendigung_vertraulich.pdf"),
            "Dateiname haette als offengelegt gemeldet werden muessen"
        );
        assert!(auf.warnings.size_exposed.is_some());
        assert!(auf.warnings.timestamp_exposed.is_some());
        assert!(
            auf.warnings.sender_key_exposed,
            "Absenderschluessel lag offen und wurde nicht gemeldet"
        );
        assert_eq!(auf.warnings.product_named.as_deref(), Some("Cabrik Secure"));

        // Gegenprobe: der Name steht tatsaechlich unverschluesselt drin.
        let roh = b64(core::str::from_utf8(env).unwrap());
        let text = String::from_utf8_lossy(&roh);
        assert!(
            text.contains("Kuendigung_vertraulich.pdf"),
            "Erwartung verfehlt -- der Name sollte im Klartext stehen"
        );
    }
}

#[test]
fn signaturpflicht_wird_durchgesetzt() {
    let doc = load();
    let mut schluessel = [0u8; 32];
    for v in doc["keyfiles"].as_array().unwrap() {
        if v["id"] == "kf-v1-signing" {
            schluessel = b64(v["expected"]["enc_sk_b64"].as_str().unwrap())
                .try_into()
                .unwrap();
        }
    }

    for v in doc["envelopes"].as_array().unwrap() {
        let env = v["input"]["envelope_b64"].as_str().unwrap().as_bytes();
        let signiert = v["expected"]["signed"].as_bool().unwrap();
        let r = envelope::open(env, &schluessel, true);
        if signiert {
            assert!(r.is_ok(), "{}: signiert, aber abgelehnt", v["id"]);
        } else {
            assert_eq!(
                r.unwrap_err().code(),
                "SIGNATURE_MISSING",
                "{}: unsigniert, aber akzeptiert",
                v["id"]
            );
        }
    }
}

#[test]
fn v2_daten_werden_nicht_als_v1_erkannt() {
    // Magic-Bytes eines v2-Envelopes bzw. -Keyfiles.
    assert!(!envelope::looks_like_v1(&[0xCA, 0x02, 0x00, 0x01]));
    assert!(!keyfile::looks_like_v1(&[0xCA, 0x4B, 0x02]));
    assert!(!cabrik_v1::is_v1(&[0xCA, 0x02, 0x00, 0x01]));
}
