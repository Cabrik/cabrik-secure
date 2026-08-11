//! Je Befehl ein Modul.

pub mod kontakte;
pub mod krypto;
pub mod schluessel;
pub mod werkzeuge;

use crate::fehler::{Ergebnis, Fehler};
use std::path::Path;

/// Liest eine Datei, oder von der Standardeingabe bei `-`.
///
/// # Fehler
///
/// Dateizugriff.
pub fn lies_eingabe(pfad: &Path) -> Ergebnis<Vec<u8>> {
    use std::io::Read as _;

    if pfad.as_os_str() == "-" {
        let mut puffer = Vec::new();
        std::io::stdin()
            .read_to_end(&mut puffer)
            .map_err(|e| Fehler::datei("<stdin>", e))?;
        return Ok(puffer);
    }
    std::fs::read(pfad).map_err(|e| Fehler::datei(pfad, e))
}

/// Schreibt eine Datei und weigert sich, eine bestehende zu überschreiben.
///
/// # Warum keine stille Überschreibung
///
/// Ein Werkzeug, das Dateien vernichtet, ohne zu fragen, ist ein
/// Datenverlustwerkzeug. Der Schaden ist besonders bitter, wenn die
/// überschriebene Datei die einzige entschlüsselte Fassung war.
///
/// Die Weigerung nennt den Ausweg. Eine Meldung, die nur „geht nicht" sagt,
/// zwingt den Nutzer zu dem, was er ohnehin vermeiden wollte: die
/// bestehende Datei wegzuräumen.
///
/// # Fehler
///
/// - [`Fehler::Bedienung`], wenn die Datei existiert
/// - Dateisystemfehler
pub fn schreib_ausgabe(pfad: &Path, daten: &[u8]) -> Ergebnis<()> {
    if pfad.exists() {
        return Err(Fehler::bedienung(format!(
            "{} existiert bereits und wird nicht überschrieben.\n\n\
             Mit --out <anderer-name> ein anderes Ziel wählen.",
            pfad.display()
        )));
    }
    if let Some(v) = pfad.parent()
        && !v.as_os_str().is_empty()
    {
        std::fs::create_dir_all(v).map_err(|e| Fehler::datei(v, e))?;
    }
    std::fs::write(pfad, daten).map_err(|e| Fehler::datei(pfad, e))
}

/// Jetzt in Unix-Sekunden.
#[must_use]
pub fn jetzt() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
