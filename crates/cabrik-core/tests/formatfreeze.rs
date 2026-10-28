//! Der Formatfreeze: Was sich nicht mehr ändern darf.
//!
//! # Warum es diese Datei gibt
//!
//! Ab der ersten Veröffentlichung liegen Envelopes und Schlüsseldateien bei
//! Menschen. Sie müssen in zehn Jahren noch aufgehen — auch mit einer
//! Umsetzung, die jemand anderes aus der Spezifikation gebaut hat.
//!
//! Die Vektortests in `vectors.rs` prüfen **Verhalten** gegen Vorlagen. Sie
//! decken nicht ab, was hier steht: die **Zahlen und Zeichenketten selbst**,
//! die eine fremde Umsetzung aus `spec/envelope-v2.md` und
//! `spec/keyfile-v2.md` abliest. Ändert jemand `MAGIC` oder eine
//! Suite-Kennung, bricht womöglich kein einziger Vektortest — aber jede
//! Datei auf der Welt.
//!
//! # Was ein Fehlschlag hier bedeutet
//!
//! **Nicht**, dass ein Wert falsch ist. Sondern dass jemand das Format
//! geändert hat, und zwar entweder versehentlich — dann gehört es
//! zurückgenommen — oder mit Absicht. Mit Absicht heißt: eine neue
//! Formatfassung, eine neue Kennung, und die alte bleibt **lesbar**.
//!
//! Die Zusage lautet: **lesen immer, schreiben nur in der eingefrorenen
//! Fassung.** Diesen Test anzupassen, um ihn grün zu bekommen, bricht sie.
//!
//! # Was hier NICHT hineingehört
//!
//! Interna. Puffergrößen, Schleifengrenzen, Namen von Feldern im
//! Quelltext — alles, was eine fremde Umsetzung nicht sehen kann, darf
//! sich ändern. Eingefroren ist, was **auf der Platte steht** oder über
//! eine Leitung geht.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use cabrik_core::rng::testing::DeterministicRng;
use cabrik_core::{Identity, Suite, armor, envelope, kem, keyfile, trust};
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// Erkennungsmerkmale
// ---------------------------------------------------------------------------

#[test]
fn die_magic_bytes_stehen_fest() {
    // Woran ein Leser die Datei erkennt, bevor er irgendetwas anderes tut.
    // `CA` fuer Cabrik, danach die Formatfassung.
    assert_eq!(envelope::MAGIC, [0xCA, 0x02], "Envelope");
    assert_eq!(keyfile::MAGIC, [0xCA, 0x4B], "Keyfile");
    assert_eq!(keyfile::VERSION, 0x02, "Keyfile-Fassung");
}

#[test]
fn die_suite_kennungen_stehen_fest() {
    // Sie stehen im Prolog jedes Envelopes. Eine Vertauschung machte aus
    // klassisch verschluesselten Dateien scheinbar Post-Quantum-Dateien --
    // und der Leser griffe nach dem falschen Schluessel.
    assert_eq!(Suite::Classical.id(), 0x0001);
    assert_eq!(Suite::Hybrid.id(), 0x0002);
}

#[test]
fn die_dateiendung_steht_fest() {
    // Sie ist kein Format-Merkmal im engeren Sinn -- erkannt wird an den
    // Magic Bytes. Aber sie steht in der Dateizuordnung des
    // Betriebssystems, und `.cab` bleibt lesbar, weil es sie einmal gab.
    assert_eq!(envelope::ENDUNG, "cabrik");
    assert!(envelope::ALTE_ENDUNGEN.contains(&"cab"));
}

// ---------------------------------------------------------------------------
// Textformen
// ---------------------------------------------------------------------------

#[test]
fn die_armor_rahmenzeilen_stehen_fest() {
    // Wer einen Envelope als Text verschickt, verlaesst sich darauf, dass
    // die Gegenseite genau diese Zeilen erkennt. Ein geaendertes Wort
    // machte jede eingefuegte Nachricht unlesbar.
    assert_eq!(armor::KOPF, "-----BEGIN CABRIK ENVELOPE-----");
    assert_eq!(armor::FUSS, "-----END CABRIK ENVELOPE-----");
}

#[test]
fn das_praefix_der_austausch_nutzlast_steht_fest() {
    // Es entscheidet, ob eine gescannte oder eingefuegte Zeichenkette als
    // Kontakt angenommen wird. Es steht in QR-Codes, die Menschen
    // ausgedruckt weitergeben.
    let id =
        Identity::generate(&mut DeterministicRng::new([7u8; 32]), true, 1_700_000_000).unwrap();
    let nutzlast = trust::qr_payload(
        &id.enc_pub().unwrap(),
        id.sig_pub().as_ref(),
        Some(&id.xwing_pub()),
    );

    assert!(
        nutzlast.starts_with("cabrik:v2:"),
        "das Praefix traegt die Formatfassung: {}",
        &nutzlast[..20.min(nutzlast.len())]
    );
    // Fuenf durch Doppelpunkt getrennte Teile hinter dem Praefix.
    assert_eq!(
        nutzlast.split(':').count(),
        6,
        "Aufbau: cabrik:v2:<enc>:<sig>:<xwing>:<fingerprint>"
    );
}

