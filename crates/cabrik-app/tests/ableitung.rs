//! Speichergrenze und Rückfrage beim Entsperren (`spec/entsperrung.md`
//! §4.5, `spec/keyfile-v2.md` §4.2).
//!
//! # Was hier geprüft wird — und was bewusst nicht
//!
//! Beide Regeln greifen **vor** der Ableitung: Die Rückfrage, weil niemand
//! eine fremde Datei minutenlang rechnen lassen soll, ohne gefragt zu sein;
//! die Speichergrenze, weil sonst angelegt würde, was nicht hineinpasst.
//!
//! Deshalb braucht keiner dieser Tests eine teure Ableitung. Die
//! „ungewöhnliche“ Schlüsseldatei entsteht, indem im Klartextkopf einer
//! sparsamen die Werte hochgesetzt werden. Ableiten ließe sie sich danach
//! nicht mehr — der Kopf ist Teil der AAD —, aber dahin kommt es in diesen
//! Tests auch nie. Genau das ist der Punkt.

#![expect(clippy::expect_used, reason = "Fehlschlag soll den Test abbrechen")]

use cabrik_app::Sitzung;
use cabrik_bruecke::{KdfStufe, Sperrfrist};
use cabrik_core::Speichergrenze;
use cabrik_core::keyfile::{self, KdfStufe as KernStufe};
use cabrik_core::{Identity, OsRandom};

const PASSWORT: &str = "vier zufaellige woerter hier";
const MIB: u64 = 1 << 20;

/// Eine Schlüsseldatei der Stufe „Min“: 64 MiB, drei Durchgänge.
fn sparsame_datei() -> Vec<u8> {
    let id = Identity::generate(&mut OsRandom, true, 1_700_000_000).expect("Identität");
    keyfile::write(
        &id,
        PASSWORT.as_bytes(),
        &KernStufe::Min.params(),
        Speichergrenze::UNBEKANNT,
        &mut OsRandom,
    )
    .expect("schreiben")
}

/// Dieselbe Datei, aber mit 2 GiB im Kopf — mehr, als Cabrik je schreibt.
///
/// `m_cost` steht ab Byte 3, vier Bytes, groß-endian (`keyfile-v2.md` §2).
fn ungewoehnliche_datei() -> Vec<u8> {
    let mut datei = sparsame_datei();
    let zwei_gib: u32 = 2 * 1024 * 1024;
    datei
        .get_mut(3..7)
        .expect("Kopf")
        .copy_from_slice(&zwei_gib.to_be_bytes());
    datei
}

fn gesperrt(datei: Vec<u8>) -> Sitzung {
    Sitzung::neu(datei, None, Sperrfrist::FuenfzehnMinuten)
}

#[test]
fn der_bedarf_steht_fest_bevor_jemand_tippt() {
    let s = gesperrt(sparsame_datei());
    let bedarf = s
        .entsperrbedarf(Speichergrenze::bytes(1024 * MIB))
        .expect("Bedarf");

    assert_eq!(bedarf.speicher_bytes, 64 * MIB);
    assert_eq!(bedarf.durchgaenge, 3);
    assert!(
        !bedarf.ungewoehnlich,
        "die eigene Stufe Min ist nicht ungewoehnlich"
    );
    assert_eq!(bedarf.grenze_bytes, Some(1024 * MIB));
    assert!(bedarf.passt);
}

/// Unbekannte Grenze heißt nicht „passt nicht“: Abgelehnt wird dann erst,
/// wenn das System das Anlegen verweigert.
#[test]
fn ohne_bekannte_grenze_passt_es() {
    let s = gesperrt(sparsame_datei());
    let bedarf = s.entsperrbedarf(Speichergrenze::UNBEKANNT).expect("Bedarf");
    assert_eq!(bedarf.grenze_bytes, None);
    assert!(bedarf.passt);
}

/// Der Fall, für den es die ganze Unterscheidung gibt: Wer zu wenig
/// Speicher hat, liest das — und nicht, sein Passwort sei falsch.
#[test]
fn zu_wenig_speicher_ist_kein_falsches_passwort() {
    let mut s = gesperrt(sparsame_datei());
    let eng = Speichergrenze::bytes(MIB);

    assert!(!s.entsperrbedarf(eng).expect("Bedarf").passt);

    let fehler = s
        .entsperren(PASSWORT.as_bytes(), 1_000, eng)
        .expect_err("darf nicht aufgehen");
    assert!(
        !fehler.meldung.contains("Passwort passt nicht"),
        "irrefuehrend: {}",
        fehler.meldung
    );
    assert!(
        fehler.meldung.contains("Arbeitsspeicher"),
        "{}",
        fehler.meldung
    );
    assert!(s.ist_gesperrt());

    // Dieselbe Datei geht auf, sobald genug da ist.
    s.entsperren(PASSWORT.as_bytes(), 1_000, Speichergrenze::bytes(64 * MIB))
        .expect("mit genug Speicher");
}

