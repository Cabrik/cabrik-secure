//! Blockweise versiegeln und öffnen (`spec/grosse-dateien.md` §3).
//!
//! Zwei Zusagen werden hier gehalten:
//!
//! 1. **Dieselben Bytes.** Gleich, in welchen Stücken der Klartext kommt —
//!    heraus kommt Byte für Byte der Envelope von `seal`, und beim Öffnen
//!    Byte für Byte der Klartext von `open`.
//! 2. **Nichts gilt vor der Prüfung.** Was die Ablage unterwegs bekommt,
//!    wird nur freigegeben, wenn Blöcke, Trailer, Signatur und Füllung
//!    stimmen — sonst verworfen, und zwar jedes Mal (`envelope-v2.md` §8.4).
//!
//! Dass `seal` selbst sich durch den Umbau nicht verschoben hat, hält die
//! Rückschau in `formatfreeze.rs` fest.

// In Tests sind `unwrap`, `expect` und `panic` erwünscht: ein Fehlschlag
// soll den Test sofort und laut abbrechen.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use cabrik_core::envelope::{
    self, Ablage, Bericht, ContentType, Geprueft, KOPF_HOECHSTENS, Oeffnungsfehler, Opener,
    SealOptions, Signer,
};
use cabrik_core::rng::testing::DeterministicRng;
use cabrik_core::stream::CHUNK_SIZE;
use cabrik_core::{Error, Identity, Suite, kem};

// ---------------------------------------------------------------------------
// Werkzeug
// ---------------------------------------------------------------------------

/// Eine Ablage, die mitschreibt, was mit ihr geschieht.
#[derive(Default)]
struct Spur {
    geschrieben: Vec<u8>,
    freigegeben: bool,
    verworfen: usize,
    schreiben_scheitert: bool,
    freigeben_scheitert: bool,
}

impl Ablage for Spur {
    type Fehler = &'static str;

    fn schreiben(&mut self, klartext: &[u8]) -> Result<(), Self::Fehler> {
        if self.schreiben_scheitert {
            return Err("Platte voll");
        }
        self.geschrieben.extend_from_slice(klartext);
        Ok(())
    }

    fn freigeben(&mut self, _beleg: &Geprueft) -> Result<(), Self::Fehler> {
        if self.freigeben_scheitert {
            return Err("Umbenennen verweigert");
        }
        self.freigegeben = true;
        Ok(())
    }

    fn verwerfen(&mut self) {
        self.verworfen += 1;
        self.geschrieben.clear();
    }
}

/// Was ein Fall braucht: Empfänger, Absender, Klartext, Einstellungen.
struct Fall {
    suite: Suite,
    empfaenger: Identity,
    absender: Option<Identity>,
    klartext: Vec<u8>,
    art: ContentType,
    padding: Option<bool>,
}

impl Fall {
    fn neu(seed: u8, suite: Suite, signiert: bool, art: ContentType, laenge: usize) -> Self {
        Self::mit_padding(seed, suite, signiert, art, None, laenge)
    }

    fn mit_padding(
        seed: u8,
        suite: Suite,
        signiert: bool,
        art: ContentType,
        padding: Option<bool>,
        laenge: usize,
    ) -> Self {
        let mut rng = DeterministicRng::new([seed; 32]);
        let empfaenger = Identity::generate(&mut rng, false, 1_700_000_000).unwrap();
        let absender = Identity::generate(&mut rng, true, 1_700_000_000).unwrap();
        Self {
            suite,
            empfaenger,
            absender: signiert.then_some(absender),
            klartext: (0..laenge).map(|i| (i % 249) as u8).collect(),
            art,
            padding,
        }
    }

    fn pk(&self) -> Vec<u8> {
        if self.suite == Suite::Hybrid {
            kem::pq_public_key(&self.empfaenger.pq_seed).to_vec()
        } else {
            kem::public_key(&self.empfaenger.enc_sk).unwrap().to_vec()
        }
    }

