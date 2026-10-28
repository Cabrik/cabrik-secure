//! Eine Sicherung einspielen.
//!
//! # Worum es geht
//!
//! Wer seine Schlüsseldatei gesichert hat — gegen eine kaputte Platte,
//! für einen neuen Rechner, oder weil er auf einer Reise keinen Schlüssel
//! dabeihaben will —, muss sie auch wieder hereinbekommen. Bis zum
//! 28.10.2026 ging das nur, indem man die Datei von Hand in den
//! Programmordner kopierte.
//!
//! # Die Regeln, um die es geht
//!
//! 1. **Dieselbe Identität.** Kein neuer Schlüssel, keine neue Hülle,
//!    derselbe Fingerprint.
//! 2. **Erst öffnen, dann schreiben.** Eine Sicherung, die sich mit dem
//!    Passwort nicht öffnen lässt, kommt gar nicht erst auf die Platte.
//! 3. **Nur passende Kontakte.** Eine Kontaktdatei, die sich mit dem
//!    eingespielten Schlüssel nicht öffnen lässt, wird nicht genommen.
//! 4. **Was keine Sicherung ist, wird benannt**, bevor das Passwort zählt.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "Fehlschlag soll den Test abbrechen"
)]

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use cabrik_app::{Einspielung, Sitzung, sicherung_pruefen};
use cabrik_bruecke::{Kontaktherkunft, Sperrfrist, Verifikationsweg, Vertrauen};
use cabrik_core::Speichergrenze;
use cabrik_core::keyfile::{self, KdfStufe as KernStufe};
use cabrik_core::{Identity, OsRandom};
use std::path::PathBuf;

const PASSWORT: &[u8] = b"vier zufaellige woerter hier";
const MIB: u64 = 1 << 20;

/// Eine echte Schlüsseldatei mit der schwächsten Ableitung — sonst rechnet
/// jeder Test Sekunden.
fn schluesseldatei() -> Vec<u8> {
    let id = Identity::generate(&mut OsRandom, true, 1_700_000_000).expect("Identität");
    keyfile::write(
        &id,
        PASSWORT,
        &KernStufe::Min.params(),
        Speichergrenze::UNBEKANNT,
        &mut OsRandom,
    )
    .expect("schreiben")
}

/// Die Kontaktdatei zu `datei`, mit so vielen Kontakten wie `namen` —
/// der erste davon verifiziert.
fn kontaktdatei(datei: &[u8], namen: &[&str]) -> Vec<u8> {
    let mut s = Sitzung::neu(datei.to_vec(), None, Sperrfrist::FuenfzehnMinuten);
    s.entsperren(PASSWORT, 1_000, Speichergrenze::UNBEKANNT)
        .expect("entsperren");
    let o = s.offen(1_000).expect("offen");
    for (i, name) in namen.iter().enumerate() {
        let nr = u8::try_from(i).expect("wenige");
        let n = cabrik_core::trust::qr_payload(
            &[0x40_u8.wrapping_add(nr); 32],
            Some(&[0x60_u8.wrapping_add(nr); 32]),
            None,
        );
        let k = o.kontakt_aus_nutzlast(name, &n, 1_000).expect("aufnehmen");
        if i == 0 {
            o.kontakt_verifizieren(&k.fingerprint, Verifikationsweg::Qr, 1_000)
                .expect("verifizieren");
        }
    }
    s.kontakte_sichern(1_000, &mut OsRandom).expect("sichern")
}

fn einspielen(
    datei: &[u8],
    passwort: &[u8],
    hier: Option<&[u8]>,
    gesichert: Option<&[u8]>,
) -> cabrik_app::Befehlsergebnis<(Sitzung, Kontaktherkunft)> {
    Sitzung::einspielen(
        Einspielung {
            schluesseldatei: datei,
            passwort,
            kontakte_hier: hier,
            kontakte_gesichert: gesichert,
            ungewoehnliche_zugelassen: false,
            frist: Sperrfrist::FuenfzehnMinuten,
        },
        2_000,
        Speichergrenze::UNBEKANNT,
    )
}

fn namen(s: &mut Sitzung) -> Vec<String> {
    let mut n: Vec<String> = s
        .offen(2_000)
        .expect("offen")
        .kontakte()
        .into_iter()
        .map(|k| k.name)
        .collect();
    n.sort();
    n
}

/// Der Fingerprint, wie die eigene Identität ihn nennt.
fn fingerprint(s: &mut Sitzung) -> String {
    let datei = s.schluesseldatei().to_vec();
    s.offen(2_000)
        .expect("offen")
        .identitaet(&datei, String::new())
        .expect("Identität")
        .fingerprint
}

// ---------------------------------------------------------------------------
// Dieselbe Identität
// ---------------------------------------------------------------------------

