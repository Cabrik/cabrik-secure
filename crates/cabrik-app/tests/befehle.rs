//! Die Befehle gegen einen echten Kontaktspeicher.
//!
//! # Warum das reicht, um Tauri vorzubereiten
//!
//! Was hier geprüft wird, ist genau das, was ein `#[tauri::command]`
//! aufrufen wird. Die Hülle darum reicht Argumente durch und wandelt einen
//! Fehler in eine Antwort — sie kann die Regeln unten weder herstellen noch
//! brechen.
//!
//! # Die Regeln, um die es geht
//!
//! 1. Ein aufgenommener Kontakt ist **gesehen**, nie verifiziert.
//! 2. **Ohne Entsperrung geht gar nichts** — und zwar über den Typ, nicht
//!    über eine Prüfung, die jemand vergessen kann.

// Ein Test, der seine Vorbedingung nicht herstellen kann, hat kein
// Ergebnis, sondern einen kaputten Test.
#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "Fehlschlag soll den Test abbrechen"
)]

use cabrik_app::Sitzung;
use cabrik_bruecke::{Nutzlastbefund, Sperrfrist, Verifikationsweg, Vertrauen};
use cabrik_core::keyfile::{self, KdfParams};
use cabrik_core::{Identity, OsRandom};
use zeroize::Zeroizing;

const PASSWORT: &str = "vier zufaellige woerter hier";

/// Eine echte Schlüsseldatei — mit der schwächsten Ableitung, damit die
/// Tests nicht sekundenlang rechnen.
fn schluesseldatei() -> Vec<u8> {
    let id = Identity::generate(&mut OsRandom, true, 1_700_000_000).expect("Identität");
    // Die Untergrenze der Spezifikation: 64 MiB, drei Durchgaenge. Die
    // empfohlenen 256 MiB kosten je Test rund sieben Sekunden im
    // Debug-Bau -- bei zwanzig Tests waeren das zwei Minuten fuer nichts.
    let sparsam = KdfParams {
        m_cost: KdfParams::M_COST_MIN,
        t_cost: KdfParams::T_COST_MIN,
        p_cost: 4,
    };
    keyfile::write(&id, PASSWORT.as_bytes(), &sparsam, &mut OsRandom).expect("schreiben")
}

fn passwort() -> Zeroizing<String> {
    Zeroizing::new(PASSWORT.to_owned())
}

/// Eine entsperrte Sitzung mit drei Kontakten.
fn sitzung() -> Sitzung {
    let mut s = Sitzung::neu(schluesseldatei(), None, Sperrfrist::FuenfzehnMinuten);
    s.entsperren(&passwort(), 1_000).expect("entsperren");

    let n = cabrik_core::trust::qr_payload(&[0x11; 32], Some(&[0x21; 32]), None);
    let m = cabrik_core::trust::qr_payload(&[0x12; 32], Some(&[0x22; 32]), None);
    let o = cabrik_core::trust::qr_payload(&[0x13; 32], Some(&[0x23; 32]), None);
    let auf = s.offen(1_000).expect("offen");
    for (name, nutzlast) in [("Anna", &n), ("Bert", &m), ("Cora", &o)] {
        auf.kontakt_aus_nutzlast(name, nutzlast, 1_000)
            .expect("aufnehmen");
    }
    s
}

fn fingerprint_von(s: &mut Sitzung, name: &str) -> String {
    s.offen(1_000)
        .expect("offen")
        .kontakte()
        .into_iter()
        .find(|k| k.name == name)
        .expect("Kontakt")
        .fingerprint
}

/// Eine echte Nutzlast, aus echten Schlüsseln gebildet.
fn nutzlast() -> String {
    cabrik_core::trust::qr_payload(&[0x77; 32], Some(&[0x78; 32]), None)
}

// ---------------------------------------------------------------------------
// Entsperren
// ---------------------------------------------------------------------------

#[test]
fn eine_neue_sitzung_ist_gesperrt() {
    // Der Anfangszustand der Anwendung (`spec/entsperrung.md` §2.3).
    let s = Sitzung::neu(schluesseldatei(), None, Sperrfrist::FuenfzehnMinuten);
    assert!(s.ist_gesperrt());
}