// ---------------------------------------------------------------------------
// Die Stromchiffre
// ---------------------------------------------------------------------------

#[test]
fn die_blockgroesse_steht_fest() {
    /*
     * Sie bestimmt, wo im Envelope jeder Block anfaengt. Eine Aenderung
     * machte jede bestehende Datei unlesbar -- und zwar OHNE dass die
     * Magic Bytes oder die Suite-Kennung es anzeigten. Der Leser fiele
     * mitten im Strom auseinander, und die Meldung lautete
     * "Authentifizierung fehlgeschlagen": ununterscheidbar von einer
     * mutwilligen Veraenderung.
     */
    assert_eq!(cabrik_core::stream::CHUNK_SIZE, 65_536);
    assert_eq!(cabrik_core::stream::TAG_LEN, 16);
    assert_eq!(
        cabrik_core::stream::CHUNK_CIPHERTEXT_SIZE,
        65_536 + 16,
        "Klartextblock plus Authentifizierungsmerkmal"
    );
}

#[test]
fn die_hoechstzahl_der_empfaenger_steht_fest() {
    // Sie steht als Zahl im Prolog. Ein Leser, der 32 erwartet und 64
    // vorfindet, muss abweisen -- also gehoert die Grenze zum Format.
    assert_eq!(envelope::MAX_RECIPIENTS, 32);
}

// ---------------------------------------------------------------------------
// Die Byteanordnung
// ---------------------------------------------------------------------------

/// Ein Envelope aus fester Zufallsquelle — Byte für Byte vergleichbar.
fn envelope_bauen(suite: Suite, inhalt: &[u8]) -> Vec<u8> {
    let mut rng = DeterministicRng::new([42u8; 32]);
    let empfaenger = Identity::generate(&mut rng, true, 1_700_000_000).unwrap();
    // Der Empfaenger ist der rohe oeffentliche Schluessel -- welcher, haengt
    // an der Suite: X25519 beim klassischen Weg, X-Wing beim hybriden.
    let pk: Vec<u8> = if suite == Suite::Hybrid {
        kem::pq_public_key(&empfaenger.pq_seed).to_vec()
    } else {
        kem::public_key(&empfaenger.enc_sk).unwrap().to_vec()
    };
    envelope::seal(
        suite,
        &[&pk[..]],
        None,
        inhalt,
        None,
        &envelope::SealOptions::default(),
        &mut rng,
    )
    .unwrap()
}

#[test]
fn der_prolog_beginnt_mit_magic_und_suite() {
    /*
     * Die ersten Bytes jeder Datei, in dieser Reihenfolge:
     *
     *     0..2   Magic          CA 02
     *     2..4   Suite          0001 oder 0002, gross-endian
     *
     * Das ist das Wenige, was ein fremder Leser lesen kann, BEVOR er
     * irgendetwas entschluesselt hat. Es entscheidet, ob er die Datei
     * ueberhaupt anfasst und mit welchem Verfahren.
     */
    for (suite, kennung) in [(Suite::Classical, 0x0001u16), (Suite::Hybrid, 0x0002)] {
        let e = envelope_bauen(suite, b"Inhalt");

        assert_eq!(&e[0..2], &[0xCA, 0x02], "Magic bei {suite:?}");
        assert_eq!(
            u16::from_be_bytes([e[2], e[3]]),
            kennung,
            "Suite-Kennung bei {suite:?}, gross-endian"
        );
    }
}

#[test]
fn dieselbe_eingabe_ergibt_denselben_envelope() {
    /*
     * Der Test, der den ganzen Rest traegt.
     *
     * Bei fester Zufallsquelle muss zweimal dasselbe herauskommen -- sonst
     * flieszt irgendwo eine Quelle mit, die niemand angegeben hat, und
     * jede Aussage ueber die Byteanordnung waere Zufall.
     *
     * Er ist zugleich die Bedingung dafuer, dass die Testvektoren unter
     * `testvectors/` ueberhaupt reproduzierbar sind.
     */
    let a = envelope_bauen(Suite::Hybrid, b"derselbe Inhalt");
    let b = envelope_bauen(Suite::Hybrid, b"derselbe Inhalt");

    assert_eq!(
        a, b,
        "die Erzeugung muss bei fester Quelle deterministisch sein"
    );
}

#[test]
fn das_keyfile_beginnt_mit_magic_und_fassung() {
    //     0..2   Magic     CA 4B
    //     2      Fassung   02
    let mut rng = DeterministicRng::new([9u8; 32]);
    let id = Identity::generate(&mut rng, true, 1_700_000_000).unwrap();
    let datei = keyfile::write(
        &id,
        b"vier zufaellige woerter hier",
        &cabrik_core::KdfParams {
            m_cost: cabrik_core::KdfParams::M_COST_MIN,
            t_cost: cabrik_core::KdfParams::T_COST_MIN,
            p_cost: 1,
        },
        cabrik_core::Speichergrenze::UNBEKANNT,
        &mut rng,
    )
    .unwrap();

    assert_eq!(&datei[0..2], &[0xCA, 0x4B], "Keyfile-Magic");
    assert_eq!(datei[2], 0x02, "Keyfile-Fassung");
}

