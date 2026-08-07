//! Der Weg vom signierten Envelope bis zur Vertrauensaussage.
//!
//! Die Einzelteile sind in den Modultests geprüft. Hier läuft der ganze Weg
//! einmal durch — genau die Richtung, in der die Modultests blind sind: Ob
//! der Signierschlüssel, den `open` liefert, tatsächlich derselbe ist, über
//! den der Trust Store nachschlägt.
//!
//! Und es ist die Antwort auf den Befund, mit dem alles anfing: In v1 meldete
//! jede selbst mitgelieferte Signatur `signature_valid: true`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use cabrik_core::envelope::{self, Opener, SealOptions};
use cabrik_core::trust::{Authenticity, Contact, TrustState, TrustStore, VerifiedVia};
use cabrik_core::{Identity, OsRandom, Suite, kem};

fn identitaet(signing: bool) -> Identity {
    Identity::generate(&mut OsRandom, signing, 1_700_000_000).unwrap()
}

fn sig_pub_von(id: &Identity) -> [u8; 32] {
    ed25519_dalek::SigningKey::from_bytes(id.sig_sk.as_ref().unwrap())
        .verifying_key()
        .to_bytes()
}

fn kontakt_fuer(name: &str, id: &Identity) -> Contact {
    Contact::new_seen(
        name,
        kem::public_key(&id.enc_sk).unwrap(),
        Some(sig_pub_von(id)),
        Some(Box::new(kem::pq_public_key(&id.pq_seed))),
        1_700_000_000,
    )
    .unwrap()
}

/// Verschlüsselt an `empf`, signiert von `abs`, und gibt den Envelope zurück.
fn signierte_nachricht(empf: &Identity, abs: &Identity, text: &[u8]) -> Vec<u8> {
    envelope::seal(
        Suite::Classical,
        &[&kem::public_key(&empf.enc_sk).unwrap()[..]],
        None,
        text,
        Some(abs),
        &SealOptions::default(),
        &mut OsRandom,
    )
    .unwrap()
}

/// Der Kernbefund aus v1, als durchlaufender Nachweis.
#[test]
fn eine_selbst_mitgelieferte_signatur_ergibt_keine_authentizitaet() {
    let empf = identitaet(false);
    let angreifer = identitaet(true);

    // Der Angreifer erzeugt ein eigenes Schluesselpaar und signiert damit --
    // genau das Vorgehen, das v1 mit `signature_valid: true` quittierte.
    let env = signierte_nachricht(&empf, &angreifer, b"Ich bin Alice, wirklich!");

    let auf = envelope::open(&Opener::Identity(&empf), &env, true).unwrap();
    // Kryptographisch ist die Signatur einwandfrei.
    assert!(matches!(auf.signer, cabrik_core::Signer::Key(_)));

    // Aber ohne Eintrag im Trust Store ist sie keine Aussage ueber die Person.
    let leer = TrustStore::new();
    let auth = leer.resolve(&auf.signer);

    assert!(matches!(auth, Authenticity::SignedUnknown { .. }));
    assert!(
        !auth.may_show_green(),
        "das ist der Fehler, den v1 gemacht hat"
    );
}

#[test]
fn verifizierter_kontakt_wird_namentlich_aufgeloest() {
    let empf = identitaet(false);
    let alice = identitaet(true);

    let mut store = TrustStore::new();
    let mut c = kontakt_fuer("Alice", &alice);
    c.verify(VerifiedVia::QrCode, 1_700_000_500).unwrap();
    store.add(c).unwrap();

    let env = signierte_nachricht(&empf, &alice, b"Unterlagen im Anhang");
    let auf = envelope::open(&Opener::Identity(&empf), &env, true).unwrap();

    match store.resolve(&auf.signer) {
        Authenticity::SignedVerified {
            name, verified_at, ..
        } => {
            assert_eq!(name, "Alice");
            assert_eq!(verified_at, Some(1_700_000_500));
        }
        other => panic!("erwartete SignedVerified, bekam {other:?}"),
    }
}

#[test]
fn bekannt_aber_unverifiziert_ist_nicht_gruen() {
    let empf = identitaet(false);
    let bob = identitaet(true);

    let mut store = TrustStore::new();
    store.add(kontakt_fuer("Bob", &bob)).unwrap();

    let env = signierte_nachricht(&empf, &bob, b"Hallo");
    let auf = envelope::open(&Opener::Identity(&empf), &env, true).unwrap();

    let auth = store.resolve(&auf.signer);
    assert!(matches!(auth, Authenticity::SignedSeen { .. }));
    assert!(!auth.may_show_green(), "Gesehen ist nicht Verifiziert");
    assert!(!auth.is_warning(), "und auch kein Warnfall");
}

