//! Ein Envelope, mit einem Passwort geöffnet.
//!
//! Eigener Zweig: Die Kapselsuche unterscheidet sich, und die
//! Kostenangaben der Schlüsselableitung kommen aus der Datei — also vom
//! Angreifer. `KdfParams::validate` muss sie abweisen, bevor jemand
//! versucht, vier Gigabyte anzufordern.
#![no_main]

use cabrik_core::envelope::{self, Opener};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|daten: &[u8]| {
    let _ = envelope::open(&Opener::Password(b"geheim"), daten, false);
});