    fn opts(&self) -> SealOptions<'static> {
        SealOptions {
            content_type: self.art,
            filename: (self.art == ContentType::File).then_some("urlaub.mp4"),
            timestamp: Some(1_700_000_999),
            padding: self.padding,
            dummy_stanzas: true,
        }
    }

    /// Am Stück, wie bisher.
    fn am_stueck(&self) -> Vec<u8> {
        let pk = self.pk();
        envelope::seal(
            self.suite,
            &[&pk[..]],
            None,
            &self.klartext,
            self.absender.as_ref(),
            &self.opts(),
            &mut DeterministicRng::new([99; 32]),
        )
        .unwrap()
    }

    /// Blockweise, in Stücken der Länge `stueck`.
    fn blockweise(&self, stueck: usize) -> Vec<u8> {
        let pk = self.pk();
        let (mut aus, mut v) = envelope::versiegeln_beginnen(
            self.suite,
            &[&pk[..]],
            None,
            self.klartext.len() as u64,
            self.absender.as_ref(),
            &self.opts(),
            &mut DeterministicRng::new([99; 32]),
        )
        .unwrap();
        for s in self.klartext.chunks(stueck) {
            v.weiter(s, &mut aus).unwrap();
        }
        v.abschliessen(&mut aus).unwrap();
        aus
    }
}

/// Öffnet blockweise: den Anfang höchstens bis [`KOPF_HOECHSTENS`], den Rest
/// in Stücken der Länge `stueck`.
fn oeffnen(
    env: &[u8],
    empfaenger: &Identity,
    stueck: usize,
    spur: &mut Spur,
) -> Result<Bericht, Oeffnungsfehler<&'static str>> {
    let anfang = &env[..env.len().min(KOPF_HOECHSTENS)];
    let (mut o, verbraucht) =
        envelope::oeffnen_beginnen(&Opener::Identity(empfaenger), anfang, false)?;
    for s in env[verbraucht..].chunks(stueck) {
        o.weiter(s, spur)?;
    }
    o.abschliessen(spur)
}

fn kern(f: Oeffnungsfehler<&'static str>) -> Error {
    match f {
        Oeffnungsfehler::Kern(e) => e,
        Oeffnungsfehler::Ablage(a) => panic!("Ablagefehler statt Kernfehler: {a}"),
        Oeffnungsfehler::Veraendert => panic!("„verändert“ statt Kernfehler"),
    }
}

const STUECKE: [usize; 7] = [
    1,
    7,
    4096,
    CHUNK_SIZE - 1,
    CHUNK_SIZE,
    CHUNK_SIZE + 1,
    1 << 20,
];

// ---------------------------------------------------------------------------
// 1. Dieselben Bytes
// ---------------------------------------------------------------------------

