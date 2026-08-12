//! Prüft Ton und Bewegtbild an **echten**, von ffmpeg erzeugten Dateien.
//!
//! Diese Tests prüfen die Bytestruktur. Was sie **nicht** können, ist die
//! Frage beantworten, die am Ende zählt: *Lässt sich die Datei danach noch
//! abspielen?* Dafür legen sie ihre Ergebnisse als `*.stripped` ab, und
//! `testvectors/tools/verify_medien_stripped.py` öffnet sie mit demselben
//! ffmpeg, das sie erzeugt hat.
//!
//! Warum das nötig ist, zeigte Ogg: Ein erster Entwurf packte alle
//! Kopfpakete in eine Seite. Die Struktur war einwandfrei, ffmpeg spielte die
//! Datei ab — und mutagen las die Tondaten als Kommentar, weil die Vorbis-Norm
//! das Identifikationspaket **allein** auf der ersten Seite verlangt.
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

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testvectors/metadata")
}

fn lade(name: &str) -> Option<Vec<u8>> {
    std::fs::read(dir().join(name)).ok()
}

fn enthaelt(daten: &[u8], spur: &[u8]) -> bool {
    daten.windows(spur.len()).any(|f| f == spur)
}

/// Legt das Ergebnis für die unabhängige Prüfung mit ffmpeg ab.
fn schreibe_ergebnis(name: &str, daten: &[u8]) {
    let p = dir().join(format!("{name}.stripped"));
    std::fs::write(&p, daten).unwrap_or_else(|e| panic!("{} nicht schreibbar: {e}", p.display()));
}

/// Die Spuren, die in allen Vorlagen stecken.
const SPUREN: [&[u8]; 4] = [
    b"Dr. Anna Beispiel",
    b"Nicht an den Kunden geben",
    b"Angebot Nordstern",
    b"Interner Rohschnitt",
];

fn keine_spuren(daten: &[u8], name: &str) {
    for spur in SPUREN {
        assert!(
            !enthaelt(daten, spur),
            "{name}: noch lesbar — {}",
            String::from_utf8_lossy(spur)
        );
    }
}

// ---------------------------------------------------------------------------
// MP3
// ---------------------------------------------------------------------------

