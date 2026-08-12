//! Je Befehl ein Modul.

pub mod kontakte;
pub mod krypto;
pub mod schluessel;
pub mod werkzeuge;

use crate::fehler::{Ergebnis, Fehler};
use std::path::Path;

/// Größte Datei, die ohne ausdrückliche Erlaubnis verarbeitet wird.
///
/// # Woher die Zahl kommt
///
/// Das Programm verarbeitet Dateien **im Arbeitsspeicher**, nicht
/// blockweise von Platte zu Platte. Gemessen wurde der Bedarf beim
/// Verschlüsseln:
///
/// | Dateigröße | Spitzenbedarf | Faktor |
/// |---|---|---|
/// | 200 MB | 460 MB | 2,3 |
///
/// Bei 2 GB sind das rund 4,6 GB — auf einem Rechner mit 8 GB gerade noch
/// vertretbar. Darüber steigt das Risiko, dass der Vorgang mitten in der
/// Arbeit abbricht.
///
/// **Warum überhaupt eine Grenze:** Ohne sie endet eine zu große Datei in
/// einem Speicherfehler — für den Nutzer ein Absturz ohne Erklärung. Eine
/// Grenze macht daraus eine verständliche Auskunft, die er obendrein
/// aufheben kann.
pub const MAX_DATEI_VOREINSTELLUNG: u64 = 2 * 1024 * 1024 * 1024;

/// Was das Verarbeiten an Arbeitsspeicher kostet, als Vielfaches der
/// Dateigröße. Gemessen, nicht geschätzt — siehe [`MAX_DATEI_VOREINSTELLUNG`].
const SPEICHERFAKTOR: f64 = 2.3;

/// Prüft die Größe, bevor gelesen wird.
///
/// # Fehler
///
/// [`Fehler::Bedienung`], wenn die Datei über der Grenze liegt.
pub fn pruefe_groesse(pfad: &Path, grenze: Option<u64>) -> Ergebnis<()> {
    if pfad.as_os_str() == "-" {
        // Bei der Standardeingabe ist die Größe vorher nicht bekannt.
        return Ok(());
    }
    let Ok(meta) = std::fs::metadata(pfad) else {
        return Ok(());
    };
    let grenze = grenze.unwrap_or(MAX_DATEI_VOREINSTELLUNG);
    let groesse = meta.len();
    if groesse <= grenze {
        return Ok(());
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "nur zur Anzeige einer Groessenordnung"
    )]
    let bedarf = (groesse as f64 * SPEICHERFAKTOR) / (1024.0 * 1024.0 * 1024.0);
    #[expect(
        clippy::cast_precision_loss,
        reason = "nur zur Anzeige einer Groessenordnung"
    )]
    let gb = groesse as f64 / (1024.0 * 1024.0 * 1024.0);

    Err(Fehler::bedienung(format!(
        "{} ist {gb:.1} GB groß.\n\n\
         Dieses Programm verarbeitet Dateien im Arbeitsspeicher, nicht blockweise \
         von Platte zu Platte.\nDafür wären rund {bedarf:.1} GB frei nötig — \
         gemessen wurde das {SPEICHERFAKTOR}-fache der Dateigröße.\n\n\
         Wenn Ihr Rechner so viel Arbeitsspeicher frei hat, heben Sie die Grenze auf:\n  \
         --max-size {}\n\n\
         Die Grenze gibt es, damit Sie das hier lesen statt einen Speicherfehler \
         mitten im Vorgang.",
        pfad.display(),
        // Auf das nächste volle Gigabyte aufrunden — eine Zahl, die sich
        // abtippen lässt und in jedem Fall über der Dateigröße liegt.
        groesse
            .div_ceil(1024 * 1024 * 1024)
            .saturating_mul(1024 * 1024 * 1024),
    )))
}

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
