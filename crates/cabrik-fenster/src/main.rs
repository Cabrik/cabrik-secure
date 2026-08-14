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

use cabrik_app::{Betroffen, Sitzung};
use cabrik_bruecke::{
    Identitaet, KdfStufe, Kontakt, Nutzlastbefund, Sitzungsstand, Sperrfrist, Verifikationsweg,
};
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
    /// Wohin die Schlüsseldatei geschrieben wird — und wo sie herkam.
    schluesselpfad: PathBuf,
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
    let kontaktpfad = zustand.kontaktpfad.clone();
    let mut z = sperre(&zustand)?;
    // Das Passwort wird sofort in `Zeroizing` gefasst. Die Kopien davor --
    // die JavaScript-Zeichenkette und der Übergabepuffer -- lassen sich
    // nicht überschreiben (`spec/entsperrung.md` §5.1). Diese hier schon.
    let geschuetzt = Zeroizing::new(passwort);
    sitzung(&mut z)?
        .entsperren(&geschuetzt, jetzt())
        .map_err(|e| match e.betrifft {
            // Der Pfad gehört in die Meldung: Sonst säße jemand mit
            // richtigem Passwort vor einer verschlossenen Tür und wüsste
            // nicht, welche Datei im Weg liegt. Die Sitzungsschicht kennt
            // ihn nicht -- sie sieht Bytes.
            Betroffen::Kontaktspeicher => format!(
                "{} Sie können die Datei umbenennen oder wegräumen; die                  Identität selbst ist davon nicht betroffen.

{}",
                e.meldung,
                kontaktpfad.display()
            ),
            _ => e.meldung,
        })
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
// Identität
// ---------------------------------------------------------------------------

/// Legt eine Identität an und beginnt damit sofort eine offene Sitzung.
///
/// # Zwei Sperren gegen denselben Fehlgriff
///
/// Eine neue Identität über eine bestehende zu schreiben ist der
/// folgenschwerste Vorgang, den dieses Programm zulassen könnte: Alles, was
/// an den alten Fingerprint gerichtet war, wäre danach dauerhaft unlesbar.
/// Deshalb steht die Prüfung **zweimal**, und zwar absichtlich:
///
/// 1. Hier, weil die laufende Sitzung es weiß — und der Nutzer einen Satz
///    lesen soll, der zu seiner Lage passt.
/// 2. In `cabrik_ablage::schreib_neu`, weil zwischen dem Start des
///    Programms und diesem Aufruf jemand anders eine Datei angelegt haben
///    kann, und weil der nächste Aufrufer die erste Prüfung vergessen wird.
///
/// Die zweite ist die verlässliche. Die erste ist die höfliche.
#[tauri::command]
fn identitaet_anlegen(
    zustand: State<'_, Zustand>,
    bezeichnung: Option<String>,
    passwort: String,
    mit_signierschluessel: bool,
    stufe: KdfStufe,
) -> Result<Identitaet, String> {
    let pfad = zustand.schluesselpfad.clone();
    let kontaktpfad = zustand.kontaktpfad.clone();
    let mut z = sperre(&zustand)?;

    if z.is_some() {
        return Err("Auf diesem Rechner liegt bereits eine Identität. Eine                     zweite anzulegen, würde die bisherige überschreiben — und                     damit alles unlesbar machen, was an sie gerichtet ist."
            .to_owned());
    }

    let geschuetzt = Zeroizing::new(passwort);
    let neu = Sitzung::anlegen(
        bezeichnung,
        &geschuetzt,
        mit_signierschluessel,
        stufe,
        Sperrfrist::default(),
        jetzt(),
        &mut OsRandom,
    )
    .map_err(wort)?;

    // Erst schreiben, dann übernehmen. Andersherum stünde nach einem
    // Schreibfehler eine offene Sitzung über einer Datei, die es nicht
    // gibt -- beim nächsten Start wäre sie spurlos verschwunden.
    cabrik_ablage::schreib_neu(&pfad, neu.schluesseldatei()).map_err(|e| e.meldung)?;

    // Ein Kontaktspeicher, der jetzt noch daliegt, ist eine Waise: Da
    // `schreib_neu` gerade bewiesen hat, dass es KEINE Schlüsseldatei gab,
    // gehört er zu einer Identität, die es nicht mehr gibt -- und ist an
    // sie versiegelt, also dauerhaft unlesbar.
    //
    // Bliebe er liegen, scheiterte beim nächsten Start das Entsperren, mit
    // richtigem Passwort, an einer Datei, die niemand mehr braucht. Die
    // Identität wäre unerreichbar, ohne dass irgendetwas darauf hinwiese.
    cabrik_ablage::verschiebe_beiseite(&kontaktpfad).map_err(|e| e.meldung)?;

    *z = Some(neu);
    lies_identitaet(&mut z, &pfad)
}