#[test]
fn gesperrt_geht_gar_nichts() {
    // Die Regel steht im Typ: Ohne `offen()` gibt es den Empfänger der
    // Kontaktbefehle nicht. Dieser Test hält fest, dass `offen()` selbst
    // sich weigert.
    let mut s = Sitzung::neu(schluesseldatei(), None, Sperrfrist::FuenfzehnMinuten);
    let fehler = s.offen(1_000).expect_err("muss fehlschlagen");

    assert!(
        fehler.meldung.contains("gesperrt"),
        "die Meldung muss sagen, was los ist: {}",
        fehler.meldung
    );
}

#[test]
fn ein_falsches_passwort_verraet_nicht_wie_falsch() {
    // spec/entsperrung.md §4.3: nicht „fast richtig", nicht die Länge,
    // nicht die Zahl übereinstimmender Zeichen.
    let mut s = Sitzung::neu(schluesseldatei(), None, Sperrfrist::FuenfzehnMinuten);
    let fast = Zeroizing::new("vier zufaellige woerter hie".to_owned());

    let fehler = s.entsperren(&fast, 1_000).expect_err("muss fehlschlagen");

    assert_eq!(fehler.meldung, "Das Passwort passt nicht.");
    assert!(s.ist_gesperrt());
    for verraeterisch in ["fast", "Zeichen", "Länge", "kurz", "lang"] {
        assert!(
            !fehler.meldung.contains(verraeterisch),
            "die Meldung verrät zu viel: {}",
            fehler.meldung
        );
    }
}

#[test]
fn ohne_kontaktdatei_ist_das_verzeichnis_leer_und_das_ist_kein_fehler() {
    let mut s = Sitzung::neu(schluesseldatei(), None, Sperrfrist::FuenfzehnMinuten);
    s.entsperren(&passwort(), 1_000).expect("entsperren");

    assert!(s.offen(1_000).expect("offen").kontakte().is_empty());
}

#[test]
fn eine_fremde_kontaktdatei_wird_benannt_statt_verschluckt() {
    // Sie gehört zu einer anderen Identität. Still ein leeres Verzeichnis
    // zu zeigen wäre das Schlimmste: Der Nutzer hielte seine Kontakte für
    // verloren, statt zu erfahren, dass die Datei nicht zu ihm gehört.
    let fremde = {
        let id = Identity::generate(&mut OsRandom, true, 1).expect("Identität");
        cabrik_core::trust::seal_store(
            &cabrik_core::trust::TrustStore::new(),
            &id,
            &mut OsRandom,
        )
        .expect("sichern")
    };
    let mut s = Sitzung::neu(schluesseldatei(), Some(fremde), Sperrfrist::FuenfzehnMinuten);

    let fehler = s.entsperren(&passwort(), 1_000).expect_err("muss scheitern");
    assert!(fehler.meldung.contains("andere"), "{}", fehler.meldung);
    assert!(s.ist_gesperrt(), "nach einem Fehlschlag bleibt gesperrt");
}

// ---------------------------------------------------------------------------
// Die Sperre nach Untätigkeit
// ---------------------------------------------------------------------------

#[test]
fn nach_der_frist_wird_gesperrt() {
    let mut s = Sitzung::neu(schluesseldatei(), None, Sperrfrist::EineMinute);
    s.entsperren(&passwort(), 1_000).expect("entsperren");
    assert!(s.offen(1_059).is_ok(), "eine Sekunde vorher noch offen");

    let mut s2 = Sitzung::neu(schluesseldatei(), None, Sperrfrist::EineMinute);
    s2.entsperren(&passwort(), 1_000).expect("entsperren");
    assert!(s2.offen(1_060).is_err(), "nach 60 Sekunden gesperrt");
    assert!(s2.ist_gesperrt());
}

