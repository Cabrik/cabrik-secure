//! Die eigene Austausch-Nutzlast.
//!
//! # Warum das die fehlende Hälfte war
//!
//! Ohne sie ist das Programm einseitig: Man kann Kontakte aufnehmen, aber
//! niemand kann einem schreiben. Wer nur das Fenster hat, konnte sich
//! niemandem mitteilen.
//!
//! # Die Zusicherung, die zählt
//!
//! **Was drinsteht, muss die eigene Identität sein — und nichts weiter.**
//! Eine Nutzlast, die nicht zum eigenen Schlüssel gehört, wäre schlimmer
//! als keine: Wer sie weitergibt, bekommt nie eine Nachricht und erfährt
//! nie, warum.

#![expect(clippy::expect_used, reason = "Fehlschlag soll den Test abbrechen")]

use cabrik_app::Sitzung;
use cabrik_bruecke::Sperrfrist;
use cabrik_core::keyfile::{self, KdfParams};
use cabrik_core::{Identity, OsRandom};
use zeroize::Zeroizing;

const PASSWORT: &str = "vier zufaellige woerter hier";

fn sparsam() -> KdfParams {
    KdfParams {
        m_cost: KdfParams::M_COST_MIN,
        t_cost: KdfParams::T_COST_MIN,
        p_cost: 4,
    }
}

fn passwort() -> Zeroizing<Vec<u8>> {
    Zeroizing::new(PASSWORT.as_bytes().to_vec())
}

fn wer(signieren: bool) -> Sitzung {
    let id = Identity::generate(&mut OsRandom, signieren, 1_700_000_000).expect("Identität");
    let datei =
        keyfile::write(&id, PASSWORT.as_bytes(), &sparsam(), &mut OsRandom).expect("schreiben");
    let mut s = Sitzung::neu(datei, None, Sperrfrist::FuenfzehnMinuten);
    s.entsperren(&passwort(), 1_000).expect("entsperren");
    s
}

#[test]
fn die_nutzlast_gehoert_zur_eigenen_identitaet() {
    // Die Zusicherung, die zaehlt. Eine Nutzlast, die nicht zum eigenen
    // Schluessel gehoert, waere schlimmer als keine: Wer sie weitergibt,
    // bekommt nie eine Nachricht und erfaehrt nie, warum.
    let mut ich = wer(true);
    let datei = ich.schluesseldatei().to_vec();
    let offen = ich.offen(1_000).expect("offen");

    let nutzlast = offen.eigene_nutzlast().expect("Nutzlast");
    let ich_selbst = offen
        .identitaet(&datei, "egal".to_owned())
        .expect("Identitaet");

    // Der Fingerprint wird aus den Schluesseln NEU berechnet -- so, wie es
    // die Gegenseite tut. Ihn aus der Nutzlast zu uebernehmen hiesse, dem
    // Absender zu glauben.
    let gelesen = cabrik_core::trust::parse_qr(&nutzlast).expect("lesbar");
    let daraus = cabrik_core::fingerprint::Fingerprint::compute(
        &gelesen.enc_pub,
        gelesen.sig_pub.as_ref(),
        gelesen.xwing_pub.as_deref(),
    );
    assert_eq!(
        daraus.display_full(),
        ich_selbst.fingerprint,
        "die Nutzlast gehoert nicht zur eigenen Identitaet"
    );
}

