//! Einen Schlüssel aus Version 1 übernehmen.
//!
//! # Warum das kein Komfort ist
//!
//! Wer die ausgelieferte v1 benutzt hat, kommt ohne diesen Weg an
//! **nichts** mehr heran, was an ihn gerichtet wurde. Das ist ein
//! verschlossenes Schloss, kein fehlender Bedienknopf.
//!
//! # Woher die Vorlagen kommen
//!
//! Aus `testvectors/v1-compat.json`, und die erzeugt die
//! **Python-Referenz** — nicht dieser Quelltext. Die Gegenprobe baut sie
//! bei jedem Lauf neu (`gen_v1_compat.py`). Ein Test, der seine eigenen
//! v1-Dateien schriebe, prüfte nur, ob der Leser zum eigenen Schreiber
//! passt, und nicht, ob beide recht haben.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "Fehlschlag soll den Test abbrechen"
)]

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use cabrik_app::{Sitzung, Uebernahme};
use cabrik_bruecke::{KdfStufe, Sperrfrist};
use cabrik_core::OsRandom;
use std::path::PathBuf;

const NEUES_PASSWORT: &[u8] = b"vier zufaellige woerter hier";

/// Eine Vorlage aus der Referenz: Dateiinhalt und altes Passwort.
fn vorlage(id: &str) -> (Vec<u8>, Vec<u8>) {
    let pfad = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testvectors/v1-compat.json")
        .canonicalize()
        .expect("testvectors/v1-compat.json");
    let roh = std::fs::read_to_string(&pfad).expect("lesen");
    let doc: serde_json::Value = serde_json::from_str(&roh).expect("JSON");

    for v in doc["keyfiles"].as_array().expect("keyfiles") {
        if v["id"].as_str() == Some(id) {
            let datei = STANDARD
                .decode(v["input"]["keyfile_b64"].as_str().expect("keyfile_b64"))
                .expect("Base64");
            let passwort = v["input"]["password"]
                .as_str()
                .expect("password")
                .as_bytes()
                .to_vec();
            return (datei, passwort);
        }
    }
    panic!("Vorlage {id} steht nicht in v1-compat.json");
}

fn uebernehmen(
    id: &str,
    altes: Option<&[u8]>,
    neues: &[u8],
) -> cabrik_app::Befehlsergebnis<Sitzung> {
    let (datei, echtes_altes) = vorlage(id);
    Sitzung::aus_v1_uebernehmen(
        Uebernahme {
            v1_datei: &datei,
            altes_passwort: altes.unwrap_or(&echtes_altes),
            neues_passwort: neues,
            bezeichnung: Some("Aus Version 1".to_owned()),
            stufe: KdfStufe::Min,
            frist: Sperrfrist::default(),
        },
        1_000,
        &mut OsRandom,
    )
}

#[test]
fn ein_v1_schluessel_wird_uebernommen_und_ist_sofort_offen() {
    let s = uebernehmen("kf-v1-signing", None, NEUES_PASSWORT).expect("übernehmen");

    assert!(!s.ist_gesperrt(), "nach der Übernahme muss offen sein");
    assert!(
        s.kann_signieren(),
        "der Vektor mit Signierschlüssel muss signieren können"
    );
    assert!(
        !s.schluesseldatei().is_empty(),
        "ohne neue Hülle wäre nichts gewonnen"
    );
}

#[test]
fn die_neue_huelle_haengt_am_neuen_passwort_und_nicht_am_alten() {
    // DER PUNKT DER GANZEN SACHE. Würde die neue Datei weiterhin mit dem
    // alten Passwort verschlossen, hätte die getrennte Abfrage nichts
    // gebracht -- und die womöglich schwache alte Wahl liefe weiter mit.
    let (_, altes) = vorlage("kf-v1-signing");
    let mut s = uebernehmen("kf-v1-signing", None, NEUES_PASSWORT).expect("übernehmen");

    s.sperren();
    s.entsperren(&altes, 2_000)
        .expect_err("das ALTE Passwort darf die neue Hülle nicht öffnen");
    s.entsperren(NEUES_PASSWORT, 2_000)
        .expect("das neue Passwort muss sie öffnen");
}

