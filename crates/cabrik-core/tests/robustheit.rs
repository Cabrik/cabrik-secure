//! Der Parser darf bei **keiner** Eingabe abstürzen.
//!
//! # Warum das eine eigene Prüfung braucht
//!
//! Alle anderen Tests füttern den Envelope-Leser mit Daten, die dieses
//! Projekt selbst erzeugt hat. Sie prüfen, ob der Leser zum eigenen
//! Schreiber passt — nicht, was er mit einer Datei anstellt, die ein
//! **Angreifer** geschickt hat.
//!
//! Ein Envelope kommt immer von außen. Er ist die einzige Eingabe, die ein
//! Gegner vollständig unter Kontrolle hat, und sie wird verarbeitet, **bevor**
//! irgendetwas beglaubigt ist: Längenfelder, Anzahlen und Versätze müssen
//! gelesen werden, um überhaupt an die Beglaubigung zu kommen.
//!
//! Ein Absturz wäre hier kein Schönheitsfehler. Er ist der Unterschied
//! zwischen „die Datei wird abgelehnt" und „das Programm bricht ab" — bei
//! einem Werkzeug, das Journalisten benutzen sollen, ein Zustand, den ein
//! Gegner auslösen können soll.
//!
//! # Zwei Hälften
//!
//! - **Hier**: gezielte Verstümmelung mit festem Startwert, läuft auf stable
//!   in jedem `cargo test`. Deterministisch, also reproduzierbar.
//! - **`fuzz/`**: `cargo fuzz` mit libFuzzer, braucht nightly und läuft
//!   stundenlang. Findet, was hier nicht gedacht wurde.
//!
//! Was das Fuzzing findet, wird als Datei unter `testvectors/fuzz/` abgelegt
//! und von [`korpus_bleibt_beherrschbar`] für immer mitgeprüft. Fuzzing
//! findet, der Korpus hält fest.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use cabrik_core::envelope::{self, Opener, SealOptions};
use cabrik_core::rng::OsRandom;
use cabrik_core::suite::Suite;
use cabrik_core::{Identity, kem};

/// Ein Zufallszahlengeber mit festem Startwert.
///
/// Bewusst **nicht** der aus dem Kern: Der ist für Schlüssel da. Hier geht es
/// nur darum, immer dieselbe Folge von Verstümmelungen zu erzeugen — schlägt
/// der Test fehl, lässt er sich mit derselben Nummer nachstellen.
struct Wuerfel(u64);

impl Wuerfel {
    const fn neu(saat: u64) -> Self {
        Self(saat ^ 0x2545_F491_4F6C_DD1D)
    }

