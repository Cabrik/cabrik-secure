//! Der Leser für das Altformat aus Version 1.
//!
//! Er verdient eigenes Fuzzing, weil er Dateien liest, die **Jahre alt**
//! sein können und deren Kopf im Klartext steht — bei v1 stand dort unter
//! anderem der Dateiname, ungeschützt und damit beliebig manipulierbar.
#![no_main]

use cabrik_core::rng::OsRandom;
use cabrik_core::Identity;
use libfuzzer_sys::fuzz_target;
use std::sync::OnceLock;

fn identitaet() -> &'static Identity {
    static EINMAL: OnceLock<Identity> = OnceLock::new();
    EINMAL.get_or_init(|| {
        Identity::generate(&mut OsRandom, true, 1_700_000_000)
            .expect("Identitaet liess sich nicht erzeugen")
    })
}

fuzz_target!(|daten: &[u8]| {
    let _ = cabrik_v1::envelope::open(daten, &identitaet().enc_sk, false);
});
