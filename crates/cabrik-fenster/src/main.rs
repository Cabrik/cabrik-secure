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
    Bereinigung, Geoeffnet, Identitaet, KdfStufe, Kontakt, Nutzlastbefund, Sendedatei,
    Sitzungsstand,
    Speicherergebnis, Sperrfrist, Verifikationsweg, Versandbericht, Versandergebnis,
};
use cabrik_core::OsRandom;
use tauri::{Manager as _, State};
use tauri_plugin_dialog::DialogExt as _;
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
// Dateien ansehen
// ---------------------------------------------------------------------------

/// Lässt Dateien auswählen.
///
/// # Warum der Dialog in Rust steht und nicht in der Webansicht
///
/// Weil die Naht dann bleibt, wie sie überall ist: Die Oberfläche fragt,
/// der Kern tut. Der Weg über JavaScript hätte ein npm-Paket und eine
/// Berechtigung für die Webansicht gebraucht — also eine Stelle mehr, an
/// der die Webansicht etwas darf, was sie sonst nicht darf.
///
/// # Warum `(async)`
///
/// Ein gewöhnlicher Befehl läuft bei Tauri auf dem Hauptfaden. Ein
/// blockierender Dialog dort hielte die Ereignisschleife an, die er selbst
/// zum Anzeigen braucht. `(async)` schiebt ihn auf einen eigenen Faden.
///
/// Eine leere Liste heißt **abgebrochen** und ist kein Fehler: Wer den
/// Dialog schließt, hat sich entschieden.
#[tauri::command(async)]
fn dateien_waehlen(app: tauri::AppHandle) -> Vec<String> {
    app.dialog()
        .file()
        .blocking_pick_files()
        .unwrap_or_default()
        .into_iter()
        // Was sich nicht in einen Pfad übersetzen lässt, fällt hier weg.
        // Der Dialog liefert auf dem Desktop stets Pfade; ein Eintrag, der
        // es nicht ist, wäre für alles Weitere ohnehin unbrauchbar.
        .filter_map(|f| f.into_path().ok())
        .map(|p| p.display().to_string())
        .collect()
}

/// Sieht Dateien an, **ohne etwas zu verändern**.
///
/// # Was nicht zurückgeht
///
/// Der Inhalt. Die Bytes werden gelesen, geprüft und fallengelassen; über
/// die Brücke geht nur der Befund. Eine Oberfläche, die Dateiinhalte hält,
/// hätte sie in einem Speicher, den wir weder überschreiben noch begrenzen
/// können — und bei einem Stapel aus vierzig Bildern wären das hunderte
/// Megabyte in einer Webansicht.
///
/// # Warum jede Datei einzeln scheitern darf
///
/// Weil ein Stapel aus vierzig Dateien nicht an einer scheitern soll, die
/// gerade in Benutzung ist. Was sich nicht lesen ließ, kommt als
/// [`Bereinigung::Fehler`] zurück und steht mit seinem Grund im Stapel —
/// sichtbar, statt stillschweigend zu fehlen.
#[tauri::command]
fn dateien_pruefen(pfade: Vec<String>) -> Vec<Sendedatei> {
    pfade
        .into_iter()
        .map(|p| {
            let pfad = Path::new(&p);
            let name = pfad
                .file_name()
                .map_or_else(|| p.clone(), |n| n.to_string_lossy().into_owned());

            match std::fs::read(pfad) {
                Ok(daten) => cabrik_app::datei_pruefen(&p, &name, &daten),
                Err(e) => Sendedatei {
                    pfad: p.clone(),
                    name,
                    groesse_bytes: 0,
                    befund: Bereinigung::Fehler {
                        grund: e.to_string(),
                    },
                    fassungen: Vec::new(),
                },
            }
        })
        .collect()
}

