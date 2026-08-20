//! Was das Fenster tut — von der leeren Ablage bis zur sichtbaren Identität.
//!
//! # Warum das hier steht und nicht in einem der beiden Kästen
//!
//! Weil der Fehler dazwischen lag. `befehle.rs` prüft die Sitzung, ohne je
//! eine Datei anzufassen; `dateien.rs` prüft die Ablage, ohne zu wissen, was
//! in den Bytes steht. Beide waren grün, während der Weg, den ein Mensch
//! geht, nicht funktionierte.
//!
//! Diese Datei geht ihn ab: anlegen, schreiben, Fenster schließen, neu
//! laden, entsperren, anzeigen.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "Fehlschlag soll den Test abbrechen"
)]

use cabrik_app::Sitzung;
use cabrik_bruecke::{KdfStufe, Sperrfrist};
use cabrik_core::OsRandom;
use std::path::PathBuf;
use zeroize::Zeroizing;

const PASSWORT: &str = "vier zufaellige woerter hier";

fn werkbank(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("cabrik-rundweg-{name}"));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("Verzeichnis");
    p
}

fn passwort() -> Zeroizing<Vec<u8>> {
    Zeroizing::new(PASSWORT.as_bytes().to_vec())
}

/// Wie `identitaet_anlegen` im Fenster: anlegen, dann schreiben.
fn anlegen(pfad: &std::path::Path, jetzt: u64) -> Sitzung {
    let s = Sitzung::anlegen(
        Some("Cabrik".to_owned()),
        &passwort(),
        true,
        KdfStufe::Min,
        Sperrfrist::FuenfzehnMinuten,
        jetzt,
        &mut OsRandom,
    )
    .expect("anlegen");
    cabrik_ablage::schreib_neu(pfad, s.schluesseldatei()).expect("schreiben");
    s
}

#[test]
fn nach_dem_anlegen_ist_die_identitaet_abrufbar() {
    // Genau der gemeldete Fehler: Der Fingerprint stand auf dem
    // Abschlussbildschirm, aber der nächste Abruf lieferte nichts.
    let verzeichnis = werkbank("abrufbar");
    let pfad = verzeichnis.join("identity.cabrik-key");
    let mut s = anlegen(&pfad, 1_000);

    let datei = s.schluesseldatei().to_vec();
    let i = s
        .offen(1_000)
        .expect("offen")
        .identitaet(&datei, pfad.display().to_string())
        .expect("Identitaet");

    assert_eq!(i.bezeichnung.as_deref(), Some("Cabrik"));
    assert!(!i.fingerprint.is_empty());
}

#[test]
fn zweimal_hintereinander_abrufen_geht_auch() {
    // Die Oberfläche fragt im Sekundentakt nach. Ein Abruf, der nur beim
    // ersten Mal gelingt, fiele erst eine Sekunde nach dem Anlegen auf --
    // also lange nach jedem Klick.
    let verzeichnis = werkbank("zweimal");
    let pfad = verzeichnis.join("identity.cabrik-key");
    let mut s = anlegen(&pfad, 1_000);
    let datei = s.schluesseldatei().to_vec();

    for runde in 1..=3_u64 {
        let jetzt = 1_000 + runde;
        s.offen(jetzt)
            .expect("offen")
            .identitaet(&datei, pfad.display().to_string())
            .unwrap_or_else(|e| panic!("Runde {runde}: {}", e.meldung));
    }
}

#[test]
fn die_datei_liegt_danach_wirklich_da() {
    let verzeichnis = werkbank("geschrieben");
    let pfad = verzeichnis.join("identity.cabrik-key");
    let s = anlegen(&pfad, 1_000);

    let von_platte = cabrik_ablage::lies(&pfad).expect("lesen").expect("da");
    assert_eq!(von_platte, s.schluesseldatei());
}

#[test]
fn der_naechste_start_findet_sie_und_oeffnet_sie() {
    // Fenster zu, Fenster auf. Der Weg, auf dem eine Identität verloren
    // ginge, ohne dass es beim Anlegen aufgefallen wäre.
    let verzeichnis = werkbank("neustart");
    let pfad = verzeichnis.join("identity.cabrik-key");
    drop(anlegen(&pfad, 1_000));

    let schluessel = cabrik_ablage::lies(&pfad).expect("lesen").expect("da");
    let mut zweite = Sitzung::neu(schluessel, None, Sperrfrist::FuenfzehnMinuten);
    zweite.entsperren(&passwort(), 2_000).expect("entsperren");

    let datei = zweite.schluesseldatei().to_vec();
    let i = zweite
        .offen(2_000)
        .expect("offen")
        .identitaet(&datei, pfad.display().to_string())
        .expect("Identitaet");

    assert_eq!(i.bezeichnung.as_deref(), Some("Cabrik"));
}

