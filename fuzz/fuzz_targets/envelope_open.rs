//! Ein Envelope, mit einer Identität geöffnet.
//!
//! Das ist die einzige Eingabe, die ein Gegner **vollständig** beherrscht,
//! und sie wird verarbeitet, bevor irgendetwas beglaubigt ist: Längen,
//! Anzahlen und Versätze müssen gelesen werden, um überhaupt an die
//! Beglaubigung zu kommen.
//!
//! Der Rückgabewert ist gleichgültig — `Err` ist das erwartete Ergebnis.
//! Gesucht wird allein nach Eingaben, bei denen die Funktion **nicht
//! zurückkehrt**.
#![no_main]

use cabrik_core::envelope::{self, Opener};
use cabrik_core::rng::OsRandom;
use cabrik_core::Identity;
use libfuzzer_sys::fuzz_target;
use std::sync::OnceLock;

/// Einmal erzeugt und wiederverwendet. Eine Identität je Durchlauf zu bauen
/// kostete mehr Zeit als das Fuzzing selbst.
fn identitaet() -> &'static Identity {
    static EINMAL: OnceLock<Identity> = OnceLock::new();
    EINMAL.get_or_init(|| {
        Identity::generate(&mut OsRandom, true, 1_700_000_000)
            .expect("Identitaet liess sich nicht erzeugen")
    })
}

fuzz_target!(|daten: &[u8]| {
    let _ = envelope::open(&Opener::Identity(identitaet()), daten, false);
    // Auch mit erzwungener Signaturprüfung: ein anderer Zweig.
    let _ = envelope::open(&Opener::Identity(identitaet()), daten, true);
});
