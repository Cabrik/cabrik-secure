//! Wo die Dateien liegen und wie sie geschrieben werden.
//!
//! # Warum eine eigene Schicht
//!
//! Bis eben stand das in `cabrik-cli`. Beim Anschließen des Fensters wäre
//! es ein zweites Mal entstanden — und zwei Umsetzungen desselben
//! Verzeichnisses laufen auseinander: Die CLI schriebe dann woanders hin,
//! als die Anwendung liest, und niemandem fiele es auf, bis Kontakte
//! verschwinden.
//!
//! Dieselbe Überlegung wie beim Dateiformat, das aus demselben Grund in den
//! Kern gewandert ist.
//!
//! # Was hier nicht passiert
//!
//! Krypto. Diese Schicht liest und schreibt **Bytes**. Was sie bedeuten,
//! weiß `cabrik-core`; wer sie entschlüsseln darf, entscheidet
//! `cabrik-app`. Ein Dateizugriff, der nebenbei entschlüsselt, wäre an
//! beiden Stellen schwer zu prüfen.
//!
//! Vom Kern kennt sie eine einzige Schnittstelle: die
//! [`Ablage`](cabrik_core::envelope::Ablage), in die er beim blockweisen
//! Öffnen den Klartext gibt. Die [`Zwischendatei`] nimmt ihn an und
//! benennt erst um, wenn der Kern ihr den Beleg dafür gibt.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

/// Was beim Dateizugriff schiefgehen kann.
#[derive(Debug)]
pub struct Ablagefehler {
    /// Was dem Nutzer gesagt wird — mit Pfad, sonst sucht er selbst.
    pub meldung: String,
}

impl core::fmt::Display for Ablagefehler {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.meldung)
    }
}

impl core::error::Error for Ablagefehler {}

/// Ergebnis eines Dateizugriffs.
pub type Ergebnis<T> = core::result::Result<T, Ablagefehler>;

fn fehler(pfad: &Path, e: &std::io::Error) -> Ablagefehler {
    Ablagefehler {
        meldung: format!("{}: {e}", pfad.display()),
    }
}

// ---------------------------------------------------------------------------
// Wo
// ---------------------------------------------------------------------------

/// Der Name des Verzeichnisses unter `%APPDATA%` (Windows).
///
/// Öffentlich, weil ihn außer [`verzeichnis`] noch jemand kennen muss: der
/// Deinstaller des Fensters, der dieses Verzeichnis auf Wunsch löscht
/// (`crates/cabrik-fenster/nsis/haken.nsh`). Ein Test dort vergleicht
/// beide. Wer den Namen hier ändert, ohne dort nachzuziehen, ließe den
/// Deinstaller ins Leere löschen — und Schlüssel liegen, deren Löschen er
/// gerade zugesagt hat.
pub const ORDNER_WINDOWS: &str = "CabrikSecure";

/// Das Verzeichnis für Schlüssel und Kontakte.
///
/// Windows: `%APPDATA%\CabrikSecure`. Unix: `$XDG_CONFIG_HOME/cabrik`,
/// sonst `~/.config/cabrik`.
///
/// # Fehler
///
/// Wenn keine der Umgebungsvariablen gesetzt ist. Dann bleibt nur, den Pfad
/// ausdrücklich anzugeben — raten wäre schlechter, als zu fragen.
pub fn verzeichnis() -> Ergebnis<PathBuf> {
    if let Ok(appdata) = std::env::var("APPDATA")
        && !appdata.is_empty()
    {
        return Ok(Path::new(&appdata).join(ORDNER_WINDOWS));
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        return Ok(Path::new(&xdg).join("cabrik"));
    }
    if let Ok(home) = std::env::var("HOME")
        && !home.is_empty()
    {
        return Ok(Path::new(&home).join(".config").join("cabrik"));
    }
    Err(Ablagefehler {
        meldung: "Kein Konfigurationsverzeichnis feststellbar — bitte den Pfad \
                  ausdrücklich angeben."
            .to_owned(),
    })
}