#[test]
fn ein_anderer_kann_mir_damit_wirklich_schreiben() {
    // Der Rundweg, der alles Weitere traegt: Wer meine Nutzlast aufnimmt
    // und mir schreibt, dessen Envelope muss bei mir aufgehen.
    let mut ich = wer(true);
    let nutzlast = ich
        .offen(1_000)
        .expect("offen")
        .eigene_nutzlast()
        .expect("Nutzlast");

    // Die Gegenseite nimmt mich auf und schreibt.
    let mut anderer = wer(true);
    let offen = anderer.offen(1_000).expect("offen");
    offen
        .kontakt_aus_nutzlast("Ich", &nutzlast, 1_000)
        .expect("aufnehmen");
    let fp = offen
        .kontakte()
        .into_iter()
        .next()
        .expect("Kontakt")
        .fingerprint;
    let plan = offen.versand_planen(&[fp], true).expect("Plan");
    let envelope = offen
        .verschluesseln(&plan, "gruss.txt", b"komm gut heim", &mut OsRandom)
        .expect("verschluesseln");

    // Und bei mir geht er auf.
    let bericht = ich
        .offen(1_000)
        .expect("offen")
        .envelope_oeffnen(&envelope, false)
        .expect("oeffnen");

    assert_eq!(bericht.dateiname.as_deref(), Some("gruss.txt"));
    let (inhalt, _) = ich.offen(1_000).expect("offen").nutzlast().expect("Inhalt");
    assert_eq!(inhalt, b"komm gut heim");
}

#[test]
fn die_nutzlast_traegt_den_post_quantum_schluessel_mit() {
    // Ohne ihn faellt jede Nachricht an mich auf das klassische Verfahren
    // zurueck -- und niemand wuesste, warum.
    let mut ich = wer(true);
    let nutzlast = ich
        .offen(1_000)
        .expect("offen")
        .eigene_nutzlast()
        .expect("Nutzlast");

    let gelesen = cabrik_core::trust::parse_qr(&nutzlast).expect("lesbar");
    assert!(
        gelesen.xwing_pub.is_some(),
        "ohne Post-Quantum-Schluessel waere jede Nachricht an mich klassisch"
    );
}

#[test]
fn eine_identitaet_ohne_signierschluessel_gibt_auch_keinen_weiter() {
    // Ein gewaehlter Modus: Wer anonym bleibt, hat keinen -- und die
    // Nutzlast darf keinen erfinden.
    let mut ich = wer(false);
    let nutzlast = ich
        .offen(1_000)
        .expect("offen")
        .eigene_nutzlast()
        .expect("Nutzlast");

    let gelesen = cabrik_core::trust::parse_qr(&nutzlast).expect("lesbar");
    assert!(gelesen.sig_pub.is_none());
}

#[test]
fn in_der_nutzlast_steht_kein_privater_schluessel() {
    // Sie geht ueber Mail, Messenger, Aushang. Was hier hineingeriete,
    // waere fuer immer draussen.
    let mut ich = wer(true);
    let nutzlast = ich
        .offen(1_000)
        .expect("offen")
        .eigene_nutzlast()
        .expect("Nutzlast");

    // Der private Teil laesst sich nur ueber das Passwort gewinnen. Wenn
    // die Nutzlast ihn enthielte, liesse sich damit oeffnen -- die Probe
    // dagegen ist, dass ein Envelope AN mich mit ihr allein nicht aufgeht.
    let gelesen = cabrik_core::trust::parse_qr(&nutzlast).expect("lesbar");
    let roh = format!("{gelesen:?}");
    for verboten in ["enc_sk", "sig_sk", "pq_seed", "private"] {
        assert!(
            !roh.to_lowercase().contains(verboten),
            "„{verboten}“ hat in einer Austausch-Nutzlast nichts zu suchen"
        );
    }
}

#[test]
fn zweimal_gefragt_ergibt_dasselbe() {
    // Sie ist eine Eigenschaft der Identitaet, kein Vorgang mit Zufall.
    // Waere sie jedes Mal anders, koennte niemand einen Fingerprint
    // vergleichen, den er gestern abgeschrieben hat.
    let mut ich = wer(true);
    let offen = ich.offen(1_000).expect("offen");

    let a = offen.eigene_nutzlast().expect("erste");
    let b = offen.eigene_nutzlast().expect("zweite");

    assert_eq!(a, b);
}