/// Speichert die bereinigten Fassungen — **ohne zu verschlüsseln**.
///
/// # Warum es das gibt
///
/// Weil Metadaten zu entfernen ein eigener Zweck ist. Wer ein Foto irgendwo
/// hochlädt, will kein Envelope, sondern ein Bild ohne Ortsangabe.
///
/// # Was dabei nicht passiert
///
/// **Die Ausgangsdatei bleibt unverändert liegen.** Es entsteht eine zweite
/// Datei daneben, und damit liegen danach zwei unverschlüsselte Fassungen
/// auf der Platte — eine davon mit allem, was drinstand. Wer das nicht
/// sagt, lässt jemanden im Glauben, er habe etwas bereinigt.
///
/// # Wohin
///
/// Bei einer Datei fragt ein Speichern-Dialog nach Name und Ort. Bei
/// mehreren fragt ein Ordner-Dialog, und die Namen entstehen daraus:
/// `Foto.jpg` wird zu `Foto.bereinigt.jpg`. Einen Dialog je Datei
/// vierzigmal zu beantworten wäre keine Wahl, sondern eine Strafe.
///
/// **Nichts wird überschrieben.** Liegt der Zielname schon da, wird
/// durchnummeriert.
#[tauri::command(async)]
fn bereinigt_speichern(app: tauri::AppHandle, pfade: Vec<String>) -> Vec<Speicherergebnis> {
    let Some(ziel) = ziel_erfragen(&app, &pfade) else {
        // Abgebrochen. Eine leere Liste heisst „nichts getan" und ist kein
        // Fehler: Wer den Dialog schliesst, hat sich entschieden.
        return Vec::new();
    };

    pfade.into_iter().map(|p| eine_speichern(&p, &ziel)).collect()
}

/// Wohin gespeichert wird — eine Datei oder ein ganzer Ordner.
enum Ablageziel {
    Datei(PathBuf),
    Ordner(PathBuf),
}

fn ziel_erfragen(app: &tauri::AppHandle, pfade: &[String]) -> Option<Ablageziel> {
    match pfade {
        [] => None,
        [einzeln] => {
            let vorschlag = bereinigter_name(Path::new(einzeln));
            app.dialog()
                .file()
                .set_file_name(&vorschlag)
                .blocking_save_file()
                .and_then(|f| f.into_path().ok())
                .map(Ablageziel::Datei)
        }
        _ => app
            .dialog()
            .file()
            .blocking_pick_folder()
            .and_then(|f| f.into_path().ok())
            .map(Ablageziel::Ordner),
    }
}

fn eine_speichern(quelle: &str, ziel: &Ablageziel) -> Speicherergebnis {
    let pfad = Path::new(quelle);

    let daten = match std::fs::read(pfad) {
        Ok(d) => d,
        Err(e) => {
            return Speicherergebnis {
                quelle: quelle.to_owned(),
                ziel: None,
                befund: Bereinigung::Fehler {
                    grund: e.to_string(),
                },
                fehler: Some(e.to_string()),
            };
        }
    };

    let (sauber, befund) = cabrik_app::datei_bereinigen(&daten);
    let Some(inhalt) = sauber else {
        return Speicherergebnis {
            quelle: quelle.to_owned(),
            ziel: None,
            befund,
            fehler: Some(
                "Für diese Datei gibt es keine bereinigte Fassung — das Format \
                 wurde nicht verstanden."
                    .to_owned(),
            ),
        };
    };

    let wohin = freier_name(&match ziel {
        Ablageziel::Datei(d) => d.clone(),
        Ablageziel::Ordner(o) => o.join(bereinigter_name(pfad)),
    });

    match cabrik_ablage::schreib_neu(&wohin, &inhalt) {
        Ok(()) => Speicherergebnis {
            quelle: quelle.to_owned(),
            ziel: Some(wohin.display().to_string()),
            befund,
            fehler: None,
        },
        Err(e) => Speicherergebnis {
            quelle: quelle.to_owned(),
            ziel: None,
            befund,
            fehler: Some(e.meldung),
        },
    }
}

/// `Foto.jpg` wird zu `Foto.bereinigt.jpg`.
///
/// Der Zusatz steht **vor** der Endung, damit das Betriebssystem die Datei
/// weiter als Bild erkennt und sie sich mit einem Doppelklick öffnen lässt.
fn bereinigter_name(pfad: &Path) -> String {
    let stamm = pfad
        .file_stem()
        .map_or_else(|| "datei".to_owned(), |s| s.to_string_lossy().into_owned());
    pfad.extension().map_or_else(
        || format!("{stamm}.bereinigt"),
        |e| format!("{stamm}.bereinigt.{}", e.to_string_lossy()),
    )
}

