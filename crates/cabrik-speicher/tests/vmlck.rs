//! Der Beweis von außen: Bestätigt der Kern selbst, dass er festnagelt?
//!
//! # Warum das nicht bei den anderen Tests steht
//!
//! Weil es der einzige Test ist, der eine **prozessweite** Zahl liest.
//! `VmLck` in `/proc/self/status` sagt, wie viel Speicher *dieser Prozess*
//! festgenagelt hat — nicht, wie viel dieser eine Puffer festgenagelt hat.
//!
//! `cargo test` führt die Tests einer Datei als Fäden desselben Prozesses
//! **gleichzeitig** aus. Zwischen zwei Messungen erzeugen und verwerfen
//! also ein Dutzend andere Tests ihre eigenen Puffer. Genau daran ist die
//! erste Fassung gescheitert: Sie stand bei den übrigen Tests, und die CI
//! meldete unter Linux „beim Wegwerfen nicht wieder geloest" — obwohl
//! gelöst wurde. Es war der Nachbar, dessen Seiten noch standen.
//!
//! Eine eigene Datei ist ein eigenes Testprogramm, und Cargo führt
//! Testprogramme nacheinander aus. Hier ist der Puffer der einzige.
//!
//! # Warum es diesen Test überhaupt gibt
//!
//! Ohne ihn bewiese die ganze Kiste nur, dass unsere eigene Funktion
//! `true` zurückgibt. Dass tatsächlich etwas festgenagelt wurde, kann uns
//! nur jemand bestätigen, der es wissen muss — der Kern.
//!
//! Windows kennt kein Gegenstück, das ohne weiteren Systemaufruf lesbar
//! wäre. Dort bleibt es beim Rückgabewert von `VirtualLock`.

#![cfg(target_os = "linux")]
#![expect(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "Fehlschlag soll den Test abbrechen"
)]

use cabrik_speicher::Festgenagelt;

/// Was der Kern über festgenagelten Speicher dieses Prozesses meldet, in KiB.
fn vmlck_kib() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    status
        .lines()
        .find_map(|zeile| zeile.strip_prefix("VmLck:"))
        .and_then(|rest| rest.trim().trim_end_matches(" kB").trim().parse().ok())
        .unwrap()
}

#[test]
fn der_kern_bestaetigt_das_festnageln_und_das_loesen() {
    let vorher = vmlck_kib();

    // 64 KiB: gross genug, dass der Zuwachs in einer KiB-genauen Zahl
    // eindeutig ist, klein genug fuer ein knappes `RLIMIT_MEMLOCK`.
    let puffer = Festgenagelt::neu(64 * 1024);
    assert!(
        puffer.ist_festgenagelt(),
        "schon unsere eigene Meldung sagt nein -- RLIMIT_MEMLOCK ansehen \
         (ulimit -l), dann erst diesen Test verdaechtigen"
    );

    let waehrend = vmlck_kib();
    assert!(
        waehrend >= vorher + 64,
        "der Kern meldet keinen Zuwachs von mindestens 64 KiB: {vorher} -> {waehrend}"
    );

    drop(puffer);

    assert_eq!(
        vmlck_kib(),
        vorher,
        "nach dem Wegwerfen sind die Seiten noch festgenagelt"
    );
}
