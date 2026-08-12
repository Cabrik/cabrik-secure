//! Das Bereinigen — und die Prüfung, dass es sich wiederholen lässt.
//!
//! Der zweite Durchlauf ist der interessante: Er füttert den Leser mit dem,
//! was der eigene Schreiber erzeugt hat. Genau dort steckten in diesem
//! Projekt schon zwei Fehler (die EPUB-Regression und das doppelte Melden
//! bereits geleerter Blöcke).
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|daten: &[u8]| {
    if let Ok((sauber, _)) = cabrik_metadata::strip(daten) {
        let _ = cabrik_metadata::strip(&sauber);
    }
});
