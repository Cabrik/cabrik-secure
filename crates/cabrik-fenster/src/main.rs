//! Die Fensterhülle.
//!
//! # Warum diese Datei so dünn ist
//!
//! Weil sie es sein soll. Alles, was etwas entscheidet, steht in
//! `cabrik-app` und ist dort ohne Tauri geprüft — ohne Fenster, ohne
//! Webansicht, ohne Ereignisschleife. Hier stehen nur die Zeilen, die
//! Tauri braucht, um eine Funktion aufrufen zu können.
//!
//! # Was hier nie hineingehört
//!
//! Eine Regel. Sobald in einem `#[tauri::command]` ein `if` steht, das über
//! Vertrauen, Metadaten oder Schlüssel entscheidet, ist es an der falschen
//! Stelle: Es wäre dann nur noch mit laufender Webansicht prüfbar.
//!
//! Die Sperre ist dafür das Musterbeispiel. Sie wird hier **nicht** geprüft
//! — sie steht im Typ: An die Kontaktbefehle kommt man nur über
//! `Sitzung::offen`, und das prüft die Frist selbst.
//!
//! # Was hier sehr wohl hineingehört
//!
//! Das Schreiben. `cabrik-app` fasst bewusst kein Dateisystem an — dadurch
//! laufen seine dreißig Tests ohne eine einzige Datei. Wo etwas auf die
//! Platte muss, ist hier.
//!
//! # Die Sperre gegen das Konsolenfenster
//!
//! `windows_subsystem = "windows"` verhindert, dass unter Windows neben dem
//! Fenster eine Konsole aufgeht. Bei einem Werkzeug, das mit vertraulichen
//! Dateien umgeht, ist das kein Schönheitsfehler: Eine Konsole nimmt
//! Ausgaben entgegen, die niemand sehen soll.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use cabrik_app::Sitzung;
use cabrik_bruecke::{Kontakt, Nutzlastbefund, Sitzungsstand, Sperrfrist, Verifikationsweg};
use cabrik_core::OsRandom;
use tauri::{Manager as _, State};
use zeroize::Zeroizing;

/// Der Zustand, den Tauri zwischen den Aufrufen hält.
///
/// Ein `Mutex`, weil Tauri Befehle nebenläufig ausführt. Kein
/// Schlüsselmaterial — [`Sitzung`] hat kein Feld dafür.
///
/// `None` heißt: Es gibt noch keine Schlüsseldatei. Das ist **nicht**
/// dasselbe wie „gesperrt“, und die Oberfläche muss beides unterscheiden
/// können — im einen Fall führt der Weg zum Passwortfeld, im anderen zur
/// Einrichtung.
struct Zustand {
    sitzung: Mutex<Option<Sitzung>>,
    /// Wohin der Kontaktspeicher geschrieben wird.
    kontaktpfad: PathBuf,
}

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
fn sperre(z: &Zustand) -> Result<std::sync::MutexGuard<'_, Option<Sitzung>>, String> {
    z.sitzung.lock().map_err(|_| {
        "Die Sitzung ist in einen unklaren Zustand geraten. Bitte das \
         Programm neu starten."
            .to_owned()
    })
}

/// Die Sitzung — oder ein Satz darüber, dass es keine gibt.
fn sitzung(z: &mut Option<Sitzung>) -> Result<&mut Sitzung, String> {
    z.as_mut().ok_or_else(|| {
        "Auf diesem Rechner liegt noch keine Identität. Legen Sie unter \
         „Einrichtung“ eine an."
            .to_owned()
    })
}

/// Schreibt den Kontaktspeicher zurück.
///
/// Nach **jeder** Änderung, und zwar in derselben Funktion, die sie
/// auslöst. Den Aufrufer daran zu erinnern wäre die schlechtere Lösung:
/// Wer es einmal vergisst, verliert stillschweigend eine Verifikation, und
/// der Nutzer merkt es erst beim nächsten Start.
fn sichern(pfad: &Path, s: &mut Sitzung) -> Result<(), String> {
    let daten = s.kontakte_sichern(jetzt(), &mut OsRandom).map_err(wort)?;
    cabrik_ablage::schreib_atomar(pfad, &daten).map_err(|e| e.meldung)
}

