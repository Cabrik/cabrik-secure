//! Die Zwischendatei: erst daneben, dann umbenennen
//! (`spec/grosse-dateien.md` §4).
//!
//! Drei Zusagen:
//!
//! 1. **Unter dem Zielnamen steht nie eine halbe Datei.** Bis zum
//!    Umbenennen heißt sie `.<ziel>.cabrik-teil`.
//! 2. **Fremdes bleibt unberührt.** Weder eine Datei des Nutzers neben dem
//!    Ziel noch ein Ziel, das inzwischen jemand angelegt hat.
//! 3. **Was nicht fertig wird, verschwindet** — beim Verwerfen, beim
//!    Fallenlassen und wenn der Kern die Prüfung verweigert.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::{Path, PathBuf};

use cabrik_ablage::{Zwischendatei, teil_pfad};
use cabrik_core::envelope::{self, KOPF_HOECHSTENS, Oeffnungsfehler, Opener, SealOptions};
use cabrik_core::{Identity, OsRandom, Suite, kem};

/// Ein eigenes Verzeichnis je Test, damit sie sich nicht ins Gehege kommen.
fn werkbank(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("cabrik-zwischendatei-{name}"));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("Verzeichnis");
    p
}

/// Was im Verzeichnis liegt, sortiert.
fn inhalt(ordner: &Path) -> Vec<String> {
    let mut namen: Vec<String> = std::fs::read_dir(ordner)
        .expect("lesen")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    namen.sort();
    namen
}

// ---------------------------------------------------------------------------
// 1. Erst daneben
// ---------------------------------------------------------------------------

#[test]
fn die_zwischendatei_heisst_wie_das_ziel_mit_punkt_und_teil() {
    let ziel = Path::new("ordner").join("bericht.pdf");
    assert_eq!(
        teil_pfad(&ziel).unwrap(),
        Path::new("ordner").join(".bericht.pdf.cabrik-teil")
    );
}

#[test]
fn bis_zum_umbenennen_steht_unter_dem_zielnamen_nichts() {
    let ordner = werkbank("daneben");
    let ziel = ordner.join("urlaub.mp4");

    let mut z = Zwischendatei::neben(&ziel).unwrap();
    z.schreiben(b"erster Teil, ").unwrap();
    z.schreiben(b"zweiter Teil").unwrap();
    assert!(
        !ziel.exists(),
        "das Ziel stand schon da, bevor es fertig war"
    );
    assert_eq!(inhalt(&ordner), [".urlaub.mp4.cabrik-teil"]);
    assert_eq!(z.geschrieben(), 25);

    assert_eq!(z.als_neu().unwrap(), ziel);
    assert_eq!(std::fs::read(&ziel).unwrap(), b"erster Teil, zweiter Teil");
    assert_eq!(inhalt(&ordner), ["urlaub.mp4"]);
}

// ---------------------------------------------------------------------------
// 2. Fremdes bleibt unberührt
// ---------------------------------------------------------------------------

#[test]
fn eine_tmp_datei_des_nutzers_bleibt_unberuehrt() {
    // Bis Oktober 2026 hiess die Zwischendatei `<ziel ohne Endung>.tmp`
    // und wurde mit `File::create` angelegt: Diese Datei waere
    // ueberschrieben und danach umbenannt worden -- fort.
    let ordner = werkbank("fremde-tmp");
    std::fs::write(ordner.join("bericht.tmp"), b"meins").unwrap();

    cabrik_ablage::schreib_neu(&ordner.join("bericht.pdf"), b"neu").unwrap();
    cabrik_ablage::schreib_atomar(&ordner.join("bericht.pdf"), b"neuer").unwrap();

    assert_eq!(std::fs::read(ordner.join("bericht.tmp")).unwrap(), b"meins");
    assert_eq!(std::fs::read(ordner.join("bericht.pdf")).unwrap(), b"neuer");
    assert_eq!(inhalt(&ordner), ["bericht.pdf", "bericht.tmp"]);
}

