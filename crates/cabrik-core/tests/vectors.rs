//! Prüft die Implementierung gegen die Vektordateien unter `testvectors/`.
//!
//! Diese Tests sind der Grund, warum die Vektoren vor dem Code entstanden
//! sind: Sie prüfen gegen die Spezifikation, nicht gegen das, was der Code
//! zufällig tut. Später prüfen dieselben Dateien die Swift- und
//! Kotlin-Implementierungen.

// In Tests sind `unwrap`, `expect` und `panic` erwünscht: ein Fehlschlag
// *soll* den Test abbrechen, mit einer Meldung, die den Vektor benennt.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use cabrik_core::fingerprint::{Fingerprint, safety_number};
use cabrik_core::padme::{padding_len, padme};

fn vector_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR zeigt auf crates/cabrik-core.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("testvectors")
}

fn load(name: &str) -> serde_json::Value {
    let path = vector_dir().join(name);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Vektordatei {} nicht lesbar: {e}", path.display()));
    serde_json::from_str(&raw).unwrap_or_else(|e| {
        panic!(
            "Vektordatei {} ist kein gueltiges JSON: {e}",
            path.display()
        )
    })
}

#[test]
fn padme_vektoren() {
    let doc = load("padme.json");

    assert_eq!(doc["spec_version"], "2.0");
    assert_eq!(doc["kind"], "padme");

    let vectors = doc["vectors"].as_array().expect("vectors ist kein Array");
    assert!(!vectors.is_empty(), "Vektordatei ist leer");

    let mut geprueft = 0;
    for v in vectors {
        let id = v["id"].as_str().expect("id fehlt");
        let input = v["input"]["plaintext_size"]
            .as_u64()
            .expect("plaintext_size fehlt");
        let erwartet_padded = v["expected"]["padded_size"]
            .as_u64()
            .expect("padded_size fehlt");
        let erwartet_pad = v["expected"]["padding_len"]
            .as_u64()
            .expect("padding_len fehlt");

        let got_padded = padme(input).unwrap_or_else(|e| panic!("{id}: padme schlug fehl: {e}"));
        let got_pad =
            padding_len(input).unwrap_or_else(|e| panic!("{id}: padding_len schlug fehl: {e}"));

        assert_eq!(
            got_padded, erwartet_padded,
            "{id}: PADME({input}) ergab {got_padded}, erwartet {erwartet_padded}"
        );
        assert_eq!(
            got_pad, erwartet_pad,
            "{id}: padding_len({input}) ergab {got_pad}, erwartet {erwartet_pad}"
        );
        geprueft += 1;
    }

    assert!(
        geprueft >= 16,
        "nur {geprueft} Vektoren geprueft — Datei unvollstaendig?"
    );
}

/// Baut einen Schluessel aus Wiederholungen eines Bytes, wie in der
/// Vektordatei beschrieben.
fn key<const N: usize>(v: &serde_json::Value, feld: &str, vorhanden: bool) -> Option<[u8; N]> {
    if !vorhanden {
        return None;
    }
    let byte = u8::try_from(v[feld].as_u64().expect("Byteangabe fehlt")).expect("Byte zu gross");
    Some([byte; N])
}

#[test]
fn fingerprint_vektoren() {
    let doc = load("fingerprint.json");
    assert_eq!(doc["kind"], "fingerprint");

    let vectors = doc["fingerprints"]
        .as_array()
        .expect("fingerprints ist kein Array");

    let mut nach_id = std::collections::HashMap::new();

    for v in vectors {
        let id = v["id"].as_str().expect("id fehlt");
        let input = &v["input"];

        let enc: [u8; 32] = key(input, "enc_pub_byte", true).expect("enc_pub fehlt");
        let sig: Option<[u8; 32]> = key(
            input,
            "sig_pub_byte",
            input["has_sig"].as_bool().expect("has_sig fehlt"),
        );
        let mlkem: Option<[u8; 1184]> = key(
            input,
            "mlkem_pub_byte",
            input["has_mlkem"].as_bool().expect("has_mlkem fehlt"),
        );

        let fp = Fingerprint::compute(&enc, sig.as_ref(), mlkem.as_ref());
        let erwartet = v["expected"]["fingerprint_hex"]
            .as_str()
            .expect("fingerprint_hex fehlt");

        let got: String = fp.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(got, erwartet, "{id}: Fingerprint weicht ab");

        assert_eq!(
            fp.display_full(),
            v["expected"]["display_full"].as_str().unwrap(),
            "{id}: display_full weicht ab"
        );
        assert_eq!(
            fp.display(),
            v["expected"]["display"].as_str().unwrap(),
            "{id}: display weicht ab"
        );
        assert_eq!(
            fp.short(),
            v["expected"]["short"].as_str().unwrap(),
            "{id}: short weicht ab"
        );

        nach_id.insert(id.to_owned(), fp);
    }

    // Der Angriffsfall aus spec/trust-store.md §2.1.
    assert_ne!(
        nach_id["fp-neither"], nach_id["fp-zero-sig"],
        "Praesenz-Byte fuer sig_pub ist wirkungslos"
    );
    assert_ne!(
        nach_id["fp-neither"], nach_id["fp-zero-mlkem"],
        "Praesenz-Byte fuer mlkem_pub ist wirkungslos"
    );

    // Safety Numbers
    let sn_vectors = doc["safety_numbers"]
        .as_array()
        .expect("safety_numbers ist kein Array");
    assert!(!sn_vectors.is_empty());

    for v in sn_vectors {
        let id = v["id"].as_str().expect("id fehlt");
        let a = fp_from_hex(v["input"]["fingerprint_a"].as_str().unwrap());
        let b = fp_from_hex(v["input"]["fingerprint_b"].as_str().unwrap());
        let erwartet = v["expected"]["safety_number"].as_str().unwrap();

        assert_eq!(
            safety_number(&a, &b),
            erwartet,
            "{id}: Safety Number weicht ab"
        );
        assert_eq!(
            safety_number(&b, &a),
            erwartet,
            "{id}: Safety Number ist reihenfolgeabhaengig"
        );
    }
}

/// Erzeugt einen Fingerprint direkt aus rohen Bytes — nur fuer Tests, damit
/// Safety-Number-Vektoren unabhaengig von der Fingerprint-Berechnung pruefbar
/// sind.
fn fp_from_hex(hex: &str) -> Fingerprint {
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("kein Hex"))
        .collect();
    let arr: [u8; 32] = bytes.try_into().expect("Fingerprint ist nicht 32 Bytes");
    Fingerprint::from_bytes(arr)
}
