//! Was die Ablage leisten muss.
//!
//! Zwei Eigenschaften tragen alles Weitere:
//!
//! 1. **Eine fehlende Datei ist kein Fehler.** Beim ersten Start gibt es
//!    weder Schlüssel noch Kontakte, und das ist der Normalfall — nicht
//!    eine Störung, über die jemand eine Meldung lesen müsste.
//! 2. **Ein Schreibvorgang ist unteilbar.** Ein Absturz mittendrin darf
//!    nicht alle Kontakte vernichten.

#![expect(clippy::expect_used, reason = "Fehlschlag soll den Test abbrechen")]

use std::path::PathBuf;

/// Ein eigenes Verzeichnis je Test, damit sie sich nicht ins Gehege kommen.
fn werkbank(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("cabrik-ablage-{name}"));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("Verzeichnis");
    p
}

#[test]
fn eine_fehlende_datei_ist_kein_fehler() {
    // Der erste Start. Ein `Err` hier hieße, dass die Anwendung eine
    // Meldung zeigen müsste, wo nichts vorgefallen ist.
    let pfad = werkbank("fehlt").join("gibtesnicht.bin");
    assert_eq!(cabrik_ablage::lies(&pfad).expect("kein Fehler"), None);
}

#[test]
fn geschriebenes_kommt_zurueck() {
    let pfad = werkbank("rundweg").join("datei.bin");
    let daten = b"ein paar Bytes".to_vec();

    cabrik_ablage::schreib_atomar(&pfad, &daten).expect("schreiben");

    assert_eq!(cabrik_ablage::lies(&pfad).expect("lesen"), Some(daten));
}

#[test]
fn das_verzeichnis_wird_angelegt() {
    // Beim ersten Start gibt es weder Datei noch Ordner. Den Nutzer
    // aufzufordern, ihn selbst anzulegen, wäre eine Zumutung.
    let pfad = werkbank("anlegen").join("tief").join("tiefer").join("d.bin");

    cabrik_ablage::schreib_atomar(&pfad, b"x").expect("schreiben");

    assert!(pfad.exists());
}

#[test]
fn zweimal_schreiben_ersetzt_statt_anzuhaengen() {
    let pfad = werkbank("ersetzen").join("datei.bin");

    cabrik_ablage::schreib_atomar(&pfad, b"alt und lang").expect("erstes");
    cabrik_ablage::schreib_atomar(&pfad, b"neu").expect("zweites");

    assert_eq!(
        cabrik_ablage::lies(&pfad).expect("lesen"),
        Some(b"neu".to_vec()),
        "Reste der alten Fassung duerfen nicht stehen bleiben"
    );
}