/// Ein Name, den es noch nicht gibt.
///
/// **Nichts wird überschrieben.** Wer zweimal speichert, hat zweimal
/// gespeichert — nicht einmal, und die erste Fassung ist nicht fort.
fn freier_name(ziel: &Path) -> PathBuf {
    if !ziel.exists() {
        return ziel.to_path_buf();
    }
    let stamm = ziel
        .file_stem()
        .map_or_else(|| "datei".to_owned(), |s| s.to_string_lossy().into_owned());
    let endung = ziel
        .extension()
        .map_or_else(String::new, |e| format!(".{}", e.to_string_lossy()));
    let ordner = ziel.parent().unwrap_or_else(|| Path::new("."));

    (2..1_000_u32)
        .map(|nr| ordner.join(format!("{stamm} ({nr}){endung}")))
        .find(|v| !v.exists())
        .unwrap_or_else(|| ziel.to_path_buf())
}

/// Verschlüsselt die ausgewählten Dateien.
///
/// # Warum die Prüfung vor dem ersten Byte steht
///
/// `versand_planen` prüft die Empfänger einmal für den ganzen Stapel. Ein
/// Vorgang, der bei Datei siebenunddreißig an einem widerrufenen Schlüssel
/// abbricht, hätte sechsunddreißig Envelopes hinterlassen, die niemand
/// bestellt hat — und der Nutzer müsste sie einzeln wieder wegräumen.
///
/// # Wohin
///
/// Neben die Ausgangsdatei: `Foto.jpg` wird zu `Foto.jpg.cab`. Kein
/// Dialog — bei vierzig Dateien wären es vierzig Dialoge, und der Ort ist
/// ohnehin der, an dem man sie sucht. **Nichts wird überschrieben.**
///
/// # Was danach dasteht
///
/// Die Ausgangsdateien, unverändert. Verschlüsseln legt eine zweite Datei
/// daneben; es ersetzt die erste nicht.
#[tauri::command(async)]
fn verschluesseln(
    zustand: State<'_, Zustand>,
    pfade: Vec<String>,
    empfaenger: Vec<String>,
    signieren: bool,
    original: Vec<String>,
) -> Result<Versandbericht, String> {
    let mut z = sperre(&zustand)?;
    let offen = sitzung(&mut z)?.offen(jetzt()).map_err(wort)?;

    // Erst der Plan. Schlaegt er fehl, entsteht keine einzige Datei.
    let plan = offen.versand_planen(&empfaenger, signieren).map_err(wort)?;

    let dateien = pfade
        .into_iter()
        .map(|p| {
            let quelle = Path::new(&p);
            let name = quelle
                .file_name()
                .map_or_else(|| p.clone(), |n| n.to_string_lossy().into_owned());

            let roh = match std::fs::read(quelle) {
                Ok(d) => d,
                Err(e) => return cabrik_app::versand_fehler(&p, e.to_string()),
            };

            // Wer das Original verschicken will, bekommt das Original --
            // das war ja gerade die Entscheidung.
            let (nutzdaten, befund) = if original.contains(&p) {
                (roh, None)
            } else {
                let (sauber, b) = cabrik_app::datei_bereinigen(&roh);
                // Gibt es keine bereinigte Fassung, geht das Original
                // hinaus. Das ist kein Fehler: Bei einem nicht verstandenen
                // Format WEISS das Programm nicht, was es entfernen soll --
                // und der Befund sagt genau das.
                (sauber.unwrap_or(roh), Some(b))
            };

            let envelope = match offen.verschluesseln(&plan, &name, &nutzdaten, &mut OsRandom) {
                Ok(e) => e,
                Err(e) => return cabrik_app::versand_fehler(&p, e.meldung),
            };

            let ziel = freier_name(&quelle.with_file_name(cabrik_app::envelope_name(&name)));
            match cabrik_ablage::schreib_neu(&ziel, &envelope) {
                Ok(()) => Versandergebnis {
                    quelle: p.clone(),
                    ziel: Some(ziel.display().to_string()),
                    bytes: envelope.len(),
                    befund,
                    fehler: None,
                },
                Err(e) => cabrik_app::versand_fehler(&p, e.meldung),
            }
        })
        .collect();

    Ok(Versandbericht {
        suite: plan.suite_name().to_owned(),
        signiert: plan.signiert(),
        empfaenger: plan.empfaenger(),
        vorbehalte: plan.vorbehalte.clone(),
        dateien,
    })
}

