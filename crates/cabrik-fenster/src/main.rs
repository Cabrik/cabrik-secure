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

use cabrik_ablage::Zwischendatei;
use cabrik_app::{Betroffen, Einspielung, Sitzung, Versandplan};
use cabrik_bruecke::{
    Abgelegt, Bereinigung, Eingespielt, Entsperrbedarf, Fortschritt, Geoeffnet, Identitaet,
    KdfStufe, Kontakt, Kontaktherkunft, Loeschbeurteilung, Loeschergebnis, Loeschkandidat,
    Nutzlastbefund, Passwortweg, QrCode, Ruheschutz, Schritt, Sendedatei, Sicherung, Sitzungsstand,
    Speicherergebnis, Sperrfrist, Startfehler, Verifikationsweg, Versandbericht, Versandergebnis,
    Vertrauen,
};
use cabrik_core::envelope::{self, Ablage, Oeffnungsfehler};
use cabrik_core::{OsRandom, Speichergrenze};
use cabrik_speicher::eingabe;
use cabrik_speicher::ruhezustand::{self, Meldung};
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
    /// Ob vor dem Einschlafen gesperrt wird — und mit welcher Zusage.
    ///
    /// Kein `Mutex`: Der Wert entsteht beim Start und ändert sich nie.
    ruheschutz: Ruheschutz,
}