/// Unix-Sekunden.
fn jetzt() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

// ---------------------------------------------------------------------------
// Sitzung
// ---------------------------------------------------------------------------

#[tauri::command]
fn sitzungsstand(zustand: State<'_, Zustand>) -> Result<Option<Sitzungsstand>, String> {
    let mut z = sperre(&zustand)?;
    // `None` heißt „keine Identität" -- und das unterscheidet sich von
    // „gesperrt". Die Oberfläche führt im einen Fall zur Einrichtung, im
    // anderen zum Passwortfeld.
    Ok(z.as_mut().map(|s| s.stand(jetzt())))
}

#[tauri::command]
fn entsperren(zustand: State<'_, Zustand>, passwort: String) -> Result<(), String> {
    let mut z = sperre(&zustand)?;
    // Das Passwort wird sofort in `Zeroizing` gefasst. Die Kopien davor --
    // die JavaScript-Zeichenkette und der Übergabepuffer -- lassen sich
    // nicht überschreiben (`spec/entsperrung.md` §5.1). Diese hier schon.
    let geschuetzt = Zeroizing::new(passwort);
    sitzung(&mut z)?
        .entsperren(&geschuetzt, jetzt())
        .map_err(wort)
}

#[tauri::command]
fn sperren(zustand: State<'_, Zustand>) -> Result<(), String> {
    let mut z = sperre(&zustand)?;
    sitzung(&mut z)?.sperren();
    Ok(())
}

#[tauri::command]
fn frist_setzen(zustand: State<'_, Zustand>, frist: Sperrfrist) -> Result<(), String> {
    let mut z = sperre(&zustand)?;
    sitzung(&mut z)?.frist_setzen(frist, jetzt());
    Ok(())
}

/// Der Nutzer hat gehandelt — Taste, Klick, Rollen.
///
/// Die Oberfläche ruft das gedrosselt auf, nicht bei jedem Tastendruck: Es
/// geht um Anwesenheit, und die ändert sich nicht im Zehntelsekundentakt.
///
/// Gibt bewusst **nichts** zurück, auch keinen Fehler bei fehlender
/// Identität: Eine Meldung, die bei jedem Tastendruck erscheinen könnte,
/// wäre binnen Minuten unerträglich.
#[tauri::command]
fn taetigkeit(zustand: State<'_, Zustand>) {
    if let Ok(mut z) = zustand.sitzung.lock()
        && let Some(s) = z.as_mut()
    {
        s.taetigkeit(jetzt());
    }
}

// ---------------------------------------------------------------------------
// Kontakte
// ---------------------------------------------------------------------------

#[tauri::command]
fn kontakte(zustand: State<'_, Zustand>) -> Result<Vec<Kontakt>, String> {
    let mut z = sperre(&zustand)?;
    Ok(sitzung(&mut z)?.offen(jetzt()).map_err(wort)?.kontakte())
}

#[tauri::command]
fn nutzlast_lesen(
    zustand: State<'_, Zustand>,
    nutzlast: String,
) -> Result<Nutzlastbefund, String> {
    let mut z = sperre(&zustand)?;
    Ok(sitzung(&mut z)?
        .offen(jetzt())
        .map_err(wort)?
        .nutzlast_lesen(&nutzlast))
}

#[tauri::command]
fn kontakt_aufnehmen(
    zustand: State<'_, Zustand>,
    name: String,
    nutzlast: String,
) -> Result<Kontakt, String> {
    let pfad = zustand.kontaktpfad.clone();
    let mut z = sperre(&zustand)?;
    let n = jetzt();
    let s = sitzung(&mut z)?;
    let k = s
        .offen(n)
        .map_err(wort)?
        .kontakt_aus_nutzlast(&name, &nutzlast, n)
        .map_err(wort)?;
    sichern(&pfad, s)?;
    Ok(k)
}

