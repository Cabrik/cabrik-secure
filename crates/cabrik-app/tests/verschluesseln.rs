//! Was beim Verschlüsseln gelten muss.
//!
//! # Die eine Zusicherung, um die es geht
//!
//! **An einen widerrufenen Schlüssel wird nicht verschlüsselt.** Wer ihn
//! widerrufen hat, hat festgestellt, dass jemand anders den privaten Teil
//! besitzt. Ein Envelope an diesen Schlüssel wäre keine Nachricht, sondern
//! eine Übergabe.
//!
//! Das ist der einzige Fall, in dem dieses Programm sich weigert. Alles
//! andere — ein nicht verifizierter Kontakt, ein gewechselter Schlüssel —
//! wird gesagt, nicht verhindert.

#![expect(clippy::expect_used, reason = "Fehlschlag soll den Test abbrechen")]

use cabrik_app::Sitzung;
use cabrik_bruecke::{Sperrfrist, Verifikationsweg};
use cabrik_core::keyfile::{self, KdfParams};
use cabrik_core::{Identity, OsRandom};
use zeroize::Zeroizing;

const PASSWORT: &str = "vier zufaellige woerter hier";

fn schluesseldatei(signieren: bool) -> Vec<u8> {
    let id = Identity::generate(&mut OsRandom, signieren, 1_700_000_000).expect("Identität");
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

/// Die Austausch-Nutzlast einer **echten** Identität.
///
/// Erfundene Bytes gäben keinen X-Wing-Schlüssel her — und damit ließe
/// sich weder die Wahl des Verfahrens noch der Rundweg prüfen.
fn nutzlast_von(id: &Identity) -> String {
    cabrik_core::trust::qr_payload(
        &id.enc_pub().expect("enc_pub"),
        id.sig_pub().as_ref(),
        Some(&id.xwing_pub()),
    )
}

/// Eine entsperrte Sitzung mit drei Kontakten: Anna, Bert, Cora.
fn sitzung(signieren: bool) -> Sitzung {
    let mut s = Sitzung::neu(
        schluesseldatei(signieren),
        None,
        Sperrfrist::FuenfzehnMinuten,
    );
    s.entsperren(&passwort(), 1_000).expect("entsperren");

    let auf = s.offen(1_000).expect("offen");
    for name in ["Anna", "Bert", "Cora"] {
        let id = Identity::generate(&mut OsRandom, true, 1_700_000_000).expect("Identität");
        auf.kontakt_aus_nutzlast(name, &nutzlast_von(&id), 1_000)
            .expect("aufnehmen");
    }
    s
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

// ---------------------------------------------------------------------------
// Die Weigerung
// ---------------------------------------------------------------------------

#[test]
fn an_einen_widerrufenen_schluessel_wird_nicht_verschluesselt() {
    // Der einzige Fall, in dem dieses Programm sich weigert. Wer den
    // Schluessel widerrufen hat, hat festgestellt, dass jemand anders den
    // privaten Teil besitzt.
    let mut s = sitzung(true);
    let anna = fingerprint(&mut s, "Anna");
    s.offen(1_000)
        .expect("offen")
        .kontakt_widerrufen(&anna, 1_100, None)
        .expect("widerrufen");

    let fehler = s
        .offen(1_000)
        .expect("offen")
        .versand_planen(&[anna], true)
        .expect_err("das muss scheitern");

    assert!(
        fehler.meldung.contains("kompromittiert"),
        "{}",
        fehler.meldung
    );
    assert!(fehler.meldung.contains("Anna"), "der Name gehoert dazu");
}

#[test]
fn ein_widerrufener_verhindert_den_ganzen_stapel() {
    // Nicht „die anderen gehen trotzdem": Wer drei Empfaenger waehlt und
    // zwei bekommt, hat etwas verschickt, das er so nicht wollte.
    let mut s = sitzung(true);
    let anna = fingerprint(&mut s, "Anna");
    let bert = fingerprint(&mut s, "Bert");
    s.offen(1_000)
        .expect("offen")
        .kontakt_widerrufen(&anna, 1_100, None)
        .expect("widerrufen");

    assert!(
        s.offen(1_000)
            .expect("offen")
            .versand_planen(&[bert, anna], true)
            .is_err()
    );
}

#[test]
fn ohne_empfaenger_gibt_es_keinen_plan() {
    // Ein Envelope ohne Kapsel und ohne Passwort liesse sich von niemandem
    // oeffnen -- auch vom Absender nicht.
    let mut s = sitzung(true);

    assert!(
        s.offen(1_000)
            .expect("offen")
            .versand_planen(&[], true)
            .is_err()
    );
}

#[test]
fn ein_unbekannter_fingerprint_scheitert_statt_uebergangen_zu_werden() {
    let mut s = sitzung(true);

    let fehler = s
        .offen(1_000)
        .expect("offen")
        .versand_planen(&["GIBT-ES-NICHT".to_owned()], true)
        .expect_err("muss scheitern");

    assert!(fehler.meldung.contains("Verzeichnis"), "{}", fehler.meldung);
}

// ---------------------------------------------------------------------------
// Vorbehalte: gesagt, nicht verhindert
// ---------------------------------------------------------------------------

#[test]
fn ein_nicht_verifizierter_kontakt_wird_vermerkt_aber_nicht_verhindert() {
    let mut s = sitzung(true);
    let anna = fingerprint(&mut s, "Anna");

    let plan = s
        .offen(1_000)
        .expect("offen")
        .versand_planen(&[anna], true)
        .expect("das darf nicht scheitern");

    assert!(
        plan.vorbehalte
            .iter()
            .any(|v| v.contains("nicht verifiziert")),
        "{:?}",
        plan.vorbehalte
    );
}

#[test]
fn ein_verifizierter_kontakt_hat_keinen_vorbehalt() {
    // Die Gegenprobe: Vorbehalte, die immer erscheinen, liest niemand.
    let mut s = sitzung(true);
    let anna = fingerprint(&mut s, "Anna");
    s.offen(1_000)
        .expect("offen")
        .kontakt_verifizieren(&anna, Verifikationsweg::Qr, 1_100)
        .expect("verifizieren");

    let plan = s
        .offen(1_000)
        .expect("offen")
        .versand_planen(&[anna], true)
        .expect("Plan");

    assert!(plan.vorbehalte.is_empty(), "{:?}", plan.vorbehalte);
}

// ---------------------------------------------------------------------------
// Das Verfahren
// ---------------------------------------------------------------------------

#[test]
fn mit_post_quantum_faehigen_empfaengern_wird_hybrid_gewaehlt() {
    let mut s = sitzung(true);
    let anna = fingerprint(&mut s, "Anna");

    let plan = s
        .offen(1_000)
        .expect("offen")
        .versand_planen(&[anna], true)
        .expect("Plan");

    assert!(
        plan.suite_name().contains("Post-Quantum"),
        "{}",
        plan.suite_name()
    );
}

#[test]
fn eine_identitaet_ohne_signierschluessel_signiert_nicht_und_sagt_es() {
    // Ein gewaehlter Modus, kein Mangel -- aber er darf nicht
    // stillschweigend unterbleiben. Wer „signieren" ankreuzt und nichts
    // signiert bekommt, glaubt an eine Zusicherung, die es nicht gibt.
    let mut s = Sitzung::neu(schluesseldatei(false), None, Sperrfrist::FuenfzehnMinuten);
    s.entsperren(&passwort(), 1_000).expect("entsperren");
    let n = cabrik_core::trust::qr_payload(&[0x11; 32], Some(&[0x21; 32]), None);
    s.offen(1_000)
        .expect("offen")
        .kontakt_aus_nutzlast("Anna", &n, 1_000)
        .expect("aufnehmen");
    let anna = fingerprint(&mut s, "Anna");

    let plan = s
        .offen(1_000)
        .expect("offen")
        .versand_planen(&[anna], true)
        .expect("Plan");

    assert!(
        !plan.signiert(),
        "ohne Schluessel kann nicht signiert werden"
    );
}

// ---------------------------------------------------------------------------
// Der Rundweg
// ---------------------------------------------------------------------------

#[test]
fn was_verschluesselt_wurde_laesst_sich_wieder_oeffnen() {
    // Die Probe aufs Ganze. Ein Envelope, den niemand oeffnen kann, ist
    // kein Schutz, sondern Datenverlust -- und das faellt sonst erst dem
    // Empfaenger auf, also zu spaet.
    let anna = Identity::generate(&mut OsRandom, true, 1_700_000_000).expect("Anna");

    let mut ich = Sitzung::neu(schluesseldatei(true), None, Sperrfrist::FuenfzehnMinuten);
    ich.entsperren(&passwort(), 1_000).expect("entsperren");
    ich.offen(1_000)
        .expect("offen")
        .kontakt_aus_nutzlast("Anna", &nutzlast_von(&anna), 1_000)
        .expect("aufnehmen");
    let fp = fingerprint(&mut ich, "Anna");

    let offen = ich.offen(1_000).expect("offen");
    let plan = offen.versand_planen(&[fp], true).expect("Plan");
    let envelope = offen
        .verschluesseln(&plan, "bericht.pdf", b"streng vertraulich", &mut OsRandom)
        .expect("verschluesseln");

    assert!(
        !envelope.windows(18).any(|f| f == b"streng vertraulich"),
        "der Klartext darf nicht im Envelope stehen"
    );

    // Und jetzt Annas Seite.
    let opener = cabrik_core::envelope::Opener::Identity(&anna);
    let geoeffnet = cabrik_core::envelope::open(&opener, &envelope, false).expect("oeffnen");

    assert_eq!(geoeffnet.plaintext.as_slice(), b"streng vertraulich");
    assert_eq!(geoeffnet.filename.as_deref(), Some("bericht.pdf"));
}

#[test]
fn wer_nicht_empfaenger_ist_kommt_nicht_hinein() {
    // Die Gegenprobe. Ohne sie bewiese der Rundweg nur, dass sich etwas
    // oeffnen laesst -- nicht, dass es verschlossen war.
    let anna = Identity::generate(&mut OsRandom, true, 1_700_000_000).expect("Anna");
    let fremder = Identity::generate(&mut OsRandom, true, 1_700_000_000).expect("Fremder");

    let mut ich = Sitzung::neu(schluesseldatei(true), None, Sperrfrist::FuenfzehnMinuten);
    ich.entsperren(&passwort(), 1_000).expect("entsperren");
    ich.offen(1_000)
        .expect("offen")
        .kontakt_aus_nutzlast("Anna", &nutzlast_von(&anna), 1_000)
        .expect("aufnehmen");
    let fp = fingerprint(&mut ich, "Anna");

    let offen = ich.offen(1_000).expect("offen");
    let plan = offen.versand_planen(&[fp], true).expect("Plan");
    let envelope = offen
        .verschluesseln(&plan, "bericht.pdf", b"streng vertraulich", &mut OsRandom)
        .expect("verschluesseln");

    let opener = cabrik_core::envelope::Opener::Identity(&fremder);
    assert!(
        cabrik_core::envelope::open(&opener, &envelope, false).is_err(),
        "ein Fremder darf nicht hineinkommen"
    );
}

#[test]
fn der_envelope_heisst_wie_die_datei_plus_endung() {
    // Angehaengt, nicht ersetzt: Sonst kollidierten `bericht.pdf` und
    // `bericht.docx` in derselben Datei, und die zweite ueberschriebe die
    // erste.
    assert_eq!(
        cabrik_app::envelope_name("bericht.pdf"),
        "bericht.pdf.cabrik"
    );
    assert_eq!(
        cabrik_app::envelope_name("bericht.docx"),
        "bericht.docx.cabrik"
    );
    assert_ne!(
        cabrik_app::envelope_name("bericht.pdf"),
        cabrik_app::envelope_name("bericht.docx")
    );
}

#[test]
fn die_endung_ist_keine_fremde() {
    // `.cab` ist Microsoft Cabinet und in Windows fest an den Explorer
    // vergeben. Eine Dateizuordnung darauf hiesse, einen Systemdateityp zu
    // kapern -- ein Verhalten, an dem Virenscanner anschlagen. Dieser Test
    // haelt fest, dass der Wechsel nicht versehentlich rueckgaengig gemacht
    // wird.
    let endung = cabrik_core::envelope::ENDUNG;

    for fremd in ["cab", "zip", "7z", "rar", "gz", "enc"] {
        assert_ne!(endung, fremd, "die Endung darf keine belegte sein");
    }
    assert!(
        cabrik_core::envelope::ALTE_ENDUNGEN.contains(&"cab"),
        "vorhandene .cab-Dateien muessen weiter zu finden sein"
    );
}
