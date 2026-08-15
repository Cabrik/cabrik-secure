//! Das Passwort ändern.
//!
//! # Die Erwartung, die am häufigsten danebenliegt
//!
//! **Die Identität bleibt dieselbe.** Es wird nur die Hülle neu
//! verschlossen: derselbe Fingerprint, dieselben Kontakte, dieselben alten
//! Envelopes gehen weiter auf. Ein geändertes Passwort schützt **nicht**
//! davor, dass jemand den privaten Schlüssel schon hat — dafür braucht es
//! eine neue Identität.
//!
//! Und die unbequeme Folge davon: Eine **alte Sicherungskopie** öffnet sich
//! weiter mit dem alten Passwort. Wer wechselt, weil das alte verbrannt
//! ist, muss auch die Kopien austauschen.

#![expect(clippy::expect_used, reason = "Fehlschlag soll den Test abbrechen")]

use cabrik_app::Sitzung;
use cabrik_bruecke::Sperrfrist;
use cabrik_core::keyfile::{self, KdfParams, KdfStufe};
use cabrik_core::{Identity, OsRandom};
use zeroize::Zeroizing;

const ALT: &str = "vier zufaellige woerter hier";
const NEU: &str = "ganz andere vier woerter";

/// Die schwaechste benannte Stufe -- damit die Tests nicht sekundenlang
/// rechnen, und damit `von_params` sie wiedererkennt.
fn sparsam() -> KdfParams {
    KdfStufe::Min.params()
}

fn pw(s: &str) -> Zeroizing<String> {
    Zeroizing::new(s.to_owned())
}

fn sitzung() -> Sitzung {
    let id = Identity::generate(&mut OsRandom, true, 1_700_000_000).expect("Identität");
    let datei = keyfile::write(&id, ALT.as_bytes(), &sparsam(), &mut OsRandom).expect("schreiben");
    let mut s = Sitzung::neu(datei, None, Sperrfrist::FuenfzehnMinuten);
    s.entsperren(&pw(ALT), 1_000).expect("entsperren");
    s
}

// ---------------------------------------------------------------------------
// Der Wechsel
// ---------------------------------------------------------------------------

#[test]
fn danach_oeffnet_das_neue_passwort() {
    let mut s = sitzung();

    s.passwort_aendern(&pw(ALT), &pw(NEU), &mut OsRandom)
        .expect("aendern");

    let mut zweite = Sitzung::neu(
        s.schluesseldatei().to_vec(),
        None,
        Sperrfrist::FuenfzehnMinuten,
    );
    assert!(zweite.entsperren(&pw(NEU), 2_000).is_ok());
}

#[test]
fn das_alte_passwort_oeffnet_die_neue_datei_nicht_mehr() {
    // Die Gegenprobe. Ohne sie bewiese der Test oben nur, dass IRGENDEIN
    // Passwort geht.
    let mut s = sitzung();

    s.passwort_aendern(&pw(ALT), &pw(NEU), &mut OsRandom)
        .expect("aendern");

    let mut zweite = Sitzung::neu(
        s.schluesseldatei().to_vec(),
        None,
        Sperrfrist::FuenfzehnMinuten,
    );
    assert!(zweite.entsperren(&pw(ALT), 2_000).is_err());
}

#[test]
fn ein_falsches_altes_passwort_aendert_nichts() {
    // Weder die Datei noch die Sitzung. Wer sich vertippt, darf nicht
    // ausgesperrt werden.
    let mut s = sitzung();
    let vorher = s.schluesseldatei().to_vec();

    let fehler = s
        .passwort_aendern(&pw("etwas ganz anderes"), &pw(NEU), &mut OsRandom)
        .expect_err("muss scheitern");

    assert!(fehler.meldung.contains("bisherige Passwort"), "{}", fehler.meldung);
    assert_eq!(s.schluesseldatei(), vorher, "die Datei muss unangetastet sein");
}

#[test]
fn ein_leeres_neues_passwort_wird_abgelehnt() {
    let mut s = sitzung();
    let vorher = s.schluesseldatei().to_vec();

    assert!(s.passwort_aendern(&pw(ALT), &pw("   "), &mut OsRandom).is_err());
    assert_eq!(s.schluesseldatei(), vorher);
}

// ---------------------------------------------------------------------------
// Was gleich bleibt
// ---------------------------------------------------------------------------

#[test]
fn der_fingerprint_bleibt_derselbe() {
    // Die Erwartung, die am haeufigsten danebenliegt. Ein geaendertes
    // Passwort ist KEIN neuer Schluessel -- wer das glaubt, haelt sich fuer
    // geschuetzt, wo er es nicht ist.
    let mut s = sitzung();
    let datei_vorher = s.schluesseldatei().to_vec();
    let vorher = s
        .offen(1_000)
        .expect("offen")
        .identitaet(&datei_vorher, "p".to_owned())
        .expect("Identitaet")
        .fingerprint;

    s.passwort_aendern(&pw(ALT), &pw(NEU), &mut OsRandom)
        .expect("aendern");

    let datei_nachher = s.schluesseldatei().to_vec();
    let nachher = s
        .offen(1_000)
        .expect("offen")
        .identitaet(&datei_nachher, "p".to_owned())
        .expect("Identitaet")
        .fingerprint;

    assert_eq!(vorher, nachher);
}