#[tauri::command]
fn kontakt_verifizieren(
    zustand: State<'_, Zustand>,
    fingerprint: String,
    weg: Verifikationsweg,
) -> Result<Kontakt, String> {
    let pfad = zustand.kontaktpfad.clone();
    let mut z = sperre(&zustand)?;
    let n = jetzt();
    let s = sitzung(&mut z)?;
    let k = s
        .offen(n)
        .map_err(wort)?
        .kontakt_verifizieren(&fingerprint, weg, n)
        .map_err(wort)?;
    sichern(&pfad, s)?;
    Ok(k)
}

#[tauri::command]
fn kontakt_zuruecksetzen(
    zustand: State<'_, Zustand>,
    fingerprint: String,
) -> Result<Kontakt, String> {
    let pfad = zustand.kontaktpfad.clone();
    let mut z = sperre(&zustand)?;
    let s = sitzung(&mut z)?;
    let k = s
        .offen(jetzt())
        .map_err(wort)?
        .kontakt_zuruecksetzen(&fingerprint)
        .map_err(wort)?;
    sichern(&pfad, s)?;
    Ok(k)
}

#[tauri::command]
fn kontakt_widerrufen(
    zustand: State<'_, Zustand>,
    fingerprint: String,
    grund: Option<String>,
) -> Result<Kontakt, String> {
    let pfad = zustand.kontaktpfad.clone();
    let mut z = sperre(&zustand)?;
    let n = jetzt();
    let s = sitzung(&mut z)?;
    let k = s
        .offen(n)
        .map_err(wort)?
        .kontakt_widerrufen(&fingerprint, n, grund.as_deref())
        .map_err(wort)?;
    sichern(&pfad, s)?;
    Ok(k)
}

#[tauri::command]
fn kontakt_loeschen(zustand: State<'_, Zustand>, fingerprint: String) -> Result<(), String> {
    let pfad = zustand.kontaktpfad.clone();
    let mut z = sperre(&zustand)?;
    let s = sitzung(&mut z)?;
    s.offen(jetzt())
        .map_err(wort)?
        .kontakt_loeschen(&fingerprint)
        .map_err(wort)?;
    sichern(&pfad, s)
}

// ---------------------------------------------------------------------------

fn main() -> std::process::ExitCode {
    // Beide Dateien liegen dort, wo auch die CLI sie sucht -- dieselbe
    // Schicht bestimmt den Pfad. Zwei Umsetzungen liefen auseinander, und
    // dann schriebe die eine, wo die andere nicht liest.
    let (schluesselpfad, kontaktpfad) = match (
        cabrik_ablage::keyfile_pfad(None),
        cabrik_ablage::kontakte_pfad(None),
    ) {
        (Ok(k), Ok(c)) => (k, c),
        _ => {
            eprintln!("Kein Konfigurationsverzeichnis feststellbar.");
            return std::process::ExitCode::FAILURE;
        }
    };

    // Ohne Schlüsseldatei bleibt die Sitzung `None`. Das ist NICHT dasselbe
    // wie gesperrt: Der Weg führt dann zur Einrichtung, nicht zum
    // Passwortfeld.
    let sitzung = match cabrik_ablage::lies(&schluesselpfad) {
        Ok(Some(schluessel)) => {
            // Eine fehlende Kontaktdatei ist beim ersten Start der
            // Normalfall. Eine unlesbare fällt erst beim Entsperren auf --
            // und wird dort benannt, statt hier zu einem Abbruch zu führen.
            let kontakte = cabrik_ablage::lies(&kontaktpfad).ok().flatten();
            Some(Sitzung::neu(schluessel, kontakte, Sperrfrist::default()))
        }
        Ok(None) => None,
        Err(e) => {
            eprintln!("Die Schlüsseldatei ließ sich nicht lesen: {}", e.meldung);
            return std::process::ExitCode::FAILURE;
        }
    };

    let lauf = tauri::Builder::default()
        .setup(move |app| {
            app.manage(Zustand {
                sitzung: Mutex::new(sitzung),
                kontaktpfad,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            sitzungsstand,
            entsperren,
            sperren,
            frist_setzen,
            taetigkeit,
            kontakte,
            nutzlast_lesen,
            kontakt_aufnehmen,
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