#[test]
fn es_bleibt_keine_zwischendatei_liegen() {
    // Sonst läge beim nächsten Mal eine `.tmp` im Weg -- und schlimmer:
    // Sie enthielte eine ältere Fassung des Kontaktspeichers, lesbar mit
    // demselben Schlüssel.
    let verzeichnis = werkbank("aufraeumen");
    let pfad = verzeichnis.join("datei.bin");

    cabrik_ablage::schreib_atomar(&pfad, b"inhalt").expect("schreiben");

    let uebrig: Vec<_> = std::fs::read_dir(&verzeichnis)
        .expect("lesen")
        .filter_map(std::result::Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();

    assert_eq!(uebrig, vec!["datei.bin".to_owned()], "uebrig: {uebrig:?}");
}

#[test]
fn ein_verzeichnis_statt_einer_datei_ist_ein_fehler() {
    // Anders als „gibt es nicht": Hier stimmt etwas nicht, und der Nutzer
    // soll den Pfad im Klartext lesen.
    let verzeichnis = werkbank("verwechselt");

    let fehler = cabrik_ablage::lies(&verzeichnis).expect_err("muss scheitern");

    assert!(
        fehler.meldung.contains("verwechselt"),
        "der Pfad gehoert in die Meldung: {}",
        fehler.meldung
    );
}

#[test]
fn die_pfade_liegen_im_selben_verzeichnis() {
    // Sonst suchte die CLI woanders als die Anwendung -- und Kontakte
    // verschwaenden, ohne dass jemand versteht, warum.
    let k = cabrik_ablage::keyfile_pfad(None).expect("Keyfile");
    let c = cabrik_ablage::kontakte_pfad(None).expect("Kontakte");

    assert_eq!(k.parent(), c.parent());
    assert_ne!(k.file_name(), c.file_name());
}

#[test]
fn eine_ausdrueckliche_angabe_schlaegt_die_voreinstellung() {
    let eigen = PathBuf::from("D:").join("woanders").join("meine.key");
    let ergebnis = cabrik_ablage::keyfile_pfad(Some(&eigen)).expect("Pfad");

    assert_eq!(ergebnis, eigen);
}

#[test]
fn eine_neue_datei_ueberschreibt_keine_bestehende() {
    // Der folgenschwerste Fehlgriff, den dieses Programm zulassen koennte:
    // eine neue Identitaet ueber eine bestehende. Danach ist unlesbar, was
    // an die alte gerichtet war -- dauerhaft.
    let pfad = werkbank("nicht-ueberschreiben").join("identity.key");
    cabrik_ablage::schreib_atomar(&pfad, b"die bestehende Identitaet").expect("erste");

    let fehler = cabrik_ablage::schreib_neu(&pfad, b"eine neue").expect_err("muss scheitern");

    assert!(fehler.meldung.contains("gibt es bereits"), "{}", fehler.meldung);
    assert_eq!(
        cabrik_ablage::lies(&pfad).expect("lesen"),
        Some(b"die bestehende Identitaet".to_vec()),
        "die alte Datei muss unangetastet dastehen"
    );
}

#[test]
fn eine_neue_datei_entsteht_wenn_es_sie_nicht_gibt() {
    // Die Gegenprobe: Eine Sperre, die immer sperrt, waere keine.
    let pfad = werkbank("neu-anlegen").join("identity.key");

    cabrik_ablage::schreib_neu(&pfad, b"frisch").expect("anlegen");

    assert_eq!(cabrik_ablage::lies(&pfad).expect("lesen"), Some(b"frisch".to_vec()));
}

#[test]
fn der_pfad_steht_in_der_meldung() {
    // Sonst sucht der Nutzer selbst, welche Datei gemeint ist.
    let pfad = werkbank("pfad-nennen").join("identity.key");
    cabrik_ablage::schreib_atomar(&pfad, b"da").expect("erste");

    let fehler = cabrik_ablage::schreib_neu(&pfad, b"neu").expect_err("muss scheitern");

    assert!(fehler.meldung.contains("identity.key"), "{}", fehler.meldung);
}

#[test]
fn loeschen_entfernt_die_datei() {
    let pfad = werkbank("loeschen").join("datei.bin");
    cabrik_ablage::schreib_atomar(&pfad, b"weg damit").expect("schreiben");

    cabrik_ablage::loesche(&pfad).expect("loeschen");

    assert_eq!(cabrik_ablage::lies(&pfad).expect("lesen"), None);
}

#[test]
fn was_nicht_da_ist_zu_loeschen_ist_kein_fehler() {
    // Wer loeschen wollte, was nicht da ist, hat sein Ziel erreicht.
    let pfad = werkbank("schon-weg").join("gibtesnicht.bin");

    assert!(cabrik_ablage::loesche(&pfad).is_ok());
}

#[test]
fn beiseiteschieben_macht_den_weg_frei_ohne_zu_vernichten() {
    // Der verwaiste Kontaktspeicher: dauerhaft unlesbar, aber im Weg. Wer
    // ihn liegen laesst, kann beim naechsten Start nicht mehr entsperren --
    // mit richtigem Passwort.
    let verzeichnis = werkbank("beiseite");
    let pfad = verzeichnis.join("contacts.cabrik-contacts");
    cabrik_ablage::schreib_atomar(&pfad, b"alter Speicher").expect("schreiben");

    let ziel = cabrik_ablage::verschiebe_beiseite(&pfad)
        .expect("verschieben")
        .expect("es lag etwas da");

    assert!(!pfad.exists(), "der Weg muss frei sein");
    assert_eq!(
        std::fs::read(&ziel).expect("lesen"),
        b"alter Speicher",
        "und nichts darf vernichtet worden sein"
    );
}

#[test]
fn beiseiteschieben_ohne_datei_ist_kein_fehler() {
    // Der Normalfall beim ersten Anlegen ueberhaupt.
    let pfad = werkbank("nichts-beiseite").join("gibtesnicht.bin");

    assert_eq!(
        cabrik_ablage::verschiebe_beiseite(&pfad).expect("kein Fehler"),
        None
    );
}

#[test]
fn zweimal_beiseiteschieben_ueberschreibt_das_erste_nicht() {
    let verzeichnis = werkbank("zweimal-beiseite");
    let pfad = verzeichnis.join("c.bin");

    cabrik_ablage::schreib_atomar(&pfad, b"erster").expect("schreiben");
    let a = cabrik_ablage::verschiebe_beiseite(&pfad).expect("a").expect("da");
    cabrik_ablage::schreib_atomar(&pfad, b"zweiter").expect("schreiben");
    let b = cabrik_ablage::verschiebe_beiseite(&pfad).expect("b").expect("da");

    assert_ne!(a, b);
    assert_eq!(std::fs::read(&a).expect("lesen"), b"erster");
    assert_eq!(std::fs::read(&b).expect("lesen"), b"zweiter");
}
