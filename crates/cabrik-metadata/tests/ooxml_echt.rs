//! Prüft die OOXML-Bereinigung an **echten** Office-Dateien.
//!
//! Die Modultests in `ooxml.rs` bauen ein Dokument aus einzelnen XML-Teilen.
//! Das prüft die Logik, nicht die Wirklichkeit. Word und LibreOffice schreiben
//! Teile, an die niemand denkt — `docProps/thumbnail.jpeg`, `customXml/` mit
//! einer festen GUID, neun `rsid`-Werte allein aus der Vorlage. Alle drei kamen
//! erst heraus, als das erste echte Dokument durch den Prüfer lief.
//!
//! Die Vorlagen entstehen mit:
//!
//! ```text
//! python testvectors/tools/gen_ooxml_fixtures.py
//! ```
//!
//! Fehlen sie, wird der Test **übersprungen** statt fälschlich zu bestehen.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use cabrik_metadata::container;
use cabrik_metadata::{FindingKind, Severity, StripResult, inspect, strip};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testvectors/metadata")
}

/// Lädt eine Vorlage, oder `None`, wenn sie fehlt.
fn lade(name: &str) -> Option<Vec<u8>> {
    std::fs::read(dir().join(name)).ok()
}

/// Legt das Ergebnis für eine unabhängige Prüfung von außen ab.
fn schreibe_ergebnis(name: &str, daten: &[u8]) {
    let _ = std::fs::write(dir().join(name), daten);
}

/// Sucht in allen Teilen eines Containers nach einer Byte-Folge.
///
/// Der schärfste verfügbare Test: Es genügt nicht, dass ein Feld leer
/// **gemeldet** wird — der Wert darf in der ganzen Datei nicht mehr
/// vorkommen, in keinem Teil, auch nicht in einem übersehenen.
fn steckt_noch_drin(daten: &[u8], spur: &[u8]) -> bool {
    let Ok(eintraege) = container::lies(daten) else {
        return daten.windows(spur.len()).any(|f| f == spur);
    };
    eintraege
        .iter()
        .any(|e| e.inhalt.windows(spur.len()).any(|f| f == spur))
}

const SPUREN: [&str; 7] = [
    "Dr. Anna Beispiel",
    "Prof. Carl Chef",
    "Nicht an den Kunden geben",
    "Angebot Projekt Nordstern",
    "Normal.dotm",
    "Macintosh",
    "EF278816",
];

#[test]
fn ein_echtes_worddokument_wird_vollstaendig_bereinigt() {
    let Some(daten) = lade("dokument_mit_metadaten.docx") else {
        eprintln!("uebersprungen: gen_ooxml_fixtures.py wurde nicht ausgefuehrt");
        return;
    };

    // --- vorher ---------------------------------------------------------
    let vorher = inspect(&daten).unwrap();
    assert!(vorher.understood, "OOXML wurde nicht erkannt");
    assert!(
        vorher.findings.len() >= 15,
        "in einem echten Word-Dokument steckt mehr: {} Funde",
        vorher.findings.len()
    );

    for spur in SPUREN {
        assert!(
            steckt_noch_drin(&daten, spur.as_bytes()),
            "die Vorlage enthaelt „{spur}\" gar nicht — Test waere wertlos"
        );
    }

    // --- bereinigen ------------------------------------------------------
    let (sauber, ergebnis) = strip(&daten).unwrap();
    schreibe_ergebnis("dokument_mit_metadaten.stripped.docx", &sauber);

    assert!(
        ergebnis.may_show_clean(),
        "ohne Kommentare und Aenderungen ist Complete gerechtfertigt: {ergebnis:?}"
    );

    // --- nachher ---------------------------------------------------------
    for spur in SPUREN {
        assert!(
            !steckt_noch_drin(&sauber, spur.as_bytes()),
            "„{spur}\" steht weiterhin in der Datei"
        );
    }
}

/// Das Vorschaubild ist eine **zweite Kopie des Dokumentinhalts**. Word legt
/// es unaufgefordert an; kaum jemand weiß davon.
#[test]
fn das_vorschaubild_wird_erkannt_und_entfernt() {
    let Some(daten) = lade("dokument_mit_metadaten.docx") else {
        return;
    };

    let vorher = inspect(&daten).unwrap();
    let fund = vorher
        .findings
        .iter()
        .find(|f| f.kind == FindingKind::EmbeddedPreview && f.location.contains("thumbnail"))
        .expect("Vorschaubild nicht erkannt");
    assert_eq!(fund.severity, Severity::Critical);

    let (sauber, _) = strip(&daten).unwrap();
    let eintraege = container::lies(&sauber).unwrap();
    assert!(
        !eintraege.iter().any(|e| e.name.contains("thumbnail")),
        "Vorschaubild blieb im Archiv"
    );
}

/// `customXml/itemProps*.xml` trägt eine feste GUID. Sie ist in **jedem** aus
/// derselben Vorlage erzeugten Dokument gleich und verknüpft sie damit über
/// Empfänger hinweg — auch wenn sonst alles bereinigt wurde.
#[test]
fn die_feste_kennung_im_angehaengten_xml_verschwindet() {
    let Some(daten) = lade("dokument_mit_metadaten.docx") else {
        return;
    };

    let vorher = inspect(&daten).unwrap();
    let fund = vorher
        .findings
        .iter()
        .find(|f| f.location.contains("customXml") && f.severity == Severity::Critical)
        .expect("feste Kennung nicht erkannt");
    assert!(
        fund.value.as_deref().unwrap_or_default().contains('{'),
        "die GUID gehoert in die Meldung: {fund:?}"
    );

    let (sauber, _) = strip(&daten).unwrap();
    let eintraege = container::lies(&sauber).unwrap();
    assert!(
        !eintraege.iter().any(|e| e.name.starts_with("customXml/")),
        "angehaengtes XML blieb im Archiv"
    );
}