/// Der Fall, an dem Messenger historisch scheitern: ein stiller
/// Schlüsselwechsel.
#[test]
fn schluesselwechsel_eines_verifizierten_kontakts_warnt() {
    let empf = identitaet(false);
    let alice_alt = identitaet(true);
    let alice_neu = identitaet(true);

    let mut store = TrustStore::new();
    let mut c = kontakt_fuer("Alice", &alice_alt);
    c.verify(VerifiedVia::SafetyNumber, 1_700_000_500).unwrap();
    store.add(c).unwrap();

    // Alice taucht mit neuem Schluessel auf; der Nutzer traegt ihn ein.
    let eintrag = store.find_by_sig_pub_mut(&sig_pub_von(&alice_alt)).unwrap();
    eintrag
        .replace_keys(
            kem::public_key(&alice_neu.enc_sk).unwrap(),
            Some(sig_pub_von(&alice_neu)),
            Some(Box::new(kem::pq_public_key(&alice_neu.pq_seed))),
            1_700_000_600,
        )
        .unwrap();

    let env = signierte_nachricht(&empf, &alice_neu, b"Ich bin es, neues Geraet");
    let auf = envelope::open(&Opener::Identity(&empf), &env, true).unwrap();

    let auth = store.resolve(&auf.signer);
    assert!(auth.is_warning(), "stiller Schluesselwechsel blieb stumm");
    assert!(!auth.may_show_green());
    match auth {
        Authenticity::SignedChanged {
            name,
            previous_was_verified,
            ..
        } => {
            assert_eq!(name, "Alice");
            assert!(
                previous_was_verified,
                "der Wechsel eines verifizierten Schluessels wiegt schwerer"
            );
        }
        other => panic!("erwartete SignedChanged, bekam {other:?}"),
    }
}

#[test]
fn nachricht_mit_ausgemustertem_schluessel_faellt_auf() {
    let empf = identitaet(false);
    let alt = identitaet(true);
    let neu = identitaet(true);

    let mut store = TrustStore::new();
    store.add(kontakt_fuer("Carol", &alt)).unwrap();
    store
        .find_by_sig_pub_mut(&sig_pub_von(&alt))
        .unwrap()
        .replace_keys(
            kem::public_key(&neu.enc_sk).unwrap(),
            Some(sig_pub_von(&neu)),
            None,
            1_700_000_600,
        )
        .unwrap();

    // Nachricht kommt noch mit dem ALTEN Schluessel.
    let env = signierte_nachricht(&empf, &alt, b"noch mit dem alten Schluessel");
    let auf = envelope::open(&Opener::Identity(&empf), &env, true).unwrap();

    assert!(
        store.resolve(&auf.signer).is_warning(),
        "ausgemusterter Schluessel blieb unbemerkt"
    );
}

#[test]
fn widerrufener_kontakt_warnt_auch_bei_gueltiger_signatur() {
    let empf = identitaet(false);
    let mallory = identitaet(true);

    let mut store = TrustStore::new();
    let mut c = kontakt_fuer("Mallory", &mallory);
    c.verify(VerifiedVia::Fingerprint, 1).unwrap();
    c.revoke(1_700_000_900, Some("Geraet verloren")).unwrap();
    store.add(c).unwrap();

    let env = signierte_nachricht(&empf, &mallory, b"alles gut bei mir");
    let auf = envelope::open(&Opener::Identity(&empf), &env, true).unwrap();

    let auth = store.resolve(&auf.signer);
    assert!(matches!(auth, Authenticity::SignedRevoked { .. }));
    assert!(auth.is_warning());
    assert!(
        !auth.may_show_green(),
        "eine gueltige Signatur eines widerrufenen Schluessels bleibt ein Warnfall"
    );
}

#[test]
fn anonymer_versand_bleibt_ein_legitimer_modus() {
    let empf = identitaet(false);
    let env = envelope::seal(
        Suite::Classical,
        &[&kem::public_key(&empf.enc_sk).unwrap()[..]],
        None,
        b"ohne Absender",
        None,
        &SealOptions::default(),
        &mut OsRandom,
    )
    .unwrap();

    let auf = envelope::open(&Opener::Identity(&empf), &env, false).unwrap();
    let auth = TrustStore::new().resolve(&auf.signer);

    assert_eq!(auth, Authenticity::Unsigned);
    assert!(!auth.is_warning(), "anonym ist kein Fehler");
    assert!(!auth.may_show_green());
}

/// Der Fingerprint umfasst alle drei Schlüssel — auch den
/// Post-Quantum-Anteil.
#[test]
fn fingerprint_des_kontakts_deckt_den_post_quantum_schluessel_ab() {
    let alice = identitaet(true);
    let mit_pq = kontakt_fuer("Alice", &alice);

    let mut ohne_pq = mit_pq.clone();
    ohne_pq.xwing_pub = None;

    assert_ne!(
        mit_pq.fingerprint(),
        ohne_pq.fingerprint(),
        "ohne diesen Unterschied liesse sich ein PQ-Schluessel unterschieben"
    );
    assert!(mit_pq.supports_post_quantum());
    assert!(!ohne_pq.supports_post_quantum());
}

#[test]
fn speicher_ueberlebt_die_ablage() {
    let alice = identitaet(true);
    let bob = identitaet(true);

    let mut store = TrustStore::new();
    let mut a = kontakt_fuer("Alice", &alice);
    a.verify(VerifiedVia::QrCode, 1_700_000_500).unwrap();
    store.add(a).unwrap();
    store.add(kontakt_fuer("Bob", &bob)).unwrap();

    let bytes = cabrik_core::trust::serialize(&store).unwrap();
    let zurueck = cabrik_core::trust::deserialize(&bytes).unwrap();

    assert_eq!(zurueck.len(), 2);
    let a2 = zurueck.find_by_sig_pub(&sig_pub_von(&alice)).unwrap();
    assert_eq!(a2.name, "Alice");
    assert_eq!(a2.state, TrustState::Verified);
    assert!(a2.supports_post_quantum());
}
