//! Erzeugt den Startkorpus für das Fuzzing.
//!
//! Ein Fuzzer, der bei null anfängt, verbringt Stunden damit, überhaupt
//! einen gültigen Kopf zu erraten. Mit ein paar echten Dateien als Saat
//! fängt er dort an, wo es interessant wird.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use cabrik_core::envelope::{self, SealOptions};
use cabrik_core::rng::OsRandom;
use cabrik_core::suite::Suite;
use cabrik_core::{Identity, kem};
use std::path::PathBuf;

fn ordner(ziel: &str) -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testvectors/fuzz")
        .join(ziel);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// Schreibt nur, was noch nicht da ist.
///
/// Ein Envelope enthält Zufallswerte — jeder Lauf ergäbe andere Bytes. Der
/// Korpus soll aber ein **stabiler Bestand** sein: Er wandert mit ins
/// Verzeichnis, wird vom Fuzzer erweitert und von den Korpus-Tests bei jedem
/// Lauf erneut geprüft. Dateien, die sich bei jedem `cargo test` ändern,
/// wären dafür unbrauchbar.
fn lege_ab(ziel: &str, name: &str, daten: &[u8]) {
    let pfad = ordner(ziel).join(name);
    if pfad.exists() {
        return;
    }
    std::fs::write(pfad, daten).unwrap();
}

#[test]
fn erzeuge_startkorpus() {
    let id = Identity::generate(&mut OsRandom, true, 1_700_000_000).unwrap();
    let pk = kem::public_key(&id.enc_sk).unwrap();

    // Ein gewöhnlicher Envelope.
    let einfach = envelope::seal(
        Suite::Classical,
        &[&pk[..]],
        None,
        b"Startkorpus",
        None,
        &SealOptions::default(),
        &mut OsRandom,
    )
    .unwrap();
    lege_ab("envelope", "einfach.env", &einfach);

    // Mit Signatur.
    let signiert = envelope::seal(
        Suite::Classical,
        &[&pk[..]],
        None,
        b"signiert",
        Some(&id),
        &SealOptions::default(),
        &mut OsRandom,
    )
    .unwrap();
    lege_ab("envelope", "signiert.env", &signiert);

    // Post-Quantum-Suite: andere Kapselgrößen.
    let pq_pk = kem::pq_public_key(&id.pq_seed);
    let pq = envelope::seal(
        Suite::Hybrid,
        &[&pq_pk[..]],
        None,
        b"pq",
        None,
        &SealOptions::default(),
        &mut OsRandom,
    );
    if let Ok(v) = pq {
        lege_ab("envelope", "hybrid.env", &v);
    }

    // Passwort.
    let passwort = envelope::seal(
        Suite::Classical,
        &[],
        Some(b"geheim"),
        b"per Passwort",
        None,
        &SealOptions::default(),
        &mut OsRandom,
    )
    .unwrap();
    lege_ab("envelope", "passwort.env", &passwort);

    // Über eine Blockgrenze hinaus: der Strom hat dann mehrere Abschnitte.
    let gross = vec![0x5Au8; 200_000];
    let mehrblock = envelope::seal(
        Suite::Classical,
        &[&pk[..]],
        None,
        &gross,
        None,
        &SealOptions::default(),
        &mut OsRandom,
    )
    .unwrap();
    lege_ab("envelope", "mehrere_bloecke.env", &mehrblock);

    // Kurze Sonderfälle, die kein Fuzzer von selbst findet.
    lege_ab("envelope", "leer.bin", b"");
    lege_ab(
        "envelope",
        "nur_magie.bin",
        &einfach[..einfach.len().min(8)],
    );
    lege_ab("envelope", "halb.bin", &einfach[..einfach.len() / 2]);
}