#[test]
fn jede_handlung_setzt_die_messung_zurueck() {
    // Die Grenze zählt ab der LETZTEN HANDLUNG, nicht ab dem Entsperren:
    // Wer eine Stunde am Stück arbeitet, wird nicht mitten hinein gesperrt.
    let mut s = Sitzung::neu(schluesseldatei(), None, Sperrfrist::EineMinute);
    s.entsperren(&passwort(), 1_000).expect("entsperren");

    for t in [1_030_u64, 1_080, 1_130, 1_180] {
        assert!(s.offen(t).is_ok(), "bei {t} hätte es offen sein müssen");
    }
    assert!(!s.ist_gesperrt(), "nach 180 s Arbeit noch offen");
}

#[test]
fn nachfragen_ist_keine_handlung() {
    // `stand()` darf die Messung nicht zurücksetzen -- sonst hielte allein
    // das Anzeigen der Restzeit die Sitzung offen. Genau das täte eine
    // Oberfläche, die jede Sekunde nachfragt.
    let mut s = Sitzung::neu(schluesseldatei(), None, Sperrfrist::EineMinute);
    s.entsperren(&passwort(), 1_000).expect("entsperren");

    for t in 1_001_u64..1_060 {
        assert!(!s.stand(t).gesperrt, "bei {t} noch offen");
    }
    assert!(s.stand(1_060).gesperrt, "die Frist muss trotzdem greifen");
}

#[test]
fn die_restzeit_zaehlt_herunter() {
    let mut s = Sitzung::neu(schluesseldatei(), None, Sperrfrist::EineMinute);
    s.entsperren(&passwort(), 1_000).expect("entsperren");

    assert_eq!(s.stand(1_000).restsekunden, Some(60));
    assert_eq!(s.stand(1_040).restsekunden, Some(20));
    assert_eq!(s.stand(1_059).restsekunden, Some(1));
}

#[test]
fn bis_zum_schliessen_sperrt_nie_von_selbst() {
    let mut s = Sitzung::neu(schluesseldatei(), None, Sperrfrist::BisZumSchliessen);
    s.entsperren(&passwort(), 1_000).expect("entsperren");

    // Ein Jahr später.
    assert!(s.offen(1_000 + 31_536_000).is_ok());
    assert_eq!(s.stand(1_000).restsekunden, None);
}

#[test]
fn jetzt_sperren_wirkt_sofort() {
    let mut s = sitzung();
    assert!(!s.ist_gesperrt());

    s.sperren();

    assert!(s.ist_gesperrt());
    assert!(s.offen(1_001).is_err());
}

#[test]
fn eine_neue_frist_beginnt_von_vorn() {
    // Wer von einer Stunde auf eine Minute wechselt, hat gerade gehandelt.
    let mut s = Sitzung::neu(schluesseldatei(), None, Sperrfrist::EineStunde);
    s.entsperren(&passwort(), 1_000).expect("entsperren");

    s.frist_setzen(Sperrfrist::EineMinute, 3_000);

    assert!(s.offen(3_030).is_ok(), "30 s nach dem Wechsel noch offen");
}

#[test]
fn nach_dem_sperren_ist_wieder_entsperren_moeglich() {
    let mut s = Sitzung::neu(schluesseldatei(), None, Sperrfrist::EineMinute);
    s.entsperren(&passwort(), 1_000).expect("entsperren");
    s.sperren();

    s.entsperren(&passwort(), 5_000).expect("wieder entsperren");
    assert!(s.offen(5_000).is_ok());
}

#[test]
fn der_gesicherte_speicher_laesst_sich_wieder_oeffnen() {
    let datei = schluesseldatei();
    let mut s = Sitzung::neu(datei.clone(), None, Sperrfrist::FuenfzehnMinuten);
    s.entsperren(&passwort(), 1_000).expect("entsperren");
    s.offen(1_000)
        .expect("offen")
        .kontakt_aus_nutzlast("Neu", &nutzlast(), 1_000)
        .expect("aufnehmen");

    let gesichert = s.kontakte_sichern(1_000, &mut OsRandom).expect("sichern");

    let mut neu = Sitzung::neu(datei, Some(gesichert), Sperrfrist::FuenfzehnMinuten);
    neu.entsperren(&passwort(), 2_000).expect("entsperren");

    let kontakte = neu.offen(2_000).expect("offen").kontakte();
    assert_eq!(kontakte.len(), 1);
    assert_eq!(kontakte[0].name, "Neu");
}