/// Die eigene Identität — nur im entsperrten Zustand.
#[tauri::command]
fn identitaet(zustand: State<'_, Zustand>) -> Result<Identitaet, String> {
    let pfad = zustand.schluesselpfad.clone();
    let mut z = sperre(&zustand)?;
    lies_identitaet(&mut z, &pfad)
}

fn lies_identitaet(z: &mut Option<Sitzung>, pfad: &Path) -> Result<Identitaet, String> {
    let s = sitzung(z)?;
    // Die Datei wird aus der Sitzung genommen und nicht neu von der Platte
    // gelesen: Sonst zeigte die Anzeige etwas anderes an, als gerade offen
    // ist, wenn jemand die Datei nebenher austauscht.
    let datei = s.schluesseldatei().to_vec();
    s.offen(jetzt())
        .map_err(wort)?
        .identitaet(&datei, pfad.display().to_string())
        .map_err(wort)
}

/// Löscht die Identität — der folgenschwerste Vorgang des Programms.
///
/// # Warum nur im entsperrten Zustand
///
/// Es schützt die Datei nicht: Wer am Rechner sitzt, kann sie auch im
/// Dateimanager wegwerfen. Es schützt gegen etwas anderes — dagegen, dass
/// das Programm selbst einen Knopf anbietet, mit dem jemand ohne Passwort
/// in zwei Klicks alles vernichtet, was an diesen Schlüssel gerichtet war.
///
/// Der Preis ist ehrlich zu nennen: Wer sein Passwort vergessen hat und neu
/// anfangen will, kommt hier nicht durch und muss die Datei selbst
/// entfernen.
///
/// # Warum der Kontaktspeicher mitgeht
///
/// Er ist an die Identität versiegelt (`spec/trust-store.md` §6). Ohne sie
/// ist er nicht mehr zu öffnen — er stehen zu lassen hieße, eine Datei
/// zurückzulassen, die niemand je wieder lesen kann und die trotzdem
/// aussieht, als enthielte sie etwas.
///
/// **Die Schlüsseldatei zuerst.** Andersherum wären bei einem Fehlschlag
/// die Kontakte fort und die Identität noch da — der schlechtere von zwei
/// halben Zuständen.
#[tauri::command]
fn identitaet_loeschen(zustand: State<'_, Zustand>) -> Result<(), String> {
    let schluesselpfad = zustand.schluesselpfad.clone();
    let kontaktpfad = zustand.kontaktpfad.clone();
    let mut z = sperre(&zustand)?;

    // Der Aufruf ist die Prüfung: `offen` scheitert, wenn gesperrt ist.
    sitzung(&mut z)?.offen(jetzt()).map_err(wort)?;

    cabrik_ablage::loesche(&schluesselpfad).map_err(|e| e.meldung)?;
    let kontakte = cabrik_ablage::loesche(&kontaktpfad);

    // Erst danach vergessen: Solange die Datei noch da ist, soll die
    // Oberfläche nicht behaupten, es gäbe keine Identität mehr.
    *z = None;

    kontakte.map_err(|e| e.meldung)
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
                schluesselpfad,
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
            identitaet,
            identitaet_anlegen,
            identitaet_loeschen,
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