/// Nach dem Entfernen eines Teils darf keine Beziehung mehr auf ihn zeigen —
/// sonst beantwortet Word das Öffnen mit einer Reparaturabfrage.
#[test]
fn keine_beziehung_zeigt_ins_leere() {
    let Some(daten) = lade("dokument_mit_metadaten.docx") else {
        return;
    };
    let (sauber, _) = strip(&daten).unwrap();
    let eintraege = container::lies(&sauber).unwrap();

    let vorhanden: Vec<&str> = eintraege.iter().map(|e| e.name.as_str()).collect();

    for e in &eintraege {
        if !e.name.ends_with(".rels") {
            continue;
        }
        let Some(text) = e.text() else { continue };
        for ziel in cabrik_metadata::xml::attribut_werte(text, "Relationship", "Target") {
            // Externe Ziele (http://…) und übergeordnete Pfade lassen sich
            // hier nicht prüfen; nur interne, direkt benannte.
            if ziel.starts_with("http") || ziel.contains("..") {
                continue;
            }
            let voll = if e.name == "_rels/.rels" {
                ziel.clone()
            } else {
                // word/_rels/document.xml.rels → word/<ziel>
                let stamm = e.name.split("/_rels/").next().unwrap_or("");
                format!("{stamm}/{ziel}")
            };
            assert!(
                vorhanden.contains(&voll.as_str()),
                "{} verweist auf {voll}, das es nicht mehr gibt",
                e.name
            );
        }
    }
}

/// Ein Bild im Dokument bringt **seine eigenen** Metadaten mit. v1 kannte
/// diesen Fall gar nicht: Wer ein Urlaubsfoto in einen Bericht einfügt,
/// verschickt dessen GPS-Koordinaten mit.
#[test]
fn metadaten_eingebetteter_bilder_werden_mitentfernt() {
    let Some(daten) = lade("dokument_mit_bild.docx") else {
        eprintln!("uebersprungen: dokument_mit_bild.docx fehlt");
        return;
    };

    let vorher = inspect(&daten).unwrap();
    let gps = vorher
        .findings
        .iter()
        .find(|f| f.kind == FindingKind::Gps)
        .expect("GPS im eingebetteten Bild nicht gefunden");
    assert!(
        gps.location.contains("media/"),
        "der Pfad muss zum Bild fuehren: {}",
        gps.location
    );

    let (sauber, _) = strip(&daten).unwrap();
    schreibe_ergebnis("dokument_mit_bild.stripped.docx", &sauber);

    let nachher = inspect(&sauber).unwrap();
    assert!(
        !nachher.findings.iter().any(|f| f.kind == FindingKind::Gps),
        "GPS blieb im eingebetteten Bild: {:?}",
        nachher.findings
    );

    // Das Bild muss ein Bild bleiben.
    let eintraege = container::lies(&sauber).unwrap();
    let bild = eintraege
        .iter()
        .find(|e| e.name.starts_with("word/media/"))
        .expect("das Bild wurde entfernt statt bereinigt");
    assert_eq!(
        bild.inhalt.get(..2),
        Some(&[0xFF, 0xD8][..]),
        "kein JPEG mehr"
    );
}

/// Eine Tabelle ist derselbe Behälter mit anderem Inhalt.
#[test]
fn auch_eine_tabelle_wird_bereinigt() {
    let Some(daten) = lade("tabelle_mit_metadaten.xlsx") else {
        return;
    };

    let vorher = inspect(&daten).unwrap();
    assert_eq!(vorher.format.as_deref(), Some("OOXML (Excel)"));
    assert!(!vorher.findings.is_empty());

    let (sauber, _) = strip(&daten).unwrap();
    schreibe_ergebnis("tabelle_mit_metadaten.stripped.xlsx", &sauber);

    for spur in ["Dr. Anna Beispiel", "Prof. Carl Chef", "nicht weitergeben"] {
        assert!(
            !steckt_noch_drin(&sauber, spur.as_bytes()),
            "„{spur}\" blieb in der Tabelle"
        );
    }

    // Der Zellinhalt muss erhalten bleiben.
    assert!(
        steckt_noch_drin(&sauber, b"Kalkulation") || steckt_noch_drin(&sauber, b"Entwicklung"),
        "der Tabelleninhalt ging verloren"
    );
}

/// Zweimal bereinigen muss zweimal dieselben Bytes ergeben — sonst verriete
/// schon der Unterschied, wann bereinigt wurde (`spec/metadata.md` §5).
#[test]
fn die_bereinigung_ist_bit_genau_wiederholbar() {
    let Some(daten) = lade("dokument_mit_metadaten.docx") else {
        return;
    };
    let a = strip(&daten).unwrap().0;
    let b = strip(&daten).unwrap().0;
    assert_eq!(a, b, "die Ausgabe ist nicht reproduzierbar");
}

/// Eine zweite Bereinigung des Ergebnisses darf nichts mehr finden.
#[test]
fn nach_der_bereinigung_ist_nichts_mehr_zu_holen() {
    let Some(daten) = lade("dokument_mit_metadaten.docx") else {
        return;
    };
    let (einmal, _) = strip(&daten).unwrap();
    let (zweimal, ergebnis) = strip(&einmal).unwrap();

    assert_eq!(einmal, zweimal, "der zweite Durchlauf aenderte noch etwas");
    match ergebnis {
        StripResult::Complete { removed } => {
            assert!(
                removed.is_empty(),
                "beim zweiten Mal wurde noch etwas gefunden: {removed:?}"
            );
        }
        other => panic!("erwartete Complete, bekam {other:?}"),
    }
}
