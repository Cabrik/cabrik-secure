//! Die Formaterkennung und alle siebzehn Leser.
//!
//! `inspect` verändert nichts und ist deshalb der Eingang, den ein Nutzer
//! zuerst benutzt — auch bei einer Datei, der er nicht traut. Er muss jede
//! Eingabe überstehen.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|daten: &[u8]| {
    let _ = cabrik_metadata::inspect(daten);
});
