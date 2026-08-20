//! Was beim Öffnen eines Envelopes gelten muss.
//!
//! # Die Zusicherung, um die es geht
//!
//! **Der Klartext geht nicht über die Brücke.** Er bleibt in Rust, bis
//! jemand sagt, wohin er soll. Ihn der Oberfläche zu reichen hieße, ihn in
//! eine Webansicht zu legen, die wir weder überschreiben noch begrenzen
//! können — und dort bliebe er, solange das Fenster steht.
//!
//! # Die zweite
//!
//! **„Gültig signiert“ heißt nicht „von Anna“.** Der Envelope sagt nur,
//! welcher Schlüssel signiert hat. Wem der gehört, weiß allein der
//! Kontaktspeicher. Genau an diesem Unterschied entscheidet sich, ob
//! jemand einer Nachricht zu Recht traut.

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

/// Eine entsperrte Sitzung über einer frischen Identität.
fn wer() -> (Sitzung, Identity) {
    let id = Identity::generate(&mut OsRandom, true, 1_700_000_000).expect("Identität");
    let datei =
        keyfile::write(&id, PASSWORT.as_bytes(), &sparsam(), &mut OsRandom).expect("schreiben");
    let mut s = Sitzung::neu(datei, None, Sperrfrist::FuenfzehnMinuten);
    s.entsperren(&passwort(), 1_000).expect("entsperren");
    (s, id)
}

fn nutzlast_von(id: &Identity) -> String {
    cabrik_core::trust::qr_payload(
        &id.enc_pub().expect("enc_pub"),
        id.sig_pub().as_ref(),
        Some(&id.xwing_pub()),
    )
}

fn fingerprint(s: &mut Sitzung, name: &str) -> String {
    s.offen(1_000)
        .expect("offen")
        .kontakte()
        .into_iter()
        .find(|k| k.name == name)
        .expect("Kontakt")
        .fingerprint
}

/// Ich schicke an die Gegenseite. `kennt_mich` steuert, ob sie mich kennt.
fn hin_und_zurueck(kennt_mich: bool, signieren: bool) -> (Sitzung, Vec<u8>) {
    let (mut ich, meine_id) = wer();
    let (mut gegen, gegen_id) = wer();

    if kennt_mich {
        gegen
            .offen(1_000)
            .expect("offen")
            .kontakt_aus_nutzlast("Ich", &nutzlast_von(&meine_id), 1_000)
            .expect("aufnehmen");
    }

    ich.offen(1_000)
        .expect("offen")
        .kontakt_aus_nutzlast("Gegenseite", &nutzlast_von(&gegen_id), 1_000)
        .expect("aufnehmen");
    let fp = fingerprint(&mut ich, "Gegenseite");

    let offen = ich.offen(1_000).expect("offen");
    let plan = offen.versand_planen(&[fp], signieren).expect("Plan");
    let envelope = offen
        .verschluesseln(&plan, "bericht.pdf", b"streng vertraulich", &mut OsRandom)
        .expect("verschluesseln");

    (gegen, envelope)
}

// ---------------------------------------------------------------------------
// Der Klartext bleibt hier
// ---------------------------------------------------------------------------

#[test]
fn der_bericht_traegt_den_klartext_nicht() {
    // Die Architekturregel als Test. Der Bericht sagt, WAS ankam -- nicht,
    // was drinsteht.
    let (mut gegen, envelope) = hin_und_zurueck(true, true);
    let offen = gegen.offen(1_000).expect("offen");

    let bericht = offen.envelope_oeffnen(&envelope, false).expect("oeffnen");

    assert!(bericht.text.is_none(), "eine Datei hat keinen Text");
    let ausgegeben = format!("{bericht:?}");
    assert!(
        !ausgegeben.contains("streng vertraulich"),
        "der Klartext darf im Bericht nirgends auftauchen"
    );
}

#[test]
fn der_klartext_ist_trotzdem_da_fuer_den_der_schreibt() {
    // Die Gegenprobe: Eine Regel, die den Inhalt ganz verschwinden liesse,
    // machte das Entschluesseln nutzlos.
    let (mut gegen, envelope) = hin_und_zurueck(true, true);
    let offen = gegen.offen(1_000).expect("offen");
    offen.envelope_oeffnen(&envelope, false).expect("oeffnen");

    let (inhalt, name) = offen.nutzlast().expect("Nutzlast");

    assert_eq!(inhalt, b"streng vertraulich");
    assert_eq!(name, Some("bericht.pdf"));
}

#[test]
fn die_nutzlast_geht_mit_der_sperre() {
    // Ein entschluesselter Inhalt darf eine Sperre nicht ueberleben.
    let (mut gegen, envelope) = hin_und_zurueck(true, true);
    gegen
        .offen(1_000)
        .expect("offen")
        .envelope_oeffnen(&envelope, false)
        .expect("oeffnen");
    assert!(gegen.offen(1_000).expect("offen").nutzlast().is_some());

    gegen.sperren();
    gegen.entsperren(&passwort(), 2_000).expect("entsperren");

    assert!(
        gegen.offen(2_000).expect("offen").nutzlast().is_none(),
        "nach dem Sperren darf nichts mehr dastehen"
    );
}