/// Was geschieht, wenn das Betriebssystem sich meldet.
///
/// **Eine eigene Funktion, damit sie geprüft werden kann.** Der Rückruf
/// selbst kommt nur, wenn ein Rechner tatsächlich einschläft — das nimmt
/// ein Test sich nicht heraus. Die Entscheidung dahinter lässt sich
/// dagegen sehr wohl prüfen, und sie ist der Teil, in dem ein Fehler
/// unbemerkt bliebe.
fn auf_meldung(meldung: Meldung, fach: &Mutex<Option<Sitzung>>) {
    // NUR das Einschlafen. `WiederWach` entsperrt ausdrücklich nicht --
    // wer aufwacht, gibt sein Passwort neu ein --, und `Belanglos` heißt,
    // dass jemand das Netzteil gezogen hat. Beides wäre ein schlechter
    // Grund, jemanden mitten im Satz auszusperren.
    if meldung != Meldung::LegtSichSchlafen {
        return;
    }
    // Ein vergifteter Mutex wird hier NICHT beklagt, sondern übergangen:
    // Es gibt in diesem Augenblick niemanden, dem man es sagen könnte, und
    // gleich ist der Rechner aus.
    if let Ok(mut z) = fach.lock()
        && let Some(s) = z.as_mut()
    {
        s.sperren();
    }
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

/// Wie viel Arbeitsspeicher eine Passwortableitung gerade belegen darf
/// (`spec/keyfile-v2.md` §4.2).
///
/// **Bei jedem Aufruf neu gefragt**, nicht einmal beim Start: Was frei ist,
/// ändert sich, sobald jemand ein anderes Programm öffnet oder schließt —
/// und genau dazu rät die Meldung, wenn es nicht reicht.
fn ableitungsgrenze() -> Speichergrenze {
    cabrik_speicher::ableitungsgrenze().map_or(Speichergrenze::UNBEKANNT, Speichergrenze::bytes)
}

/// Was ein fehlgeschlagenes Entsperren sagt — für beide Wege dasselbe.
///
/// Stand bis zum 26.10.2026 zweimal da, wortgleich, und in beiden
/// Abschriften war die Zeilenfortsetzung verloren gegangen: Statt
/// `die \` mit Umbruch standen achtzehn Leerzeichen im Satz und zwei echte
/// Zeilenumbrüche im Quelltext. Zu sehen war es nicht — HTML zieht
/// Leerraum zusammen —, und genau deshalb fiel es niemandem auf.
fn entsperrfehler(e: cabrik_app::Befehlsfehler, kontaktpfad: &Path) -> String {
    match e.betrifft {
        // Der Pfad gehört in die Meldung: Sonst säße jemand mit richtigem
        // Passwort vor einer verschlossenen Tür und wüsste nicht, welche
        // Datei im Weg liegt. Die Sitzungsschicht kennt ihn nicht -- sie
        // sieht Bytes.
        Betroffen::Kontaktspeicher => format!(
            "{} Sie können die Datei umbenennen oder wegräumen; die \
             Identität selbst ist davon nicht betroffen.\n\n{}",
            e.meldung,
            kontaktpfad.display()
        ),
        _ => e.meldung,
    }
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
    mut je: impl FnMut(&str, &Melder<'_>) -> T,
) -> Vec<T> {
    let gesamt = pfade.len();
    pfade
        .iter()
        .enumerate()
        .map(|(erledigt, p)| {
            let melder = Melder {
                kanal: melden,
                erledigt,
                gesamt,
                name: dateiname(p),
                zuletzt: std::cell::Cell::new(0),
            };
            // Die erste Meldung noch VOR der Arbeit: Sonst stünde beim
            // Beginn einer großen Datei sekundenlang der vorige Name da.
            melder.schritt(Schritt::Arbeiten);
            je(p, &melder)
        })
        .collect()
}

/// Meldet, was mit **dieser** Datei gerade geschieht.
///
/// # Warum es das braucht
///
/// Weil `stapel` nur die Datei zählt und der lange Teil **innerhalb**
/// einer Datei liegt. Bei einer 3-GB-Datei stünde sonst minutenlang
/// „1 von 1" da, und das sieht aus wie ein hängendes Programm.
///
/// Vier Dinge dauern dort nacheinander lange — Lesen, Bereinigen,
/// Verschlüsseln, Schreiben —, und keines davon überwiegt so deutlich,
/// dass man die anderen weglassen könnte.
struct Melder<'a> {
    kanal: &'a Channel<Fortschritt>,
    erledigt: usize,
    gesamt: usize,
    name: String,
    /// Wie viele Bytes bei der letzten Bytemeldung dastanden.
    ///
    /// `Cell`, weil [`Melder::schritt`] nur eine geteilte Referenz
    /// bekommt: Der Melder wird durch den Verschluss gereicht, und ihn
    /// veränderlich zu machen zöge sich durch jeden Aufrufer.
    zuletzt: std::cell::Cell<u64>,
}

/// Ab wie vielen neuen Bytes wieder gemeldet wird.
///
/// # Warum überhaupt gedrosselt wird
///
/// Weil jede Meldung über die Brücke geht. Bei einer 3-GB-Datei und
/// Blöcken von 64 KiB wären das rund **48 000 Nachrichten** — die
/// Oberfläche käme mit dem Zeichnen nicht nach, und ein Balken, der die
/// Anzeige lahmlegt, ist schlechter als keiner.
///
/// Vier MiB ergeben bei 3 GB rund 750 Meldungen. Das ist alle paar
/// Zehntelsekunden eine, und feiner kann ein Mensch es ohnehin nicht
/// lesen.
const MELDEABSTAND: u64 = 4 * 1024 * 1024;

impl<'a> Melder<'a> {
    /// Ein Melder für eine einzelne Datei, ohne Stapel drumherum — beim
    /// Öffnen und Ablegen eines Envelopes.
    fn einzeln(kanal: &'a Channel<Fortschritt>, pfad: &str) -> Self {
        Self {
            kanal,
            erledigt: 0,
            gesamt: 1,
            name: dateiname(pfad),
            zuletzt: std::cell::Cell::new(0),
        }
    }

    /// Sagt, welcher Schritt jetzt beginnt.
    ///
    /// Ein Fehlschlag beim Senden wird verworfen: Der Kanal gehört zu
    /// einem Aufruf, und wenn die Oberfläche ihn nicht mehr hört, ist der
    /// Vorgang trotzdem zu Ende zu bringen. Abzubrechen, weil niemand
    /// zusieht, wäre die schlechtere Antwort.
    fn schritt(&self, schritt: Schritt) {
        // Der Zähler beginnt bei jedem Schritt von vorn: Die Bytes des
        // Lesens haben mit denen des Schreibens nichts zu tun.
        self.zuletzt.set(0);
        self.sende(schritt, None, None);
    }

    /// Sagt, wie weit es **innerhalb** dieser Datei ist.
    ///
    /// Gedrosselt — siehe [`MELDEABSTAND`]. Die **letzte** Meldung geht
    /// immer durch: Ohne sie bliebe der Balken kurz vor dem Ende stehen,
    /// und genau das lässt jemanden glauben, das Programm hänge.
    fn bytes(&self, schritt: Schritt, fertig: u64, gesamt: u64) {
        let am_ende = fertig >= gesamt;
        if !am_ende && fertig.saturating_sub(self.zuletzt.get()) < MELDEABSTAND {
            return;
        }
        self.zuletzt.set(fertig);
        self.sende(schritt, Some(fertig), Some(gesamt));
    }

    /// Ein Fehlschlag beim Senden wird verworfen: Der Kanal gehört zu
    /// einem Aufruf, und wenn die Oberfläche ihn nicht mehr hört, ist der
    /// Vorgang trotzdem zu Ende zu bringen. Abzubrechen, weil niemand
    /// zusieht, wäre die schlechtere Antwort.
    fn sende(&self, schritt: Schritt, bytes_erledigt: Option<u64>, bytes_gesamt: Option<u64>) {
        let _ = self.kanal.send(Fortschritt {
            erledigt: self.erledigt,
            gesamt: self.gesamt,
            laeuft: self.name.clone(),
            schritt,
            bytes_erledigt,
            bytes_gesamt,
        });
    }
}

/// Schreibt eine Datei und meldet dabei, wie weit sie ist.
///
/// Die Drosselung sitzt **hier** und nicht in `cabrik-ablage`: Jene
/// Schicht weiß nicht, wohin die Meldung geht — über eine Brücke zur
/// Oberfläche oder in eine Zählvariable. Wer sie dort einbaute, zwänge
/// jedem Aufrufer denselben Abstand auf.
fn schreib_gemeldet(
    ziel: &Path,
    daten: &[u8],
    melder: &Melder<'_>,
) -> Result<(), cabrik_ablage::Ablagefehler> {
    cabrik_ablage::schreib_neu_gemeldet(ziel, daten, &mut |fertig, gesamt| {
        melder.bytes(Schritt::Schreiben, fertig, gesamt);
    })
}

/// Liest eine Datei und meldet dabei, wie weit sie ist.
///
/// # Warum nicht `fs::read`
///
/// Weil das eine einzige Zeile ist, die Minuten dauern kann. Bei einer
/// 3-GB-Datei auf einer langsamen Platte stünde der Balken die ganze Zeit
/// auf „Lese urlaub.mp4" und rührte sich nicht.
///
/// # Was das **nicht** behebt
///
/// Den Arbeitsspeicher. Die Datei landet weiterhin vollständig im
/// Speicher, und danach noch einmal als bereinigte Fassung und als
/// Envelope. Echtes Strömen von der Platte wäre ein eigener Umbau; hier
/// geht es allein darum, dass der Balken sich bewegt.
fn lies_gemeldet(pfad: &Path, melder: &Melder<'_>) -> std::io::Result<Vec<u8>> {
    use std::io::Read as _;

    let datei = std::fs::File::open(pfad)?;
    let gesamt = datei.metadata()?.len();

    let mut leser = std::io::BufReader::new(datei);
    let mut alles = Vec::with_capacity(usize::try_from(gesamt).unwrap_or(0));
    let mut block = vec![0_u8; 1024 * 1024];

    loop {
        let gelesen = leser.read(&mut block)?;
        if gelesen == 0 {
            break;
        }
        let Some(stueck) = block.get(..gelesen) else {
            break;
        };
        alles.extend_from_slice(stueck);
        melder.bytes(Schritt::Lesen, alles.len() as u64, gesamt);
    }

    // Die Dateigröße stammt aus den Metadaten und kann sich zwischen dem
    // Abfragen und dem Lesen geändert haben -- jemand schreibt weiter,
    // während wir lesen. Gemeldet wird deshalb zum Schluss die tatsächlich
    // gelesene Menge, nicht die erwartete.
    let gelesen = alles.len() as u64;
    melder.bytes(Schritt::Lesen, gelesen, gelesen);
    Ok(alles)
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
fn datei_aus_argumenten(
    argumente: impl IntoIterator<Item = impl Into<OsString>>,
) -> Option<String> {
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

/// Ob vor Bereitschaft und Ruhezustand gesperrt wird.
///
/// Die Oberfläche fragt das **einmal**, nicht im Sekundentakt: Der Wert
/// entsteht beim Start und ändert sich nie. Ihn an den Sitzungsstand zu
/// hängen hieße, ihn sechzigmal je Minute für nichts zu senden.
#[tauri::command]
fn ruheschutz(zustand: State<'_, Zustand>) -> Ruheschutz {
    zustand.ruheschutz.clone()
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
        .entsperren(geschuetzt.as_bytes(), jetzt(), ableitungsgrenze())
        .map_err(|e| entsperrfehler(e, &kontaktpfad))
}

/// Was das Entsperren kosten wird — gefragt, **bevor** jemand ein Passwort
/// tippt (`spec/entsperrung.md` §4.5).
///
/// `None` heißt wie bei [`sitzungsstand`]: Auf diesem Rechner liegt keine
/// Identität.
#[tauri::command]
fn entsperrbedarf(zustand: State<'_, Zustand>) -> Result<Option<Entsperrbedarf>, String> {
    let z = sperre(&zustand)?;
    z.as_ref()
        .map(|s| s.entsperrbedarf(ableitungsgrenze()).map_err(wort))
        .transpose()
}

/// Die Antwort auf die Rückfrage: ungewöhnliche Ableitungswerte zulassen.
///
/// Gilt, bis das Programm endet — nicht nur bis zur nächsten Sperre.
#[tauri::command]
fn ungewoehnliche_ableitung_zulassen(zustand: State<'_, Zustand>) -> Result<(), String> {
    let mut z = sperre(&zustand)?;
    sitzung(&mut z)?.ungewoehnliche_ableitung_zulassen();
    Ok(())
}

/// Auf welchem Weg das Passwort ins Programm kommt.
///
/// Die Oberfläche fragt das **einmal**: Der Wert hängt am Betriebssystem
/// und ändert sich zur Laufzeit nicht.
///
/// # Warum gefragt und nicht geraten
///
/// Weil „Windows“ nicht dasselbe ist wie „hier geht ein Fenster auf“: Die
/// Fensterklasse kann sich nicht anmelden lassen. Gefragt wird deshalb
/// die Kiste selbst — und zwar **ohne** ein Fenster zu zeigen. Eines
/// aufzumachen und gleich wieder zu schließen wäre die gründlichere Probe
/// und die schlechtere: Es blitzte kurz auf, und niemand hätte es
/// gewollt.
#[tauri::command(async)]
fn passwortweg() -> Passwortweg {
    match eingabe::moeglich() {
        Ok(()) => Passwortweg::EigenesFenster,
        Err(f) => Passwortweg::Webansicht { grund: f.grund },
    }
}

/// Entsperrt über das **eigene Fenster**.
///
/// # Was das besser macht
///
/// Die Zeichen gehen unmittelbar in einen festgenagelten Puffer. Die
/// beiden Kopien, die bei der Webansicht entstehen — die
/// JavaScript-Zeichenkette und der Übergabepuffer — entfallen ersatzlos
/// (`spec/entsperrung.md` §5.2).
///
/// # Der Rückgabewert
///
/// `false` heißt **abgebrochen**, nicht „falsch". Wer das Fenster
/// schließt, hat sich entschieden; eine Fehlermeldung darüber wäre eine
/// Störung ohne Vorfall.
///
/// `(async)` ist zwingend: Der Aufruf blockiert, bis der Nutzer fertig
/// ist.
#[tauri::command(async)]
fn entsperren_nativ(app: tauri::AppHandle, zustand: State<'_, Zustand>) -> Result<bool, String> {
    let kontaktpfad = zustand.kontaktpfad.clone();

    // Das Hauptfenster als Besitzer. Ohne ihn kann das Passwortfeld
    // dahinter erscheinen — Windows hält ein besessenes Fenster immer über
    // seinem Besitzer —, und man könnte nebenher weiterklicken.
    //
    // `None` ist kein Hindernis: Dann geht das Feld eben ohne Besitzer
    // auf. Den ganzen Vorgang daran scheitern zu lassen, wäre die
    // schlechtere Antwort.
    let besitzer = besitzerfenster(&app);

    // Erst fragen, ob ueberhaupt entsperrt werden DARF -- ungewoehnliche
    // Werte ohne Zulassung, oder zu wenig Speicher. Steht das Ergebnis
    // schon fest, soll niemand erst sein Passwort tippen
    // (`spec/entsperrung.md` §4.5). Die Sperre gilt nur fuer diesen Block:
    // Waehrend das Fenster offen steht, muessen andere Befehle durchkommen.
    {
        let mut z = sperre(&zustand)?;
        sitzung(&mut z)?
            .entsperren_moeglich(ableitungsgrenze())
            .map_err(wort)?;
    }

    // Das Fenster VOR der Sperre aufmachen: Es steht, solange jemand
    // tippt, und die Sperre so lange zu halten hiesse, jeden anderen
    // Befehl mit warten zu lassen.
    let antwort = eingabe::abfragen("Passwort für diesen Schlüssel", PASSWORTLAENGE, besitzer)
        .map_err(|f| f.grund)?;

    let eingabe::Antwort::Eingegeben(puffer) = antwort else {
        return Ok(false);
    };

    let mut z = sperre(&zustand)?;
    sitzung(&mut z)?
        .entsperren(puffer.als_bytes(), jetzt(), ableitungsgrenze())
        .map_err(|e| entsperrfehler(e, &kontaktpfad))?;
    /// Das Handle des Hauptfensters, sofern es eines gibt.
    ///
    /// Es wird als **Zahl** weitergereicht und nicht als Windows-Typ: Die
    /// Kiste `cabrik-speicher` hat für alle drei Systeme dieselbe Form, und
    /// ein `HWND` in der Signatur zwänge jedem Aufrufer einen eigenen Zweig
    /// auf.
    #[cfg(windows)]
    fn besitzerfenster(app: &tauri::AppHandle) -> Option<isize> {
        app.get_webview_window("main")
            .and_then(|f| f.hwnd().ok())
            .map(|h| h.0 as isize)
    }

    /// Auf anderen Systemen gibt es kein Handle dieser Art — und dort gibt es
    /// auch kein eigenes Passwortfeld.
    #[cfg(not(windows))]
    fn besitzerfenster(_app: &tauri::AppHandle) -> Option<isize> {
        None
    }

    Ok(true)
}

/// Wie viele **Bytes** das Passwortfeld aufnimmt.
///
/// Nicht Zeichen: Der Puffer zählt Bytes, und ein Emoji sind vier davon.
/// Vierhundert reichen für jede Wortfolge, die sich ein Mensch merkt, und
/// halten den festgenagelten Bereich auf einer Seite.
const PASSWORTLAENGE: usize = 400;

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
        geschuetzt.as_bytes(),
        mit_signierschluessel,
        stufe,
        Sperrfrist::default(),
        jetzt(),
        ableitungsgrenze(),
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

/// Sucht die Schlüsseldatei der alten Version aus.
///
/// Eine einzelne Datei, kein Stapel: Es gibt genau eine zu übernehmen.
#[tauri::command(async)]
fn v1_datei_waehlen(app: tauri::AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .blocking_pick_file()
        .map(|p| p.to_string())
}

/// Übernimmt einen Schlüssel aus **Version 1**.
///
/// # Warum das kein Komfort ist
///
/// Wer die ausgelieferte v1 benutzt hat, kommt ohne diesen Weg an
/// **nichts** mehr heran, was an ihn gerichtet wurde. Bis heute hing
/// `cabrik-v1` nur an der Befehlszeile — für jemanden, der nur das
/// Fenster kennt, war das ein verschlossenes Schloss.
///
/// # `(async)`, und zwar zwingend
///
/// Ohne das Wort liefe dieser Befehl auf dem Faden der Oberfläche, und
/// die stünde sekundenlang still: Version 1 leitet **langsam** ab, und
/// danach wird die neue Hülle noch einmal abgeleitet. Genau das ist der
/// Fall, für den es das Wort gibt.
///
/// # Dieselben zwei Sperren wie beim Anlegen
///
/// Eine Übernahme über eine bestehende Identität zu schreiben wäre genauso
/// folgenschwer wie eine Neuanlage darüber — siehe
/// [`identitaet_anlegen`]. Die höfliche Prüfung steht hier, die
/// verlässliche in `cabrik_ablage::schreib_neu`.
#[tauri::command(async)]
fn v1_uebernehmen(
    zustand: State<'_, Zustand>,
    quelle: String,
    altes_passwort: String,
    neues_passwort: String,
    bezeichnung: Option<String>,
    stufe: KdfStufe,
) -> Result<Identitaet, String> {
    let pfad = zustand.schluesselpfad.clone();
    let kontaktpfad = zustand.kontaktpfad.clone();

    // Die Datei VOR der Sperre lesen: Sie kann groß sein und liegt womöglich
    // auf einem langsamen Datenträger. Die Sperre so lange zu halten hieße,
    // jeden anderen Befehl mit warten zu lassen.
    let alt =
        std::fs::read(&quelle).map_err(|e| format!("{quelle} lässt sich nicht lesen: {e}"))?;

    let mut z = sperre(&zustand)?;
    if z.is_some() {
        return Err("Auf diesem Rechner liegt bereits eine Identität. Eine \
                    zweite anzulegen — auch aus Version 1 — würde die \
                    bisherige überschreiben und damit alles unlesbar machen, \
                    was an sie gerichtet ist."
            .to_owned());
    }

    // Beide sofort in `Zeroizing`. Die Kopien davor lassen sich nicht
    // überschreiben (`spec/entsperrung.md` §5.1), diese hier schon.
    let altes = Zeroizing::new(altes_passwort);
    let neues = Zeroizing::new(neues_passwort);

    let neu = Sitzung::aus_v1_uebernehmen(
        cabrik_app::Uebernahme {
            v1_datei: &alt,
            altes_passwort: altes.as_bytes(),
            neues_passwort: neues.as_bytes(),
            bezeichnung,
            stufe,
            frist: Sperrfrist::default(),
        },
        jetzt(),
        ableitungsgrenze(),
        &mut OsRandom,
    )
    .map_err(wort)?;

    // Erst schreiben, dann übernehmen — wie beim Anlegen, und aus demselben
    // Grund: Andersherum stünde nach einem Schreibfehler eine offene
    // Sitzung über einer Datei, die es nicht gibt.
    cabrik_ablage::schreib_neu(&pfad, neu.schluesseldatei()).map_err(|e| e.meldung)?;
    cabrik_ablage::verschiebe_beiseite(&kontaktpfad).map_err(|e| e.meldung)?;

    *z = Some(neu);
    lies_identitaet(&mut z, &pfad)
}

/// Wie groß eine Sicherung höchstens sein darf.
///
/// Eine Schlüsseldatei hat wenige Kilobyte. Die Grenze ist nicht knapp
/// bemessen; sie ist dafür da, dass ein versehentlich gewähltes Video nicht
/// erst ganz in den Speicher wandert, bevor jemand sagt, dass es keine
/// Schlüsseldatei ist.
const SICHERUNG_HOECHSTENS: u64 = 1 << 20;

/// Wo die Kontaktdatei zu einer Sicherung liegt: daneben, mit demselben
/// Namen und eigener Endung.
///
/// An **einer** Stelle, weil Sichern und Einspielen sich darin einig sein
/// müssen. Liefen sie auseinander, fände das Einspielen die Kontakte nicht,
/// die das Sichern gerade abgelegt hat — und niemand merkte es, denn ohne
/// Kontakte gelingt das Einspielen trotzdem.
fn kontakte_neben(schluessel: &Path) -> PathBuf {
    schluessel.with_extension("cabrik-contacts")
}

/// Sucht eine Sicherung aus. `None` heißt abgebrochen.
///
/// Mit Filter auf die Endung, weil neben der Schlüsseldatei meist die
/// Kontaktdatei liegt und die beiden sonst gleich aussehen. „Alle Dateien“
/// bleibt wählbar: Wer seine Sicherung umbenannt hat, soll sie trotzdem
/// finden.
#[tauri::command(async)]
fn sicherung_waehlen(app: tauri::AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .add_filter("Schlüsseldatei", &["cabrik-key"])
        .add_filter("Alle Dateien", &["*"])
        .blocking_pick_file()
        .map(|p| p.to_string())
}

fn sicherung_lesen(pfad: &Path) -> Result<Vec<u8>, String> {
    let groesse = std::fs::metadata(pfad)
        .map_err(|e| format!("{}: {e}", pfad.display()))?
        .len();
    if groesse > SICHERUNG_HOECHSTENS {
        return Err("Diese Datei ist keine Schlüsseldatei von CabrikSecure — \
                    dafür ist sie viel zu groß."
            .to_owned());
    }
    std::fs::read(pfad).map_err(|e| format!("{}: {e}", pfad.display()))
}

/// Prüft eine gewählte Sicherung — **ohne** Passwort.
///
/// Sobald die Datei gewählt ist: Ist es die Kontaktdatei oder ein
/// Schlüssel aus Version 1, soll das dastehen, bevor jemand ein Passwort
/// tippt. Und verlangt sie ungewöhnlich viel, kommt erst die Rückfrage.
#[tauri::command(async)]
fn sicherung_pruefen(quelle: String) -> Result<Entsperrbedarf, String> {
    let daten = sicherung_lesen(Path::new(&quelle))?;
    cabrik_app::sicherung_pruefen(&daten, ableitungsgrenze()).map_err(wort)
}

/// Spielt eine Sicherung ein — und ist danach entsperrt, wie nach dem
/// Anlegen.
///
/// # Warum das kein Kopieren von Hand ist
///
/// Kopieren ginge auch. Aber erst hier wird die Sicherung **vor** dem
/// Ablegen mit dem Passwort geöffnet: Eine falsche Datei oder ein altes
/// Passwort ließe sonst einen Schlüssel auf dem Rechner zurück, den das
/// Programm weder öffnen noch löschen kann. Und erst hier wird geprüft, ob
/// die Kontaktdatei daneben zu diesem Schlüssel gehört.
///
/// # Dieselben zwei Sperren wie beim Anlegen
///
/// Eine Sicherung über eine bestehende Identität zu schreiben wäre genauso
/// folgenschwer wie eine Neuanlage darüber — siehe [`identitaet_anlegen`].
///
/// # `(async)`
///
/// Die Ableitung dauert, bei „Stark“ mehrere Sekunden. Auf dem Faden der
/// Oberfläche stünde das Fenster so lange still.
#[tauri::command(async)]
fn sicherung_einspielen(
    zustand: State<'_, Zustand>,
    quelle: String,
    passwort: String,
    ungewoehnliche_zulassen: bool,
) -> Result<Eingespielt, String> {
    let pfad = zustand.schluesselpfad.clone();
    let kontaktpfad = zustand.kontaktpfad.clone();

    // Beide Dateien VOR der Sperre lesen -- wie bei der Uebernahme: Sie
    // liegen womoeglich auf einem langsamen Stick.
    let quelle = PathBuf::from(quelle);
    let daten = sicherung_lesen(&quelle)?;
    let neben = kontakte_neben(&quelle);
    let gesichert = if neben == quelle {
        None
    } else {
        cabrik_ablage::lies(&neben).map_err(|e| e.meldung)?
    };

    let mut z = sperre(&zustand)?;
    if z.is_some() {
        return Err("Auf diesem Rechner liegt bereits eine Identität. Eine \
                    Sicherung darüber einzuspielen, würde sie überschreiben \
                    — und damit alles unlesbar machen, was an sie gerichtet \
                    ist."
            .to_owned());
    }
    let hier = cabrik_ablage::lies(&kontaktpfad).map_err(|e| e.meldung)?;

    // Sofort in `Zeroizing`. Die Kopien davor lassen sich nicht
    // ueberschreiben (`spec/entsperrung.md` §5.1), diese hier schon.
    let geschuetzt = Zeroizing::new(passwort);
    let (neu, herkunft) = Sitzung::einspielen(
        Einspielung {
            schluesseldatei: &daten,
            passwort: geschuetzt.as_bytes(),
            kontakte_hier: hier.as_deref(),
            kontakte_gesichert: gesichert.as_deref(),
            ungewoehnliche_zugelassen: ungewoehnliche_zulassen,
            frist: Sperrfrist::default(),
        },
        jetzt(),
        ableitungsgrenze(),
    )
    .map_err(wort)?;

    // Erst schreiben, dann uebernehmen -- wie beim Anlegen.
    cabrik_ablage::schreib_neu(&pfad, neu.schluesseldatei()).map_err(|e| e.meldung)?;
    if herkunft != Kontaktherkunft::Programmordner {
        // Was im Programmordner liegt, gehoert nicht zu diesem Schluessel
        // -- sonst waere es genommen worden. Beiseite, nicht geloescht:
        // Es koennte zu einer Sicherung gehoeren, die noch jemand findet.
        cabrik_ablage::verschiebe_beiseite(&kontaktpfad).map_err(|e| e.meldung)?;
    }
    if let (Kontaktherkunft::Sicherung, Some(k)) = (herkunft, &gesichert) {
        cabrik_ablage::schreib_neu(&kontaktpfad, k).map_err(|e| e.meldung)?;
    }

    *z = Some(neu);
    let identitaet = lies_identitaet(&mut z, &pfad)?;
    let kontakte = sitzung(&mut z)?.offen(jetzt()).map_err(wort)?.kontakte();
    let verifiziert = kontakte
        .iter()
        .filter(|k| k.vertrauen == Vertrauen::Verifiziert)
        .count();
    Ok(Eingespielt {
        identitaet,
        herkunft,
        kontakte: u32::try_from(kontakte.len()).unwrap_or(u32::MAX),
        verifiziert: u32::try_from(verifiziert).unwrap_or(u32::MAX),
    })
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
    stapel(pfade, &fortschritt, |p, melder| {
        let pfad = Path::new(p);
        let name = dateiname(p);

        melder.schritt(Schritt::Lesen);
        match lies_gemeldet(pfad, melder) {
            Ok(daten) => {
                melder.schritt(Schritt::Bereinigen);
                cabrik_app::datei_pruefen(p, &name, &daten)
            }
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
    stapel(pfade, &fortschritt, |p, _| eine_speichern(p, &ziel))
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
    freier_name_wo(ziel, |p| !p.exists())
}

/// Wie [`freier_name`], mit eigener Vorstellung davon, was frei ist — beim
/// Sichern muss es der Name für zwei Dateien sein.
fn freier_name_wo(ziel: &Path, frei: impl Fn(&Path) -> bool) -> PathBuf {
    if frei(ziel) {
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
        .find(|v| frei(v))
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

    let dateien = stapel(pfade, &fortschritt, |p, melder| {
        let quelle = Path::new(p);
        let name = dateiname(p);

        // Das Original muss nicht bereinigt werden, also auch nicht im
        // Speicher liegen (`spec/grosse-dateien.md` §3.1).
        if original.iter().any(|o| o == p) {
            return original_stroemen(offen, &plan, p, &name, melder)
                .unwrap_or_else(|grund| cabrik_app::versand_fehler(p, grund));
        }

        melder.schritt(Schritt::Lesen);
        let roh = match lies_gemeldet(quelle, melder) {
            Ok(d) => d,
            Err(e) => return cabrik_app::versand_fehler(p, e.to_string()),
        };

        melder.schritt(Schritt::Bereinigen);
        let (sauber, b) = cabrik_app::datei_bereinigen(&roh);
        // Gibt es keine bereinigte Fassung, geht das Original hinaus. Das
        // ist kein Fehler: Bei einem nicht verstandenen Format WEISS das
        // Programm nicht, was es entfernen soll -- und der Befund sagt
        // genau das.
        let (nutzdaten, befund) = (sauber.unwrap_or(roh), Some(b));

        melder.schritt(Schritt::Verschluesseln);
        let envelope = match offen.verschluesseln_gemeldet(
            &plan,
            &name,
            &nutzdaten,
            &mut OsRandom,
            &mut |fertig, gesamt| melder.bytes(Schritt::Verschluesseln, fertig, gesamt),
        ) {
            Ok(e) => e,
            Err(e) => return cabrik_app::versand_fehler(p, e.meldung),
        };

        melder.schritt(Schritt::Schreiben);
        let ziel = freier_name(&quelle.with_file_name(cabrik_app::envelope_name(&name)));
        match schreib_gemeldet(&ziel, &envelope, melder) {
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

/// Verschlüsselt ein Original, ohne es ganz zu laden
/// (`spec/grosse-dateien.md` §3.1, §4).
///
/// Wer das Original verschicken will, bekommt das Original — das war ja
/// gerade die Entscheidung. Bereinigt wird nichts, also muss auch nichts
/// im Speicher liegen: Die Datei geht in Blöcken zu einem MiB durch die
/// Versiegelung in die Zwischendatei neben dem Ziel.
///
/// Die Länge steht im Header, bevor das erste Byte gelesen ist. Wird die
/// Datei beim Lesen kürzer oder länger, entsteht kein Envelope.
fn original_stroemen(
    offen: &cabrik_app::Offen,
    plan: &Versandplan,
    p: &str,
    name: &str,
    melder: &Melder<'_>,
) -> Result<Versandergebnis, String> {
    use std::io::Read as _;

    let quelle = Path::new(p);
    let mut datei = std::fs::File::open(quelle).map_err(|e| format!("{p}: {e}"))?;
    let laenge = datei.metadata().map_err(|e| format!("{p}: {e}"))?.len();

    let ziel = freier_name(&quelle.with_file_name(cabrik_app::envelope_name(name)));
    let mut aus = Zwischendatei::neben(&ziel).map_err(|e| e.meldung)?;
    let (kopf, mut versiegelung) = offen
        .versiegelung_beginnen(plan, name, laenge, &mut OsRandom)
        .map_err(wort)?;
    aus.schreiben(&kopf).map_err(|e| e.meldung)?;

    melder.schritt(Schritt::Verschluesseln);
    let mut block = Zeroizing::new(vec![0_u8; 1024 * 1024]);
    let mut verschluesselt = Vec::new();
    let mut gelesen = 0_u64;
    loop {
        let n = datei.read(&mut block).map_err(|e| format!("{p}: {e}"))?;
        if n == 0 {
            break;
        }
        gelesen = gelesen.saturating_add(n as u64);
        if gelesen > laenge {
            return Err(format!(
                "{name} wurde beim Lesen länger. Verschlüsselt wird nur eine \
                 Datei, die sich dabei nicht verändert."
            ));
        }
        let stueck = block.get(..n).ok_or("Lesepuffer zu klein")?;
        versiegelung
            .weiter(stueck, &mut verschluesselt)
            .map_err(|e| wort(e.into()))?;
        aus.schreiben(&verschluesselt).map_err(|e| e.meldung)?;
        verschluesselt.clear();
        melder.bytes(Schritt::Verschluesseln, gelesen, laenge);
    }
    if gelesen < laenge {
        return Err(format!(
            "{name} wurde beim Lesen kürzer. Verschlüsselt wird nur eine Datei, \
             die sich dabei nicht verändert."
        ));
    }
    versiegelung
        .abschliessen(&mut verschluesselt)
        .map_err(|e| wort(e.into()))?;
    aus.schreiben(&verschluesselt).map_err(|e| e.meldung)?;

    let bytes = aus.geschrieben();
    aus.als_neu().map_err(|e| e.meldung)?;
    melder.bytes(Schritt::Verschluesseln, laenge, laenge);

    Ok(Versandergebnis {
        quelle: p.to_owned(),
        ziel: Some(ziel.display().to_string()),
        bytes: usize::try_from(bytes).unwrap_or(usize::MAX),
        befund: None,
        fehler: None,
    })
}

// ---------------------------------------------------------------------------
// Entschlüsseln
// ---------------------------------------------------------------------------

/// Die Ablage des ersten Durchgangs: Sie nimmt den Klartext und vergisst
/// ihn (`spec/grosse-dateien.md` §6).
///
/// Jeder Block ist im Kern schon `Zeroizing` und wird überschrieben, sobald
/// er hier vorbei ist. Geprüft wird trotzdem alles — Blöcke, Trailer,
/// Signatur —, und genau das ist der Zweck: Der Bericht soll stimmen, bevor
/// jemand entscheidet, ob er die Datei haben will.
struct Verwerfer;

impl Ablage for Verwerfer {
    type Fehler = std::convert::Infallible;

    fn schreiben(&mut self, _klartext: &[u8]) -> Result<(), Self::Fehler> {
        Ok(())
    }

    fn freigeben(&mut self, _beleg: &envelope::Geprueft) -> Result<(), Self::Fehler> {
        Ok(())
    }

    fn verwerfen(&mut self) {}
}

/// Wie oft ein langer Durchgang nachsieht, ob die Sitzung noch offen ist.
///
/// Alle 64 MiB — auf einer SSD einige Male je Sekunde, auf einem langsamen
/// Stick seltener. Wer sperrt, bricht damit den Durchgang ab, und zwar bald:
/// Die Schlüssel dieses Envelopes sollen nicht weiterarbeiten, wenn die
/// Identität es nicht mehr tut.
const SITZUNG_PRUEFEN_ALLE: u64 = 64 * 1024 * 1024;

/// Was beim blockweisen Öffnen scheitert, in Worten.
fn oeffnungsfehler_wort<F: std::fmt::Display>(f: Oeffnungsfehler<F>) -> String {
    match f {
        Oeffnungsfehler::Kern(e) => wort(cabrik_app::oeffnen_fehler(e)),
        Oeffnungsfehler::Ablage(e) => e.to_string(),
        Oeffnungsfehler::Veraendert => "Der Envelope hat sich seit dem Öffnen verändert — \
                                        er ist nicht mehr der, den Sie geprüft haben. Nichts \
                                        wurde abgelegt. Öffnen Sie ihn bitte noch einmal."
            .to_owned(),
    }
}

/// Öffnet einen Envelope blockweise in eine Ablage
/// (`spec/grosse-dateien.md` §6).
///
/// # Die Sitzung nur am Anfang
///
/// Gebraucht wird sie zum Beginnen: Dort steckt die Identität. Danach trägt
/// die Öffnung die Schlüssel dieses einen Envelopes, und die Sitzung wird
/// wieder frei gegeben — sonst stünde während eines Durchgangs über drei
/// Gigabyte jeder andere Befehl still, auch die Anzeige, ob gesperrt ist.
///
/// Zwischen den Blöcken wird nachgesehen, ob sie noch offen ist
/// ([`SITZUNG_PRUEFEN_ALLE`]). Das zählt als Handlung, wie jeder andere
/// Befehl auch: Wer gerade eine große Datei öffnet, ist da.
///
/// # Fehler
///
/// Mit Worten. Die Ablage ist dann verworfen — vom Kern, wenn er den Fehler
/// sah, sonst hier.
fn blockweise_oeffnen<A>(
    zustand: &Zustand,
    quelle: &Path,
    signatur_verlangt: bool,
    erwartet: Option<[u8; 32]>,
    ablage: &mut A,
    melder: &Melder<'_>,
) -> Result<envelope::Bericht, String>
where
    A: Ablage,
    A::Fehler: std::fmt::Display,
{
    use std::io::Read as _;

    let ergebnis = (|| {
        let mut datei =
            std::fs::File::open(quelle).map_err(|e| format!("{}: {e}", quelle.display()))?;
        let gesamt = datei
            .metadata()
            .map_err(|e| format!("{}: {e}", quelle.display()))?
            .len();

        let mut anfang = vec![
            0_u8;
            usize::try_from(gesamt)
                .unwrap_or(usize::MAX)
                .min(envelope::KOPF_HOECHSTENS)
        ];
        datei
            .read_exact(&mut anfang)
            .map_err(|e| format!("{}: {e}", quelle.display()))?;

        let (mut oeffnung, verbraucht) = {
            let mut z = sperre(zustand)?;
            sitzung(&mut z)?
                .offen(jetzt())
                .map_err(wort)?
                .oeffnung_beginnen(&anfang, signatur_verlangt)
                .map_err(wort)?
        };
        if let Some(f) = erwartet {
            oeffnung.fingerabdruck_erwarten(f);
        }

        melder.schritt(Schritt::Oeffnen);
        let rest = anfang.get(verbraucht..).unwrap_or(&[]);
        oeffnung
            .weiter(rest, ablage)
            .map_err(oeffnungsfehler_wort)?;

        let mut gelesen = anfang.len() as u64;
        let mut naechste_pruefung = gelesen.saturating_add(SITZUNG_PRUEFEN_ALLE);
        let mut block = vec![0_u8; 1024 * 1024];
        loop {
            let n = datei
                .read(&mut block)
                .map_err(|e| format!("{}: {e}", quelle.display()))?;
            if n == 0 {
                break;
            }
            let stueck = block.get(..n).ok_or("Lesepuffer zu klein")?;
            oeffnung
                .weiter(stueck, ablage)
                .map_err(oeffnungsfehler_wort)?;
            gelesen = gelesen.saturating_add(n as u64);
            melder.bytes(Schritt::Oeffnen, gelesen, gesamt);

            if gelesen >= naechste_pruefung {
                let mut z = sperre(zustand)?;
                sitzung(&mut z)?.offen(jetzt()).map_err(wort)?;
                naechste_pruefung = gelesen.saturating_add(SITZUNG_PRUEFEN_ALLE);
            }
        }
        let bericht = oeffnung
            .abschliessen(ablage)
            .map_err(oeffnungsfehler_wort)?;
        melder.bytes(Schritt::Oeffnen, gelesen, gelesen);
        Ok(bericht)
    })();

    // Den Fehler hat der Kern womöglich nicht gesehen -- eine Datei, die
    // nicht weiterlas, eine Sitzung, die gesperrt wurde. Verworfen wird
    // dann hier; zweimal zu verwerfen schadet nicht.
    if ergebnis.is_err() {
        ablage.verwerfen();
    }
    ergebnis
}

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
        .add_filter("CabrikSecure-Envelope", &endungen)
        .blocking_pick_file()
        .and_then(|f| f.into_path().ok())
        .map(|p| p.display().to_string())
}

/// Öffnet einen Envelope. **Der Klartext bleibt in Rust.**
///
/// Zurück geht ein Bericht: Wer geschickt hat, wie die Datei heißt, wie
/// groß sie ist. Der Inhalt liegt in der Sitzung, bis jemand sagt, wohin
/// er soll — oder bis gesperrt wird, dann ist er fort.
///
/// # Große Dateien
///
/// Über [`cabrik_app::IM_SPEICHER_BIS`] wird blockweise entschlüsselt und
/// geprüft, der Klartext aber verworfen (`spec/grosse-dateien.md` §6). Der
/// Bericht ist derselbe; beim Speichern wird ein zweites Mal entschlüsselt.
///
/// `async`, und zwar ausdrücklich: Ohne den Zusatz liefe der Befehl unter
/// Windows auf dem Faden, der das Fenster zeichnet, und das Fenster stünde
/// still, solange eine große Datei gelesen wird.
#[tauri::command(async)]
fn envelope_oeffnen(
    zustand: State<'_, Zustand>,
    pfad: String,
    signatur_verlangt: bool,
    fortschritt: Channel<Fortschritt>,
) -> Result<Geoeffnet, String> {
    let melder = Melder::einzeln(&fortschritt, &pfad);
    melder.schritt(Schritt::Arbeiten);
    let laenge = std::fs::metadata(&pfad)
        .map_err(|e| format!("{pfad}: {e}"))?
        .len();

    if laenge <= cabrik_app::IM_SPEICHER_BIS {
        melder.schritt(Schritt::Lesen);
        let daten = lies_gemeldet(Path::new(&pfad), &melder).map_err(|e| e.to_string())?;
        melder.schritt(Schritt::Oeffnen);
        let mut z = sperre(&zustand)?;
        return sitzung(&mut z)?
            .offen(jetzt())
            .map_err(wort)?
            .envelope_oeffnen(&daten, signatur_verlangt)
            .map_err(wort);
    }

    let bericht = blockweise_oeffnen(
        &zustand,
        Path::new(&pfad),
        signatur_verlangt,
        None,
        &mut Verwerfer,
        &melder,
    )?;
    let mut z = sperre(&zustand)?;
    Ok(sitzung(&mut z)?
        .offen(jetzt())
        .map_err(wort)?
        .gross_geoeffnet(bericht, pfad, signatur_verlangt))
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
///
/// # Große Dateien
///
/// Sie liegen nicht bereit. Hier läuft der zweite Durchgang
/// (`spec/grosse-dateien.md` §6): Derselbe Envelope wird noch einmal
/// entschlüsselt, direkt in die Zwischendatei neben dem Ziel, und umbenannt
/// wird nur, wenn sein Fingerabdruck derselbe ist wie beim Öffnen.
#[tauri::command(async)]
fn nutzlast_speichern(
    zustand: State<'_, Zustand>,
    app: tauri::AppHandle,
    fortschritt: Channel<Fortschritt>,
) -> Result<Option<Abgelegt>, String> {
    // Erst den Namen holen -- der Dialog laeuft ohne gehaltene Sperre,
    // sonst stuende die ganze Anwendung, solange er offen ist.
    let (vorschlag, gross) = {
        let mut z = sperre(&zustand)?;
        let offen = sitzung(&mut z)?.offen(jetzt()).map_err(wort)?;
        if let Some(g) = offen.grosse_nutzlast() {
            let name = g.dateiname.clone().unwrap_or_else(|| "datei".to_owned());
            (name, Some(g.clone()))
        } else {
            let (_, name) = offen
                .nutzlast()
                .ok_or_else(|| "Es ist nichts geöffnet.".to_owned())?;
            (name.unwrap_or("nachricht").to_owned(), None)
        }
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

    let ziel = freier_name(&ziel);
    let mut datei = Zwischendatei::neben(&ziel).map_err(|e| e.meldung)?;
    let rest_entfernt = datei.alter_rest_entfernt();

    if let Some(g) = gross {
        let melder = Melder::einzeln(&fortschritt, &ziel.display().to_string());
        blockweise_oeffnen(
            &zustand,
            Path::new(&g.quelle),
            g.signatur_verlangt,
            Some(g.fingerabdruck),
            &mut datei,
            &melder,
        )?;
    } else {
        let mut z = sperre(&zustand)?;
        let offen = sitzung(&mut z)?.offen(jetzt()).map_err(wort)?;
        let (inhalt, _) = offen
            .nutzlast()
            .ok_or_else(|| "Es ist nichts geöffnet.".to_owned())?;
        datei.schreiben(inhalt).map_err(|e| e.meldung)?;
        drop(z);
        datei.als_neu().map_err(|e| e.meldung)?;
    }

    Ok(Some(Abgelegt {
        pfad: ziel.display().to_string(),
        rest_entfernt,
    }))
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

/// Legt eine Kopie der Schlüsseldatei ab — und auf Wunsch der
/// Kontaktdatei daneben. `None` heißt abgebrochen.
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
///
/// # Die Kontakte
///
/// Sie sind an die Identität versiegelt (`spec/trust-store.md` §6): Wer die
/// Kopie öffnen kann, kann auch die Schlüsseldatei öffnen. Sie verrät dann
/// aber zusätzlich, **mit wem** man zu tun hat. Deshalb fragt die
/// Oberfläche, statt sie immer mitzunehmen.
#[tauri::command(async)]
fn schluessel_sichern(
    zustand: State<'_, Zustand>,
    app: tauri::AppHandle,
    mit_kontakten: bool,
) -> Result<Option<Sicherung>, String> {
    let (daten, kontakte) = {
        let mut z = sperre(&zustand)?;
        let s = sitzung(&mut z)?;
        let kontakte = if mit_kontakten {
            Some(s.kontakte_sichern(jetzt(), &mut OsRandom).map_err(wort)?)
        } else {
            None
        };
        (s.schluesseldatei().to_vec(), kontakte)
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

    // Endete der gewaehlte Name schon auf die Endung der Kontaktdatei,
    // fielen beide auf denselben Pfad.
    let ziel = if kontakte_neben(&ziel) == ziel {
        ziel.with_extension("cabrik-key")
    } else {
        ziel
    };
    // Frei muss der Name fuer BEIDE sein. Sonst stuende die neue
    // Schluesseldatei neben einer alten Kontaktdatei -- und das Einspielen
    // naehme die.
    let ziel = freier_name_wo(&ziel, |p| {
        !p.exists() && (kontakte.is_none() || !kontakte_neben(p).exists())
    });
    cabrik_ablage::schreib_neu(&ziel, &daten).map_err(|e| e.meldung)?;

    let kontakte = match kontakte {
        Some(k) => {
            let neben = kontakte_neben(&ziel);
            cabrik_ablage::schreib_neu(&neben, &k).map_err(|e| e.meldung)?;
            Some(neben.display().to_string())
        }
        None => None,
    };
    Ok(Some(Sicherung {
        schluessel: ziel.display().to_string(),
        kontakte,
    }))
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
fn passwort_aendern(zustand: State<'_, Zustand>, alt: String, neu: String) -> Result<(), String> {
    let pfad = zustand.schluesselpfad.clone();
    // Beide sofort in `Zeroizing`. Die Kopien davor -- die
    // JavaScript-Zeichenketten und der Übergabepuffer -- lassen sich nicht
    // überschreiben (`spec/entsperrung.md` §5.1).
    let alt = Zeroizing::new(alt);
    let neu = Zeroizing::new(neu);

    let mut z = sperre(&zustand)?;
    let s = sitzung(&mut z)?;
    s.passwort_aendern(
        alt.as_bytes(),
        neu.as_bytes(),
        ableitungsgrenze(),
        &mut OsRandom,
    )
    .map_err(wort)?;

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
    stapel(pfade, &fortschritt, |p, _| {
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
    stapel(pfade, &fortschritt, |p, melder| {
        melder.schritt(Schritt::Ueberschreiben);
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
fn nutzlast_lesen(zustand: State<'_, Zustand>, nutzlast: String) -> Result<Nutzlastbefund, String> {
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
                meldung: "CabrikSecure hat auf diesem Rechner kein Verzeichnis \
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
                        meldung: format!("Die Kontaktdatei ließ sich nicht lesen: {}", e.meldung),
                        pfad: Some(kontaktpfad.display().to_string()),
                        rat: "Löschen Sie die Datei nicht — solange sie da \
                              ist, sind Ihre Kontakte nicht verloren. Legen \
                              Sie sie beiseite; CabrikSecure startet dann mit einem \
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
                      Starten Sie CabrikSecure danach neu. Haben Sie eine \
                      Sicherung, spielen Sie sie in der Einrichtung ein."
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
        .plugin(tauri_plugin_single_instance::init(
            |app, argumente, _ordner| {
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
            },
        ))
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            // Vor dem Einschlafen sperren (`spec/entsperrung.md` §3.4).
            //
            // Die Anmeldung steht VOR dem `manage`, obwohl der Rückruf den
            // Zustand braucht: Er holt ihn sich über `try_state` erst,
            // wenn er tatsächlich aufgerufen wird — und bis dahin steht
            // der Zustand längst. Andersherum ginge es nicht, denn die
            // Auskunft, ob es geklappt hat, gehört in eben diesen Zustand.
            let griff = app.handle().clone();
            let (wacht, ruheschutz) = match ruhezustand::anmelden(move |meldung| {
                if let Some(z) = griff.try_state::<Zustand>() {
                    auf_meldung(meldung, &z.sitzung);
                }
            }) {
                Ok(w) => {
                    let schutz = if w.hat_aufschub() {
                        Ruheschutz::MitAufschub
                    } else {
                        Ruheschutz::OhneAufschub
                    };
                    (Some(w), schutz)
                }
                // Kein Abbruch und keine Meldung beim Start: Ohne diese
                // Anmeldung ist niemand schlechter dran als vorher, es
                // gilt dann allein die Frist. Aber es wird vermerkt, denn
                // behaupten darf man es dann nicht.
                Err(f) => (None, Ruheschutz::Nicht { grund: f.grund }),
            };

            app.manage(Zustand {
                sitzung: Mutex::new(sitzung),
                schluesselpfad,
                kontaktpfad,
                hereingereicht: Mutex::new(beim_start),
                startfehler: fehler_beim_start,
                ruheschutz,
            });

            // Am Leben halten. Fiele die Wacht hier, meldete das System
            // nichts mehr -- und das Sperren oben liefe ins Leere.
            if let Some(w) = wacht {
                app.manage(w);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            sitzungsstand,
            entsperren,
            entsperren_nativ,
            entsperrbedarf,
            ungewoehnliche_ableitung_zulassen,
            passwortweg,
            sperren,
            frist_setzen,
            taetigkeit,
            identitaet,
            identitaet_anlegen,
            v1_datei_waehlen,
            v1_uebernehmen,
            sicherung_waehlen,
            sicherung_pruefen,
            sicherung_einspielen,
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
            ruheschutz,
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
            .set_title("CabrikSecure lässt sich nicht starten")
            .set_description(format!(
                "Das Programmfenster ließ sich nicht öffnen.\n\n\
                 {e}\n\n\
                 Unter Windows fehlt in diesem Fall meist die \
                 WebView2-Laufzeit. Sie lässt sich bei Microsoft kostenlos \
                 nachinstallieren; danach startet CabrikSecure wieder.\n\n\
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
        clippy::indexing_slicing,
        reason = "Fehlschlag soll den Test abbrechen"
    )]

    use super::{
        Channel, Fortschritt, Melder, Meldung, OsRandom, Ruheschutz, Schritt, Sitzung,
        Speichergrenze, Verwerfer, Zustand, Zwischendatei, auf_meldung, blockweise_oeffnen,
        datei_aus_argumenten, freier_name_wo, kontakte_neben, lade, lies_gemeldet,
        original_stroemen,
    };
    use cabrik_bruecke::{KdfStufe, Sperrfrist};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    /// Ein eigener Ordner je Test — sonst sehen sie einander.
    fn ordner(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cabrik-start-{name}"));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("Ordner");
        p
    }

    #[test]
    fn die_kontakte_liegen_neben_der_sicherung() {
        // Sichern und Einspielen muessen sich darin einig sein. Liefen sie
        // auseinander, faende das Einspielen die Kontakte nicht -- und
        // niemand merkte es, denn ohne sie gelingt es trotzdem.
        let o = ordner("neben");
        assert_eq!(
            kontakte_neben(&o.join("identity.cabrik-key")),
            o.join("identity.cabrik-contacts")
        );
        assert_eq!(
            kontakte_neben(&o.join("Reise 2026")),
            o.join("Reise 2026.cabrik-contacts")
        );
    }

    #[test]
    fn der_name_der_sicherung_ist_fuer_beide_dateien_frei() {
        // Eine alte Kontaktdatei ohne ihre Schluesseldatei: Die neue
        // Sicherung darf sich nicht daneben legen, sonst naehme das
        // Einspielen die alten Kontakte.
        let o = ordner("frei-fuer-beide");
        std::fs::write(o.join("identity.cabrik-contacts"), b"alt").expect("schreiben");

        let ziel = freier_name_wo(&o.join("identity.cabrik-key"), |p| {
            !p.exists() && !kontakte_neben(p).exists()
        });

        assert_eq!(ziel, o.join("identity (2).cabrik-key"));
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

        assert!(
            fehler.meldung.contains("Schlüsseldatei"),
            "{}",
            fehler.meldung
        );
        assert_eq!(
            fehler.pfad.as_deref(),
            Some(pfad.display().to_string().as_str())
        );
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

        assert!(
            fehler.meldung.contains("Kontaktdatei"),
            "{}",
            fehler.meldung
        );
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
        let gefunden = datei_aus_argumenten(["--webview-exe-name=cabrik.exe", "--flag", PFAD]);

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
    // -- Byteweises Lesen ------------------------------------------------

    /// Ein Melder, der seine Meldungen in einer Liste sammelt.
    ///
    /// `Channel::new` nimmt einen Rückruf und braucht kein Fenster — die
    /// Drosselung und die Zählung lassen sich damit hier prüfen, ohne
    /// Tauri zu starten.
    fn mitschrift() -> (Channel<Fortschritt>, Arc<Mutex<Vec<Fortschritt>>>) {
        let liste = Arc::new(Mutex::new(Vec::new()));
        let hinein = Arc::clone(&liste);
        let kanal = Channel::new(move |leib| {
            // Der Kanal liefert den serialisierten Leib. Ihn hier wieder zu
            // lesen prueft nebenbei, dass `Fortschritt` ueber die Bruecke
            // ueberhaupt geht.
            if let tauri::ipc::InvokeResponseBody::Json(text) = leib
                && let Ok(f) = serde_json::from_str::<serde_json::Value>(&text)
            {
                hinein.lock().expect("Mutex").push(Fortschritt {
                    erledigt: f["erledigt"].as_u64().unwrap_or(0) as usize,
                    gesamt: f["gesamt"].as_u64().unwrap_or(0) as usize,
                    laeuft: f["laeuft"].as_str().unwrap_or("").to_owned(),
                    schritt: Schritt::Arbeiten,
                    bytes_erledigt: f["bytesErledigt"].as_u64(),
                    bytes_gesamt: f["bytesGesamt"].as_u64(),
                });
            }
            Ok(())
        });
        (kanal, liste)
    }

    fn melder_fuer<'a>(kanal: &'a Channel<Fortschritt>) -> Melder<'a> {
        Melder {
            kanal,
            erledigt: 0,
            gesamt: 1,
            name: "probe.bin".to_owned(),
            zuletzt: std::cell::Cell::new(0),
        }
    }

    #[test]
    fn gelesen_wird_vollstaendig_und_unveraendert() {
        // ZUERST das Naheliegende: Ein Fortschrittsbalken, der beim Lesen
        // Bytes verliert, waere ein Datenverlust fuer eine Anzeige.
        let ordner = ordner("lesen-inhalt");
        let pfad = ordner.join("probe.bin");
        // Groesser als ein Block (1 MiB), damit die Schleife mehrfach laeuft.
        let inhalt: Vec<u8> = (0..3_500_000_u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&pfad, &inhalt).expect("schreiben");

        let (kanal, _) = mitschrift();
        let melder = melder_fuer(&kanal);
        let gelesen = lies_gemeldet(&pfad, &melder).expect("lesen");

        assert_eq!(gelesen, inhalt, "der Inhalt hat sich beim Lesen veraendert");
    }

    #[test]
    fn beim_lesen_wird_der_fortschritt_gemeldet_und_endet_vollstaendig() {
        let ordner = ordner("lesen-fortschritt");
        let pfad = ordner.join("probe.bin");
        let groesse = 9_000_000_usize;
        std::fs::write(&pfad, vec![7_u8; groesse]).expect("schreiben");

        let (kanal, liste) = mitschrift();
        let melder = melder_fuer(&kanal);
        lies_gemeldet(&pfad, &melder).expect("lesen");

        let meldungen = liste.lock().expect("Mutex").clone();
        let mit_bytes: Vec<&Fortschritt> = meldungen
            .iter()
            .filter(|f| f.bytes_erledigt.is_some())
            .collect();

        assert!(!mit_bytes.is_empty(), "es wurde gar nichts gemeldet");

        // Die LETZTE Meldung steht auf voll. Ohne sie bliebe der Balken
        // kurz vor dem Ende stehen -- und genau das laesst jemanden
        // glauben, das Programm haenge.
        let letzte = mit_bytes.last().expect("mindestens eine");
        assert_eq!(letzte.bytes_erledigt, Some(groesse as u64));
        assert_eq!(letzte.bytes_gesamt, Some(groesse as u64));

        // Und nie rueckwaerts.
        let mut vorher = 0;
        for f in &mit_bytes {
            let jetzt = f.bytes_erledigt.unwrap_or(0);
            assert!(jetzt >= vorher, "der Fortschritt ist zurueckgesprungen");
            vorher = jetzt;
        }
    }

    #[test]
    fn die_drosselung_haelt_die_zahl_der_meldungen_klein() {
        // DER PUNKT DER DROSSELUNG. Ohne sie kaeme je Block eine Meldung
        // ueber die Bruecke; bei 9 MB und 1-MiB-Bloecken waeren das neun,
        // bei 3 GB rund dreitausend. Die Oberflaeche kaeme mit dem
        // Zeichnen nicht nach.
        let ordner = ordner("lesen-drosselung");
        let pfad = ordner.join("probe.bin");
        let groesse = 9_000_000_usize;
        std::fs::write(&pfad, vec![1_u8; groesse]).expect("schreiben");

        let (kanal, liste) = mitschrift();
        let melder = melder_fuer(&kanal);
        lies_gemeldet(&pfad, &melder).expect("lesen");

        let anzahl = liste
            .lock()
            .expect("Mutex")
            .iter()
            .filter(|f| f.bytes_erledigt.is_some())
            .count();

        // 9 MB bei 4 MiB Abstand: zwei Zwischenmeldungen plus die letzte.
        assert!(
            anzahl <= 4,
            "zu viele Meldungen fuer 9 MB: {anzahl} -- die Drosselung greift nicht"
        );
    }

    // -- Sperren vor dem Ruhezustand ------------------------------------

    /// Eine entsperrte Sitzung, so billig wie die Ableitung es zulaesst.
    fn offene_sitzung() -> Sitzung {
        Sitzung::anlegen(
            None,
            b"vier zufaellige woerter hier",
            false,
            KdfStufe::Min,
            Sperrfrist::default(),
            1_000,
            Speichergrenze::UNBEKANNT,
            &mut OsRandom,
        )
        .expect("anlegen")
    }

    fn ist_gesperrt(fach: &Mutex<Option<Sitzung>>) -> bool {
        fach.lock()
            .expect("Mutex")
            .as_ref()
            .expect("Sitzung")
            .ist_gesperrt()
    }

    #[test]
    fn nur_das_einschlafen_sperrt() {
        let fach = Mutex::new(Some(offene_sitzung()));
        assert!(!ist_gesperrt(&fach), "Voraussetzung: offen");

        // GEGENPROBE ZUERST. Wer nur prueft, dass Einschlafen sperrt,
        // uebersieht die schlimmere Haelfte: eine Umsetzung, die bei JEDER
        // Meldung sperrt, bestuende diesen Test zur Haelfte -- und wuerde
        // den Nutzer aussperren, sobald er das Netzteil zieht.
        auf_meldung(Meldung::Belanglos, &fach);
        assert!(!ist_gesperrt(&fach), "eine belanglose Meldung hat gesperrt");

        auf_meldung(Meldung::WiederWach, &fach);
        assert!(!ist_gesperrt(&fach), "das Aufwachen hat gesperrt");

        auf_meldung(Meldung::LegtSichSchlafen, &fach);
        assert!(
            ist_gesperrt(&fach),
            "vor dem Einschlafen wurde nicht gesperrt"
        );
    }

    #[test]
    fn ein_zweites_mal_schadet_nicht() {
        // Windows darf dieselbe Meldung mehrfach zustellen, und beim
        // Aufwachen-und-wieder-Einschlafen kommt sie ohnehin erneut.
        let fach = Mutex::new(Some(offene_sitzung()));
        auf_meldung(Meldung::LegtSichSchlafen, &fach);
        auf_meldung(Meldung::LegtSichSchlafen, &fach);
        assert!(ist_gesperrt(&fach));
    }

    #[test]
    fn ohne_identitaet_geschieht_nichts() {
        // Auf einem frischen Rechner gibt es noch keine Schluesseldatei.
        // Der Rueckruf kommt trotzdem -- und darf nicht abstuerzen, schon
        // gar nicht auf einem fremden Faden kurz vor dem Einschlafen.
        let fach: Mutex<Option<Sitzung>> = Mutex::new(None);
        auf_meldung(Meldung::LegtSichSchlafen, &fach);
        assert!(fach.lock().expect("Mutex").is_none());
    }

    // -- Große Dateien (`spec/grosse-dateien.md` §6) ----------------------

    /// Eine offene Sitzung, die sich selbst als Kontakt kennt — damit sie
    /// sich selbst etwas schicken kann.
    fn sitzung_mit_mir() -> (Sitzung, String) {
        // Zur echten Uhrzeit: `blockweise_oeffnen` fragt mit ihr nach, und
        // eine Sitzung von 1970 waere laengst abgelaufen. Mit
        // Signierschluessel, damit die Envelopes signiert sind.
        let mut s = Sitzung::anlegen(
            None,
            b"vier zufaellige woerter hier",
            true,
            KdfStufe::Min,
            Sperrfrist::default(),
            super::jetzt(),
            Speichergrenze::UNBEKANNT,
            &mut OsRandom,
        )
        .expect("anlegen");
        let offen = s.offen(super::jetzt()).expect("offen");
        let nutzlast = offen.eigene_nutzlast().expect("Nutzlast");
        offen
            .kontakt_aus_nutzlast("Ich", &nutzlast, super::jetzt())
            .expect("aufnehmen");
        let fp = offen
            .kontakte()
            .into_iter()
            .find(|k| k.name == "Ich")
            .expect("Kontakt")
            .fingerprint;
        (s, fp)
    }

    /// Ein Envelope an mich selbst, als Datei abgelegt.
    fn envelope_an_mich(s: &mut Sitzung, fp: &str, inhalt: &[u8], pfad: &std::path::Path) {
        let offen = s.offen(super::jetzt()).expect("offen");
        let plan = offen.versand_planen(&[fp.to_owned()], true).expect("Plan");
        let env = offen
            .verschluesseln(&plan, "urlaub.mp4", inhalt, &mut OsRandom)
            .expect("verschluesseln");
        std::fs::write(pfad, env).expect("schreiben");
    }

    fn zustand_mit(s: Sitzung, o: &std::path::Path) -> Zustand {
        Zustand {
            sitzung: Mutex::new(Some(s)),
            schluesselpfad: o.join("identity.cabrik-key"),
            kontaktpfad: o.join("contacts.cabrik-contacts"),
            hereingereicht: Mutex::new(None),
            startfehler: None,
            ruheschutz: Ruheschutz::OhneAufschub,
        }
    }

    fn inhalt() -> Vec<u8> {
        // Mehrere Lesebloecke zu einem MiB, und der letzte halb voll.
        (0..3_500_000_u32).map(|i| (i % 253) as u8).collect()
    }

    #[test]
    fn zwei_durchgaenge_legen_ab_was_der_erste_geprueft_hat() {
        let o = ordner("zwei-durchgaenge");
        let (mut s, fp) = sitzung_mit_mir();
        let quelle = o.join("urlaub.mp4.cabrik");
        envelope_an_mich(&mut s, &fp, &inhalt(), &quelle);
        let zustand = zustand_mit(s, &o);
        let (kanal, meldungen) = mitschrift();
        let melder = melder_fuer(&kanal);

        // Der erste: pruefen und verwerfen.
        let erster = blockweise_oeffnen(&zustand, &quelle, false, None, &mut Verwerfer, &melder)
            .expect("erster Durchgang");
        assert_eq!(erster.filename.as_deref(), Some("urlaub.mp4"));
        assert_eq!(erster.klartext_laenge, 3_500_000);
        let zuletzt = meldungen
            .lock()
            .expect("Mutex")
            .last()
            .cloned()
            .expect("Meldung");
        assert_eq!(
            zuletzt.bytes_erledigt, zuletzt.bytes_gesamt,
            "der Balken endet nicht auf voll"
        );

        // Der zweite: ablegen, nur mit demselben Fingerabdruck.
        let ziel = o.join("urlaub.mp4");
        let mut datei = Zwischendatei::neben(&ziel).expect("Zwischendatei");
        blockweise_oeffnen(
            &zustand,
            &quelle,
            false,
            Some(erster.fingerabdruck),
            &mut datei,
            &melder,
        )
        .expect("zweiter Durchgang");
        drop(datei);

        assert_eq!(std::fs::read(&ziel).expect("lesen"), inhalt());
        assert!(
            !cabrik_ablage::teil_pfad(&ziel).expect("Teil").exists(),
            "die Zwischendatei blieb liegen"
        );
    }

    #[test]
    fn ein_ausgetauschter_envelope_wird_nicht_abgelegt() {
        // Zwischen den Durchgaengen ersetzt jemand die Datei -- durch einen
        // einwandfreien Envelope an mich. Er ist trotzdem nicht der, den der
        // Nutzer gesehen hat.
        let o = ordner("ausgetauscht");
        let (mut s, fp) = sitzung_mit_mir();
        let quelle = o.join("urlaub.mp4.cabrik");
        envelope_an_mich(&mut s, &fp, &inhalt(), &quelle);
        let zustand = zustand_mit(s, &o);
        let (kanal, _) = mitschrift();
        let melder = melder_fuer(&kanal);

        let erster = blockweise_oeffnen(&zustand, &quelle, false, None, &mut Verwerfer, &melder)
            .expect("erster Durchgang");

        {
            let mut z = zustand.sitzung.lock().expect("Mutex");
            let s = z.as_mut().expect("Sitzung");
            envelope_an_mich(s, &fp, b"etwas ganz anderes", &quelle);
        }

        let ziel = o.join("urlaub.mp4");
        let mut datei = Zwischendatei::neben(&ziel).expect("Zwischendatei");
        let fehler = blockweise_oeffnen(
            &zustand,
            &quelle,
            false,
            Some(erster.fingerabdruck),
            &mut datei,
            &melder,
        )
        .expect_err("ein anderer Envelope wurde abgelegt");
        drop(datei);

        assert!(fehler.contains("verändert"), "{fehler}");
        assert!(!ziel.exists(), "unter dem Ziel steht etwas");
        assert!(
            !cabrik_ablage::teil_pfad(&ziel).expect("Teil").exists(),
            "die Zwischendatei blieb liegen"
        );
    }

    #[test]
    fn eine_gesperrte_sitzung_oeffnet_nichts() {
        let o = ordner("gesperrt");
        let (mut s, fp) = sitzung_mit_mir();
        let quelle = o.join("urlaub.mp4.cabrik");
        envelope_an_mich(&mut s, &fp, b"geheim", &quelle);
        s.sperren();
        let zustand = zustand_mit(s, &o);
        let (kanal, _) = mitschrift();
        let melder = melder_fuer(&kanal);

        let fehler = blockweise_oeffnen(&zustand, &quelle, false, None, &mut Verwerfer, &melder)
            .expect_err("gesperrt und trotzdem geoeffnet");
        assert!(fehler.contains("gesperrt"), "{fehler}");
    }

    #[test]
    fn das_original_stroemt_in_einen_lesbaren_envelope() {
        let o = ordner("original");
        let (mut s, fp) = sitzung_mit_mir();
        let quelle = o.join("urlaub.mp4");
        std::fs::write(&quelle, inhalt()).expect("schreiben");
        let (kanal, _) = mitschrift();
        let melder = melder_fuer(&kanal);

        let offen = s.offen(super::jetzt()).expect("offen");
        let plan = offen.versand_planen(&[fp], true).expect("Plan");
        let ergebnis = original_stroemen(
            offen,
            &plan,
            &quelle.display().to_string(),
            "urlaub.mp4",
            &melder,
        )
        .expect("stroemen");

        let ziel = ergebnis.ziel.expect("Ziel");
        assert!(ziel.ends_with("urlaub.mp4.cabrik"), "{ziel}");
        assert!(
            ergebnis.befund.is_none(),
            "das Original wird nicht bereinigt"
        );
        let env = std::fs::read(&ziel).expect("lesen");
        assert_eq!(ergebnis.bytes, env.len());

        offen.envelope_oeffnen(&env, true).expect("oeffnen");
        let (klartext, name) = offen.nutzlast().expect("Nutzlast");
        assert_eq!(klartext, inhalt().as_slice());
        assert_eq!(name, Some("urlaub.mp4"));
    }
}