// ---------------------------------------------------------------------------
// Die Rückschau
// ---------------------------------------------------------------------------

/// Die Prüfsumme über alle Envelopes aus [`rueckschau_faelle`], erzeugt mit
/// dem Stand **vor** der blockweisen Versiegelung (`main`, 73fc121, am
/// 29.10.2026).
///
/// Kein Testvektor im Sinne von `testvectors/README.md`: Den erzeugt eine
/// unabhängige Referenz, diese Zahl der Rust-Code selbst. Sie belegt nicht,
/// dass das Format stimmt, sondern dass es **dasselbe geblieben** ist — und
/// genau das stand mit dem Umbau auf dem Spiel. `seal` lief bis dahin in
/// einem Zug; seitdem setzt es sich aus `versiegeln_beginnen`, den Blöcken
/// und dem Abschluss zusammen (`spec/grosse-dateien.md` §3.1).
const RUECKSCHAU: &str = "ff2ff6730649b09d33be8e515b182f6a8be1a8871a14c0656a5e9251358aee82";

/// 81 Envelopes: beide Suiten, signiert und nicht, Text, Datei, Datei mit
/// Füllung, Längen um die Blockgrenze, Füllung über Blockgrenzen hinweg
/// und ein Passwort-Envelope.
fn rueckschau_faelle() -> Vec<Vec<u8>> {
    use envelope::{ContentType, SealOptions};

    fn bauen(
        seed: u8,
        suite: Suite,
        signiert: bool,
        opts: &SealOptions<'_>,
        inhalt: &[u8],
    ) -> Vec<u8> {
        let mut rng = DeterministicRng::new([seed; 32]);
        let empf = Identity::generate(&mut rng, false, 1_700_000_000).unwrap();
        let abs = Identity::generate(&mut rng, true, 1_700_000_000).unwrap();
        let pk: Vec<u8> = if suite == Suite::Hybrid {
            kem::pq_public_key(&empf.pq_seed).to_vec()
        } else {
            kem::public_key(&empf.enc_sk).unwrap().to_vec()
        };
        envelope::seal(
            suite,
            &[&pk[..]],
            None,
            inhalt,
            signiert.then_some(&abs),
            opts,
            &mut rng,
        )
        .unwrap()
    }

    let mut alle = Vec::new();
    for suite in [Suite::Classical, Suite::Hybrid] {
        for signiert in [false, true] {
            for (art, padding) in [
                (ContentType::Text, None),
                (ContentType::File, None),
                (ContentType::File, Some(true)),
            ] {
                for laenge in [0usize, 1, 65_535, 65_536, 65_537, 200_001] {
                    let inhalt: Vec<u8> = (0..laenge).map(|i| (i % 251) as u8).collect();
                    let opts = SealOptions {
                        content_type: art,
                        filename: (art == ContentType::File).then_some("bericht.pdf"),
                        timestamp: Some(1_700_000_123),
                        padding,
                        dummy_stanzas: laenge % 2 == 1,
                    };
                    alle.push(bauen(7, suite, signiert, &opts, &inhalt));
                }
            }
        }
    }

    // Füllung an den Blockgrenzen: 196 000 wird genau auf drei volle Blöcke
    // aufgefüllt, 4 194 305 um 131 071 Bytes — darin ein Block nur aus
    // Füllung.
    for suite in [Suite::Classical, Suite::Hybrid] {
        for signiert in [false, true] {
            for laenge in [196_000usize, 4_194_305] {
                let inhalt: Vec<u8> = (0..laenge).map(|i| (i % 253) as u8).collect();
                let opts = SealOptions {
                    content_type: ContentType::File,
                    filename: Some("gross.bin"),
                    timestamp: None,
                    padding: Some(true),
                    dummy_stanzas: false,
                };
                alle.push(bauen(8, suite, signiert, &opts, &inhalt));
            }
        }
    }

    // Einmal mit Passwort, signiert und gefüllt.
    let mut rng = DeterministicRng::new([9u8; 32]);
    let abs = Identity::generate(&mut rng, true, 1_700_000_000).unwrap();
    alle.push(
        envelope::seal(
            Suite::Classical,
            &[],
            Some(b"korrekt pferd batterie heftklammer"),
            b"Nur mit Passwort",
            Some(&abs),
            &SealOptions::default(),
            &mut rng,
        )
        .unwrap(),
    );
    alle
}

#[test]
fn ein_envelope_bleibt_byte_fuer_byte_derselbe() {
    let alle = rueckschau_faelle();
    assert_eq!(alle.len(), 81);

    let mut h = Sha256::new();
    for e in &alle {
        h.update(Sha256::digest(e));
    }
    let summe: [u8; 32] = h.finalize().into();
    let hex: String = summe.iter().map(|b| format!("{b:02x}")).collect();

    assert_eq!(
        hex, RUECKSCHAU,
        "seal schreibt andere Bytes als vor dem Umbau. Das ist ein Formatbruch, \
         keine Testpflege — siehe den Kopf dieser Datei"
    );
}