#[test]
fn als_neu_ueberschreibt_kein_ziel_das_inzwischen_dasteht() {
    let ordner = werkbank("inzwischen");
    let ziel = ordner.join("bild.jpg");

    let mut z = Zwischendatei::neben(&ziel).unwrap();
    z.schreiben(b"entschluesselt").unwrap();
    std::fs::write(&ziel, b"fremd").unwrap();

    let e = z.als_neu().unwrap_err();
    assert!(e.meldung.contains("bild.jpg"), "{}", e.meldung);
    assert_eq!(std::fs::read(&ziel).unwrap(), b"fremd");
    assert_eq!(
        inhalt(&ordner),
        ["bild.jpg"],
        "die Zwischendatei blieb liegen"
    );
}

#[test]
fn ersetzend_ersetzt() {
    let ordner = werkbank("ersetzend");
    let ziel = ordner.join("contacts.cabrik-contacts");
    std::fs::write(&ziel, b"alt und lang").unwrap();

    let mut z = Zwischendatei::neben(&ziel).unwrap();
    z.schreiben(b"neu").unwrap();
    z.ersetzend().unwrap();

    assert_eq!(std::fs::read(&ziel).unwrap(), b"neu");
    assert_eq!(inhalt(&ordner), ["contacts.cabrik-contacts"]);
}

#[test]
fn ein_alter_rest_desselben_ziels_wird_entfernt_und_gemeldet() {
    // Ein Absturz mitten im Ablegen: Die Zwischendatei blieb liegen, mit
    // ungeprueftem Inhalt. Beim naechsten Mal unter demselben Namen wird
    // sie weggeraeumt -- und gesagt.
    let ordner = werkbank("alter-rest");
    let ziel = ordner.join("nachricht.txt");
    std::fs::write(teil_pfad(&ziel).unwrap(), b"halber Rest").unwrap();

    let mut z = Zwischendatei::neben(&ziel).unwrap();
    assert!(z.alter_rest_entfernt());
    z.schreiben(b"ganz").unwrap();
    z.als_neu().unwrap();

    assert_eq!(std::fs::read(&ziel).unwrap(), b"ganz");
    assert_eq!(inhalt(&ordner), ["nachricht.txt"]);
}

#[test]
fn reste_anderer_ziele_bleiben_liegen() {
    // Ob gerade jemand anderes dorthin schreibt, weiss diese Schicht nicht.
    let ordner = werkbank("andere-reste");
    let fremd = teil_pfad(&ordner.join("anderes.mp4")).unwrap();
    std::fs::write(&fremd, b"laeuft vielleicht noch").unwrap();

    let z = Zwischendatei::neben(&ordner.join("bericht.pdf")).unwrap();
    assert!(!z.alter_rest_entfernt());
    z.als_neu().unwrap();

    assert!(fremd.exists());
}

// ---------------------------------------------------------------------------
// 3. Was nicht fertig wird, verschwindet
// ---------------------------------------------------------------------------

#[test]
fn verwerfen_raeumt_auf() {
    let ordner = werkbank("verwerfen");
    let mut z = Zwischendatei::neben(&ordner.join("x.bin")).unwrap();
    z.schreiben(b"weg damit").unwrap();
    z.verwerfen();
    assert!(inhalt(&ordner).is_empty());
}

#[test]
fn fallen_lassen_raeumt_auf() {
    // Der Aufrufer bricht ab -- seine Quelle las nicht weiter, oder ein
    // Fehler kam von woanders. Dann darf nichts liegen bleiben.
    let ordner = werkbank("fallen-lassen");
    {
        let mut z = Zwischendatei::neben(&ordner.join("x.bin")).unwrap();
        z.schreiben(b"angefangen").unwrap();
    }
    assert!(inhalt(&ordner).is_empty());
}

// ---------------------------------------------------------------------------
// Mit dem Kern: blockweise öffnen in eine Datei
// ---------------------------------------------------------------------------

/// Ein signierter Envelope an `id`.
fn envelope_an(id: &Identity, klartext: &[u8]) -> Vec<u8> {
    let pk = kem::public_key(&id.enc_sk).unwrap();
    let opts = SealOptions {
        content_type: cabrik_core::ContentType::File,
        filename: Some("urlaub.mp4"),
        ..SealOptions::default()
    };
    envelope::seal(
        Suite::Classical,
        &[&pk[..]],
        None,
        klartext,
        Some(id),
        &opts,
        &mut OsRandom,
    )
    .unwrap()
}