#[test]
fn blockweise_versiegelt_ergibt_byte_fuer_byte_denselben_envelope() {
    for suite in [Suite::Classical, Suite::Hybrid] {
        for signiert in [false, true] {
            for art in [ContentType::Text, ContentType::File] {
                for laenge in [0, 1, CHUNK_SIZE, CHUNK_SIZE + 1, 3 * CHUNK_SIZE + 17] {
                    let f = Fall::neu(1, suite, signiert, art, laenge);
                    let erwartet = f.am_stueck();
                    for stueck in STUECKE {
                        assert_eq!(
                            f.blockweise(stueck),
                            erwartet,
                            "{suite:?}, signiert {signiert}, {art:?}, {laenge} Bytes \
                             in Stücken zu {stueck}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn auch_wenn_die_fuellung_einen_ganzen_block_belegt() {
    // 4 194 305 Bytes werden um 131 071 aufgefüllt: Die Füllung reicht über
    // zwei Blockgrenzen, und ein Block besteht nur aus ihr.
    let f = Fall::mit_padding(
        2,
        Suite::Hybrid,
        true,
        ContentType::File,
        Some(true),
        4_194_305,
    );
    let erwartet = f.am_stueck();
    for stueck in [CHUNK_SIZE + 1, 1 << 20] {
        assert_eq!(f.blockweise(stueck), erwartet, "in Stücken zu {stueck}");
    }

    let mut spur = Spur::default();
    let bericht = oeffnen(&erwartet, &f.empfaenger, 777_777, &mut spur).unwrap();
    assert_eq!(
        spur.geschrieben, f.klartext,
        "die Füllung ging mit an die Ablage"
    );
    assert_eq!(bericht.klartext_laenge, 4_194_305);
}

#[test]
fn blockweise_geoeffnet_ergibt_denselben_klartext_und_bericht() {
    for suite in [Suite::Classical, Suite::Hybrid] {
        for signiert in [false, true] {
            for art in [ContentType::Text, ContentType::File] {
                for laenge in [0, 1, CHUNK_SIZE, 2 * CHUNK_SIZE + 5] {
                    let f = Fall::neu(3, suite, signiert, art, laenge);
                    let env = f.am_stueck();
                    let auf =
                        envelope::open(&Opener::Identity(&f.empfaenger), &env, false).unwrap();

                    for stueck in STUECKE {
                        let mut spur = Spur::default();
                        let b = oeffnen(&env, &f.empfaenger, stueck, &mut spur).unwrap();
                        let fall = format!("{suite:?}, {signiert}, {art:?}, {laenge}, {stueck}");
                        assert_eq!(spur.geschrieben, *auf.plaintext, "{fall}");
                        assert!(spur.freigegeben, "{fall}: nicht freigegeben");
                        assert_eq!(spur.verworfen, 0, "{fall}: verworfen");
                        assert_eq!(b.signer, auf.signer, "{fall}");
                        assert_eq!(b.filename, auf.filename, "{fall}");
                        assert_eq!(b.timestamp, auf.timestamp, "{fall}");
                        assert_eq!(b.content_type, auf.content_type, "{fall}");
                        assert_eq!(b.klartext_laenge, laenge as u64, "{fall}");
                    }
                }
            }
        }
    }
}

#[test]
fn ein_kopf_ueber_zwei_mib_ist_kein_hindernis() {
    // Eine Datei über `KOPF_HOECHSTENS`: Der Anfang reicht für den Kopf,
    // alles danach kommt blockweise.
    let f = Fall::neu(4, Suite::Classical, true, ContentType::File, 3 << 20);
    let env = f.am_stueck();
    assert!(env.len() > KOPF_HOECHSTENS);

    let mut spur = Spur::default();
    let b = oeffnen(&env, &f.empfaenger, 1 << 20, &mut spur).unwrap();
    assert_eq!(spur.geschrieben, f.klartext);
    assert!(matches!(b.signer, Signer::Key(_)));
}

#[test]
fn der_kopf_passt_immer_in_kopf_hoechstens() {
    // Aus den Grenzen des Formats: 255 Kapseln zu höchstens 4096 Bytes
    // (§5), die Headerlänge, acht TLV-Felder zu höchstens 3 + 65 535 Bytes
    // (§7.2) und das Tag. Wächst eine dieser Grenzen, muss die Konstante mit.
    let prolog = 5 + 255 * (3 + 4096);
    let header = 8 * (3 + 65_535) + 16;
    assert!(prolog + 4 + header <= KOPF_HOECHSTENS);
}

// ---------------------------------------------------------------------------
// 2. Nichts gilt vor der Prüfung
// ---------------------------------------------------------------------------

/// Öffnet einen gestörten Envelope und verlangt einen bestimmten Kode,
/// ohne Freigabe und mit genau einem Verwerfen.
fn scheitert_ohne_freigabe(env: &[u8], empfaenger: &Identity, kode: &str) {
    for stueck in [1, 4096, 1 << 20] {
        let mut spur = Spur::default();
        let e = kern(oeffnen(env, empfaenger, stueck, &mut spur).unwrap_err());
        assert_eq!(e.code(), kode, "in Stücken zu {stueck}");
        assert!(!spur.freigegeben, "freigegeben trotz {kode}");
        assert_eq!(spur.verworfen, 1, "nicht genau einmal verworfen bei {kode}");
        assert!(
            spur.geschrieben.is_empty(),
            "Klartext blieb liegen bei {kode}"
        );

        // Am Stück scheitert es genauso.
        let am_stueck = envelope::open(&Opener::Identity(empfaenger), env, false).unwrap_err();
        assert_eq!(am_stueck.code(), kode, "am Stück");
    }
}

#[test]
fn ein_veraenderter_trailer_gibt_nichts_frei() {
    let f = Fall::neu(
        5,
        Suite::Hybrid,
        true,
        ContentType::File,
        2 * CHUNK_SIZE + 3,
    );
    let mut env = f.am_stueck();
    let letzt = env.len() - 1;
    env[letzt] ^= 0x01;
    scheitert_ohne_freigabe(&env, &f.empfaenger, "SIGNATURE_INVALID");
}

#[test]
fn ein_veraenderter_block_gibt_nichts_frei() {
    let f = Fall::neu(6, Suite::Classical, true, ContentType::File, 3 * CHUNK_SIZE);
    let mut env = f.am_stueck();
    // Mitten in den zweiten Block.
    let stelle = env.len() - 80 - CHUNK_SIZE - 100;
    env[stelle] ^= 0x80;
    scheitert_ohne_freigabe(&env, &f.empfaenger, "AUTH_FAILED");
}

#[test]
fn ein_abgeschnittener_envelope_gibt_nichts_frei() {
    for signiert in [false, true] {
        let f = Fall::neu(
            7,
            Suite::Classical,
            signiert,
            ContentType::File,
            2 * CHUNK_SIZE,
        );
        let env = f.am_stueck();
        for weg in [1, 10, 81, CHUNK_SIZE] {
            scheitert_ohne_freigabe(&env[..env.len() - weg], &f.empfaenger, "TRUNCATED");
        }
    }
}

#[test]
fn angehaengte_bytes_geben_nichts_frei() {
    for signiert in [false, true] {
        let f = Fall::neu(8, Suite::Classical, signiert, ContentType::File, 1000);
        let mut env = f.am_stueck();
        env.extend_from_slice(b"angehaengt");
        scheitert_ohne_freigabe(&env, &f.empfaenger, "MALFORMED");
    }
}

#[test]
fn die_signaturpflicht_greift_vor_dem_ersten_block() {
    let f = Fall::neu(9, Suite::Classical, false, ContentType::File, 5000);
    let env = f.am_stueck();
    let e = envelope::oeffnen_beginnen(&Opener::Identity(&f.empfaenger), &env, true).unwrap_err();
    assert_eq!(e.code(), "SIGNATURE_MISSING");
}

#[test]
fn eine_ablage_die_nicht_schreibt_wird_verworfen() {
    let f = Fall::neu(
        10,
        Suite::Classical,
        true,
        ContentType::File,
        2 * CHUNK_SIZE,
    );
    let env = f.am_stueck();
    let mut spur = Spur {
        schreiben_scheitert: true,
        ..Spur::default()
    };
    match oeffnen(&env, &f.empfaenger, 4096, &mut spur) {
        Err(Oeffnungsfehler::Ablage(grund)) => assert_eq!(grund, "Platte voll"),
        anders => panic!("erwartet war der Ablagefehler, kam: {anders:?}"),
    }
    assert!(!spur.freigegeben);
    assert_eq!(spur.verworfen, 1);
}

#[test]
fn eine_ablage_die_nicht_freigeben_kann_wird_verworfen() {
    let f = Fall::neu(11, Suite::Classical, true, ContentType::File, 1234);
    let env = f.am_stueck();
    let mut spur = Spur {
        freigeben_scheitert: true,
        ..Spur::default()
    };
    match oeffnen(&env, &f.empfaenger, 4096, &mut spur) {
        Err(Oeffnungsfehler::Ablage(grund)) => assert_eq!(grund, "Umbenennen verweigert"),
        anders => panic!("erwartet war der Ablagefehler, kam: {anders:?}"),
    }
    assert!(!spur.freigegeben);
    assert_eq!(spur.verworfen, 1);
    assert!(spur.geschrieben.is_empty());
}

// ---------------------------------------------------------------------------
// Länge und Fingerabdruck
// ---------------------------------------------------------------------------

#[test]
fn mehr_klartext_als_angekuendigt_bricht_ab() {
    let f = Fall::neu(12, Suite::Classical, false, ContentType::File, 0);
    let pk = f.pk();
    let (mut aus, mut v) = envelope::versiegeln_beginnen(
        Suite::Classical,
        &[&pk[..]],
        None,
        10,
        None,
        &f.opts(),
        &mut DeterministicRng::new([1; 32]),
    )
    .unwrap();
    v.weiter(b"123456789", &mut aus).unwrap();
    let e = v.weiter(b"ab", &mut aus).unwrap_err();
    assert_eq!(e.code(), "MALFORMED");
}

#[test]
fn weniger_klartext_als_angekuendigt_bricht_ab() {
    // Die Datei ist beim Lesen geschrumpft. Der Header nennt schon die alte
    // Länge — ein Envelope daraus wäre falsch.
    let f = Fall::neu(13, Suite::Classical, false, ContentType::File, 0);
    let pk = f.pk();
    let (mut aus, mut v) = envelope::versiegeln_beginnen(
        Suite::Classical,
        &[&pk[..]],
        None,
        10,
        None,
        &f.opts(),
        &mut DeterministicRng::new([1; 32]),
    )
    .unwrap();
    v.weiter(b"123456789", &mut aus).unwrap();
    assert_eq!(v.abschliessen(&mut aus).unwrap_err().code(), "MALFORMED");
}

#[test]
fn der_fingerabdruck_kennt_den_envelope_und_nicht_die_stuecke() {
    let a = Fall::neu(
        14,
        Suite::Hybrid,
        false,
        ContentType::File,
        2 * CHUNK_SIZE + 9,
    );
    let env_a = a.am_stueck();

    let mut s1 = Spur::default();
    let mut s2 = Spur::default();
    let b1 = oeffnen(&env_a, &a.empfaenger, 1, &mut s1).unwrap();
    let b2 = oeffnen(&env_a, &a.empfaenger, 1 << 20, &mut s2).unwrap();
    assert_eq!(
        b1.fingerabdruck, b2.fingerabdruck,
        "zwei Durchgänge, zwei Werte"
    );

    // Derselbe Klartext an denselben Empfänger, aber ein anderer Envelope
    // (anderer Zufall): ein anderer Fingerabdruck.
    let pk = a.pk();
    let anders = envelope::seal(
        a.suite,
        &[&pk[..]],
        None,
        &a.klartext,
        None,
        &a.opts(),
        &mut DeterministicRng::new([100; 32]),
    )
    .unwrap();
    let mut s3 = Spur::default();
    let b3 = oeffnen(&anders, &a.empfaenger, 4096, &mut s3).unwrap();
    assert_ne!(b1.fingerabdruck, b3.fingerabdruck);
}

// ---------------------------------------------------------------------------
// Der zweite Durchgang (`spec/grosse-dateien.md` §6)
// ---------------------------------------------------------------------------

/// Öffnet blockweise und verlangt einen Fingerabdruck.
fn oeffnen_erwartet(
    env: &[u8],
    empfaenger: &Identity,
    erwartet: [u8; 32],
    spur: &mut Spur,
) -> Result<Bericht, Oeffnungsfehler<&'static str>> {
    let (mut o, verbraucht) =
        envelope::oeffnen_beginnen(&Opener::Identity(empfaenger), env, false)?;
    o.fingerabdruck_erwarten(erwartet);
    o.weiter(&env[verbraucht..], spur)?;
    o.abschliessen(spur)
}

#[test]
fn derselbe_envelope_wird_im_zweiten_durchgang_freigegeben() {
    let f = Fall::neu(
        15,
        Suite::Hybrid,
        true,
        ContentType::File,
        2 * CHUNK_SIZE + 1,
    );
    let env = f.am_stueck();

    let mut erster = Spur::default();
    let b = oeffnen(&env, &f.empfaenger, 4096, &mut erster).unwrap();

    let mut zweiter = Spur::default();
    oeffnen_erwartet(&env, &f.empfaenger, b.fingerabdruck, &mut zweiter).unwrap();
    assert!(zweiter.freigegeben);
    assert_eq!(zweiter.geschrieben, f.klartext);
}

#[test]
fn ein_ausgetauschter_envelope_wird_im_zweiten_durchgang_verworfen() {
    // Zwischen den Durchgaengen hat jemand die Datei ersetzt -- durch einen
    // einwandfreien Envelope, sogar vom selben Absender. Er ist trotzdem
    // nicht der, den der Nutzer gesehen hat.
    let f = Fall::neu(16, Suite::Classical, true, ContentType::File, 5000);
    let env = f.am_stueck();
    let mut erster = Spur::default();
    let b = oeffnen(&env, &f.empfaenger, 4096, &mut erster).unwrap();

    let pk = f.pk();
    let anderer = envelope::seal(
        f.suite,
        &[&pk[..]],
        None,
        b"ganz etwas anderes",
        f.absender.as_ref(),
        &f.opts(),
        &mut DeterministicRng::new([77; 32]),
    )
    .unwrap();

    let mut zweiter = Spur::default();
    match oeffnen_erwartet(&anderer, &f.empfaenger, b.fingerabdruck, &mut zweiter) {
        Err(Oeffnungsfehler::Veraendert) => {}
        anders => panic!("erwartet war „verändert“, kam {anders:?}"),
    }
    assert!(!zweiter.freigegeben);
    assert_eq!(zweiter.verworfen, 1);
    assert!(zweiter.geschrieben.is_empty());
}
