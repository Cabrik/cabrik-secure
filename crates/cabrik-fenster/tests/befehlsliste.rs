//! Welche Befehle das Fenster anbietet — als Prüfmuster.
//!
//! # Warum das nötig ist
//!
//! Es ist die einzige Stelle im ganzen Aufbau, an der etwas stumm
//! auseinanderlaufen kann. Überall sonst passt ein Übersetzer auf: Die
//! Typen sind beidseitig festgenagelt, die Muster vergleichen Rust gegen
//! TypeScript, und die Schnittstelle erzwingt, dass beide Umsetzungen
//! dasselbe können.
//!
//! Die **Namen der Befehle** sind davon ausgenommen. Sie stehen hier als
//! Rust-Funktionen und drüben als Zeichenketten. Wer eine Funktion
//! umbenennt, merkt nichts — bis zur Laufzeit im Fenster, wo die Meldung
//! „command not found“ lautet und nichts darüber sagt, welche Seite recht
//! hat.
//!
//! Deshalb schreibt dieser Test die Liste dorthin, wo das Frontend sie
//! lesen kann, und **vergleicht** sie mit der eingecheckten Fassung.
//!
//! Neu erzeugen mit:
//!
//! ```text
//! MUSTER_SCHREIBEN=1 cargo test -p cabrik-fenster
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "Fehlschlag soll den Test abbrechen"
)]

use std::path::PathBuf;

/// Der eigene Quelltext — zur Bauzeit eingebettet.
const QUELLE: &str = include_str!("../src/main.rs");

/// Liest die Namen aus dem `generate_handler!`-Block.
///
/// Bewusst aus der **Anmeldung** und nicht aus den `#[tauri::command]`:
/// Eine Funktion, die dort fehlt, ist nicht erreichbar, auch wenn sie das
/// Attribut trägt. Was zählt, ist die Liste.
fn angemeldete() -> Vec<String> {
    let start = QUELLE
        .find("generate_handler![")
        .expect("generate_handler! muss es geben");
    let rest = &QUELLE[start..];
    let ende = rest.find(']').expect("die Liste muss enden");

    rest[..ende]
        .lines()
        .skip(1)
        .map(|z| z.trim().trim_end_matches(',').trim())
        .filter(|z| !z.is_empty() && !z.starts_with("//"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn die_liste_stimmt_mit_der_eingecheckten_ueberein() {
    let namen = angemeldete();
    assert!(!namen.is_empty(), "kein einziger Befehl angemeldet");

    let json = format!(
        "[\n{}\n]\n",
        namen
            .iter()
            .map(|n| format!("  \"{n}\""))
            .collect::<Vec<_>>()
            .join(",\n")
    );

    let pfad = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../app/oberflaeche/src/lib/kern/vertrag/befehle.json");

    if std::env::var_os("MUSTER_SCHREIBEN").is_some() {
        std::fs::write(&pfad, &json).expect("schreiben");
        return;
    }

    let vorhanden = std::fs::read_to_string(&pfad).unwrap_or_else(|_| {
        panic!(
            "Prüfmuster {} fehlt.\n\
             Neu erzeugen mit: MUSTER_SCHREIBEN=1 cargo test -p cabrik-fenster",
            pfad.display()
        )
    });

    assert_eq!(
        vorhanden.replace("\r\n", "\n"),
        json,
        "\nDie Befehlsliste hat sich geaendert.\n\
         Das Frontend muss nachziehen (app/oberflaeche/src/lib/kern/tauri.ts).\n\
         Danach neu erzeugen mit: MUSTER_SCHREIBEN=1 cargo test -p cabrik-fenster\n"
    );
}

#[test]
fn jeder_angemeldete_befehl_ist_auch_definiert() {
    // Ein Name in der Liste, zu dem es keine Funktion gibt, bricht schon
    // die Übersetzung. Andersherum -- eine Funktion, die niemand anmeldet
    // -- fällt nirgends auf, und genau davor schützt dieser Test.
    for name in angemeldete() {
        assert!(
            QUELLE.contains(&format!("fn {name}(")),
            "angemeldet, aber nicht definiert: {name}"
        );
    }
}

#[test]
fn jede_befehlsfunktion_ist_auch_angemeldet() {
    let namen = angemeldete();
    for zeile in QUELLE.lines() {
        let Some(rest) = zeile.trim().strip_prefix("fn ") else {
            continue;
        };
        let Some(name) = rest.split('(').next() else {
            continue;
        };
        // Nur die Befehle, nicht die Hilfsfunktionen.
        if !name.starts_with("kontakt") {
            continue;
        }
        assert!(
            namen.iter().any(|n| n == name),
            "definiert, aber nicht angemeldet -- das Fenster erreicht sie nie: {name}"
        );
    }
}