#[test]
fn eine_sicherung_bringt_dieselbe_identitaet_zurueck() {
    let datei = schluesseldatei();
    let mut vorher = Sitzung::neu(datei.clone(), None, Sperrfrist::FuenfzehnMinuten);
    vorher
        .entsperren(PASSWORT, 2_000, Speichergrenze::UNBEKANNT)
        .expect("entsperren");

    let (mut s, herkunft) = einspielen(&datei, PASSWORT, None, None).expect("einspielen");

    // Offen, wie nach dem Anlegen: Das Passwort wurde eben getippt.
    assert!(!s.ist_gesperrt());
    assert_eq!(fingerprint(&mut s), fingerprint(&mut vorher));
    // Abgelegt wird die Datei, wie sie ist -- keine neue Huelle.
    assert_eq!(s.schluesseldatei(), datei.as_slice());
    assert_eq!(herkunft, Kontaktherkunft::Keine);
    assert!(namen(&mut s).is_empty());
}

#[test]
fn ein_falsches_passwort_wird_abgelehnt() {
    let datei = schluesseldatei();
    let fehler = einspielen(&datei, b"ein ganz anderes passwort", None, None)
        .err()
        .expect("abgelehnt");
    assert!(
        fehler.meldung.contains("passt nicht zu dieser Sicherung"),
        "{}",
        fehler.meldung
    );
}

// ---------------------------------------------------------------------------
// Was keine Sicherung ist
// ---------------------------------------------------------------------------

/// Ein Schlüssel aus Version 1, aus den Vorlagen der Python-Referenz.
fn v1_schluessel() -> Vec<u8> {
    let pfad = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testvectors/v1-compat.json")
        .canonicalize()
        .expect("testvectors/v1-compat.json");
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pfad).expect("lesen")).expect("JSON");
    let v = doc["keyfiles"]
        .as_array()
        .expect("keyfiles")
        .iter()
        .find(|v| v["id"].as_str() == Some("kf-v1-signing"))
        .expect("kf-v1-signing");
    STANDARD
        .decode(v["input"]["keyfile_b64"].as_str().expect("keyfile_b64"))
        .expect("Base64")
}

#[test]
fn was_keine_sicherung_ist_wird_benannt_bevor_das_passwort_zaehlt() {
    let datei = schluesseldatei();
    let faelle: [(&str, Vec<u8>, &str); 3] = [
        ("v1", v1_schluessel(), "Version 1"),
        (
            "Kontakte",
            kontaktdatei(&datei, &["Anna"]),
            "die Kontaktdatei",
        ),
        (
            "Unsinn",
            b"das ist ein Urlaubsfoto und kein Schluessel".to_vec(),
            "keine Schlüsseldatei",
        ),
    ];

    for (fall, daten, satz) in faelle {
        let geprueft = sicherung_pruefen(&daten, Speichergrenze::UNBEKANNT)
            .err()
            .unwrap_or_else(|| panic!("{fall}: abgelehnt"));
        assert!(
            geprueft.meldung.contains(satz),
            "{fall}: {}",
            geprueft.meldung
        );

        // Dasselbe beim Einspielen -- und zwar mit dem RICHTIGEN Passwort
        // der echten Sicherung: Schuld ist die Datei, nicht das Passwort.
        let eingespielt = einspielen(&daten, PASSWORT, None, None)
            .err()
            .unwrap_or_else(|| panic!("{fall}: abgelehnt"));
        assert!(
            eingespielt.meldung.contains(satz),
            "{fall}: {}",
            eingespielt.meldung
        );
    }
}

#[test]
fn eine_echte_sicherung_besteht_die_pruefung() {
    let bedarf =
        sicherung_pruefen(&schluesseldatei(), Speichergrenze::UNBEKANNT).expect("Sicherung");
    assert_eq!(bedarf.speicher_bytes, 64 * MIB);
    assert!(!bedarf.ungewoehnlich);
    assert!(bedarf.passt);
}

// ---------------------------------------------------------------------------
// Die Kontakte
// ---------------------------------------------------------------------------

#[test]
fn die_kontakte_neben_der_sicherung_kommen_mit_samt_pruefung() {
    let datei = schluesseldatei();
    let kontakte = kontaktdatei(&datei, &["Anna", "Bert"]);

    let (mut s, herkunft) =
        einspielen(&datei, PASSWORT, None, Some(&kontakte)).expect("einspielen");

    assert_eq!(herkunft, Kontaktherkunft::Sicherung);
    assert_eq!(namen(&mut s), ["Anna", "Bert"]);
    let anna = s
        .offen(2_000)
        .expect("offen")
        .kontakte()
        .into_iter()
        .find(|k| k.name == "Anna")
        .expect("Anna");
    assert_eq!(anna.vertrauen, Vertrauen::Verifiziert);
}

