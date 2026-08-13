//! Die Fensterhülle.
//!
//! # Warum diese Datei so dünn ist
//!
//! Weil sie es sein soll. Alles, was etwas entscheidet, steht in
//! `cabrik-app` und ist dort ohne Tauri geprüft — ohne Fenster, ohne
//! Webansicht, ohne Ereignisschleife. Hier stehen nur die Zeilen, die
//! Tauri braucht, um eine Funktion aufrufen zu können.
//!
//! Das war die ganze Absicht hinter der Reihenfolge (Leitprinzip 2): Wenn
//! am Ende etwas nicht geht, liegt es an dieser Datei oder an Tauri — nicht
//! an den Regeln darunter, denn die haben ihre Tests.
//!
//! # Was hier nie hineingehört
//!
//! Eine Regel. Sobald in einem `#[tauri::command]` ein `if` steht, das über
//! Vertrauen, Metadaten oder Schlüssel entscheidet, ist es an der falschen
//! Stelle: Es wäre dann nur noch mit laufender Webansicht prüfbar.
//!
//! # Die Sperre gegen das Konsolenfenster
//!
//! `windows_subsystem = "windows"` verhindert, dass unter Windows neben dem
//! Fenster eine Konsole aufgeht. Bei einem Werkzeug, das mit vertraulichen
//! Dateien umgeht, ist das kein Schönheitsfehler: Eine Konsole nimmt
//! Ausgaben entgegen, die niemand sehen soll.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

use std::sync::Mutex;

use cabrik_app::Sitzung;
use cabrik_bruecke::{Kontakt, Verifikationsweg};
use cabrik_core::trust::{Contact, TrustStore};
use tauri::{Manager as _, State};

/// Der Zustand, den Tauri zwischen den Aufrufen hält.
///
/// Ein `Mutex`, weil Tauri Befehle nebenläufig ausführt. Kein
/// Schlüsselmaterial — [`Sitzung`] hat kein Feld dafür.
struct Zustand(Mutex<Sitzung>);

/// Wandelt einen Befehlsfehler in etwas, das über die Brücke geht.
///
/// Tauri braucht `Serialize`; `Befehlsfehler` ist es nicht und soll es auch
/// nicht werden — die Fehlermeldung ist ein Satz, kein Datensatz.
fn wort(e: cabrik_app::Befehlsfehler) -> String {
    e.meldung
}

/// Was die Sperre selbst nicht kann: `Mutex` vergiften.
///
/// Ein vergifteter `Mutex` heißt, dass ein anderer Befehl in Panik geraten
/// ist. Weiterzuarbeiten, als wäre nichts, wäre der schlechteste Umgang
/// damit — die Oberfläche erfährt es.
fn sperre(z: &Zustand) -> Result<std::sync::MutexGuard<'_, Sitzung>, String> {
    z.0.lock().map_err(|_| {
        "Die Sitzung ist in einen unklaren Zustand geraten. Bitte das \
         Programm neu starten."
            .to_owned()
    })
}

// ---------------------------------------------------------------------------
// Befehle
// ---------------------------------------------------------------------------

#[tauri::command]
fn kontakte(zustand: State<'_, Zustand>) -> Result<Vec<Kontakt>, String> {
    Ok(sperre(&zustand)?.kontakte())
}

#[tauri::command]
fn kontakt_verifizieren(
    zustand: State<'_, Zustand>,
    fingerprint: String,
    weg: Verifikationsweg,
) -> Result<Kontakt, String> {
    sperre(&zustand)?
        .kontakt_verifizieren(&fingerprint, weg, jetzt())
        .map_err(wort)
}

#[tauri::command]
fn kontakt_zuruecksetzen(
    zustand: State<'_, Zustand>,
    fingerprint: String,
) -> Result<Kontakt, String> {
    sperre(&zustand)?
        .kontakt_zuruecksetzen(&fingerprint)
        .map_err(wort)
}

#[tauri::command]
fn kontakt_widerrufen(
    zustand: State<'_, Zustand>,
    fingerprint: String,
    grund: Option<String>,
) -> Result<Kontakt, String> {
    sperre(&zustand)?
        .kontakt_widerrufen(&fingerprint, jetzt(), grund.as_deref())
        .map_err(wort)
}

#[tauri::command]
fn kontakt_loeschen(zustand: State<'_, Zustand>, fingerprint: String) -> Result<(), String> {
    sperre(&zustand)?.kontakt_loeschen(&fingerprint).map_err(wort)
}

/// Unix-Sekunden.
fn jetzt() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

// ---------------------------------------------------------------------------

fn main() -> std::process::ExitCode {
    // Vorläufig ein leerer Speicher mit einer Wegwerf-Identität. Das Laden
    // aus der Datei kommt mit der Entsperrung; bis dahin soll das Fenster
    // aufgehen und die Befehle erreichbar sein.
    let Ok(eigener) =
        Contact::new_seen("ich", [0x99; 32], Some([0x98; 32]), None, 0).map(|k| k.fingerprint())
    else {
        eprintln!("Die Anfangsidentität ließ sich nicht bilden.");
        return std::process::ExitCode::FAILURE;
    };

    let lauf = tauri::Builder::default()
        .setup(move |app| {
            app.manage(Zustand(Mutex::new(Sitzung::neu(
                TrustStore::new(),
                eigener,
            ))));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            kontakte,
            kontakt_verifizieren,
            kontakt_zuruecksetzen,
            kontakt_widerrufen,
            kontakt_loeschen,
        ])
        .run(tauri::generate_context!());

    // Kein `expect`: Eine Panik hinterließe unter Windows nur ein Fenster,
    // das verschwindet. Ein Satz auf der Fehlerausgabe und ein Rückgabewert
    // sagen wenigstens, dass etwas schiefging.
    if let Err(e) = lauf {
        eprintln!("Das Fenster ließ sich nicht öffnen: {e}");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}
