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
        return Ok(Path::new(&appdata).join("CabrikSecure"));
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
    schon_da_pruefen(pfad)?;
    schreib_atomar_gemeldet(pfad, daten, melden)
}

/// Weigert sich, wenn die Datei schon dasteht.
fn schon_da_pruefen(pfad: &Path) -> Ergebnis<()> {
    if pfad.exists() {
        return Err(Ablagefehler {
            meldung: format!(
                "{} gibt es bereits. Eine neue Identität darüber zu schreiben                  würde alles unlesbar machen, was an die bisherige gerichtet ist.",
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
/// Dateisystemfehler. Die Zwischendatei wird dann aufgeräumt, damit nicht
/// eine `.tmp` liegen bleibt, die beim nächsten Mal im Weg ist.
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
    verzeichnis_sicherstellen(pfad)?;

    let temp = pfad.with_extension("tmp");
    if let Err(e) = schreib_blockweise(&temp, daten, melden) {
        let _ = std::fs::remove_file(&temp);
        return Err(fehler(&temp, &e));
    }
    if let Err(e) = std::fs::rename(&temp, pfad) {
        let _ = std::fs::remove_file(&temp);
        return Err(fehler(pfad, &e));
    }
    Ok(())
}

/// Ein Block von einem MiB.
///
/// Groß genug, dass der Aufwand je Block nicht ins Gewicht fällt, klein
/// genug für eine Meldung, die sich noch bewegt.
const BLOCK: usize = 1024 * 1024;

/// Schreibt die Bytes blockweise und meldet nach jedem Block.
fn schreib_blockweise(
    ziel: &Path,
    daten: &[u8],
    melden: &mut dyn FnMut(u64, u64),
) -> std::io::Result<()> {
    use std::io::Write as _;

    let datei = std::fs::File::create(ziel)?;
    let mut schreiber = std::io::BufWriter::new(datei);
    let gesamt = daten.len() as u64;

    let mut geschrieben = 0_usize;
    for block in daten.chunks(BLOCK) {
        schreiber.write_all(block)?;
        geschrieben = geschrieben.saturating_add(block.len());
        melden(geschrieben as u64, gesamt);
    }

    // Erst leeren, DANN als fertig melden. Andersherum stünde der Balken
    // auf voll, während die Puffer noch auf die Platte gehen -- und bei
    // einem Fehler dabei hätte er eine Vollendung gemeldet, die es nie
    // gab.
    schreiber.flush()?;
    melden(gesamt, gesamt);
    Ok(())
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
            "{} ließ sich nicht beiseiteschieben: Es liegen schon tausend              verwaiste Fassungen daneben.",
            pfad.display()
        ),
    })
}
