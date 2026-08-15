//! Text verschlüsseln und wieder öffnen.
//!
//! # Wofür es das gibt
//!
//! Für Kanäle, die keine Dateien nehmen: ein Chatfenster, eine E-Mail, ein
//! Ticket. Wer dort etwas verschlüsselt hinschicken will, braucht Zeichen,
//! keine Bytes.
//!
//! # Die Eigenschaft, die Text von einer Datei unterscheidet
//!
//! **Padding.** Bei Text ist die Länge die Aussage: „ja“ und „auf keinen
//! Fall, und zwar aus folgenden Gründen“ wären sonst von außen zu
//! unterscheiden, ohne ein Wort zu lesen. Der Test unten hält fest, dass
//! zwei verschieden lange Nachrichten gleich lange Envelopes ergeben.

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

fn passwort() -> Zeroizing<String> {
    Zeroizing::new(PASSWORT.to_owned())
}

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

/// Ich schreibe an die Gegenseite. Zurück kommt der Armor-Text.
fn schreiben(text: &str) -> (Sitzung, String) {
    let (mut ich, meine_id) = wer();
    let (mut gegen, gegen_id) = wer();

    gegen
        .offen(1_000)
        .expect("offen")
        .kontakt_aus_nutzlast("Ich", &nutzlast_von(&meine_id), 1_000)
        .expect("aufnehmen");

    let offen = ich.offen(1_000).expect("offen");
    offen
        .kontakt_aus_nutzlast("Gegenseite", &nutzlast_von(&gegen_id), 1_000)
        .expect("aufnehmen");
    let fp = offen
        .kontakte()
        .into_iter()
        .find(|k| k.name == "Gegenseite")
        .expect("Kontakt")
        .fingerprint;

    let plan = offen.versand_planen(&[fp], true).expect("Plan");
    let armor = offen
        .text_verschluesseln(&plan, text, &mut OsRandom)
        .expect("verschluesseln");

    (gegen, armor)
}

// ---------------------------------------------------------------------------
// Der Rundweg
// ---------------------------------------------------------------------------

#[test]
fn ein_text_geht_hin_und_kommt_zurueck() {
    let (mut gegen, armor) = schreiben("Treffpunkt um acht, wie besprochen.");

    let bericht = gegen
        .offen(1_000)
        .expect("offen")
        .text_oeffnen(&armor, false)
        .expect("oeffnen");

    assert_eq!(
        bericht.text.as_deref(),
        Some("Treffpunkt um acht, wie besprochen.")
    );
}

#[test]
fn umlaute_und_zeilenumbrueche_ueberleben() {
    let original = "Grüße aus München.\n\nZweiter Absatz — mit Gedankenstrich.\n";
    let (mut gegen, armor) = schreiben(original);

    let bericht = gegen
        .offen(1_000)
        .expect("offen")
        .text_oeffnen(&armor, false)
        .expect("oeffnen");

    assert_eq!(bericht.text.as_deref(), Some(original));
}

#[test]
fn der_klartext_steht_nicht_im_armor() {
    // Die Probe, die alles Weitere traegt.
    let (_, armor) = schreiben("Treffpunkt um acht");

    assert!(!armor.contains("Treffpunkt"), "der Klartext steht im Armor");
    assert!(!armor.contains("acht"));
}

#[test]
fn ein_text_traegt_keinen_dateinamen() {
    // Es gibt keinen, und einen zu erfinden hiesse, eine Angabe
    // mitzuschicken, die niemand gemacht hat.
    let (mut gegen, armor) = schreiben("kurz");

    let bericht = gegen
        .offen(1_000)
        .expect("offen")
        .text_oeffnen(&armor, false)
        .expect("oeffnen");

    assert!(bericht.dateiname.is_none());
}

// ---------------------------------------------------------------------------
// Die Laenge verraet nichts
// ---------------------------------------------------------------------------