#[test]
fn die_kontakte_bleiben_lesbar() {
    // Der Kontaktspeicher haengt an der Identitaet, nicht am Passwort.
    // Ginge er dabei verloren, waere „Passwort aendern" ein Datenverlust
    // mit harmlosem Namen.
    let mut s = sitzung();
    let fremde = Identity::generate(&mut OsRandom, true, 1_700_000_000).expect("fremd");
    let nutzlast = cabrik_core::trust::qr_payload(
        &fremde.enc_pub().expect("enc_pub"),
        fremde.sig_pub().as_ref(),
        Some(&fremde.xwing_pub()),
    );
    s.offen(1_000)
        .expect("offen")
        .kontakt_aus_nutzlast("Anna", &nutzlast, 1_000)
        .expect("aufnehmen");
    let gesichert = s.kontakte_sichern(1_000, &mut OsRandom).expect("sichern");

    s.passwort_aendern(&pw(ALT), &pw(NEU), &mut OsRandom)
        .expect("aendern");

    let mut zweite = Sitzung::neu(
        s.schluesseldatei().to_vec(),
        Some(gesichert),
        Sperrfrist::FuenfzehnMinuten,
    );
    zweite.entsperren(&pw(NEU), 2_000).expect("entsperren");
    assert_eq!(zweite.offen(2_000).expect("offen").kontakte().len(), 1);
}

#[test]
fn alte_envelopes_gehen_weiter_auf() {
    // Was an den alten Fingerprint gerichtet war, muss lesbar bleiben.
    // Sonst waere ein Passwortwechsel ein stiller Verlust des Archivs.
    let mut s = sitzung();
    let mut absender = {
        let id = Identity::generate(&mut OsRandom, true, 1_700_000_000).expect("Absender");
        let d = keyfile::write(&id, ALT.as_bytes(), &sparsam(), &mut OsRandom).expect("schreiben");
        let mut a = Sitzung::neu(d, None, Sperrfrist::FuenfzehnMinuten);
        a.entsperren(&pw(ALT), 1_000).expect("entsperren");
        a
    };
    let meine_nutzlast = s
        .offen(1_000)
        .expect("offen")
        .eigene_nutzlast()
        .expect("Nutzlast");
    let offen = absender.offen(1_000).expect("offen");
    offen
        .kontakt_aus_nutzlast("Ich", &meine_nutzlast, 1_000)
        .expect("aufnehmen");
    let fp = offen
        .kontakte()
        .into_iter()
        .next()
        .expect("Kontakt")
        .fingerprint;
    let plan = offen.versand_planen(&[fp], true).expect("Plan");
    let envelope = offen
        .verschluesseln(&plan, "alt.txt", b"von frueher", &mut OsRandom)
        .expect("verschluesseln");

    s.passwort_aendern(&pw(ALT), &pw(NEU), &mut OsRandom)
        .expect("aendern");
    s.sperren();
    s.entsperren(&pw(NEU), 3_000).expect("entsperren");

    let bericht = s
        .offen(3_000)
        .expect("offen")
        .envelope_oeffnen(&envelope, false)
        .expect("ein alter Envelope muss weiter aufgehen");
    assert_eq!(bericht.dateiname.as_deref(), Some("alt.txt"));
}

#[test]
fn die_staerke_der_ableitung_bleibt_wie_sie_war() {
    // „Passwort aendern" aendert das Passwort. Die Ableitung dabei
    // stillschweigend zu verschieben waere eine zweite Entscheidung unter
    // der Flagge der ersten -- und beim Entsperren fiele ploetzlich eine
    // andere Wartezeit an.
    let mut s = sitzung();
    let vorher = keyfile::params_of(s.schluesseldatei()).expect("Parameter");

    s.passwort_aendern(&pw(ALT), &pw(NEU), &mut OsRandom)
        .expect("aendern");

    let nachher = keyfile::params_of(s.schluesseldatei()).expect("Parameter");
    assert_eq!(vorher, nachher);
    assert_eq!(KdfStufe::von_params(&nachher), Some(KdfStufe::Min));
}

// ---------------------------------------------------------------------------
// Die unbequeme Folge
// ---------------------------------------------------------------------------

#[test]
fn eine_alte_sicherungskopie_oeffnet_weiter_mit_dem_alten_passwort() {
    // Das ist keine Fehlfunktion, sondern die Natur der Sache -- und
    // deshalb muss die Oberflaeche es sagen: Wer wechselt, weil das alte
    // Passwort verbrannt ist, hat mit der alten Kopie nichts gewonnen.
    let mut s = sitzung();
    let sicherung = s.schluesseldatei().to_vec();

    s.passwort_aendern(&pw(ALT), &pw(NEU), &mut OsRandom)
        .expect("aendern");

    let mut aus_sicherung = Sitzung::neu(sicherung, None, Sperrfrist::FuenfzehnMinuten);
    assert!(
        aus_sicherung.entsperren(&pw(ALT), 2_000).is_ok(),
        "die alte Kopie geht weiter mit dem alten Passwort auf"
    );
}
