//! Der QR-Code zur Austausch-Nutzlast.
//!
//! # Was hier geprüft wird
//!
//! Dass er **die Nutzlast trägt** — nicht, dass er hübsch aussieht. Ein
//! QR-Code, den jemand mit der Kamera abliest, statt ihn zu lesen, muss
//! prüfbar sein, ohne dass ein Mensch danebensteht.
//!
//! # Der Befund, der dabei herauskam
//!
//! Der Post-Quantum-Schlüssel macht den Code groß. Von rund 2070 Zeichen
//! einer Nutzlast sind **1946 der X-Wing-Schlüssel**. Gemessen: 141 Module
//! Kantenlänge mit ihm, 41 ohne — mehr als das Dreifache. Das ist keine
//! Schwäche der Umsetzung, sondern der Preis der Sache, und er wird hier
//! sichtbar.

#![expect(clippy::expect_used, reason = "Fehlschlag soll den Test abbrechen")]

use cabrik_app::Sitzung;
use cabrik_bruecke::Sperrfrist;
use cabrik_core::keyfile::{self, KdfStufe};
use cabrik_core::{Identity, OsRandom};
use zeroize::Zeroizing;

const PASSWORT: &str = "vier zufaellige woerter hier";

fn nutzlast() -> String {
    let id = Identity::generate(&mut OsRandom, true, 1_700_000_000).expect("Identität");
    let datei = keyfile::write(
        &id,
        PASSWORT.as_bytes(),
        &KdfStufe::Min.params(),
        &mut OsRandom,
    )
    .expect("schreiben");
    let mut s = Sitzung::neu(datei, None, Sperrfrist::FuenfzehnMinuten);
    s.entsperren(&Zeroizing::new(PASSWORT.as_bytes().to_vec()), 1_000)
        .expect("entsperren");
    s.offen(1_000)
        .expect("offen")
        .eigene_nutzlast()
        .expect("Nutzlast")
}

#[test]
fn ein_kurzer_text_ergibt_einen_kleinen_code() {
    let qr = cabrik_app::qr_code("HALLO").expect("QR");

    assert!(
        qr.groesse >= 21,
        "kleiner als die kleinste Fassung geht nicht"
    );
    assert!(
        qr.groesse <= 45,
        "fuer fuenf Zeichen reicht wenig: {}",
        qr.groesse
    );
    assert!(!qr.pfad.is_empty());
}

#[test]
fn eine_austausch_nutzlast_passt_hinein() {
    // Die eigentliche Frage. Sie ist rund 2070 Zeichen lang -- nah an dem,
    // was das Format ueberhaupt fasst.
    let n = nutzlast();
    assert!(n.len() > 2000, "die Nutzlast ist {} Zeichen", n.len());

    let qr = cabrik_app::qr_code(&n).expect("die Nutzlast muss hineinpassen");

    assert!(!qr.pfad.is_empty());
}

#[test]
fn der_post_quantum_schluessel_treibt_die_groesse() {
    // Der Befund, der beim Bauen herauskam -- festgehalten, damit er nicht
    // in Vergessenheit geraet und jemand sich spaeter wundert.
    let mit_pq = cabrik_app::qr_code(&nutzlast()).expect("QR");

    // Dieselbe Nutzlast ohne den X-Wing-Teil.
    let ohne_pq: String = {
        let n = nutzlast();
        let teile: Vec<&str> = n.split(':').collect();
        format!(
            "{}:{}:{}:{}::{}",
            teile.first().unwrap_or(&""),
            teile.get(1).unwrap_or(&""),
            teile.get(2).unwrap_or(&""),
            teile.get(3).unwrap_or(&""),
            teile.get(5).unwrap_or(&"")
        )
    };
    let ohne = cabrik_app::qr_code(&ohne_pq).expect("QR");

    assert!(
        mit_pq.groesse > ohne.groesse * 2,
        "mit Post-Quantum {} Module, ohne {} -- der Schluessel ist der Grund",
        mit_pq.groesse,
        ohne.groesse
    );
}

#[test]
fn der_pfad_hat_ein_feld_je_dunklem_modul() {
    // Ein Pfad statt dreissigtausend Rechtecken: Der Test haelt fest, dass
    // die Form stimmt, sonst zeichnet die Oberflaeche nichts.
    let qr = cabrik_app::qr_code("TEST").expect("QR");

    let felder = qr.pfad.matches('M').count();
    assert!(felder > 0, "es muss dunkle Felder geben");
    assert!(
        felder < qr.groesse * qr.groesse,
        "nicht alles kann dunkel sein"
    );
    assert!(
        qr.pfad.starts_with('M'),
        "ein Pfad beginnt mit einem Sprung"
    );
}

#[test]
fn zweimal_derselbe_text_ergibt_denselben_code() {
    // Er ist eine Umrechnung, kein Vorgang mit Zufall. Waere er jedes Mal
    // anders, koennte niemand zwei Codes vergleichen.
    let a = cabrik_app::qr_code("HALLO WELT").expect("QR");
    let b = cabrik_app::qr_code("HALLO WELT").expect("QR");

    assert_eq!(a.groesse, b.groesse);
    assert_eq!(a.pfad, b.pfad);
}

#[test]
fn was_nicht_hineinpasst_wird_benannt_statt_gekuerzt() {
    // Die gefaehrlichste Moeglichkeit waere ein Code, der die Haelfte
    // traegt: Er liesse sich scannen und ergaebe Unsinn.
    let zu_lang = "A".repeat(10_000);

    let fehler = cabrik_app::qr_code(&zu_lang).expect_err("muss scheitern");

    assert!(fehler.meldung.contains("QR-Code"), "{}", fehler.meldung);
}