#[test]
fn gesperrt_laesst_sich_nichts_sichern() {
    // Ohne Identität gibt es keinen Schlüssel für die Datei.
    let mut s = Sitzung::neu(schluesseldatei(), None, Sperrfrist::FuenfzehnMinuten);
    assert!(s.kontakte_sichern(1_000, &mut OsRandom).is_err());
}

// ---------------------------------------------------------------------------
// Kontakte
// ---------------------------------------------------------------------------

#[test]
fn kontakte_kommen_mit_safety_number() {
    let mut s = sitzung();
    let alle = s.offen(1_000).expect("offen").kontakte();

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
    let mut s = sitzung();
    let mut nummern: Vec<_> = s
        .offen(1_000)
        .expect("offen")
        .kontakte()
        .into_iter()
        .map(|k| k.safety_number)
        .collect();
    nummern.sort();
    nummern.dedup();

    assert_eq!(nummern.len(), 3, "die Nummern muessen sich unterscheiden");
}

#[test]
fn verifizieren_haelt_den_weg_fest() {
    let mut s = sitzung();
    let fp = fingerprint_von(&mut s, "Bert");

    let k = s
        .offen(1_000)
        .expect("offen")
        .kontakt_verifizieren(&fp, Verifikationsweg::Qr, 2_000)
        .expect("verifizieren");

    assert_eq!(k.vertrauen, Vertrauen::Verifiziert);
    assert_eq!(k.verifiziert_ueber, Some(Verifikationsweg::Qr));
    assert_eq!(k.verifiziert_am, Some(2_000));
}

#[test]
fn zuruecksetzen_widerruft_nicht() {
    let mut s = sitzung();
    let fp = fingerprint_von(&mut s, "Bert");
    let o = s.offen(1_000).expect("offen");

    o.kontakt_verifizieren(&fp, Verifikationsweg::SafetyNumber, 2_000)
        .expect("verifizieren");
    let k = o.kontakt_zuruecksetzen(&fp).expect("zuruecksetzen");

    assert_eq!(k.vertrauen, Vertrauen::Gesehen);
    assert_ne!(k.vertrauen, Vertrauen::Widerrufen);
    assert!(k.verifiziert_ueber.is_none());
}

#[test]
fn widerrufen_laesst_den_eintrag_stehen() {
    // Der Unterschied zum Loeschen: Der Eintrag bleibt und warnt kuenftig.
    let mut s = sitzung();
    let fp = fingerprint_von(&mut s, "Bert");
    let o = s.offen(1_000).expect("offen");

    let k = o
        .kontakt_widerrufen(&fp, 2_000, Some("nach dem Vorfall"))
        .expect("widerrufen");

    assert_eq!(k.vertrauen, Vertrauen::Widerrufen);
    assert_eq!(o.kontakte().len(), 3, "der Eintrag muss bleiben");
}

#[test]
fn loeschen_entfernt_ihn_und_damit_die_warnung() {
    let mut s = sitzung();
    let fp = fingerprint_von(&mut s, "Bert");
    let o = s.offen(1_000).expect("offen");

    o.kontakt_widerrufen(&fp, 2_000, None).expect("widerrufen");
    o.kontakt_loeschen(&fp).expect("loeschen");

    assert_eq!(o.kontakte().len(), 2);
    assert!(!o.kontakte().iter().any(|k| k.name == "Bert"));
}

#[test]
fn ein_unbekannter_fingerprint_schweigt_nicht() {
    let mut s = sitzung();
    let fehler = s
        .offen(1_000)
        .expect("offen")
        .kontakt_verifizieren("GIBT ES NICHT", Verifikationsweg::Qr, 2_000)
        .expect_err("muss fehlschlagen");

    assert!(fehler.meldung.contains("Kein Kontakt"), "{}", fehler.meldung);
}