#[test]
fn ein_anonymer_v1_schluessel_kann_nicht_signieren() {
    // v1 kannte Schlüssel ohne Signierteil. Das lässt sich nachträglich
    // nicht ändern, und die Oberfläche muss es sagen können -- sonst
    // scheitert jemand später an einer stumpfen Schaltfläche.
    let s = uebernehmen("kf-v1-anonymous", None, NEUES_PASSWORT).expect("übernehmen");
    assert!(!s.ist_gesperrt());
    assert!(
        !s.kann_signieren(),
        "ein Anonymitäts-Schlüssel darf sich nicht als signierfähig ausgeben"
    );
}

#[test]
fn ein_falsches_altes_passwort_wird_abgelehnt() {
    let Err(fehler) = uebernehmen(
        "kf-v1-signing",
        Some(b"ganz sicher nicht das richtige"),
        NEUES_PASSWORT,
    ) else {
        panic!("ein falsches altes Passwort darf nicht gelingen");
    };
    assert!(
        fehler.meldung.contains("alten"),
        "die Meldung soll sagen, WELCHES Passwort nicht passt: {}",
        fehler.meldung
    );
}

#[test]
fn eine_datei_die_kein_v1_ist_wird_erkannt_und_nicht_dem_passwort_angelastet() {
    // Die Reihenfolge zählt: Wer versehentlich die falsche Datei gewählt
    // hat, soll das erfahren -- statt an einem Passwort zu zweifeln, das
    // gar nicht schuld ist.
    let Err(fehler) = Sitzung::aus_v1_uebernehmen(
        Uebernahme {
            v1_datei: b"das ist ein Urlaubsfoto und kein Schluessel",
            altes_passwort: b"egal",
            neues_passwort: NEUES_PASSWORT,
            bezeichnung: None,
            stufe: KdfStufe::Min,
            frist: Sperrfrist::default(),
        },
        1_000,
        &mut OsRandom,
    ) else {
        panic!("ein Urlaubsfoto darf nicht als Schluessel durchgehen");
    };

    assert!(
        fehler.meldung.contains("Version 1"),
        "die Meldung soll die Formfrage benennen: {}",
        fehler.meldung
    );
    assert!(
        !fehler.meldung.to_lowercase().contains("passwort"),
        "sie darf NICHT nach einem Passwortfehler klingen: {}",
        fehler.meldung
    );
}

#[test]
fn ein_zu_kurzes_neues_passwort_wird_abgelehnt() {
    let Err(fehler) = uebernehmen("kf-v1-signing", None, b"kurz") else {
        panic!("ein zu kurzes neues Passwort darf nicht gelingen");
    };
    assert!(
        fehler.meldung.contains("Zeichen"),
        "die Meldung soll die Länge nennen: {}",
        fehler.meldung
    );
}

#[test]
fn zweimal_uebernehmen_ergibt_zwei_verschiedene_identitaeten() {
    // EIN BEFUND, KEINE FEINHEIT.
    //
    // Die Übernahme erzeugt einen frischen Post-Quantum-Schlüssel. Er ist
    // jedes Mal ein anderer -- also ergeben zwei Übernahmen derselben
    // v1-Datei zwei verschiedene Fingerprints.
    //
    // Wer also auf zwei Rechnern übernimmt, hat zwei Identitäten, die er
    // für eine hält. Seine Gegenüber sehen auf dem zweiten Rechner erneut
    // „Geändert" und müssen ein zweites Mal verifizieren.
    //
    // Das ist kein Fehler dieses Codes: Ein PQ-Schlüssel muss irgendwoher
    // kommen, und v1 hatte keinen. Aber es ist eine Falle, und dieser Test
    // hält sie fest, damit sie in der Anzeige benannt wird.
    let erste = uebernehmen("kf-v1-signing", None, NEUES_PASSWORT).expect("erste Übernahme");
    let zweite = uebernehmen("kf-v1-signing", None, NEUES_PASSWORT).expect("zweite Übernahme");

    let fingerprint = |mut s: Sitzung| -> String {
        let datei = s.schluesseldatei().to_vec();
        s.offen(1_000)
            .expect("offen")
            .identitaet(&datei, String::new())
            .expect("Identitaet")
            .fingerprint
    };

    assert_ne!(
        fingerprint(erste),
        fingerprint(zweite),
        "zwei Übernahmen ergäben denselben Fingerprint -- dann wäre der \
         PQ-Schlüssel nicht frisch, und die Annahme dieses Tests wäre falsch"
    );
}
