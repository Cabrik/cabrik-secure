//! Ein Envelope, mit einem Passwort geöffnet.
//!
//! Eigener Zweig: Die Kapselsuche unterscheidet sich, und die
//! Kostenangaben der Schlüsselableitung kommen aus der Datei — also vom
//! Angreifer. `KdfParams::validate` muss sie abweisen, bevor jemand
//! versucht, vier Gigabyte anzufordern.
//!
//! # Warum mit Grenze
//!
//! Erlaubt sind bis zu 4 GiB (`spec/keyfile-v2.md` §4), und libFuzzer
//! bricht bei 2 GiB ab. Im Oktober 2026 meldete dieses Ziel deshalb jede
//! zweite Nacht einen „Absturz“, der keiner war: eine gültige Datei mit
//! 2 bis 3,6 GiB `m_cost`. Ein Alarm, der ständig ohne Befund anschlägt,
//! wird überhört — und dann auch der echte.
//!
//! Die Grenze ist kein Kunstgriff für den Fuzzer, sondern derselbe Weg,
//! den das Programm nimmt (`Speichergrenze`, `spec/keyfile-v2.md` §4.2).
//! Damit prüft er nebenbei, dass sie greift, bevor etwas angelegt wird.
//!
//! **256 MiB, weil das genau die empfohlenen Werte sind:** Der
//! Korpus-Envelope `passwort.env` geht damit weiterhin auf, und der Fuzzer
//! kommt über die Kapsel hinaus bis in Header und Blockstrom.
#![no_main]

use cabrik_core::Speichergrenze;
use cabrik_core::envelope::{self, Opener};
use libfuzzer_sys::fuzz_target;

const GRENZE: Speichergrenze = Speichergrenze::bytes(256 << 20);

fuzz_target!(|daten: &[u8]| {
    let opener = Opener::Password {
        passwort: b"geheim",
        grenze: GRENZE,
    };
    let _ = envelope::open(&opener, daten, false);
});