#[test]
fn der_fingerprint_geht_hin_und_zurueck() {
    let mut s = sitzung();
    for name in ["Anna", "Bert", "Cora"] {
        let fp = fingerprint_von(&mut s, name);
        let k = s
            .offen(1_000)
            .expect("offen")
            .kontakt_verifizieren(&fp, Verifikationsweg::SafetyNumber, 2_000)
            .expect("verifizieren");
        assert_eq!(k.name, name);
        assert_eq!(k.fingerprint, fp);
    }
}

// ---------------------------------------------------------------------------
// Die Austausch-Nutzlast
// ---------------------------------------------------------------------------

#[test]
fn eine_gueltige_nutzlast_wird_gelesen() {
    let mut s = sitzung();
    match s.offen(1_000).expect("offen").nutzlast_lesen(&nutzlast()) {
        Nutzlastbefund::Gelesen {
            hat_signierschluessel,
            hat_post_quantum,
            schon_bekannt,
            ..
        } => {
            assert!(hat_signierschluessel);
            assert!(!hat_post_quantum, "dieser Satz hat keinen ML-KEM-Schluessel");
            assert!(schon_bekannt.is_none());
        }
        andere => panic!("erwartete Gelesen, bekam {andere:?}"),
    }
}

#[test]
fn der_fingerprint_kommt_aus_den_schluesseln_nicht_aus_der_nutzlast() {
    // spec/trust-store.md §5.1: Dem uebertragenen Wert wird nicht vertraut.
    let mut s = sitzung();
    let echt = nutzlast();
    let mut teile: Vec<&str> = echt.split(':').collect();
    teile.pop();
    let gefaelscht = format!("{}:AAAAAAAA", teile.join(":"));

    assert!(
        matches!(
            s.offen(1_000).expect("offen").nutzlast_lesen(&gefaelscht),
            Nutzlastbefund::Beschaedigt { .. }
        ),
        "eine falsche Pruefsumme muss auffallen"
    );
}

#[test]
fn fremdes_wird_als_fremd_erkannt() {
    let mut s = sitzung();
    let fremd = s
        .offen(1_000)
        .expect("offen")
        .nutzlast_lesen("irgendein Text aus der Zwischenablage");

    assert!(matches!(fremd, Nutzlastbefund::Unlesbar { .. }), "{fremd:?}");
}

#[test]
fn ein_bekannter_kontakt_wird_als_solcher_gemeldet() {
    let mut s = sitzung();
    let n = nutzlast();
    let o = s.offen(1_000).expect("offen");
    o.kontakt_aus_nutzlast("Neu", &n, 2_000).expect("aufnehmen");

    match o.nutzlast_lesen(&n) {
        Nutzlastbefund::Gelesen { schon_bekannt, .. } => {
            let b = schon_bekannt.expect("muss bekannt sein");
            assert_eq!(b.name, "Neu");
            assert!(b.gleicher_schluessel);
        }
        andere => panic!("erwartete Gelesen, bekam {andere:?}"),
    }
}

#[test]
fn aus_einer_nutzlast_aufgenommen_ist_gesehen() {
    // Die tragende Regel des Vertrauensmodells.
    let mut s = sitzung();
    let k = s
        .offen(1_000)
        .expect("offen")
        .kontakt_aus_nutzlast("Neu", &nutzlast(), 2_000)
        .expect("aufnehmen");

    assert_eq!(k.vertrauen, Vertrauen::Gesehen);
    assert!(k.verifiziert_am.is_none());
    assert!(k.verifiziert_ueber.is_none());
}

#[test]
fn eine_kaputte_nutzlast_legt_nichts_an() {
    let mut s = sitzung();
    let o = s.offen(1_000).expect("offen");
    let vorher = o.kontakte().len();

    assert!(o.kontakt_aus_nutzlast("Neu", "Unsinn", 2_000).is_err());
    assert_eq!(o.kontakte().len(), vorher, "es darf nichts entstanden sein");
}

#[test]
fn ohne_post_quantum_wird_es_gemeldet() {
    let mut s = sitzung();
    for k in s.offen(1_000).expect("offen").kontakte() {
        assert!(!k.hat_post_quantum, "{} sollte ohne PQ sein", k.name);
    }
}