#[test]
fn zwei_verschieden_lange_nachrichten_ergeben_gleich_lange_envelopes() {
    // Der eigentliche Grund, warum Padding bei Text an ist: Sonst waeren
    // „ja" und „auf keinen Fall, und zwar aus folgenden Gruenden" von
    // aussen zu unterscheiden, ohne ein Wort zu lesen.
    let (_, kurz) = schreiben("ja");
    let (_, lang) = schreiben("nein");

    assert_eq!(
        kurz.len(),
        lang.len(),
        "die Laenge darf den Inhalt nicht verraten"
    );
}

#[test]
fn eine_leere_nachricht_wird_abgelehnt() {
    // Ein Envelope ueber nichts ist keine Nachricht.
    let (mut ich, _) = wer();
    let (_, gegen_id) = wer();
    let offen = ich.offen(1_000).expect("offen");
    offen
        .kontakt_aus_nutzlast("G", &nutzlast_von(&gegen_id), 1_000)
        .expect("aufnehmen");
    let fp = offen
        .kontakte()
        .into_iter()
        .next()
        .expect("Kontakt")
        .fingerprint;
    let plan = offen.versand_planen(&[fp], true).expect("Plan");

    assert!(offen.text_verschluesseln(&plan, "   \n  ", &mut OsRandom).is_err());
}

// ---------------------------------------------------------------------------
// Kopiert, wie es im Leben ankommt
// ---------------------------------------------------------------------------

#[test]
fn ein_armor_mit_anrede_und_gruss_drumherum_geht_auf() {
    // So kommt er an: „Hallo, hier ist die Nachricht: ... Viele Gruesse".
    let (mut gegen, armor) = schreiben("Treffpunkt um acht");
    let wie_im_leben = format!("Hallo,\n\nhier ist es:\n\n{armor}\n\nViele Gruesse\nAnna\n");

    let bericht = gegen
        .offen(1_000)
        .expect("offen")
        .text_oeffnen(&wie_im_leben, false)
        .expect("oeffnen");

    assert_eq!(bericht.text.as_deref(), Some("Treffpunkt um acht"));
}

#[test]
fn ein_zitierter_armor_geht_auch_auf() {
    // Aus einer Antwortmail herauskopiert, mit „> " davor.
    let (mut gegen, armor) = schreiben("Treffpunkt um acht");
    let zitiert: String = armor.lines().map(|z| format!("> {z}\r\n")).collect();

    let bericht = gegen
        .offen(1_000)
        .expect("offen")
        .text_oeffnen(&zitiert, false)
        .expect("oeffnen");

    assert_eq!(bericht.text.as_deref(), Some("Treffpunkt um acht"));
}

#[test]
fn ein_text_ohne_envelope_wird_benannt_statt_verschluckt() {
    let (mut gegen, _) = schreiben("egal");

    let fehler = gegen
        .offen(1_000)
        .expect("offen")
        .text_oeffnen("Hallo, wie geht es dir?", false)
        .expect_err("muss scheitern");

    assert!(
        fehler.meldung.contains("BEGIN CABRIK ENVELOPE"),
        "die Meldung soll sagen, wonach zu suchen ist: {}",
        fehler.meldung
    );
}

#[test]
fn ein_veraenderter_armor_geht_nicht_auf() {
    let (mut gegen, armor) = schreiben("Treffpunkt um acht");
    // Ein Zeichen mitten im Inhalt austauschen.
    let zeilen: Vec<&str> = armor.lines().collect();
    let mitte = zeilen.len() / 2;
    let kaputt: String = zeilen
        .iter()
        .enumerate()
        .map(|(i, z)| {
            if i == mitte && z.len() > 4 {
                format!("{}A{}\n", &z[..2], &z[3..])
            } else {
                format!("{z}\n")
            }
        })
        .collect();

    assert!(
        gegen
            .offen(1_000)
            .expect("offen")
            .text_oeffnen(&kaputt, false)
            .is_err(),
        "ein veraenderter Armor darf nicht aufgehen"
    );
}