/// **Das einzige Format, das wirklich gekürzt wird.** Ein MP3 führt keine
/// Tabelle mit Byte-Positionen, also darf der Tonstrom nach vorn rücken.
#[test]
fn ein_echtes_mp3_verliert_alle_marken() {
    let Some(daten) = lade("ton_mit_marken.mp3") else {
        eprintln!("uebersprungen: gen_metadata_fixtures.py wurde nicht ausgefuehrt");
        return;
    };

    let vorher = inspect(&daten).unwrap();
    assert_eq!(vorher.format.as_deref(), Some("MP3"));

    // Die drei, die niemand erwartet: eingebettetes Bild, Händlerkennung und
    // der alte ID3v1-Tag am Dateiende.
    for (ort, art) in [
        ("MP3:ID3v2/APIC", FindingKind::EmbeddedPreview),
        ("MP3:ID3v2/PRIV", FindingKind::Device),
    ] {
        let f = vorher
            .findings
            .iter()
            .find(|f| f.location == ort)
            .unwrap_or_else(|| panic!("{ort} fehlt: {:?}", vorher.findings));
        assert_eq!(f.kind, art);
        assert_eq!(f.severity, Severity::Critical);
    }
    assert!(
        vorher.findings.iter().any(|f| f.location == "MP3:ID3v1"),
        "der ID3v1-Tag am Dateiende wurde uebersehen"
    );

    let (sauber, ergebnis) = strip(&daten).unwrap();
    assert!(sauber.len() < daten.len(), "es wurde nichts abgetragen");

    keine_spuren(&sauber, "mp3");
    assert!(!enthaelt(&sauber, b"Kunde-4711"), "die Kennung blieb");
    assert!(
        !enthaelt(&sauber, b"Lavf"),
        "der Kodierername im Xing-Kopf blieb stehen"
    );

    // **Die ehrliche Grenze dieses Formats.** LAME schreibt seinen Namen in
    // die Zusatzdaten der Tonrahmen, also in den Tondatenstrom selbst. Ihn zu
    // entfernen hieße, den Ton neu zu berechnen — dann wäre es nicht mehr
    // dieselbe Aufnahme. Deshalb bleibt er, und deshalb ist das Ergebnis
    // `Partial` statt `Complete`.
    let StripResult::Partial { remaining, .. } = &ergebnis else {
        panic!("erwartet wurde Partial wegen der Zusatzdaten, bekam {ergebnis:?}");
    };
    assert!(
        remaining.iter().any(|f| f.location == "MP3:Tonrahmen"),
        "die verbliebene Spur wurde nicht benannt: {remaining:?}"
    );
    assert!(
        enthaelt(&sauber, b"LAME3.100"),
        "die Tondaten wurden angetastet"
    );

    // Bis auf das genullte Namensfeld im Xing-Kopf ist der Tonstrom Byte für
    // Byte derselbe.
    let versatz = daten.len() - sauber.len() - 128; // ID3v2 vorne, ID3v1 hinten
    let vorher = &daten[versatz..versatz + sauber.len()];
    let abweichungen: Vec<usize> = (0..sauber.len())
        .filter(|i| vorher[*i] != sauber[*i])
        .collect();
    assert!(
        abweichungen.len() <= 9,
        "es wurden {} Bytes im Tonstrom geändert, erlaubt sind neun",
        abweichungen.len()
    );

    schreibe_ergebnis("ton_mit_marken.mp3", &sauber);
}

// ---------------------------------------------------------------------------
// FLAC
// ---------------------------------------------------------------------------

/// FLAC bringt seinen Platzhalter unter dem Namen `PADDING` mit — die Länge
/// bleibt deshalb auf das Byte gleich.
#[test]
fn ein_echtes_flac_wird_zu_padding() {
    let Some(daten) = lade("ton_mit_marken.flac") else {
        return;
    };

    let vorher = inspect(&daten).unwrap();
    assert_eq!(vorher.format.as_deref(), Some("FLAC"));
    assert!(
        vorher
            .findings
            .iter()
            .any(|f| f.kind == FindingKind::Author && f.severity == Severity::Critical),
        "der Verfasser fehlt: {:?}",
        vorher.findings
    );

    let (sauber, _) = strip(&daten).unwrap();
    assert_eq!(sauber.len(), daten.len(), "die Länge hat sich geändert");
    keine_spuren(&sauber, "flac");
    assert!(!enthaelt(&sauber, b"Lavf"));

    // Die MD5-Summe der Tonspur steht im STREAMINFO und beschreibt den
    // Inhalt, nicht die Person. Sie muss unverändert bleiben.
    assert_eq!(
        &sauber[4..42],
        &daten[4..42],
        "der STREAMINFO-Block wurde angetastet"
    );
    assert!(inspect(&sauber).unwrap().findings.is_empty());

    schreibe_ergebnis("ton_mit_marken.flac", &sauber);
}

// ---------------------------------------------------------------------------
// Ogg und Opus
// ---------------------------------------------------------------------------

/// **Der einzige Fall, in dem gerechnet werden muss.** Jede Ogg-Seite trägt
/// eine Prüfsumme über sich selbst.
#[test]
fn ein_echtes_ogg_wird_neu_geschrieben() {
    let Some(daten) = lade("ton_mit_marken.ogg") else {
        return;
    };

    let vorher = inspect(&daten).unwrap();
    assert_eq!(vorher.format.as_deref(), Some("Ogg Vorbis"));
    assert!(
        vorher
            .findings
            .iter()
            .any(|f| f.location == "Ogg:Kommentar/artist"),
        "{:?}",
        vorher.findings
    );

    let (sauber, ergebnis) = strip(&daten).unwrap();
    assert!(matches!(ergebnis, StripResult::Complete { .. }));
    keine_spuren(&sauber, "ogg");
    assert!(inspect(&sauber).unwrap().findings.is_empty());

    schreibe_ergebnis("ton_mit_marken.ogg", &sauber);
}

