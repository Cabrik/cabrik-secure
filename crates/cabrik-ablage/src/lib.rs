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
    verzeichnis_sicherstellen(pfad)?;

    let temp = pfad.with_extension("tmp");
    if let Err(e) = std::fs::write(&temp, daten) {
        let _ = std::fs::remove_file(&temp);
        return Err(fehler(&temp, &e));
    }
    if let Err(e) = std::fs::rename(&temp, pfad) {
        let _ = std::fs::remove_file(&temp);
        return Err(fehler(pfad, &e));
    }
    Ok(())
}