/// Voreingestellter Pfad der Schlüsseldatei.
///
/// # Fehler
///
/// Siehe [`verzeichnis`].
pub fn keyfile_pfad(angabe: Option<&Path>) -> Ergebnis<PathBuf> {
    match angabe {
        Some(p) => Ok(p.to_path_buf()),
        None => Ok(verzeichnis()?.join("identity.cabrik-key")),
    }
}

/// Voreingestellter Pfad des Kontaktspeichers.
///
/// # Fehler
///
/// Siehe [`verzeichnis`].
pub fn kontakte_pfad(angabe: Option<&Path>) -> Ergebnis<PathBuf> {
    match angabe {
        Some(p) => Ok(p.to_path_buf()),
        None => Ok(verzeichnis()?.join("contacts.cabrik-contacts")),
    }
}

/// Legt das übergeordnete Verzeichnis an, falls nötig.
///
/// # Fehler
///
/// Dateisystemfehler.
pub fn verzeichnis_sicherstellen(pfad: &Path) -> Ergebnis<()> {
    if let Some(v) = pfad.parent()
        && !v.as_os_str().is_empty()
    {
        std::fs::create_dir_all(v).map_err(|e| fehler(v, &e))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Lesen und Schreiben
// ---------------------------------------------------------------------------

/// Liest eine Datei, falls es sie gibt.
///
/// **`None` ist kein Fehler.** Beim ersten Start gibt es weder Schlüssel-
/// noch Kontaktdatei, und das ist der Normalfall — nicht eine Störung, über
/// die jemand eine Meldung lesen müsste.
///
/// # Fehler
///
/// Alles außer „gibt es nicht": Rechte, kaputter Datenträger, ein
/// Verzeichnis statt einer Datei.
pub fn lies(pfad: &Path) -> Ergebnis<Option<Vec<u8>>> {
    match std::fs::read(pfad) {
        Ok(daten) => Ok(Some(daten)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(fehler(pfad, &e)),
    }
}

/// Schreibt eine Datei, die es noch nicht geben darf.
///
/// # Wofür das da ist
///
/// Für die Schlüsseldatei. Eine neue Identität über eine bestehende zu
/// schreiben ist der folgenschwerste Fehlgriff, den dieses Programm
/// zulassen könnte: Alles, was je an den alten Fingerprint verschlüsselt
/// wurde, wäre dauerhaft unlesbar — auch das, was noch gar nicht angekommen
/// ist, denn die Gegenseite verschlüsselt weiter an einen Schlüssel, den es
/// nicht mehr gibt. Es gibt keine Sicherung beim Hersteller.
///
/// Diese Prüfung steht **hier** und nicht beim Aufrufer, weil sie sonst
/// beim zweiten Aufrufer fehlte. Wer eine Datei anlegt, kann nicht
/// versehentlich eine überschreiben — er müsste die andere Funktion nehmen,
/// und die heißt anders.
///
/// # Fehler
///
/// Wenn es die Datei schon gibt, mit dem Pfad in der Meldung. Und alles,
/// woran [`schreib_atomar`] scheitern kann.
///
/// # Was das nicht ist
///
/// Kein Schutz gegen einen Wettlauf. Zwischen Prüfung und Umbenennen liegt
/// ein Augenblick, in dem ein zweiter Vorgang dieselbe Datei anlegen
/// könnte. Für einen Menschen, der eine Identität einrichtet, ist das
/// belanglos; es soll nur niemand mehr hineinlesen, als dasteht.
pub fn schreib_neu(pfad: &Path, daten: &[u8]) -> Ergebnis<()> {
    schon_da_pruefen(pfad)?;
    schreib_atomar(pfad, daten)
}

/// Wie [`schreib_neu`], sagt aber unterwegs, wie weit es ist.
///
/// # Fehler
///
/// Wie [`schreib_neu`].
pub fn schreib_neu_gemeldet(
    pfad: &Path,
    daten: &[u8],
    melden: &mut dyn FnMut(u64, u64),
) -> Ergebnis<()> {
    // Die Prüfung zuerst, und zwar dieselbe wie oben. Sie hier zu
    // wiederholen statt `schreib_neu` zu rufen, wäre die zweite Wahrheit
    // über dieselbe Frage -- deshalb steht sie in einer Hilfsfunktion.
    //
    // Vorher, damit nicht erst Gigabytes geschrieben werden, die dann
    // keinen Namen bekommen. Beim Umbenennen wird noch einmal geprüft.
    schon_da_pruefen(pfad)?;
    let mut z = Zwischendatei::neben(pfad)?;
    schreib_gemeldet_in(&mut z, daten, melden)?;
    z.als_neu()?;
    melden(daten.len() as u64, daten.len() as u64);
    Ok(())
}

/// Weigert sich, wenn die Datei schon dasteht.
fn schon_da_pruefen(pfad: &Path) -> Ergebnis<()> {
    if pfad.exists() {
        return Err(Ablagefehler {
            meldung: format!(
                "{} gibt es bereits. Eine neue Identität darüber zu schreiben \
                 würde alles unlesbar machen, was an die bisherige gerichtet ist.",
                pfad.display()
            ),
        });
    }
    Ok(())
}

/// Schreibt eine Datei — **erst daneben, dann umbenennen**.
///
/// Ein Absturz mitten im Schreiben darf nicht alle Kontakte vernichten.
/// Das Umbenennen ist auf jedem gängigen Dateisystem unteilbar: Entweder
/// steht danach die alte Fassung da oder die neue, nie eine halbe.
///
/// # Fehler
///
/// Dateisystemfehler. Die Zwischendatei wird dann aufgeräumt, damit nichts
/// liegen bleibt, was beim nächsten Mal im Weg ist (siehe
/// [`Zwischendatei`]).
pub fn schreib_atomar(pfad: &Path, daten: &[u8]) -> Ergebnis<()> {
    schreib_atomar_gemeldet(pfad, daten, &mut |_, _| {})
}

/// Wie [`schreib_atomar`], sagt aber unterwegs, wie weit es ist.
///
/// # Warum blockweise
///
/// Weil `fs::write` eine einzige Zeile ist, die Minuten dauern kann. Bei
/// einem Envelope von drei Gigabyte stünde die Anzeige die ganze Zeit auf
/// „Schreibe …" und rührte sich nicht — und das sieht aus wie ein
/// hängendes Programm, ausgerechnet am Ende eines langen Vorgangs.
///
/// # Was der Rückruf nicht darf
///
/// Lange dauern. Er läuft zwischen zwei Blöcken; wer dort etwas
/// Aufwendiges tut, macht das Schreiben langsamer als es war. Das
/// **Drosseln** ist Sache des Aufrufers: Diese Schicht weiß nicht, wohin
/// die Meldung geht.
///
/// # Fehler
///
/// Wie [`schreib_atomar`].
pub fn schreib_atomar_gemeldet(
    pfad: &Path,
    daten: &[u8],
    melden: &mut dyn FnMut(u64, u64),
) -> Ergebnis<()> {
    let mut z = Zwischendatei::neben(pfad)?;
    schreib_gemeldet_in(&mut z, daten, melden)?;
    z.ersetzend()?;
    // Erst nach dem Umbenennen als fertig melden. Davor stünde der Balken
    // auf voll, während noch geschrieben wird -- und bei einem Fehler dabei
    // hätte er eine Vollendung gemeldet, die es nie gab.
    melden(daten.len() as u64, daten.len() as u64);
    Ok(())
}

/// Ein Block von einem MiB.
///
/// Groß genug, dass der Aufwand je Block nicht ins Gewicht fällt, klein
/// genug für eine Meldung, die sich noch bewegt.
const BLOCK: usize = 1024 * 1024;

/// Schreibt die Bytes blockweise und meldet nach jedem Block.
fn schreib_gemeldet_in(
    z: &mut Zwischendatei,
    daten: &[u8],
    melden: &mut dyn FnMut(u64, u64),
) -> Ergebnis<()> {
    let gesamt = daten.len() as u64;
    for block in daten.chunks(BLOCK) {
        z.schreiben(block)?;
        melden(z.geschrieben(), gesamt);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Die Zwischendatei
// ---------------------------------------------------------------------------

/// Die Endung der Zwischendatei, mit dem Punkt.
///
/// Ein eigener Name und keiner, den es neben dem Ziel schon geben könnte:
/// Bis Oktober 2026 hieß die Zwischendatei `<ziel ohne Endung>.tmp`, und
/// angelegt wurde sie mit `File::create`. Beim Speichern von `bericht.pdf`
/// wäre eine `bericht.tmp` des Nutzers überschrieben und danach
/// umbenannt worden — fort, ohne dass jemand es bemerkt hätte.
pub const TEIL_ENDUNG: &str = ".cabrik-teil";

/// Wie die Zwischendatei zu einem Ziel heißt: `.<name>.cabrik-teil`,
/// daneben im selben Ordner.
///
/// Im selben Ordner, weil nur dort das Umbenennen unteilbar ist — und weil
/// Klartext dort entstehen soll, wo der Nutzer ihn hinlegt, nicht in
/// `%TEMP%` (`spec/shredding.md` §3.2). Der Punkt vorn verbirgt sie unter
/// Linux und macOS.
///
/// # Fehler
///
/// Wenn das Ziel keinen Dateinamen hat.
pub fn teil_pfad(ziel: &Path) -> Ergebnis<PathBuf> {
    let name = ziel.file_name().ok_or_else(|| Ablagefehler {
        meldung: format!("{} ist kein Dateiname.", ziel.display()),
    })?;
    let mut teil = std::ffi::OsString::from(".");
    teil.push(name);
    teil.push(TEIL_ENDUNG);
    Ok(ziel.with_file_name(teil))
}

/// Eine Datei, die **neben** ihrem Ziel entsteht und erst am Ende dessen
/// Namen bekommt (`spec/grosse-dateien.md` §4).
///
/// Unter dem Zielnamen steht deshalb nie eine halbe Datei: kein halber
/// Envelope, kein halber Klartext, keine halbe Schlüsseldatei. Bis zum
/// Umbenennen heißt sie wie [`teil_pfad`].
///
/// # Wie sie endet
///
/// - [`Zwischendatei::als_neu`]: Das Ziel darf es nicht geben.
/// - [`Zwischendatei::ersetzend`]: Ein bestehendes Ziel wird ersetzt.
/// - [`Zwischendatei::verwerfen`] — oder einfach fallen lassen: Sie
///   verschwindet.
///
/// Vor dem Umbenennen wird sie auf die Platte gezwungen. Sonst könnte nach
/// einem Stromausfall unter dem Zielnamen eine leere Datei stehen — bei
/// der Schlüsseldatei der schlechteste denkbare Ausgang, und bei einem
/// USB-Stick, der gleich nach der Meldung „fertig“ abgezogen wird, nicht
/// viel besser.
///
/// # Ein alter Rest
///
/// Liegt unter dem Namen der Zwischendatei schon etwas, ist es der Rest
/// eines abgebrochenen Vorgangs **mit demselben Ziel** — etwa nach einem
/// Absturz. Er wird entfernt, und [`Zwischendatei::alter_rest_entfernt`]
/// sagt es, damit der Aufrufer es weitersagen kann. Reste mit anderen
/// Namen bleiben unberührt: Ob gerade jemand anderes dorthin schreibt,
/// weiß diese Schicht nicht.
#[derive(Debug)]
pub struct Zwischendatei {
    ziel: PathBuf,
    teil: PathBuf,
    datei: Option<std::fs::File>,
    geschrieben: u64,
    alter_rest: bool,
    erledigt: bool,
}

impl Zwischendatei {
    /// Legt die Zwischendatei neben `ziel` an.
    ///
    /// # Fehler
    ///
    /// Dateisystemfehler — mit dem Pfad.
    pub fn neben(ziel: &Path) -> Ergebnis<Self> {
        verzeichnis_sicherstellen(ziel)?;
        let teil = teil_pfad(ziel)?;

        let alter_rest = match std::fs::remove_file(&teil) {
            Ok(()) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(fehler(&teil, &e)),
        };

        // `create_new`: Gibt es den Namen doch — ein Wettlauf zwischen
        // dem Entfernen und hier —, wird nichts überschrieben.
        let datei = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&teil)
            .map_err(|e| fehler(&teil, &e))?;

        Ok(Self {
            ziel: ziel.to_path_buf(),
            teil,
            datei: Some(datei),
            geschrieben: 0,
            alter_rest,
            erledigt: false,
        })
    }

    /// Hängt Bytes an.
    ///
    /// Ohne eigenen Puffer: Was hier hereinkommt, ist beim Entschlüsseln
    /// Klartext, und ein Puffer, der unbemerkt eine Kopie davon hält, ließe
    /// sich nicht überschreiben. Die Blöcke sind ohnehin groß genug.
    ///
    /// # Fehler
    ///
    /// Dateisystemfehler, etwa eine volle Platte.
    pub fn schreiben(&mut self, daten: &[u8]) -> Ergebnis<()> {
        use std::io::Write as _;

        let datei = self.datei.as_mut().ok_or_else(|| Ablagefehler {
            meldung: format!("{} ist schon abgeschlossen.", self.teil.display()),
        })?;
        datei.write_all(daten).map_err(|e| fehler(&self.teil, &e))?;
        self.geschrieben = self.geschrieben.saturating_add(daten.len() as u64);
        Ok(())
    }

    /// Wie viele Bytes bisher geschrieben wurden.
    #[must_use]
    pub const fn geschrieben(&self) -> u64 {
        self.geschrieben
    }

    /// Das Ziel, unter dem die Datei am Ende steht.
    #[must_use]
    pub fn ziel(&self) -> &Path {
        &self.ziel
    }

    /// Ob beim Anlegen der Rest eines abgebrochenen Vorgangs entfernt
    /// wurde.
    #[must_use]
    pub const fn alter_rest_entfernt(&self) -> bool {
        self.alter_rest
    }

    /// Gibt der Datei ihren Namen — **nur, wenn es das Ziel nicht gibt**.
    ///
    /// # Fehler
    ///
    /// Wenn das Ziel inzwischen dasteht, und Dateisystemfehler. Die
    /// Zwischendatei ist dann fort, das Ziel unberührt.
    pub fn als_neu(mut self) -> Ergebnis<PathBuf> {
        self.umbenennen(false)
    }

    /// Gibt der Datei ihren Namen und ersetzt dabei, was dort stand.
    ///
    /// # Fehler
    ///
    /// Dateisystemfehler. Die Zwischendatei ist dann fort, das Ziel
    /// unberührt.
    pub fn ersetzend(mut self) -> Ergebnis<PathBuf> {
        self.umbenennen(true)
    }

    /// Entfernt die Zwischendatei. Das Ziel bleibt unberührt.
    pub fn verwerfen(mut self) {
        self.wegraeumen();
    }

    fn umbenennen(&mut self, ersetzen: bool) -> Ergebnis<PathBuf> {
        let ergebnis = self.umbenennen_ohne_aufraeumen(ersetzen);
        if ergebnis.is_err() {
            self.wegraeumen();
        }
        ergebnis
    }

    fn umbenennen_ohne_aufraeumen(&mut self, ersetzen: bool) -> Ergebnis<PathBuf> {
        if let Some(datei) = self.datei.take() {
            datei.sync_all().map_err(|e| fehler(&self.teil, &e))?;
            // Hier geschlossen, vor dem Umbenennen.
            drop(datei);
        }
        if !ersetzen && self.ziel.exists() {
            return Err(Ablagefehler {
                meldung: format!(
                    "{} gibt es inzwischen. Überschrieben wird nichts.",
                    self.ziel.display()
                ),
            });
        }
        std::fs::rename(&self.teil, &self.ziel).map_err(|e| fehler(&self.ziel, &e))?;
        self.erledigt = true;
        Ok(self.ziel.clone())
    }

    fn wegraeumen(&mut self) {
        if self.erledigt {
            return;
        }
        self.datei = None;
        let _ = std::fs::remove_file(&self.teil);
        self.erledigt = true;
    }
}

impl Drop for Zwischendatei {
    /// Wer sie fallen lässt, ohne sie abzuschließen, hat abgebrochen.
    fn drop(&mut self) {
        self.wegraeumen();
    }
}

/// Die Ablage für das blockweise Öffnen (`spec/grosse-dateien.md` §4).
///
/// Umbenannt wird nur mit dem Beleg des Kerns, also erst, wenn Blöcke,
/// Trailer, Signatur und Füllung geprüft sind — und nur als **neue**
/// Datei: Wer eine empfangene Datei ablegt, überschreibt damit nichts.
impl cabrik_core::envelope::Ablage for Zwischendatei {
    type Fehler = Ablagefehler;

    fn schreiben(&mut self, klartext: &[u8]) -> Ergebnis<()> {
        Self::schreiben(self, klartext)
    }

    fn freigeben(&mut self, _beleg: &cabrik_core::envelope::Geprueft) -> Ergebnis<()> {
        self.umbenennen(false).map(|_| ())
    }

    fn verwerfen(&mut self) {
        self.wegraeumen();
    }
}

/// Entfernt eine Datei, falls es sie gibt.
///
/// **„Gibt es nicht" ist kein Fehler.** Wer löschen wollte, was nicht da
/// ist, hat sein Ziel erreicht — eine Meldung darüber wäre eine Störung
/// ohne Vorfall.
///
/// # Was das nicht ist
///
/// Sicheres Löschen. Diese Funktion entfernt den Verzeichniseintrag; der
/// Inhalt kann auf dem Datenträger stehen bleiben. Für die Schlüsseldatei
/// genügt das trotzdem: Ohne das Passwort ist sie ein Haufen Zufall, und
/// wer das Passwort hat, brauchte die Datei nicht wiederherzustellen.
///
/// # Fehler
///
/// Rechte, gesperrte Datei, kaputter Datenträger — mit dem Pfad.
pub fn loesche(pfad: &Path) -> Ergebnis<()> {
    match std::fs::remove_file(pfad) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(fehler(pfad, &e)),
    }
}

/// Schiebt eine Datei beiseite, statt sie zu löschen.
///
/// # Wofür
///
/// Für den verwaisten Kontaktspeicher. Wird eine neue Identität angelegt,
/// während noch der Speicher einer früheren daliegt, ist dieser **dauerhaft
/// nicht mehr zu öffnen** — er ist an einen Schlüssel versiegelt, den es
/// nicht mehr gibt. Bleibt er liegen, scheitert beim nächsten Start das
/// Entsperren: mit dem richtigen Passwort, an einer Datei, die niemand mehr
/// braucht.
///
/// # Warum nicht löschen
///
/// Weil es nicht nötig ist. Wegnehmen genügt, um den Weg frei zu machen,
/// und was daneben liegt, kann niemand mehr lesen — aber es ist auch nicht
/// an uns, es zu vernichten. Wer sich vertan hat und den alten Schlüssel
/// doch noch findet, hätte sonst nichts mehr, worauf er ihn anwenden kann.
///
/// Gibt zurück, wohin verschoben wurde, oder `None`, wenn es nichts zu
/// verschieben gab.
///
/// # Fehler
///
/// Dateisystemfehler — mit dem Pfad.
pub fn verschiebe_beiseite(pfad: &Path) -> Ergebnis<Option<PathBuf>> {
    if !pfad.exists() {
        return Ok(None);
    }
    let stamm = pfad.as_os_str().to_string_lossy().into_owned();

    // Durchnummeriert, nicht mit Zeitstempel: Es soll auch dann
    // funktionieren, wenn die Uhr falsch geht -- und lesbar bleiben.
    for nr in 0..1_000_u32 {
        let ziel = PathBuf::from(if nr == 0 {
            format!("{stamm}.verwaist")
        } else {
            format!("{stamm}.verwaist-{nr}")
        });
        if ziel.exists() {
            continue;
        }
        std::fs::rename(pfad, &ziel).map_err(|e| fehler(pfad, &e))?;
        return Ok(Some(ziel));
    }
    Err(Ablagefehler {
        meldung: format!(
            "{} ließ sich nicht beiseiteschieben: Es liegen schon tausend \
             verwaiste Fassungen daneben.",
            pfad.display()
        ),
    })
}