/// Opus ist derselbe Behälter — **ohne** das Rahmenbit, das Vorbis verlangt.
#[test]
fn ein_echtes_opus_wird_neu_geschrieben() {
    let Some(daten) = lade("ton_mit_marken.opus") else {
        return;
    };

    let vorher = inspect(&daten).unwrap();
    assert_eq!(vorher.format.as_deref(), Some("Opus"));

    let (sauber, _) = strip(&daten).unwrap();
    keine_spuren(&sauber, "opus");
    assert!(inspect(&sauber).unwrap().findings.is_empty());

    schreibe_ergebnis("ton_mit_marken.opus", &sauber);
}

// ---------------------------------------------------------------------------
// WAV
// ---------------------------------------------------------------------------

/// **Der Fund, der WAV zum interessantesten Tonformat macht.** Eine Datei aus
/// dem Schnittprogramm ist nackt — eine aus dem Feldrekorder nicht.
#[test]
fn ein_echtes_wav_verliert_seinen_bext_block() {
    let Some(daten) = lade("ton_mit_marken.wav") else {
        return;
    };

    let vorher = inspect(&daten).unwrap();
    assert_eq!(vorher.format.as_deref(), Some("WAV"));

    let hole = |ort: &str| {
        vorher
            .findings
            .iter()
            .find(|f| f.location == ort)
            .unwrap_or_else(|| panic!("{ort} fehlt: {:?}", vorher.findings))
    };
    assert_eq!(
        hole("WAV:bext/Aufnehmender").value.as_deref(),
        Some("Dr. Anna Beispiel")
    );
    assert_eq!(
        hole("WAV:bext/Gerätekennung").value.as_deref(),
        Some("ZOOM-F8N-00473829")
    );
    assert_eq!(
        hole("WAV:bext/Aufnahmezeit").value.as_deref(),
        Some("09:12:00"),
        "die Uhrzeit der Aufnahme ist der heikelste Teil"
    );
    assert_eq!(hole("WAV:bext/UMID").severity, Severity::Critical);

    let (sauber, _) = strip(&daten).unwrap();
    assert_eq!(sauber.len(), daten.len());
    keine_spuren(&sauber, "wav");
    for spur in [
        &b"ZOOM-F8N-00473829"[..],
        b"anonym bleiben",
        b"2026-03-01",
        b"09:12:00",
    ] {
        assert!(!enthaelt(&sauber, spur), "noch lesbar: {spur:?}");
    }
    assert!(inspect(&sauber).unwrap().findings.is_empty());

    schreibe_ergebnis("ton_mit_marken.wav", &sauber);
}

// ---------------------------------------------------------------------------
// Bewegtbild — dieselben Vorlagen, damit ffmpeg auch sie nachprüft
// ---------------------------------------------------------------------------