#[test]
fn verwerfen_nimmt_den_klartext_weg() {
    let (mut gegen, envelope) = hin_und_zurueck(true, true);
    let offen = gegen.offen(1_000).expect("offen");
    offen.envelope_oeffnen(&envelope, false).expect("oeffnen");

    offen.nutzlast_verwerfen();

    assert!(offen.nutzlast().is_none());
}

// ---------------------------------------------------------------------------
// Wer geschickt hat
// ---------------------------------------------------------------------------

#[test]
fn was_ankam_traegt_namen_und_groesse() {
    let (mut gegen, envelope) = hin_und_zurueck(true, true);

    let bericht = gegen
        .offen(1_000)
        .expect("offen")
        .envelope_oeffnen(&envelope, false)
        .expect("oeffnen");

    assert_eq!(bericht.dateiname.as_deref(), Some("bericht.pdf"));
    assert_eq!(bericht.groesse_bytes, 18);
}

#[test]
fn ein_bekannter_absender_wird_benannt() {
    let (mut gegen, envelope) = hin_und_zurueck(true, true);

    let bericht = gegen
        .offen(1_000)
        .expect("offen")
        .envelope_oeffnen(&envelope, false)
        .expect("oeffnen");

    let gezeigt = format!("{:?}", bericht.absender);
    assert!(gezeigt.contains("Ich"), "der Name gehoert dazu: {gezeigt}");
}

#[test]
fn ein_unbekannter_absender_wird_nicht_erfunden() {
    // „Gueltig signiert" heisst nicht „von Anna". Wer den Schluessel nicht
    // kennt, weiss nicht, wem er gehoert -- und genau das ist die Aussage.
    let (mut gegen, envelope) = hin_und_zurueck(false, true);

    let bericht = gegen
        .offen(1_000)
        .expect("offen")
        .envelope_oeffnen(&envelope, false)
        .expect("oeffnen");

    let gezeigt = format!("{:?}", bericht.absender);
    assert!(gezeigt.contains("Unbekannt"), "{gezeigt}");
}

#[test]
fn eine_unsignierte_nachricht_sagt_nichts_ueber_den_absender() {
    let (mut gegen, envelope) = hin_und_zurueck(true, false);

    let bericht = gegen
        .offen(1_000)
        .expect("offen")
        .envelope_oeffnen(&envelope, false)
        .expect("oeffnen");

    let gezeigt = format!("{:?}", bericht.absender);
    assert!(gezeigt.contains("Unsigniert"), "{gezeigt}");
}

#[test]
fn wer_eine_signatur_verlangt_bekommt_ohne_sie_einen_fehler() {
    // Dieselbe Lage, anders bewertet -- je nachdem, was der Nutzer
    // verlangt hat, nicht was das Programm fuer richtig haelt.
    let (mut gegen, envelope) = hin_und_zurueck(true, false);

    assert!(
        gegen
            .offen(1_000)
            .expect("offen")
            .envelope_oeffnen(&envelope, true)
            .is_err(),
        "wer eine Signatur verlangt, bekommt ohne sie einen Fehler"
    );
    assert!(
        gegen
            .offen(1_000)
            .expect("offen")
            .envelope_oeffnen(&envelope, false)
            .is_ok(),
        "ohne Verlangen ist anonymer Versand ein legitimer Modus"
    );
}

// ---------------------------------------------------------------------------
// Was nicht aufgeht
// ---------------------------------------------------------------------------

#[test]
fn ein_fremder_envelope_wird_abgelehnt_ohne_zu_verraten_warum() {
    // „Nicht an Sie gerichtet" und „veraendert" sind nach aussen
    // ununterscheidbar -- sonst verriete die Meldung, ob jemand Empfaenger
    // eines Envelopes ist, den er gar nicht oeffnen kann.
    let (_, envelope) = hin_und_zurueck(true, true);
    let (mut dritter, _) = wer();

    let fehler = dritter
        .offen(1_000)
        .expect("offen")
        .envelope_oeffnen(&envelope, false)
        .expect_err("darf nicht aufgehen");

    assert!(
        fehler.meldung.contains("nicht öffnen"),
        "{}",
        fehler.meldung
    );
    assert!(
        !fehler.meldung.to_lowercase().contains("empfänger für"),
        "die Meldung darf nicht verraten, WARUM es nicht aufging"
    );
}

#[test]
fn ein_veraenderter_envelope_geht_nicht_auf() {
    let (mut gegen, mut envelope) = hin_und_zurueck(true, true);
    // Ein einziges Byte im hinteren Teil kippen.
    let stelle = envelope.len().saturating_sub(20);
    if let Some(b) = envelope.get_mut(stelle) {
        *b ^= 0xFF;
    }

    assert!(
        gegen
            .offen(1_000)
            .expect("offen")
            .envelope_oeffnen(&envelope, false)
            .is_err(),
        "ein veraenderter Envelope darf nicht aufgehen"
    );
}

#[test]
fn etwas_das_kein_envelope_ist_wird_benannt() {
    let (mut gegen, _) = hin_und_zurueck(true, true);

    assert!(
        gegen
            .offen(1_000)
            .expect("offen")
            .envelope_oeffnen(b"das ist keine Cabrik-Datei", false)
            .is_err()
    );
}
