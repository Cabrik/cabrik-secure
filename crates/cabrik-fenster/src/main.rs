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

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use cabrik_app::{Betroffen, Sitzung};
use cabrik_bruecke::{
    Bereinigung, Fortschritt, Geoeffnet, Identitaet, KdfStufe, Kontakt, Loeschbeurteilung,
    Loeschergebnis, Loeschkandidat, Nutzlastbefund, QrCode, Sendedatei, Sitzungsstand,
    Speicherergebnis, Sperrfrist, Startfehler, Verifikationsweg, Versandbericht, Versandergebnis,
};
use cabrik_core::OsRandom;
use cabrik_core::envelope;
use tauri::ipc::Channel;
use tauri::{Emitter as _, Manager as _, State};
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
    /// Eine Datei, die das Betriebssystem hereingereicht hat.
    ///
    /// Sie kommt aus einem Doppelklick im Explorer — beim Start als
    /// Befehlszeilenargument, bei einem laufenden Fenster über die
    /// Einmaligkeitssperre. Sie liegt hier, bis die Oberfläche sie abholt.
    ///
    /// **Ein Pfad, kein Inhalt.** Gelesen wird erst, wenn jemand entsperrt
    /// hat und tatsächlich öffnet.
    hereingereicht: Mutex<Option<String>>,
    /// Was den Start verhindert hat, sofern etwas.
    ///
    /// Steht hier etwas, zeigt die Oberfläche **nur** das — kein
    /// Passwortfeld, keine Einrichtung. Beides wäre eine Aufforderung zu
    /// etwas, das gerade nicht geht.
    ///
    /// Kein `Mutex`: Der Wert entsteht vor dem Fenster und ändert sich nie.
    startfehler: Option<Startfehler>,
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

/// Arbeitet einen Stapel ab und meldet dabei, wo er steht.
///
/// # Warum das an einer Stelle steht
///
/// Weil es fünf Befehle gibt, die einen Stapel abarbeiten — prüfen,
/// bereinigt speichern, verschlüsseln, löschen beurteilen, löschen. Fünfmal
/// dieselbe Schleife hieße fünf Gelegenheiten, die Meldung zu vergessen,
/// und ein Bildschirm ohne Fortschritt ist von einem hängenden nicht zu
/// unterscheiden.
///
/// # Warum **vor** der Datei gemeldet wird
///
/// Weil die Auskunft „arbeitet an X" heißt und nicht „X ist fertig". Bei
/// einer Datei, die eine Minute braucht, starrte man sonst auf den Namen
/// der vorigen. `erledigt` zählt die fertigen, `laeuft` nennt die laufende
/// — zusammen ergibt das einen Satz, der stimmt, während er dasteht.
///
/// # Warum ein fehlgeschlagener Bericht die Arbeit nicht abbricht
///
/// Weil der Kanal weg sein kann, wenn das Fenster zugeht — und dann ist die
/// begonnene Arbeit trotzdem zu Ende zu bringen. Eine halb gelöschte Datei
/// wäre der schlechteste denkbare Ausgang einer geschlossenen Anzeige.
fn stapel<T>(
    pfade: Vec<String>,
    melden: &Channel<Fortschritt>,
    mut je: impl FnMut(&str) -> T,
) -> Vec<T> {
    let gesamt = pfade.len();
    pfade
        .iter()
        .enumerate()
        .map(|(erledigt, p)| {
            let _ = melden.send(Fortschritt {
                erledigt,
                gesamt,
                laeuft: dateiname(p),
            });
            je(p)
        })
        .collect()
}

/// Der Name ohne den Pfad — oder der ganze Pfad, wenn es keinen gibt.
fn dateiname(pfad: &str) -> String {
    Path::new(pfad)
        .file_name()
        .map_or_else(|| pfad.to_owned(), |n| n.to_string_lossy().into_owned())
}

/// Sucht in den Befehlszeilenargumenten die Datei, die geöffnet werden soll.
///
/// # Was hier absichtlich nicht passiert
///
/// **Es wird nichts gelesen und nichts entschieden.** Zurück geht ein Pfad,
/// mehr nicht. Ob dahinter ein Envelope liegt, sagen die Magic-Bytes, und
/// die sieht sich der Kern an — erst dann, wenn jemand entsperrt hat und
/// tatsächlich öffnen will. Ein Programm, das beim Start ungefragt eine
/// Datei aufmacht, die ihm jemand untergeschoben hat, wäre eine Angriffsfläche
/// und kein Werkzeug.
///
/// # Warum kein Filter auf die Endung
///
/// Weil der Name nichts beweist. Wer `bericht.cabrik` sagt, kann alles
/// meinen; wer eine alte `.cab` doppelklickt, meint das Richtige. Die
/// Prüfung, die zählt, findet beim Öffnen statt.
///
/// # Warum nur das erste
///
/// Weil das Fenster einen Envelope zur Zeit zeigt. Mehrere Dateien
/// gleichzeitig anzunehmen und stillschweigend vier davon fallenzulassen
/// wäre schlechter als eine zu nehmen und es dabei zu belassen.
fn datei_aus_argumenten(argumente: impl IntoIterator<Item = impl Into<OsString>>) -> Option<String> {
    argumente
        .into_iter()
        .map(Into::into)
        .find(|a| {
            // Schalter überspringen. Tauri und WebView2 reichen unter
            // Windows eigene durch (`--webview-exe-name` und Verwandte);
            // eines davon für einen Dateipfad zu halten, öffnete Unsinn.
            !a.to_string_lossy().starts_with('-')
        })
        .map(|a| a.to_string_lossy().into_owned())
}

