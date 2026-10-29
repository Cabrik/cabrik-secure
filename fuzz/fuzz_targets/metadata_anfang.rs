//! Die Erkennung am Anfang gegen die über die ganze Datei
//! (`spec/grosse-dateien.md` §13).
//!
//! Nie „unbekannt“, wo die ganze Datei ein Format ergäbe, und nie ein
//! anderes Format. Sonst ginge eine Datei unbereinigt hinaus, ohne dass
//! jemand das gewählt hätte.
#![no_main]

use cabrik_metadata::{ANFANG, Anfang, Format, am_anfang};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|daten: &[u8]| {
    let ganz = Format::detect(daten);
    for n in [ANFANG, ANFANG + 1, daten.len() / 2, daten.len()] {
        if n < ANFANG.min(daten.len()) || n > daten.len() {
            continue;
        }
        match am_anfang(&daten[..n], daten.len() as u64) {
            Anfang::Bekannt(f) => assert_eq!(Some(f), ganz, "anderes Format bei {n} Bytes"),
            Anfang::Unbekannt { .. } => assert_eq!(ganz, None, "unbekannt bei {n} Bytes"),
            Anfang::GanzeDatei => {}
        }
    }
});