    fn naechste(&mut self) -> u64 {
        // xorshift64*, ausreichend für Testdaten.
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn bis(&mut self, grenze: usize) -> usize {
        if grenze == 0 {
            return 0;
        }
        usize::try_from(self.naechste() % grenze as u64).unwrap_or(0)
    }
}

fn identitaet() -> Identity {
    Identity::generate(&mut OsRandom, false, 1_700_000_000).unwrap()
}

fn gueltiger_envelope(id: &Identity) -> Vec<u8> {
    let pk = kem::public_key(&id.enc_sk).unwrap();
    envelope::seal(
        Suite::Classical,
        &[&pk[..]],
        None,
        b"Der Inhalt spielt hier keine Rolle, nur die Struktur.",
        None,
        &SealOptions::default(),
        &mut OsRandom,
    )
    .unwrap()
}

/// Die Arten von Verstümmelung, die ein Parser aushalten muss.
///
/// Sie sind nicht zufällig gewählt: Jede zielt auf eine andere Annahme, die
/// ein Leser stillschweigend treffen könnte.
#[derive(Debug, Clone, Copy)]
enum Angriff {
    /// Ein einzelnes Bit kippt. Trifft irgendeine Prüfsumme oder Kennung.
    BitKippen,
    /// Ein Byte wird ersetzt. Gröber als ein Bit, trifft auch Typfelder.
    ByteErsetzen,
    /// Die Datei bricht mittendrin ab. **Der häufigste Fall in freier
    /// Wildbahn** — abgebrochene Übertragung, volle Platte.
    Abschneiden,
    /// Ein Längenfeld wird auf einen riesigen Wert gesetzt. Zielt auf
    /// Zuweisungen, die der Angabe vertrauen, und auf Überläufe.
    LaengeAufblaehen,
    /// Ein Längenfeld wird auf null gesetzt. Zielt auf Schleifen, die
    /// Fortschritt annehmen.
    LaengeNullen,
    /// Ein Stück wird verdoppelt. Zielt auf Zustand, der zwischen zwei
    /// Durchläufen hängen bleibt.
    Verdoppeln,
}

const ANGRIFFE: [Angriff; 6] = [
    Angriff::BitKippen,
    Angriff::ByteErsetzen,
    Angriff::Abschneiden,
    Angriff::LaengeAufblaehen,
    Angriff::LaengeNullen,
    Angriff::Verdoppeln,
];

fn verstuemmle(vorlage: &[u8], w: &mut Wuerfel) -> (Vec<u8>, Angriff) {
    let mut v = vorlage.to_vec();
    if v.is_empty() {
        return (v, Angriff::BitKippen);
    }
    let art = ANGRIFFE[w.bis(ANGRIFFE.len())];

    match art {
        Angriff::BitKippen => {
            let i = w.bis(v.len());
            v[i] ^= 1u8 << (w.bis(8));
        }
        Angriff::ByteErsetzen => {
            let i = w.bis(v.len());
            v[i] = u8::try_from(w.naechste() & 0xFF).unwrap_or(0);
        }
        Angriff::Abschneiden => {
            let n = w.bis(v.len());
            v.truncate(n);
        }
        Angriff::LaengeAufblaehen => {
            if v.len() >= 4 {
                let i = w.bis(v.len() - 3);
                v[i..i + 4].copy_from_slice(&u32::MAX.to_be_bytes());
            }
        }
        Angriff::LaengeNullen => {
            if v.len() >= 4 {
                let i = w.bis(v.len() - 3);
                v[i..i + 4].copy_from_slice(&0u32.to_be_bytes());
            }
        }
        Angriff::Verdoppeln => {
            let von = w.bis(v.len());
            let bis = (von + 1 + w.bis(64)).min(v.len());
            let stueck = v[von..bis].to_vec();
            v.splice(bis..bis, stueck);
        }
    }
    (v, art)
}

/// **Die eigentliche Prüfung.** Zehntausend verstümmelte Envelopes, und
/// keiner darf das Programm abbrechen.
///
/// Der Rückgabewert ist gleichgültig — `Err` ist das erwartete Ergebnis.
/// Geprüft wird allein, dass die Funktion **zurückkehrt**.
#[test]
fn kein_verstuemmelter_envelope_bringt_den_leser_zum_absturz() {
    let id = identitaet();
    let vorlage = gueltiger_envelope(&id);
    let opener = Opener::Identity(&id);

    let mut geoeffnet = 0usize;
    let mut abgelehnt = 0usize;

    for saat in 0..10_000u64 {
        let mut w = Wuerfel::neu(saat);
        let (kaputt, art) = verstuemmle(&vorlage, &mut w);

        // Panikt das hier, schlägt der Test fehl -- mit der Saat in der
        // Meldung, also nachstellbar.
        match std::panic::catch_unwind(|| envelope::open(&opener, &kaputt, false)) {
            Ok(Ok(_)) => geoeffnet += 1,
            Ok(Err(_)) => abgelehnt += 1,
            Err(_) => panic!("Absturz bei Saat {saat} durch {art:?}"),
        }
    }

    // Fast alles muss abgelehnt werden. Dass ein paar durchgehen, ist
    // möglich -- etwa wenn die Verstümmelung nur Füllbytes traf.
    assert_eq!(
        geoeffnet + abgelehnt,
        10_000,
        "es wurden nicht alle Fälle durchlaufen"
    );
    assert!(
        abgelehnt > 9_000,
        "nur {abgelehnt} von 10000 wurden abgelehnt — die Verstümmelung trifft nichts"
    );
}

/// Auch mit einem Passwort-Öffner, denn er sucht anders nach seiner Kapsel.
///
/// # Warum hier ein Envelope **ohne** Passwortkapsel verstümmelt wird
///
/// Argon2 ist **mit Absicht langsam** — das ist sein Zweck. Gemessen im
/// Debug-Build: **6,8 Sekunden für einen einzigen Lauf** bei den
/// voreingestellten 256 MiB und drei Durchgängen. Jede Verstümmelung, die
/// eine Passwortkapsel strukturell heil lässt und nur ihr Salz trifft,
/// kostet genau so viel. Ein paar hundert davon dauern länger als die
/// gesamte übrige Testreihe — zwei Anläufe mit anderen Zuschnitten liefen
/// beide in die Zeitgrenze.
///
/// Der Ausweg ist kein Kompromiss, sondern der treffendere Zuschnitt: Ein
/// Envelope **ohne** Passwortkapsel, geöffnet mit einem Passwort-Öffner,
/// durchläuft die vollständige Kapselsuche und den gesamten gemeinsamen
/// Leser — und kommt nie zur Ableitung, weil es nichts zu entpacken gibt.
/// Genau dieser Weg ist der interessante: Ein Angreifer schickt einem
/// Empfänger, was er will, und der Empfänger versucht es mit einem Passwort.
///
/// Die Ableitung selbst wird darunter **einmal** angefasst, mit falschem
/// Passwort. Mehr braucht es nicht: Sie ist eine fremde, geprüfte Bibliothek
/// hinter einer Parameterprüfung, kein selbstgeschriebener Leser.
#[test]
fn auch_der_passwort_oeffner_haelt_verstuemmelung_aus() {
    let id = identitaet();
    let vorlage = gueltiger_envelope(&id);

    for saat in 0..3_000u64 {
        let mut w = Wuerfel::neu(saat ^ 0xA5A5);
        let (kaputt, art) = verstuemmle(&vorlage, &mut w);
        let ergebnis = std::panic::catch_unwind(|| {
            envelope::open(&Opener::Password(b"geheim"), &kaputt, false)
        });
        assert!(ergebnis.is_ok(), "Absturz bei Saat {saat} durch {art:?}");
        assert!(
            ergebnis.unwrap().is_err(),
            "Saat {saat}: geöffnet, obwohl es keine Passwortkapsel gibt"
        );
    }
}

/// Ein echter Passwort-Envelope mit falschem Passwort — genau ein Lauf.
///
/// Er kostet die vollen 6,8 Sekunden aus dem Test darüber. Wert ist er sie
/// trotzdem: Er belegt, dass der teure Pfad bis zum Ende durchläuft und mit
/// einem Fehler zurückkehrt, statt abzubrechen.
#[test]
fn ein_falsches_passwort_wird_abgelehnt_statt_abzustuerzen() {
    let env = envelope::seal(
        Suite::Classical,
        &[],
        Some(b"richtig"),
        b"Nachricht",
        None,
        &SealOptions::default(),
        &mut OsRandom,
    )
    .unwrap();

    let ergebnis =
        std::panic::catch_unwind(|| envelope::open(&Opener::Password(b"falsch"), &env, false));
    assert!(ergebnis.is_ok(), "Absturz beim falschen Passwort");
    assert!(
        ergebnis.unwrap().is_err(),
        "das falsche Passwort ging durch"
    );

    // Und die Kopfbytes verstümmelt: Das bricht ab, bevor die Ableitung
    // anläuft, kostet also nichts.
    for i in 0..8usize {
        let mut kaputt = env.clone();
        kaputt[i] ^= 0xFF;
        let ergebnis = std::panic::catch_unwind(|| {
            envelope::open(&Opener::Password(b"richtig"), &kaputt, false)
        });
        assert!(ergebnis.is_ok(), "Absturz bei verstuemmeltem Byte {i}");
    }
}

/// Eingaben, die kein Envelope sind — kurz, leer, Zufall, wiederholte Muster.
#[test]
fn auch_voelliger_unsinn_wird_nur_abgelehnt() {
    let id = identitaet();
    let opener = Opener::Identity(&id);

    let mut faelle: Vec<Vec<u8>> = vec![
        Vec::new(),
        vec![0u8],
        vec![0xFFu8; 3],
        b"CABRIK".to_vec(),
        vec![0u8; 4096],
        vec![0xFFu8; 4096],
    ];
    // Reiner Zufall in verschiedenen Längen.
    let mut w = Wuerfel::neu(4711);
    for laenge in [1usize, 7, 63, 64, 65, 255, 1023] {
        faelle.push(
            (0..laenge)
                .map(|_| u8::try_from(w.naechste() & 0xFF).unwrap_or(0))
                .collect(),
        );
    }

    for (i, f) in faelle.iter().enumerate() {
        let ergebnis = std::panic::catch_unwind(|| envelope::open(&opener, f, false));
        assert!(ergebnis.is_ok(), "Absturz bei Fall {i} ({} Bytes)", f.len());
        assert!(
            ergebnis.unwrap().is_err(),
            "Fall {i} wurde geöffnet, obwohl es kein Envelope ist"
        );
    }
}

/// Was das Fuzzing gefunden hat, bleibt für immer geprüft.
///
/// Ein leeres Verzeichnis ist kein Fehler — es heißt nur, dass noch nichts
/// gefunden wurde. Sobald `cargo fuzz` einen Absturz meldet, gehört die
/// auslösende Datei hierher, **bevor** der Fehler behoben wird.
#[test]
fn korpus_bleibt_beherrschbar() {
    let ordner = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testvectors/fuzz/envelope");
    let Ok(eintraege) = std::fs::read_dir(&ordner) else {
        eprintln!("kein Korpus unter {} -- uebersprungen", ordner.display());
        return;
    };

    let id = identitaet();
    let opener = Opener::Identity(&id);
    let mut geprueft = 0usize;

    for e in eintraege.flatten() {
        let pfad = e.path();
        if !pfad.is_file() {
            continue;
        }
        let Ok(daten) = std::fs::read(&pfad) else {
            continue;
        };
        let ergebnis = std::panic::catch_unwind(|| {
            let _ = envelope::open(&opener, &daten, false);
            let _ = envelope::open(&Opener::Password(b"pw"), &daten, false);
        });
        assert!(
            ergebnis.is_ok(),
            "Absturz bei {} -- ein alter Fehler ist zurueck",
            pfad.display()
        );
        geprueft += 1;
    }
    eprintln!("{geprueft} Korpusdatei(en) geprueft");
}