/// Holt die hereingereichte Datei ab — **und leert das Fach dabei**.
///
/// # Warum das Leeren dazugehört
///
/// Weil der Pfad sonst bei jedem Nachfragen erneut käme. Wer eine Datei
/// öffnet, sie wegklickt und den Bildschirm wechselt, bekäme sie wieder
/// vorgelegt — und hielte das für einen Fehler.
///
/// # Warum die Oberfläche fragt, statt beliefert zu werden
///
/// Weil es beim Start noch keine Webansicht gibt, die etwas empfangen
/// könnte: Die Datei liegt schon im Fach, bevor das Fenster steht. Ein
/// Ereignis allein ginge ins Leere. Also gibt es **einen** Weg — diesen —,
/// und das Ereignis ist nur der Anstoß, ihn zu gehen.
/// Was den Start verhindert hat — `None` heißt: nichts.
///
/// Die Oberfläche fragt das **vor allem anderen**. Steht hier etwas, hat
/// weder ein Passwortfeld noch eine Einrichtung einen Sinn: Beides wäre
/// eine Aufforderung zu etwas, das gerade nicht geht.
#[tauri::command]
fn startfehler(zustand: State<'_, Zustand>) -> Option<Startfehler> {
    zustand.startfehler.clone()
}

#[tauri::command]
fn datei_abholen(zustand: State<'_, Zustand>) -> Option<String> {
    zustand
        .hereingereicht
        .lock()
        .ok()
        .and_then(|mut fach| fach.take())
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
///
/// # Warum `(async)`
///
/// **Ein `#[tauri::command]` ohne diesen Zusatz läuft auf dem Hauptfaden.**
/// Das ist unter Windows derselbe Faden, der das Fenster zeichnet: Vierzig
/// Fotos zu lesen und zu untersuchen ließe die Anzeige einfrieren, und kein
/// Fortschrittsbericht käme durch — er würde ja erst zugestellt, wenn schon
/// alles fertig ist. Dieser Befehl war der einzige der fünf ohne den
/// Zusatz; die Anzeige stand still, und es sah aus wie ein Absturz.
#[tauri::command(async)]
fn dateien_pruefen(pfade: Vec<String>, fortschritt: Channel<Fortschritt>) -> Vec<Sendedatei> {
    stapel(pfade, &fortschritt, |p| {
        let pfad = Path::new(p);
        let name = dateiname(p);

        match std::fs::read(pfad) {
            Ok(daten) => cabrik_app::datei_pruefen(p, &name, &daten),
            Err(e) => Sendedatei {
                pfad: p.to_owned(),
                name,
                groesse_bytes: 0,
                befund: Bereinigung::Fehler {
                    grund: e.to_string(),
                },
                fassungen: Vec::new(),
            },
        }
    })
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
fn bereinigt_speichern(
    app: tauri::AppHandle,
    pfade: Vec<String>,
    fortschritt: Channel<Fortschritt>,
) -> Vec<Speicherergebnis> {
    let Some(ziel) = ziel_erfragen(&app, &pfade) else {
        // Abgebrochen. Eine leere Liste heisst „nichts getan" und ist kein
        // Fehler: Wer den Dialog schliesst, hat sich entschieden.
        return Vec::new();
    };

    // Erst NACH dem Dialog melden. Waehrend er offen steht, arbeitet
    // niemand -- ein Balken, der dabei laeuft, behauptete Betrieb.
    stapel(pfade, &fortschritt, |p| eine_speichern(p, &ziel))
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
    fortschritt: Channel<Fortschritt>,
) -> Result<Versandbericht, String> {
    let mut z = sperre(&zustand)?;
    let offen = sitzung(&mut z)?.offen(jetzt()).map_err(wort)?;

    // Erst der Plan. Schlaegt er fehl, entsteht keine einzige Datei.
    let plan = offen.versand_planen(&empfaenger, signieren).map_err(wort)?;

    let dateien = stapel(pfade, &fortschritt, |p| {
        let quelle = Path::new(p);
        let name = dateiname(p);

        let roh = match std::fs::read(quelle) {
            Ok(d) => d,
            Err(e) => return cabrik_app::versand_fehler(p, e.to_string()),
        };

        // Wer das Original verschicken will, bekommt das Original --
        // das war ja gerade die Entscheidung.
        let (nutzdaten, befund) = if original.iter().any(|o| o == p) {
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
            Err(e) => return cabrik_app::versand_fehler(p, e.meldung),
        };

        let ziel = freier_name(&quelle.with_file_name(cabrik_app::envelope_name(&name)));
        match cabrik_ablage::schreib_neu(&ziel, &envelope) {
            Ok(()) => Versandergebnis {
                quelle: p.to_owned(),
                ziel: Some(ziel.display().to_string()),
                bytes: envelope.len(),
                befund,
                fehler: None,
            },
            Err(e) => cabrik_app::versand_fehler(p, e.meldung),
        }
    });

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
    // Die alten Endungen stehen mit im Filter. Wer vor dem Wechsel von
    // `.cab` schon Envelopes liegen hatte, soll sie weiter mit einem Griff
    // finden -- erkannt werden sie ohnehin an den Magic-Bytes, nicht am
    // Namen.
    let mut endungen = vec![envelope::ENDUNG];
    endungen.extend_from_slice(envelope::ALTE_ENDUNGEN);

    app.dialog()
        .file()
        .add_filter("Cabrik-Envelope", &endungen)
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

/// Verschlüsselt einen Text und gibt ihn zum Kopieren zurück.
///
/// # Der Text ist ein Geheimnis, und er kommt aus der Webansicht
///
/// Wie das Passwort. Er wird hier sofort in `Zeroizing` gefasst; die Kopien
/// davor — die JavaScript-Zeichenkette und der Übergabepuffer — lassen sich
/// nicht überschreiben. Das ist dieselbe Lücke wie bei der
/// Passworteingabe und wird mit demselben Schritt geschlossen: ein natives
/// Fenster in Phase 5.
///
/// Der Aufrufer leert sein Eingabefeld, sobald der Envelope da ist.
#[tauri::command]
fn text_verschluesseln(
    zustand: State<'_, Zustand>,
    text: String,
    empfaenger: Vec<String>,
    signieren: bool,
) -> Result<String, String> {
    let geschuetzt = Zeroizing::new(text);
    let mut z = sperre(&zustand)?;
    let offen = sitzung(&mut z)?.offen(jetzt()).map_err(wort)?;
    let plan = offen.versand_planen(&empfaenger, signieren).map_err(wort)?;
    offen
        .text_verschluesseln(&plan, &geschuetzt, &mut OsRandom)
        .map_err(wort)
}

/// Öffnet einen eingefügten Armor-Text.
#[tauri::command]
fn text_oeffnen(
    zustand: State<'_, Zustand>,
    text: String,
    signatur_verlangt: bool,
) -> Result<Geoeffnet, String> {
    let mut z = sperre(&zustand)?;
    sitzung(&mut z)?
        .offen(jetzt())
        .map_err(wort)?
        .text_oeffnen(&text, signatur_verlangt)
        .map_err(wort)
}

// ---------------------------------------------------------------------------
// Die eigene Austausch-Nutzlast
// ---------------------------------------------------------------------------

/// Die eigene Austausch-Nutzlast — zum Weitergeben.
///
/// **Ausschließlich öffentliche Angaben.** Sie darf über jeden Weg gehen;
/// der Weg entscheidet allerdings nichts über Echtheit — dafür ist der
/// Fingerprint-Vergleich da.
#[tauri::command]
fn eigene_nutzlast(zustand: State<'_, Zustand>) -> Result<String, String> {
    let mut z = sperre(&zustand)?;
    sitzung(&mut z)?
        .offen(jetzt())
        .map_err(wort)?
        .eigene_nutzlast()
        .map_err(wort)
}

/// Die eigene Austausch-Nutzlast als QR-Code.
///
/// Der Code wird **groß**: Von rund 2070 Zeichen sind 1946 der
/// Post-Quantum-Schlüssel. 141 Module Kantenlänge statt 41 ohne ihn. Die
/// Anzeige braucht deshalb Fläche, und der Weg über Datei oder Text bleibt
/// der bequemere.
#[tauri::command]
fn nutzlast_als_qr(zustand: State<'_, Zustand>) -> Result<QrCode, String> {
    let mut z = sperre(&zustand)?;
    let nutzlast = sitzung(&mut z)?
        .offen(jetzt())
        .map_err(wort)?
        .eigene_nutzlast()
        .map_err(wort)?;
    cabrik_app::qr_code(&nutzlast).map_err(wort)
}

/// Legt die eigene Nutzlast als Textdatei ab. `None` heißt abgebrochen.
///
/// Als `.txt`, nicht als eigene Endung: Wer sie bekommt, soll sie mit dem
/// öffnen können, was er ohnehin hat. Eine eigene Endung nützte nur uns.
#[tauri::command(async)]
fn nutzlast_als_datei(
    zustand: State<'_, Zustand>,
    app: tauri::AppHandle,
) -> Result<Option<String>, String> {
    let nutzlast = {
        let mut z = sperre(&zustand)?;
        sitzung(&mut z)?
            .offen(jetzt())
            .map_err(wort)?
            .eigene_nutzlast()
            .map_err(wort)?
    };

    let Some(ziel) = app
        .dialog()
        .file()
        .set_file_name("cabrik-kontakt.txt")
        .add_filter("Textdatei", &["txt"])
        .blocking_save_file()
        .and_then(|f| f.into_path().ok())
    else {
        return Ok(None);
    };

    let ziel = freier_name(&ziel);
    cabrik_ablage::schreib_neu(&ziel, nutzlast.as_bytes()).map_err(|e| e.meldung)?;
    Ok(Some(ziel.display().to_string()))
}

/// Liest eine Austausch-Nutzlast aus einer Datei. `None` heißt abgebrochen.
///
/// Der Inhalt geht **ungeprüft** zurück: Was drinsteht, beurteilt
/// `nutzlast_lesen` — dieselbe Prüfung wie beim Einfügen von Hand. Zwei
/// Wege herein dürfen nicht zu zwei Urteilen führen.
#[tauri::command(async)]
fn nutzlast_aus_datei(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let Some(pfad) = app
        .dialog()
        .file()
        .add_filter("Textdatei", &["txt"])
        .blocking_pick_file()
        .and_then(|f| f.into_path().ok())
    else {
        return Ok(None);
    };
    std::fs::read_to_string(&pfad)
        .map(Some)
        .map_err(|e| format!("{}: {e}", pfad.display()))
}

// ---------------------------------------------------------------------------
// Sicherung und Passwort
// ---------------------------------------------------------------------------

/// Legt eine Kopie der Schlüsseldatei ab. `None` heißt abgebrochen.
///
/// # Warum das unbedenklich ist
///
/// Die Datei ist mit dem Passwort verschlüsselt. Wer sie findet, hat
/// nichts — außer der Möglichkeit, Passwörter durchzuprobieren, und genau
/// das macht Argon2id teuer.
///
/// # Wogegen sie schützt und wogegen nicht
///
/// Gegen eine kaputte Platte und ein versehentliches Löschen. **Nicht**
/// gegen ein vergessenes Passwort: Die Kopie ist mit demselben
/// verschlossen. Und eine Kopie auf derselben Platte hilft gegen deren
/// Ausfall gar nicht.
#[tauri::command(async)]
fn schluessel_sichern(
    zustand: State<'_, Zustand>,
    app: tauri::AppHandle,
) -> Result<Option<String>, String> {
    let daten = {
        let mut z = sperre(&zustand)?;
        sitzung(&mut z)?.schluesseldatei().to_vec()
    };

    let Some(ziel) = app
        .dialog()
        .file()
        .set_file_name("identity.cabrik-key")
        .blocking_save_file()
        .and_then(|f| f.into_path().ok())
    else {
        return Ok(None);
    };

    let ziel = freier_name(&ziel);
    cabrik_ablage::schreib_neu(&ziel, &daten).map_err(|e| e.meldung)?;
    Ok(Some(ziel.display().to_string()))
}

/// Ändert das Passwort. **Die Identität bleibt dieselbe.**
///
/// # Warum hier überschrieben wird
///
/// Dies ist die eine Stelle, an der das richtig ist: Es ist dieselbe
/// Identität, nur anders verschlossen. Überall sonst weigert sich
/// `cabrik_ablage::schreib_neu` — hier wäre eine zweite Datei daneben der
/// Fehler, denn dann läge die alte Hülle mit dem alten Passwort weiter da.
///
/// Geschrieben wird trotzdem unteilbar: erst daneben, dann umbenennen. Ein
/// Absturz mittendrin darf keine halbe Schlüsseldatei hinterlassen.
///
/// # Was danach zu tun ist
///
/// Alte Sicherungskopien austauschen. Sie öffnen sich weiter mit dem alten
/// Passwort — das ist keine Fehlfunktion, sondern die Natur der Sache.
#[tauri::command]
fn passwort_aendern(
    zustand: State<'_, Zustand>,
    alt: String,
    neu: String,
) -> Result<(), String> {
    let pfad = zustand.schluesselpfad.clone();
    // Beide sofort in `Zeroizing`. Die Kopien davor -- die
    // JavaScript-Zeichenketten und der Übergabepuffer -- lassen sich nicht
    // überschreiben (`spec/entsperrung.md` §5.1).
    let alt = Zeroizing::new(alt);
    let neu = Zeroizing::new(neu);

    let mut z = sperre(&zustand)?;
    let s = sitzung(&mut z)?;
    s.passwort_aendern(&alt, &neu, &mut OsRandom).map_err(wort)?;

    // Erst wenn der Wechsel im Speicher gelungen ist. Andersherum stünde
    // nach einem Fehlschlag eine neue Hülle auf der Platte, zu der die
    // laufende Sitzung nicht passt.
    cabrik_ablage::schreib_atomar(&pfad, s.schluesseldatei()).map_err(|e| e.meldung)
}

// ---------------------------------------------------------------------------
// Sicheres Löschen
// ---------------------------------------------------------------------------

/// Beurteilt, was Löschen bei diesen Dateien **erreicht** — ohne zu löschen.
///
/// # Warum das ein eigener Schritt ist
///
/// Weil die Auskunft vor der Tat kommen muss. Wer erst löscht und dann
/// erfährt, dass Überschreiben auf einer SSD nichts ausrichtet, kann nichts
/// mehr entscheiden — die Datei ist weg, die Kopien im Flash-Speicher
/// nicht.
///
/// Version 1 hatte drei Durchgänge voreingestellt und suggerierte damit
/// einen Nutzen, den es auf heutigen Datenträgern nicht gibt. Dieser
/// Bildschirm sagt stattdessen, was **nicht** erreicht wird.
#[tauri::command(async)]
fn loeschen_beurteilen(
    pfade: Vec<String>,
    fortschritt: Channel<Fortschritt>,
) -> Vec<Loeschkandidat> {
    stapel(pfade, &fortschritt, |p| {
        let pfad = Path::new(p);
        Loeschkandidat {
            name: dateiname(p),
            // `.ok()` und nicht `.unwrap_or(0)`: Eine Datei, die sich nicht
            // ansehen laesst, ist keine leere Datei.
            groesse_bytes: pfad.metadata().map(|m| m.len()).ok(),
            beurteilung: Loeschbeurteilung::from(&cabrik_shred::assess(pfad)),
            pfad: p.to_owned(),
        }
    })
}

/// Löscht die Dateien. **Unwiderruflich.**
///
/// Jede Datei einzeln, und jeder Schritt einzeln gemeldet: überschrieben,
/// umbenannt, entfernt. Ein pauschales „Gelöscht" wie in Version 1 gibt es
/// nicht — es wäre eine Behauptung über drei verschiedene Dinge, von denen
/// jedes einzeln scheitern kann.
///
/// `durchgaenge` wird vom Kern auf das Sinnvolle begrenzt. Mehr als einer
/// bringt auf heutigen Datenträgern nichts; die Wahl steht dem Nutzer
/// trotzdem offen, und der Bildschirm sagt dazu, was sie kostet.
#[tauri::command(async)]
fn loeschen_ausfuehren(
    pfade: Vec<String>,
    durchgaenge: u8,
    fortschritt: Channel<Fortschritt>,
) -> Vec<Loeschergebnis> {
    let opts = cabrik_shred::ShredOptions {
        passes: durchgaenge,
        // Der Dateiname bliebe sonst im MFT stehen -- und er allein kann
        // verräterisch genug sein.
        rename: true,
    };
    // Der langsamste Stapel des Programms: Ueberschreiben kostet Zeit, und
    // mit mehreren Durchgaengen ein Vielfaches davon. Ohne Fortschritt sass
    // man vor einem Fenster, das nichts tat -- bei einem Vorgang, der
    // unwiderruflich ist.
    stapel(pfade, &fortschritt, |p| {
        Loeschergebnis::from(&cabrik_shred::shred_file(Path::new(p), &opts))
    })
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

/// Was beim Start herauskommt: eine Sitzung, keine — oder ein Fehler, den
/// das Fenster **anzeigen** soll, statt ihn auf eine fehlende Konsole zu
/// schreiben.
struct Startlage {
    sitzung: Option<Sitzung>,
    schluesselpfad: PathBuf,
    kontaktpfad: PathBuf,
}

/// Liest, was auf der Platte liegt — **ohne je abzubrechen**.
///
/// # Warum diese Funktion nichts ausgibt und nichts beendet
///
/// Weil `eprintln!` hier ins Leere schriebe. Das Fenster läuft unter
/// Windows mit `windows_subsystem = "windows"` und hat keine Konsole: Wer
/// Cabrik doppelklickt und dessen Schlüsseldatei beschädigt ist, sah bis
/// hierher **gar nichts**. Kein Fenster, keine Meldung.
///
/// Version 1 stürzte in dieser Lage mit einem Traceback ab — schlecht, aber
/// sichtbar. Stillschweigend nicht zu starten ist schlechter.
///
/// Also wird der Fehler zurückgegeben, das Fenster geht auf, und die
/// Oberfläche sagt, was los ist und wo die Datei liegt.
fn startlage() -> Result<Startlage, Startfehler> {
    // Beide Dateien liegen dort, wo auch die CLI sie sucht -- dieselbe
    // Schicht bestimmt den Pfad. Zwei Umsetzungen liefen auseinander, und
    // dann schriebe die eine, wo die andere nicht liest.
    let (schluesselpfad, kontaktpfad) = match (
        cabrik_ablage::keyfile_pfad(None),
        cabrik_ablage::kontakte_pfad(None),
    ) {
        (Ok(k), Ok(c)) => (k, c),
        _ => {
            return Err(Startfehler {
                meldung: "Cabrik hat auf diesem Rechner kein Verzeichnis \
                          gefunden, in dem es seine Schlüsseldatei ablegen \
                          darf."
                    .to_owned(),
                pfad: None,
                rat: "Das liegt fast immer an einem eingeschränkten oder \
                      beschädigten Benutzerprofil. Melden Sie sich neu an; \
                      hilft das nicht, fragen Sie Ihre Systemverwaltung nach \
                      dem Zugriff auf das Anwendungsdatenverzeichnis."
                    .to_owned(),
            });
        }
    };

    lade(schluesselpfad, kontaktpfad)
}

/// Liest die beiden Dateien — der Teil, der sich ohne Fenster prüfen lässt.
///
/// Getrennt von [`startlage`], weil dort das Konfigurationsverzeichnis des
/// laufenden Systems bestimmt wird und ein Test dagegen nichts ausrichtet.
/// Was hier steht, ist die eigentliche Entscheidung: Was ist ein Fehler,
/// was ist der Normalfall, und was sagt man dazu.
fn lade(schluesselpfad: PathBuf, kontaktpfad: PathBuf) -> Result<Startlage, Startfehler> {
    // Ohne Schlüsseldatei bleibt die Sitzung `None`. Das ist NICHT dasselbe
    // wie gesperrt: Der Weg führt dann zur Einrichtung, nicht zum
    // Passwortfeld.
    let sitzung = match cabrik_ablage::lies(&schluesselpfad) {
        Ok(Some(schluessel)) => {
            // `lies` unterscheidet sauber: `Ok(None)` heißt „gibt es
            // nicht", `Err` heißt „liegt da, geht aber nicht auf". Beides
            // mit `.ok().flatten()` zusammenzuwerfen war ein stiller
            // Datenverlust: Die Sitzung startete mit einem LEEREN
            // Verzeichnis, das Entsperren gelang, alle Kontakte waren fort
            // -- und die erste Änderung schrieb die unlesbare Datei
            // einfach nieder. Danach war sie es tatsächlich.
            let kontakte = match cabrik_ablage::lies(&kontaktpfad) {
                // Beim ersten Start der Normalfall.
                Ok(k) => k,
                Err(e) => {
                    return Err(Startfehler {
                        meldung: format!(
                            "Die Kontaktdatei ließ sich nicht lesen: {}",
                            e.meldung
                        ),
                        pfad: Some(kontaktpfad.display().to_string()),
                        rat: "Löschen Sie die Datei nicht — solange sie da \
                              ist, sind Ihre Kontakte nicht verloren. Legen \
                              Sie sie beiseite; Cabrik startet dann mit einem \
                              leeren Verzeichnis, und Ihre Identität bleibt \
                              davon unberührt. Empfangen und Entschlüsseln \
                              funktionieren auch ohne Kontakte — nur die \
                              Zuordnung des Absenders fehlt dann."
                            .to_owned(),
                    });
                }
            };
            Some(Sitzung::neu(schluessel, kontakte, Sperrfrist::default()))
        }
        Ok(None) => None,
        Err(e) => {
            return Err(Startfehler {
                meldung: format!("Die Schlüsseldatei ließ sich nicht lesen: {}", e.meldung),
                // Der Pfad ist hier die eigentliche Auskunft. Ohne ihn sucht
                // jemand an der falschen Stelle -- und bei einer
                // Schlüsseldatei ist die falsche Stelle teuer.
                pfad: Some(schluesselpfad.display().to_string()),
                rat: "Legen Sie die Datei beiseite, statt sie zu löschen — \
                      solange sie da ist, ist nichts endgültig verloren. \
                      Haben Sie eine Sicherungskopie, kopieren Sie diese an \
                      dieselbe Stelle. Danach Cabrik neu starten."
                    .to_owned(),
            });
        }
    };

    Ok(Startlage {
        sitzung,
        schluesselpfad,
        kontaktpfad,
    })
}

fn main() -> std::process::ExitCode {
    let (lage, fehler_beim_start) = match startlage() {
        Ok(l) => (l, None),
        // Kein Abbruch. Das Fenster geht auf und sagt, was los ist -- die
        // Pfade sind dann Platzhalter, denn ohne sie kommt man ohnehin
        // nicht weiter.
        Err(f) => (
            Startlage {
                sitzung: None,
                schluesselpfad: PathBuf::new(),
                kontaktpfad: PathBuf::new(),
            },
            Some(f),
        ),
    };
    let Startlage {
        sitzung,
        schluesselpfad,
        kontaktpfad,
    } = lage;

    // Was das Betriebssystem beim Start hereingereicht hat -- der Doppelklick
    // im Explorer landet als Befehlszeilenargument.
    let beim_start = datei_aus_argumenten(std::env::args_os().skip(1));

    let lauf = tauri::Builder::default()
        // Die Einmaligkeitssperre MUSS als erstes Plugin stehen: Sie
        // entscheidet, ob dieser Prozess überhaupt weiterläuft.
        .plugin(tauri_plugin_single_instance::init(|app, argumente, _ordner| {
            // Ein zweiter Doppelklick, während das Fenster schon steht.
            // Der Pfad wandert in den laufenden Prozess, dieser hier endet.
            if let Some(pfad) = datei_aus_argumenten(argumente.into_iter().skip(1))
                && let Some(z) = app.try_state::<Zustand>()
                && let Ok(mut fach) = z.hereingereicht.lock()
            {
                *fach = Some(pfad);
            }
            // Nach vorn holen. Ohne das öffnete sich scheinbar nichts --
            // das Fenster stünde hinter dem Explorer.
            if let Some(f) = app.get_webview_window("main") {
                let _ = f.unminimize();
                let _ = f.set_focus();
            }
            // Die Oberfläche fragt nach. Dieses Ereignis ist nur der Anstoß;
            // den Wert gibt es allein über `datei_abholen`, damit es nicht
            // zwei Wege zu derselben Auskunft gibt.
            let _ = app.emit("datei-hereingereicht", ());
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            app.manage(Zustand {
                sitzung: Mutex::new(sitzung),
                schluesselpfad,
                kontaktpfad,
                hereingereicht: Mutex::new(beim_start),
                startfehler: fehler_beim_start,
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
            text_verschluesseln,
            text_oeffnen,
            eigene_nutzlast,
            nutzlast_als_datei,
            nutzlast_als_qr,
            nutzlast_aus_datei,
            schluessel_sichern,
            passwort_aendern,
            loeschen_beurteilen,
            loeschen_ausfuehren,
            kontakte,
            nutzlast_lesen,
            kontakt_aufnehmen,
            kontakt_verifizieren,
            kontakt_zuruecksetzen,
            kontakt_widerrufen,
            kontakt_loeschen,
            datei_abholen,
            startfehler,
        ])
        .run(tauri::generate_context!());

    // Der letzte Fall, und der einzige, für den ein Meldungsfenster das
    // Richtige ist: Das Fenster selbst geht nicht auf. Alles andere lässt
    // sich IM Fenster sagen -- das hier nicht.
    //
    // Kein `eprintln!`: Unter `windows_subsystem = "windows"` gibt es keine
    // Konsole. Wer Cabrik doppelklickt und dessen WebView2-Laufzeit fehlt,
    // sah bis hierher gar nichts -- kein Fenster, keine Meldung, nur einen
    // Prozess, der sofort wieder verschwindet.
    //
    // Kein `expect`: Eine Panik wäre genauso unsichtbar.
    if let Err(e) = lauf {
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("Cabrik Secure lässt sich nicht starten")
            .set_description(format!(
                "Das Programmfenster ließ sich nicht öffnen.\n\n\
                 {e}\n\n\
                 Unter Windows fehlt in diesem Fall meist die \
                 WebView2-Laufzeit. Sie lässt sich bei Microsoft kostenlos \
                 nachinstallieren; danach startet Cabrik wieder.\n\n\
                 Ihre Schlüsseldatei ist davon nicht betroffen — sie liegt \
                 unverändert an ihrem Platz."
            ))
            .show();
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}

// ---------------------------------------------------------------------------
// Prüfungen
// ---------------------------------------------------------------------------

/// Was beim Start schiefgehen kann — und was dann dasteht.
///
/// # Warum das hier geprüft wird und nicht in `cabrik-app`
///
/// Weil es hier passiert. `cabrik-app` fasst kein Dateisystem an; das
/// Lesen der beiden Dateien beim Start ist die Aufgabe dieser Schicht, und
/// damit auch die Entscheidung, was ein Fehler ist und was der Normalfall.
#[cfg(test)]
mod pruefungen {
    #![expect(
        clippy::expect_used,
        clippy::panic,
        reason = "Fehlschlag soll den Test abbrechen"
    )]

    use super::{datei_aus_argumenten, lade};
    use std::path::PathBuf;

    /// Ein eigener Ordner je Test — sonst sehen sie einander.
    fn ordner(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cabrik-start-{name}"));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("Ordner");
        p
    }

    #[test]
    fn ohne_dateien_gibt_es_keine_sitzung_und_keinen_fehler() {
        // Der allererste Start. Der Weg fuehrt zur Einrichtung, und das ist
        // KEIN Fehler -- verwechselte man beides, staende jemandem ohne
        // Schluessel ein Passwortfeld gegenueber.
        let o = ordner("leer");

        let Ok(lage) = lade(o.join("id.key"), o.join("kontakte")) else {
            panic!("ein leerer Ordner ist kein Fehler");
        };

        assert!(lage.sitzung.is_none());
    }

    #[test]
    fn eine_unlesbare_schluesseldatei_nennt_ihren_pfad() {
        // Der v1-Fall. Dort gab es einen Traceback; hier muss ein Satz mit
        // dem Pfad herauskommen -- ohne ihn sucht jemand an der falschen
        // Stelle.
        let o = ordner("schluessel-kaputt");
        let pfad = o.join("id.key");
        // Ein Verzeichnis dort, wo eine Datei erwartet wird: laesst sich
        // auf jedem System herstellen und ist zuverlaessig unlesbar.
        std::fs::create_dir(&pfad).expect("Verzeichnis");

        let Err(fehler) = lade(pfad.clone(), o.join("kontakte")) else {
            panic!("eine unlesbare Schlüsseldatei muss scheitern");
        };

        assert!(fehler.meldung.contains("Schlüsseldatei"), "{}", fehler.meldung);
        assert_eq!(fehler.pfad.as_deref(), Some(pfad.display().to_string().as_str()));
    }

    #[test]
    fn eine_unlesbare_kontaktdatei_ist_ein_fehler_und_kein_leeres_verzeichnis() {
        /*
         * Der stille Datenverlust, den dieser Durchgang gefunden hat.
         *
         * Vorher stand hier `.ok().flatten()`: Eine unlesbare Kontaktdatei
         * wurde damit zu „keine Kontaktdatei". Das Entsperren gelang, das
         * Verzeichnis war leer, alle Verifikationen schienen fort -- und
         * die erste Aenderung schrieb die Datei einfach nieder. Danach
         * waren sie es tatsaechlich.
         */
        let o = ordner("kontakte-kaputt");
        let schluessel = o.join("id.key");
        std::fs::write(&schluessel, b"irgendwas").expect("schreiben");
        let kontakte = o.join("kontakte");
        std::fs::create_dir(&kontakte).expect("Verzeichnis");

        let Err(fehler) = lade(schluessel, kontakte.clone()) else {
            panic!("eine unlesbare Kontaktdatei muss scheitern");
        };

        assert!(fehler.meldung.contains("Kontaktdatei"), "{}", fehler.meldung);
        assert_eq!(
            fehler.pfad.as_deref(),
            Some(kontakte.display().to_string().as_str())
        );
    }

    #[test]
    fn der_rat_zum_kontaktspeicher_raet_nicht_zum_loeschen() {
        // Der teuerste Rat, den man hier geben koennte. Solange die Datei
        // da ist, sind die Kontakte nicht verloren.
        let o = ordner("kontakte-rat");
        std::fs::write(o.join("id.key"), b"irgendwas").expect("schreiben");
        let kontakte = o.join("kontakte");
        std::fs::create_dir(&kontakte).expect("Verzeichnis");

        let Err(fehler) = lade(o.join("id.key"), kontakte) else {
            panic!("eine unlesbare Kontaktdatei muss scheitern");
        };

        assert!(fehler.rat.contains("nicht"), "{}", fehler.rat);
        assert!(
            fehler.rat.to_lowercase().contains("beiseite"),
            "der Rat muss einen Schritt nennen: {}",
            fehler.rat
        );
    }

    #[test]
    fn eine_fehlende_kontaktdatei_ist_der_normalfall() {
        // Die Gegenprobe. Waere auch das ein Fehler, kaeme niemand ueber
        // den ersten Start hinaus.
        let o = ordner("kontakte-fehlen");
        std::fs::write(o.join("id.key"), b"irgendwas").expect("schreiben");

        let Ok(lage) = lade(o.join("id.key"), o.join("gibt-es-nicht")) else {
            panic!("eine fehlende Kontaktdatei ist kein Fehler");
        };

        assert!(lage.sitzung.is_some());
    }

    // -----------------------------------------------------------------------

    /// Ein Windows-Pfad, wie ihn der Explorer uebergibt.
    const PFAD: &str = r"C:\Post\bericht.pdf.cabrik";

    #[test]
    fn der_doppelklick_liefert_den_pfad() {
        let gefunden = datei_aus_argumenten([PFAD]);

        assert_eq!(gefunden.as_deref(), Some(PFAD));
    }

    #[test]
    fn schalter_werden_nicht_fuer_dateien_gehalten() {
        // Tauri und WebView2 reichen unter Windows eigene Schalter durch.
        // Einen davon fuer einen Pfad zu halten, oeffnete Unsinn.
        let gefunden =
            datei_aus_argumenten(["--webview-exe-name=cabrik.exe", "--flag", PFAD]);

        assert_eq!(gefunden.as_deref(), Some(PFAD));
    }

    #[test]
    fn ohne_argumente_kommt_nichts() {
        assert!(datei_aus_argumenten(Vec::<String>::new()).is_none());
    }

    #[test]
    fn nur_die_erste_datei_zaehlt() {
        // Das Fenster zeigt einen Envelope zur Zeit. Mehrere anzunehmen und
        // vier stillschweigend fallenzulassen waere schlechter.
        let gefunden = datei_aus_argumenten(["eins.cabrik", "zwei.cabrik"]);

        assert_eq!(gefunden.as_deref(), Some("eins.cabrik"));
    }
}
