//! Die Befehle gegen einen echten Kontaktspeicher.
//!
//! # Warum das reicht, um Tauri vorzubereiten
//!
//! Was hier geprüft wird, ist genau das, was ein `#[tauri::command]`
//! aufrufen wird. Die Hülle darum reicht Argumente durch und wandelt einen
//! Fehler in eine Antwort — sie kann die Regeln unten weder herstellen noch
//! brechen. Steht diese Schicht, ist die Hülle mechanisch.
//!
//! # Die Regel, um die es geht
//!
//! Ein aufgenommener Kontakt ist **gesehen**, nie verifiziert. Sie steht in
//! `kern/bruecke.ts`, sie steht in `Sitzung::kontakt_aufnehmen`, und sie
//! steht hier — an drei Stellen, weil sie an einer einzigen zu leicht
//! verschwindet.

// Ein Test, der seine Vorbedingung nicht herstellen kann, hat kein
// Ergebnis, sondern einen kaputten Test.
#![expect(clippy::expect_used, reason = "Fehlschlag soll den Test abbrechen")]

use cabrik_app::Sitzung;
use cabrik_bruecke::{Vertrauen, Verifikationsweg};
use cabrik_core::trust::{Contact, TrustStore};

/// Ein Speicher mit drei Kontakten und eine Sitzung darüber.
fn sitzung() -> Sitzung {
    let mut speicher = TrustStore::new();
    // Feste Bytes statt gerechneter: Die Werkbank verbietet Arithmetik in
    // Tests, und drei Zeilen sind hier ohnehin klarer als eine Schleife mit
    // Umrechnung.
    for (name, enc, sig) in [
        ("Anna", 0x11_u8, 0x21_u8),
        ("Bert", 0x12, 0x22),
        ("Cora", 0x13, 0x23),
    ] {
        speicher
            .add(
                Contact::new_seen(name, [enc; 32], Some([sig; 32]), None, 1_000)
                    .expect("Kontakt"),
            )
            .expect("hinzufuegen");
    }
    // Der eigene Fingerprint: irgendeiner, Hauptsache stabil -- die Safety
    // Number ist paarweise, ohne ihn gibt es keine.
    let eigener = Contact::new_seen("ich", [0x99; 32], Some([0x98; 32]), None, 1)
        .expect("eigen")
        .fingerprint();
    Sitzung::neu(speicher, eigener)
}

fn fingerprint_von(s: &Sitzung, name: &str) -> String {
    s.kontakte()
        .into_iter()
        .find(|k| k.name == name)
        .expect("Kontakt")
        .fingerprint
}

// ---------------------------------------------------------------------------

#[test]
fn kontakte_kommen_mit_safety_number() {
    let s = sitzung();
    let alle = s.kontakte();

    assert_eq!(alle.len(), 3);
    for k in &alle {
        // Zwölf Fünfergruppen, sprachunabhängig -- zum Vorlesen.
        let gruppen: Vec<_> = k.safety_number.split(' ').collect();
        assert_eq!(gruppen.len(), 12, "Safety Number: {}", k.safety_number);
        for g in gruppen {
            assert_eq!(g.len(), 5);
            assert!(g.chars().all(|c| c.is_ascii_digit()));
        }
    }
}

#[test]
fn jeder_kontakt_hat_eine_eigene_safety_number() {
    let s = sitzung();
    let mut nummern: Vec<_> = s.kontakte().into_iter().map(|k| k.safety_number).collect();
    nummern.sort();
    nummern.dedup();

    assert_eq!(nummern.len(), 3, "die Nummern muessen sich unterscheiden");
}

#[test]
fn aufnehmen_legt_immer_als_gesehen_an() {
    // Die tragende Regel des Vertrauensmodells.
    let mut s = sitzung();
    let neu = s
        .kontakt_aufnehmen("Neu", [0x77; 32], Some([0x78; 32]), None, 2_000)
        .expect("aufnehmen");

    assert_eq!(neu.vertrauen, Vertrauen::Gesehen);
    assert!(neu.verifiziert_am.is_none());
    assert!(neu.verifiziert_ueber.is_none());
}

