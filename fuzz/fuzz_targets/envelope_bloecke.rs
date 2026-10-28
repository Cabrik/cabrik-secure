//! Blockweise öffnen muss dasselbe ergeben wie am Stück
//! (`spec/grosse-dateien.md` §3.2).
//!
//! Anders als `envelope_open` beginnt dieses Ziel mit einem **echten**
//! Envelope: Die Eingabe liefert Klartext, Einstellungen, eine Störung und
//! die Stücklänge. Nur so kommt der Fuzzer an den Blöcken vorbei — mit
//! erratenen Bytes scheitert jeder Versuch schon an der Kapsel.
//!
//! Geprüft wird zweierlei:
//!
//! 1. Am Stück und blockweise kommt **dasselbe** heraus — derselbe Klartext
//!    oder derselbe Fehlerkode.
//! 2. Bei jedem Fehler ist die Ablage **verworfen und nie freigegeben**.
#![no_main]

use cabrik_core::envelope::{self, Ablage, Geprueft, Oeffnungsfehler, Opener, SealOptions};
use cabrik_core::rng::OsRandom;
use cabrik_core::{ContentType, Identity, Suite, kem};
use libfuzzer_sys::fuzz_target;
use std::sync::OnceLock;

fn identitaet() -> &'static Identity {
    static EINMAL: OnceLock<Identity> = OnceLock::new();
    EINMAL.get_or_init(|| {
        Identity::generate(&mut OsRandom, true, 1_700_000_000)
            .expect("Identitaet liess sich nicht erzeugen")
    })
}

#[derive(Default)]
struct Spur {
    geschrieben: Vec<u8>,
    freigegeben: bool,
    verworfen: usize,
}

impl Ablage for Spur {
    type Fehler = core::convert::Infallible;

    fn schreiben(&mut self, klartext: &[u8]) -> Result<(), Self::Fehler> {
        self.geschrieben.extend_from_slice(klartext);
        Ok(())
    }

    fn freigeben(&mut self, _beleg: &Geprueft) -> Result<(), Self::Fehler> {
        self.freigegeben = true;
        Ok(())
    }

    fn verwerfen(&mut self) {
        self.verworfen += 1;
    }
}

fuzz_target!(|daten: &[u8]| {
    // Vier Steuerbytes, der Rest ist Klartext.
    let [art, stoerung, stelle, stueck, klartext @ ..] = daten else {
        return;
    };

    let id = identitaet();
    let pk = kem::public_key(&id.enc_sk).expect("oeffentlicher Schluessel");
    let opts = SealOptions {
        content_type: if art & 1 == 0 {
            ContentType::File
        } else {
            ContentType::Text
        },
        filename: (art & 1 == 0).then_some("datei.bin"),
        timestamp: None,
        padding: Some(art & 2 != 0),
        dummy_stanzas: art & 4 != 0,
    };
    let signierer = (art & 8 != 0).then_some(id);
    let Ok(mut env) = envelope::seal(
        Suite::Classical,
        &[&pk[..]],
        None,
        klartext,
        signierer,
        &opts,
        &mut OsRandom,
    ) else {
        return;
    };

    // Die Störung: ein Bit kippen, abschneiden oder etwas anhängen.
    let pos = (usize::from(*stelle) * 7919) % env.len();
    match stoerung % 4 {
        1 => env[pos] ^= 1 << (stoerung >> 5),
        2 => env.truncate(pos),
        3 => env.extend_from_slice(&[*stelle; 3]),
        _ => {}
    }

    let opener = Opener::Identity(id);
    let am_stueck = envelope::open(&opener, &env, false);

    // Blockweise, in Stücken von 1 bis 65 281 Bytes.
    let laenge = 1 + usize::from(*stueck) * 256 + usize::from(*stueck);
    let mut spur = Spur::default();
    let blockweise = (|| {
        let anfang = &env[..env.len().min(envelope::KOPF_HOECHSTENS)];
        let (mut o, verbraucht) = envelope::oeffnen_beginnen(&opener, anfang, false)?;
        for s in env[verbraucht..].chunks(laenge) {
            o.weiter(s, &mut spur)?;
        }
        o.abschliessen(&mut spur)
    })();

    match (am_stueck, blockweise) {
        (Ok(auf), Ok(bericht)) => {
            assert_eq!(*auf.plaintext, spur.geschrieben, "anderer Klartext");
            assert_eq!(auf.signer, bericht.signer, "andere Signaturlage");
            assert!(spur.freigegeben && spur.verworfen == 0, "Ablage falsch abgeschlossen");
        }
        (Err(a), Err(Oeffnungsfehler::Kern(b))) => {
            assert_eq!(a.code(), b.code(), "verschiedene Fehler");
            assert!(!spur.freigegeben, "freigegeben trotz Fehler");
            // Scheitert schon der Kopf, gab es noch keine Ablage zu verwerfen.
            assert!(spur.verworfen <= 1, "mehrfach verworfen");
        }
        (a, b) => panic!("am Stueck {a:?}, blockweise {b:?}"),
    }
});
