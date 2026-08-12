//! Prüft WebP, GIF und BMP an **echten**, von Pillow erzeugten Dateien.
//!
//! Die Modultests bauen die Formate von Hand — das prüft die Struktur, aber
//! nicht die Wirklichkeit. Erst eine echte Datei zeigt, wie ein Erzeuger die
//! Chunks tatsächlich anordnet.
//!
//! Der wichtigste Fund dieser Runde kam von hier: Die BMP-Erkennung verlangte,
//! dass die im Kopf angegebene Dateigröße genau der Länge entspricht — und
//! wies damit ausgerechnet die Datei mit Anhängsel ab, also genau den Fall,
//! für den das Modul gebaut wurde.
//!
//! Vorlagen erzeugen mit:
//!
//! ```text
//! python testvectors/tools/gen_metadata_fixtures.py
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use cabrik_metadata::{FindingKind, Severity, StripResult, inspect, strip};
use std::path::PathBuf;

fn lade(name: &str) -> Option<Vec<u8>> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testvectors/metadata")
        .join(name);
    std::fs::read(p).ok()
}

fn enthaelt(daten: &[u8], spur: &[u8]) -> bool {
    daten.windows(spur.len()).any(|f| f == spur)
}

/// WebP trägt seine Metadaten in RIFF-Chunks.
#[test]
fn ein_echtes_webp_wird_bereinigt() {
    let Some(daten) = lade("bild_mit_metadaten.webp") else {
        eprintln!("uebersprungen: gen_metadata_fixtures.py wurde nicht ausgefuehrt");
        return;
    };

    let vorher = inspect(&daten).unwrap();
    assert_eq!(vorher.format.as_deref(), Some("WebP"));
    assert!(
        vorher
            .findings
            .iter()
            .any(|f| f.severity == Severity::Critical),
        "der XMP-Block mit dem Namen fehlt: {:?}",
        vorher.findings
    );
    assert!(enthaelt(&daten, b"Dr. Anna Beispiel"), "Vorlage ohne Namen");

    let (sauber, ergebnis) = strip(&daten).unwrap();
    assert!(ergebnis.may_show_clean(), "{ergebnis:?}");

    for spur in [
        &b"Dr. Anna Beispiel"[..],
        b"Kamerahersteller",
        b"XY-2000",
        b"FAKE-ICC-PROFIL",
    ] {
        assert!(!enthaelt(&sauber, spur), "Spur blieb: {spur:?}");
    }

    // Es muss ein WebP bleiben.
    assert!(sauber.starts_with(b"RIFF"));
    assert_eq!(sauber.get(8..12), Some(&b"WEBP"[..]));
    assert!(inspect(&sauber).unwrap().findings.is_empty());
}

/// GIF trägt Kommentare in Erweiterungsblöcken.
#[test]
fn ein_echtes_gif_wird_bereinigt() {
    let Some(daten) = lade("bild_mit_metadaten.gif") else {
        return;
    };

    let vorher = inspect(&daten).unwrap();
    assert_eq!(vorher.format.as_deref(), Some("GIF"));
    let kommentar = vorher
        .findings
        .iter()
        .find(|f| f.kind == FindingKind::Comment)
        .expect("Kommentar nicht gefunden");
    assert!(
        kommentar
            .value
            .as_deref()
            .unwrap_or_default()
            .contains("XY-2000"),
        "{kommentar:?}"
    );

    let (sauber, ergebnis) = strip(&daten).unwrap();
    assert!(ergebnis.may_show_clean());
    assert!(!enthaelt(&sauber, b"XY-2000"));
    assert!(!enthaelt(&sauber, b"Anna Beispiel"));

    assert!(sauber.starts_with(b"GIF8"), "kein GIF mehr");
    assert_eq!(sauber.last(), Some(&0x3B), "Abschlussbyte fehlt");
    assert!(inspect(&sauber).unwrap().findings.is_empty());
}

/// **Der Fund dieser Runde.** Eine Datei mit Anhängsel ist länger als im Kopf
/// angegeben. Eine Erkennung, die auf Gleichheit prüft, übersieht genau das.
#[test]
fn ein_bmp_mit_anhaengsel_wird_erkannt_und_gekuerzt() {
    let Some(daten) = lade("bild_schlicht.bmp") else {
        return;
    };

    let vorher = inspect(&daten).unwrap();
    assert!(
        vorher.understood,
        "die Erkennung uebersieht die Datei, die sie finden soll"
    );
    assert_eq!(vorher.format.as_deref(), Some("BMP"));

    let fund = vorher
        .findings
        .iter()
        .find(|f| f.location == "BMP:Anhängsel")
        .expect("das Anhaengsel wurde nicht gefunden");
    assert!(
        fund.value.as_deref().unwrap_or_default().contains("26"),
        "{fund:?}"
    );

    let (sauber, ergebnis) = strip(&daten).unwrap();
    assert!(
        !enthaelt(&sauber, b"HEIMLICHE-NUTZLAST"),
        "Anhaengsel blieb"
    );
    assert!(sauber.len() < daten.len());
    assert!(sauber.starts_with(b"BM"));

    match ergebnis {
        StripResult::Complete { removed } => assert_eq!(removed.len(), 1),
        other => panic!("erwartete Complete, bekam {other:?}"),
    }
}

/// Zweimal bereinigen muss zweimal dasselbe ergeben — für jedes Format.
#[test]
fn die_bereinigung_ist_bei_allen_formaten_wiederholbar() {
    for name in [
        "bild_mit_metadaten.webp",
        "bild_mit_metadaten.gif",
        "bild_schlicht.bmp",
    ] {
        let Some(daten) = lade(name) else { continue };
        let einmal = strip(&daten).unwrap().0;
        let zweimal = strip(&einmal).unwrap().0;
        assert_eq!(einmal, zweimal, "{name} ist nicht stabil");
    }
}