/// **Ein iPhone benutzt die iTunes-Marken nicht.** Es legt seine Angaben in
/// Apples Schlüsselverzeichnis ab, und ein Leser, der auf `©`-Codes prüft,
/// sieht dort gar nichts. Entfernt wurde bisher trotzdem alles — gemeldet
/// wurde aber nur „614 Bytes Benutzerdaten", also gerade nicht der wichtigste
/// Fund des Moduls.
#[test]
fn ein_echtes_live_photo_verraet_ort_und_kennzeichner() {
    let Some(daten) = lade("live_photo.mov") else {
        eprintln!("uebersprungen: gen_metadata_fixtures.py wurde nicht ausgefuehrt");
        return;
    };

    let vorher = inspect(&daten).unwrap();
    assert_eq!(vorher.format.as_deref(), Some("QuickTime (MOV)"));

    let ort = vorher
        .findings
        .iter()
        .find(|f| f.kind == FindingKind::Gps)
        .expect("der Aufnahmeort wurde nicht gefunden");
    assert_eq!(ort.severity, Severity::Critical);
    assert!(ort.value.as_deref().unwrap().contains("+46.9481"));

    // Der Kennzeichner steht in BEIDEN Hälften eines Live Photo und
    // verknüpft sie. Wer nur eine bereinigt, lässt die Verbindung bestehen.
    let kennung = vorher
        .findings
        .iter()
        .find(|f| f.location.ends_with("content.identifier"))
        .expect("der Kennzeichner des Live Photo fehlt");
    assert_eq!(kennung.severity, Severity::Critical);
    assert!(kennung.value.as_deref().unwrap().contains("Live Photo"));

    let (sauber, _) = strip(&daten).unwrap();
    assert_eq!(sauber.len(), daten.len(), "es hat sich etwas verschoben");
    for spur in [
        &b"+46.9481"[..],
        b"8F3B1C2A",
        b"iPhone 15 Pro",
        b"com.apple.quicktime",
    ] {
        assert!(!enthaelt(&sauber, spur), "noch lesbar: {spur:?}");
    }
    assert!(inspect(&sauber).unwrap().findings.is_empty());

    schreibe_ergebnis("live_photo.mov", &sauber);
}

#[test]
fn die_bewegtbildvorlagen_werden_fuer_die_ffmpeg_pruefung_abgelegt() {
    for name in [
        "video_mit_ortsangabe.mp4",
        "video_mit_marken.mkv",
        "video_mit_marken.webm",
        "video_mit_marken.avi",
    ] {
        let Some(daten) = lade(name) else {
            continue;
        };
        let (sauber, _) = strip(&daten).unwrap();
        assert_eq!(
            sauber.len(),
            daten.len(),
            "{name}: es hat sich etwas verschoben"
        );
        schreibe_ergebnis(name, &sauber);
    }
}

// ---------------------------------------------------------------------------
// Formatübergreifend
// ---------------------------------------------------------------------------

/// Ein zweiter Durchlauf darf nichts mehr verändern — und außer bei MP3 darf
/// danach kein einziger Fund übrig sein.
///
/// MP3 ist die Ausnahme mit Grund: Der Kodierername steckt dort in den
/// Tonrahmen selbst. Diesen Unterschied festzuhalten ist der Sinn des Tests —
/// eine Ausnahme, die niemand aufschreibt, wird irgendwann zur Regel.
#[test]
fn alle_tonformate_sind_nach_dem_ersten_durchlauf_fertig() {
    for (name, bleibt_etwas) in [
        ("ton_mit_marken.mp3", true),
        ("ton_mit_marken.flac", false),
        ("ton_mit_marken.ogg", false),
        ("ton_mit_marken.opus", false),
        ("ton_mit_marken.wav", false),
    ] {
        let Some(daten) = lade(name) else {
            continue;
        };
        let (einmal, _) = strip(&daten).unwrap();
        let (zweimal, ergebnis) = strip(&einmal).unwrap();

        assert_eq!(
            einmal, zweimal,
            "{name}: der zweite Durchlauf änderte etwas"
        );

        let uebrig = inspect(&einmal).unwrap().findings;
        if bleibt_etwas {
            assert!(
                !ergebnis.may_show_clean(),
                "{name}: es darf keine Sauberkeit behauptet werden"
            );
            assert!(
                uebrig.iter().all(|f| f.location == "MP3:Tonrahmen"),
                "{name}: es blieb mehr übrig als erwartet: {uebrig:?}"
            );
        } else {
            assert!(ergebnis.may_show_clean(), "{name}: {ergebnis:?}");
            assert!(
                uebrig.is_empty(),
                "{name}: es blieb etwas übrig: {uebrig:?}"
            );
        }
    }
}