// ---------------------------------------------------------------------------
// Entschlüsseln
// ---------------------------------------------------------------------------

/// Lässt einen Envelope auswählen.
#[tauri::command(async)]
fn envelope_waehlen(app: tauri::AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .add_filter("Cabrik-Envelope", &["cab"])
        .blocking_pick_file()
        .and_then(|f| f.into_path().ok())
        .map(|p| p.display().to_string())
}

/// Öffnet einen Envelope. **Der Klartext bleibt in Rust.**
///
/// Zurück geht ein Bericht: Wer geschickt hat, wie die Datei heißt, wie
/// groß sie ist. Der Inhalt liegt in der Sitzung, bis jemand sagt, wohin
/// er soll — oder bis gesperrt wird, dann ist er fort.
#[tauri::command]
fn envelope_oeffnen(
    zustand: State<'_, Zustand>,
    pfad: String,
    signatur_verlangt: bool,
) -> Result<Geoeffnet, String> {
    let daten = std::fs::read(Path::new(&pfad)).map_err(|e| e.to_string())?;
    let mut z = sperre(&zustand)?;
    sitzung(&mut z)?
        .offen(jetzt())
        .map_err(wort)?
        .envelope_oeffnen(&daten, signatur_verlangt)
        .map_err(wort)
}

/// Legt die geöffnete Nutzlast ab.
///
/// # Warum das ein zweiter Schritt ist
///
/// Weil Öffnen und Ablegen zwei Entscheidungen sind. Wer eine Nachricht
/// von einem unbekannten Absender öffnet, will vielleicht nur wissen, was
/// drinsteht — und nicht, dass sie danach auf der Platte liegt. Ein
/// Programm, das beim Öffnen gleich schreibt, nimmt ihm diese Wahl.
///
/// Der Speichern-Dialog schlägt den Namen vor, der **im Envelope** stand.
/// Der Envelope-Dateiname taugt nicht: Er ist der, den ein Mitleser sieht.
///
/// **Nichts wird überschrieben.**
#[tauri::command(async)]
fn nutzlast_speichern(
    zustand: State<'_, Zustand>,
    app: tauri::AppHandle,
) -> Result<Option<String>, String> {
    // Erst den Namen holen -- der Dialog laeuft ohne gehaltene Sperre,
    // sonst stuende die ganze Anwendung, solange er offen ist.
    let vorschlag = {
        let mut z = sperre(&zustand)?;
        let offen = sitzung(&mut z)?.offen(jetzt()).map_err(wort)?;
        let (_, name) = offen
            .nutzlast()
            .ok_or_else(|| "Es ist nichts geöffnet.".to_owned())?;
        name.unwrap_or("nachricht").to_owned()
    };

    // `None` heisst abgebrochen und ist kein Fehler: Wer den Dialog
    // schliesst, hat sich entschieden.
    let Some(ziel) = app
        .dialog()
        .file()
        .set_file_name(&vorschlag)
        .blocking_save_file()
        .and_then(|f| f.into_path().ok())
    else {
        return Ok(None);
    };

    let mut z = sperre(&zustand)?;
    let offen = sitzung(&mut z)?.offen(jetzt()).map_err(wort)?;
    let (inhalt, _) = offen
        .nutzlast()
        .ok_or_else(|| "Es ist nichts geöffnet.".to_owned())?;

    let ziel = freier_name(&ziel);
    cabrik_ablage::schreib_neu(&ziel, inhalt).map_err(|e| e.meldung)?;
    Ok(Some(ziel.display().to_string()))
}

/// Wirft den geöffneten Klartext weg.
///
/// Beim Verlassen des Bildschirms. Ein entschlüsselter Inhalt, der liegen
/// bleibt, ist eine Kopie ohne Zweck.
#[tauri::command]
fn nutzlast_verwerfen(zustand: State<'_, Zustand>) {
    if let Ok(mut z) = zustand.sitzung.lock()
        && let Some(s) = z.as_mut()
        && let Ok(o) = s.offen(jetzt())
    {
        o.nutzlast_verwerfen();
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
        .plugin(tauri_plugin_dialog::init())
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
            dateien_waehlen,
            dateien_pruefen,
            bereinigt_speichern,
            verschluesseln,
            envelope_waehlen,
            envelope_oeffnen,
            nutzlast_speichern,
            nutzlast_verwerfen,
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