#[test]
fn ungewoehnliche_werte_brauchen_eine_zulassung() {
    let mut s = gesperrt(ungewoehnliche_datei());
    let grenze = Speichergrenze::bytes(64 * MIB);

    let bedarf = s.entsperrbedarf(grenze).expect("Bedarf");
    assert!(bedarf.ungewoehnlich);
    assert!(!bedarf.zugelassen);
    assert_eq!(bedarf.speicher_bytes, 2048 * MIB);

    // Ohne Zulassung wird gar nicht erst abgeleitet -- auch nicht über den
    // Weg des eigenen Passwortfensters, der vorher fragt.
    let fehler = s
        .entsperren(PASSWORT.as_bytes(), 1_000, grenze)
        .expect_err("ohne Zulassung");
    assert!(fehler.meldung.contains("bestätigen"), "{}", fehler.meldung);
    assert!(s.entsperren_moeglich(grenze).is_err());

    // Zugelassen, kommt die nächste Regel zum Zug: 2 GiB passen nicht in
    // 64 MiB. Dass jetzt der Speicher spricht und nicht die Rückfrage,
    // zeigt, dass die Zulassung gegriffen hat.
    s.ungewoehnliche_ableitung_zulassen();
    assert!(s.entsperrbedarf(grenze).expect("Bedarf").zugelassen);
    let fehler = s
        .entsperren(PASSWORT.as_bytes(), 1_000, grenze)
        .expect_err("passt nicht");
    assert!(
        fehler.meldung.contains("Arbeitsspeicher"),
        "{}",
        fehler.meldung
    );
}

/// Die Zulassung übersteht das Sperren — eine Rückfrage bei jedem
/// Entsperren würde weggeklickt, ohne gelesen zu werden.
#[test]
fn die_zulassung_gilt_fuer_die_sitzung() {
    let mut s = gesperrt(ungewoehnliche_datei());
    s.ungewoehnliche_ableitung_zulassen();
    s.sperren();
    assert!(
        s.entsperrbedarf(Speichergrenze::UNBEKANNT)
            .expect("Bedarf")
            .zugelassen
    );
}

/// … aber nicht darüber hinaus: Eine neue Sitzung fragt wieder.
#[test]
fn eine_neue_sitzung_fragt_wieder() {
    let datei = ungewoehnliche_datei();
    let mut erste = gesperrt(datei.clone());
    erste.ungewoehnliche_ableitung_zulassen();

    let zweite = gesperrt(datei);
    assert!(
        !zweite
            .entsperrbedarf(Speichergrenze::UNBEKANNT)
            .expect("Bedarf")
            .zugelassen
    );
}

/// Wer „Stark“ wählt und nicht genug frei hat, erfährt das beim Anlegen —
/// bevor ein Schlüssel erzeugt ist, nicht nach einer Ableitung, die
/// auslagert.
#[test]
fn anlegen_mit_zu_wenig_speicher_sagt_es() {
    let Err(fehler) = Sitzung::anlegen(
        None,
        PASSWORT.as_bytes(),
        true,
        KdfStufe::Stark,
        Sperrfrist::FuenfzehnMinuten,
        1_000,
        Speichergrenze::bytes(512 * MIB),
        &mut OsRandom,
    ) else {
        unreachable!("darf nicht angelegt werden");
    };
    assert!(
        fehler.meldung.contains("Arbeitsspeicher"),
        "{}",
        fehler.meldung
    );
}

#[test]
fn passwort_aendern_haelt_die_grenze() {
    let datei = sparsame_datei();
    let mut s = gesperrt(datei.clone());
    s.entsperren(PASSWORT.as_bytes(), 1_000, Speichergrenze::UNBEKANNT)
        .expect("entsperren");

    let fehler = s
        .passwort_aendern(
            PASSWORT.as_bytes(),
            b"ganz andere vier woerter",
            Speichergrenze::bytes(MIB),
            &mut OsRandom,
        )
        .expect_err("passt nicht");
    assert!(
        fehler.meldung.contains("Arbeitsspeicher"),
        "{}",
        fehler.meldung
    );
    // Und die Datei blieb, wie sie war.
    assert_eq!(s.schluesseldatei(), datei.as_slice());
}