/// Öffnet blockweise in eine Zwischendatei neben `ziel`.
fn oeffnen_nach(
    env: &[u8],
    id: &Identity,
    ziel: &Path,
) -> Result<envelope::Bericht, Oeffnungsfehler<cabrik_ablage::Ablagefehler>> {
    let mut z = Zwischendatei::neben(ziel).unwrap();
    let anfang = &env[..env.len().min(KOPF_HOECHSTENS)];
    let (mut o, verbraucht) = envelope::oeffnen_beginnen(&Opener::Identity(id), anfang, false)?;
    for stueck in env[verbraucht..].chunks(100_000) {
        o.weiter(stueck, &mut z)?;
    }
    o.abschliessen(&mut z)
}

#[test]
fn ein_gepruefter_envelope_steht_danach_unter_dem_ziel() {
    let ordner = werkbank("kern-gut");
    let id = Identity::generate(&mut OsRandom, true, 1_700_000_000).unwrap();
    let klartext: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
    let env = envelope_an(&id, &klartext);

    let ziel = ordner.join("urlaub.mp4");
    let bericht = oeffnen_nach(&env, &id, &ziel).unwrap();

    assert_eq!(bericht.filename.as_deref(), Some("urlaub.mp4"));
    assert_eq!(std::fs::read(&ziel).unwrap(), klartext);
    assert_eq!(inhalt(&ordner), ["urlaub.mp4"]);
}

#[test]
fn ein_veraenderter_envelope_hinterlaesst_nichts() {
    // Die Bloecke gingen ungeprueft in die Zwischendatei. Scheitert danach
    // die Signatur, darf davon nichts bleiben -- weder unter dem Ziel noch
    // daneben.
    let ordner = werkbank("kern-veraendert");
    let id = Identity::generate(&mut OsRandom, true, 1_700_000_000).unwrap();
    let mut env = envelope_an(&id, &[7u8; 300_000]);
    let letzt = env.len() - 1;
    env[letzt] ^= 1;

    match oeffnen_nach(&env, &id, &ordner.join("urlaub.mp4")) {
        Err(Oeffnungsfehler::Kern(e)) => assert_eq!(e.code(), "SIGNATURE_INVALID"),
        anders => panic!("erwartet war SIGNATURE_INVALID, kam {anders:?}"),
    }
    assert!(
        inhalt(&ordner).is_empty(),
        "liegen blieb: {:?}",
        inhalt(&ordner)
    );
}

#[test]
fn ein_abgeschnittener_envelope_hinterlaesst_nichts() {
    let ordner = werkbank("kern-abgeschnitten");
    let id = Identity::generate(&mut OsRandom, true, 1_700_000_000).unwrap();
    let env = envelope_an(&id, &[7u8; 300_000]);

    match oeffnen_nach(&env[..env.len() - 5_000], &id, &ordner.join("urlaub.mp4")) {
        Err(Oeffnungsfehler::Kern(e)) => assert_eq!(e.code(), "TRUNCATED"),
        anders => panic!("erwartet war TRUNCATED, kam {anders:?}"),
    }
    assert!(
        inhalt(&ordner).is_empty(),
        "liegen blieb: {:?}",
        inhalt(&ordner)
    );
}

#[test]
fn ein_ziel_das_inzwischen_dasteht_bleibt_unberuehrt() {
    let ordner = werkbank("kern-inzwischen");
    let id = Identity::generate(&mut OsRandom, true, 1_700_000_000).unwrap();
    let env = envelope_an(&id, b"geheim");
    let ziel = ordner.join("urlaub.mp4");

    let mut z = Zwischendatei::neben(&ziel).unwrap();
    let (mut o, verbraucht) =
        envelope::oeffnen_beginnen(&Opener::Identity(&id), &env, false).unwrap();
    o.weiter(&env[verbraucht..], &mut z).unwrap();
    std::fs::write(&ziel, b"fremd").unwrap();

    match o.abschliessen(&mut z) {
        Err(Oeffnungsfehler::Ablage(e)) => assert!(e.meldung.contains("urlaub.mp4")),
        anders => panic!("erwartet war der Ablagefehler, kam {anders:?}"),
    }
    drop(z);
    assert_eq!(std::fs::read(&ziel).unwrap(), b"fremd");
    assert_eq!(inhalt(&ordner), ["urlaub.mp4"]);
}