#[test]
fn eine_fremde_kontaktdatei_wird_nicht_genommen() {
    // Sie gehoert zu einer anderen Identitaet -- oder jemand hat sie neben
    // die Sicherung gelegt. Genommen wird nur, was sich mit DIESEM
    // Schluessel oeffnen laesst.
    let datei = schluesseldatei();
    let fremde = kontaktdatei(&schluesseldatei(), &["Mallory"]);

    let (mut s, herkunft) = einspielen(&datei, PASSWORT, None, Some(&fremde)).expect("einspielen");

    assert_eq!(herkunft, Kontaktherkunft::Unpassend);
    assert!(namen(&mut s).is_empty());
}

#[test]
fn die_kontakte_im_programmordner_haben_vorrang() {
    // Lag nur die Schluesseldatei nicht mehr da, ist die Kontaktdatei im
    // Programmordner der letzte Stand -- juenger als jede Sicherung.
    let datei = schluesseldatei();
    let hier = kontaktdatei(&datei, &["Anna", "Bert", "Cora"]);
    let gesichert = kontaktdatei(&datei, &["Anna"]);

    let (mut s, herkunft) =
        einspielen(&datei, PASSWORT, Some(&hier), Some(&gesichert)).expect("einspielen");

    assert_eq!(herkunft, Kontaktherkunft::Programmordner);
    assert_eq!(namen(&mut s), ["Anna", "Bert", "Cora"]);
}

#[test]
fn passen_die_im_programmordner_nicht_kommen_die_gesicherten() {
    let datei = schluesseldatei();
    let verwaist = kontaktdatei(&schluesseldatei(), &["Mallory"]);
    let gesichert = kontaktdatei(&datei, &["Anna"]);

    let (mut s, herkunft) =
        einspielen(&datei, PASSWORT, Some(&verwaist), Some(&gesichert)).expect("einspielen");

    assert_eq!(herkunft, Kontaktherkunft::Sicherung);
    assert_eq!(namen(&mut s), ["Anna"]);
}

#[test]
fn die_eingespielten_kontakte_ueberstehen_das_sperren() {
    // Die Sitzung muss die genommene Kontaktdatei halten -- sonst oeffnete
    // das naechste Entsperren ein leeres Verzeichnis.
    let datei = schluesseldatei();
    let kontakte = kontaktdatei(&datei, &["Anna", "Bert"]);
    let (mut s, _) = einspielen(&datei, PASSWORT, None, Some(&kontakte)).expect("einspielen");

    s.sperren();
    s.entsperren(PASSWORT, 2_000, Speichergrenze::UNBEKANNT)
        .expect("wieder entsperren");

    assert_eq!(namen(&mut s), ["Anna", "Bert"]);
}

// ---------------------------------------------------------------------------
// Ableitung
// ---------------------------------------------------------------------------

/// Eine Datei, deren Kopf 2 GiB verlangt. Öffnen lässt sie sich nicht —
/// der Kopf ist Teil der Authentisierung —, aber das muss sie auch nicht:
/// Geprüft wird, was VOR dem Ableiten geschieht.
fn ungewoehnliche_datei() -> Vec<u8> {
    let mut datei = schluesseldatei();
    let zwei_gib: u32 = 2 * 1024 * 1024;
    datei
        .get_mut(3..7)
        .expect("Kopf")
        .copy_from_slice(&zwei_gib.to_be_bytes());
    datei
}

#[test]
fn ungewoehnliche_werte_brauchen_die_zulassung() {
    let datei = ungewoehnliche_datei();
    let grenze = Speichergrenze::bytes(64 * MIB);

    let bedarf = sicherung_pruefen(&datei, grenze).expect("Bedarf");
    assert!(bedarf.ungewoehnlich);
    assert!(!bedarf.passt);

    let versuch = |zugelassen: bool| {
        Sitzung::einspielen(
            Einspielung {
                schluesseldatei: &datei,
                passwort: PASSWORT,
                kontakte_hier: None,
                kontakte_gesichert: None,
                ungewoehnliche_zugelassen: zugelassen,
                frist: Sperrfrist::FuenfzehnMinuten,
            },
            2_000,
            grenze,
        )
    };

    let fehler = versuch(false).err().expect("ohne Zulassung");
    assert!(fehler.meldung.contains("bestätigen"), "{}", fehler.meldung);

    // Zugelassen, spricht die naechste Regel: 2 GiB passen nicht in
    // 64 MiB. Dass jetzt der Speicher spricht, zeigt, dass die Zulassung
    // gegriffen hat.
    let fehler = versuch(true).err().expect("passt nicht");
    assert!(
        fehler.meldung.contains("Arbeitsspeicher"),
        "{}",
        fehler.meldung
    );
}