#[test]
fn verifizieren_haelt_den_weg_fest() {
    let mut s = sitzung();
    let fp = fingerprint_von(&s, "Bert");

    let k = s
        .kontakt_verifizieren(&fp, Verifikationsweg::Qr, 2_000)
        .expect("verifizieren");

    assert_eq!(k.vertrauen, Vertrauen::Verifiziert);
    assert_eq!(k.verifiziert_ueber, Some(Verifikationsweg::Qr));
    assert_eq!(k.verifiziert_am, Some(2_000));
}

#[test]
fn widerrufen_laesst_den_eintrag_stehen() {
    // Der Unterschied zum Loeschen: Der Eintrag bleibt und warnt kuenftig.
    let mut s = sitzung();
    let fp = fingerprint_von(&s, "Bert");

    let k = s
        .kontakt_widerrufen(&fp, 2_000, Some("nach dem Vorfall"))
        .expect("widerrufen");

    assert_eq!(k.vertrauen, Vertrauen::Widerrufen);
    assert_eq!(s.kontakte().len(), 3, "der Eintrag muss bleiben");
}

#[test]
fn loeschen_entfernt_ihn_und_damit_die_warnung() {
    let mut s = sitzung();
    let fp = fingerprint_von(&s, "Bert");

    s.kontakt_widerrufen(&fp, 2_000, None).expect("widerrufen");
    s.kontakt_loeschen(&fp).expect("loeschen");

    assert_eq!(s.kontakte().len(), 2);
    assert!(!s.kontakte().iter().any(|k| k.name == "Bert"));
}

#[test]
fn ein_unbekannter_fingerprint_schweigt_nicht() {
    // Stilles Nichtstun waere das Schlimmste: Die Oberflaeche meldete
    // Erfolg, und nichts waere geschehen.
    let mut s = sitzung();
    let fehler = s
        .kontakt_verifizieren("GIBT ES NICHT", Verifikationsweg::Qr, 2_000)
        .expect_err("muss fehlschlagen");

    assert!(
        fehler.meldung.contains("Kein Kontakt"),
        "die Meldung muss sagen, was fehlt: {}",
        fehler.meldung
    );
}

#[test]
fn der_fingerprint_geht_hin_und_zurueck() {
    // Die Oberflaeche schickt zurueck, was sie angezeigt bekommen hat.
    // Ginge dabei etwas verloren, traefe kein einziger Befehl sein Ziel.
    let mut s = sitzung();
    for name in ["Anna", "Bert", "Cora"] {
        let fp = fingerprint_von(&s, name);
        let k = s
            .kontakt_verifizieren(&fp, Verifikationsweg::SafetyNumber, 2_000)
            .expect("verifizieren");
        assert_eq!(k.name, name);
        assert_eq!(k.fingerprint, fp);
    }
}

#[test]
fn ein_aufgenommener_kontakt_ist_sofort_auffindbar() {
    let mut s = sitzung();
    let neu = s
        .kontakt_aufnehmen("Neu", [0x77; 32], Some([0x78; 32]), None, 2_000)
        .expect("aufnehmen");

    // Und zwar unter genau dem Fingerprint, den die Antwort nannte.
    let k = s
        .kontakt_verifizieren(&neu.fingerprint, Verifikationsweg::Fingerprint, 2_100)
        .expect("verifizieren");
    assert_eq!(k.name, "Neu");
}

#[test]
fn ohne_post_quantum_wird_es_gemeldet() {
    // Die Kontakte oben haben keinen ML-KEM-Schluessel -- an sie laesst
    // sich nur klassisch verschluesseln, und die Oberflaeche muss das
    // anzeigen koennen.
    let s = sitzung();
    for k in s.kontakte() {
        assert!(!k.hat_post_quantum, "{} sollte ohne PQ sein", k.name);
    }
}