#[test]
fn eine_zweite_identitaet_ueberschreibt_die_erste_nicht() {
    // Die Sperre, auf die es am meisten ankommt -- hier über den Weg
    // geprüft, den das Fenster nimmt.
    let verzeichnis = werkbank("keine-zweite");
    let pfad = verzeichnis.join("identity.cabrik-key");
    let erste = anlegen(&pfad, 1_000);
    let vorher = erste.schluesseldatei().to_vec();

    let zweite = Sitzung::anlegen(
        Some("Noch eine".to_owned()),
        &passwort(),
        true,
        KdfStufe::Min,
        Sperrfrist::FuenfzehnMinuten,
        2_000,
        &mut OsRandom,
    )
    .expect("anlegen");

    assert!(
        cabrik_ablage::schreib_neu(&pfad, zweite.schluesseldatei()).is_err(),
        "die bestehende Datei darf nicht ueberschrieben werden"
    );
    assert_eq!(
        cabrik_ablage::lies(&pfad).expect("lesen"),
        Some(vorher),
        "und sie muss unveraendert dastehen"
    );
}

#[test]
fn ein_verwaister_kontaktspeicher_sperrt_die_neue_identitaet_nicht_aus() {
    // Der Fund vom 14. August: Auf dem Rechner lag ein Kontaktspeicher vom
    // 12., versiegelt an eine Identitaet, die es nicht mehr gab. Beim
    // naechsten Start laedt das Fenster beide zusammen -- und `entsperren`
    // scheitert an der Kontaktdatei, mit RICHTIGEM Passwort. Die Identitaet
    // waere dauerhaft unerreichbar gewesen, ohne dass irgendetwas darauf
    // hingewiesen haette.
    let verzeichnis = werkbank("waise");
    let schluesselpfad = verzeichnis.join("identity.cabrik-key");
    let kontaktpfad = verzeichnis.join("contacts.cabrik-contacts");

    // Eine erste Identitaet mit Kontakten.
    let mut erste = anlegen(&schluesselpfad, 1_000);
    let n = cabrik_core::trust::qr_payload(&[0x11; 32], Some(&[0x21; 32]), None);
    erste
        .offen(1_000)
        .expect("offen")
        .kontakt_aus_nutzlast("Anna", &n, 1_000)
        .expect("aufnehmen");
    let gesichert = erste
        .kontakte_sichern(1_000, &mut OsRandom)
        .expect("sichern");
    cabrik_ablage::schreib_atomar(&kontaktpfad, &gesichert).expect("schreiben");

    // Sie wird geloescht -- wie `identitaet_loeschen`, aber ohne den
    // Kontaktspeicher mitzunehmen. Genau so entstand die Lage: durch die
    // CLI, durch einen Absturz, durch eine frueheare Fassung.
    drop(erste);
    cabrik_ablage::loesche(&schluesselpfad).expect("loeschen");
    assert!(kontaktpfad.exists(), "die Waise liegt noch da");

    // Eine neue Identitaet -- wie `identitaet_anlegen` es heute tut.
    drop(anlegen(&schluesselpfad, 2_000));
    cabrik_ablage::verschiebe_beiseite(&kontaktpfad).expect("beiseite");

    // Und jetzt der naechste Start.
    let schluessel = cabrik_ablage::lies(&schluesselpfad)
        .expect("lesen")
        .expect("da");
    let kontakte = cabrik_ablage::lies(&kontaktpfad).ok().flatten();
    let mut neu = Sitzung::neu(schluessel, kontakte, Sperrfrist::FuenfzehnMinuten);

    neu.entsperren(&passwort(), 3_000)
        .expect("das richtige Passwort muss aufgehen");
    assert!(
        neu.offen(3_000).expect("offen").kontakte().is_empty(),
        "die neue Identitaet beginnt mit einem leeren Verzeichnis"
    );
}

#[test]
fn ohne_beiseiteschieben_waere_die_identitaet_verloren() {
    // Die Gegenprobe. Sie haelt fest, was schiefging -- und macht
    // sichtbar, dass der Schritt oben kein Beiwerk ist.
    let verzeichnis = werkbank("waise-gegenprobe");
    let schluesselpfad = verzeichnis.join("identity.cabrik-key");
    let kontaktpfad = verzeichnis.join("contacts.cabrik-contacts");

    let mut erste = anlegen(&schluesselpfad, 1_000);
    let gesichert = erste
        .kontakte_sichern(1_000, &mut OsRandom)
        .expect("sichern");
    cabrik_ablage::schreib_atomar(&kontaktpfad, &gesichert).expect("schreiben");
    drop(erste);
    cabrik_ablage::loesche(&schluesselpfad).expect("loeschen");

    drop(anlegen(&schluesselpfad, 2_000));
    // HIER wird NICHT beiseitegeschoben.

    let schluessel = cabrik_ablage::lies(&schluesselpfad)
        .expect("lesen")
        .expect("da");
    let kontakte = cabrik_ablage::lies(&kontaktpfad).ok().flatten();
    let mut neu = Sitzung::neu(schluessel, kontakte, Sperrfrist::FuenfzehnMinuten);

    let fehler = neu
        .entsperren(&passwort(), 3_000)
        .expect_err("genau das ging schief");

    assert_eq!(fehler.betrifft, cabrik_app::Betroffen::Kontaktspeicher);
    assert!(
        fehler.meldung.contains("anderen Identität"),
        "{}",
        fehler.meldung
    );
}
