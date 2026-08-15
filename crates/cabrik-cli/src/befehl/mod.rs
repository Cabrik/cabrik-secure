//! Je Befehl ein Modul.

pub mod kontakte;
pub mod krypto;
pub mod schluessel;
pub mod werkzeuge;

use crate::fehler::{Ergebnis, Fehler};
use std::path::Path;
use zeroize::Zeroizing;

/// Größte Datei, die ohne ausdrückliche Erlaubnis verarbeitet wird.
///
/// # Woher die Zahl kommt
///
/// Das Programm verarbeitet Dateien **im Arbeitsspeicher**, nicht blockweise
/// von Platte zu Platte. Gemessen im Release-Build, Spitze des
/// Arbeitssatzes beim Verschlüsseln:
///
/// | Dateigröße | ohne Passwort | mit Passwort |
/// |---|---|---|
/// | 50 MB | 104 MB | 310 MB |
/// | 100 MB | 204 MB | 360 MB |
/// | 200 MB | 404 MB | 460 MB |
/// | 400 MB | 804 MB | 804 MB |
///
/// Daraus zwei Modelle:
///
/// ```text
/// ohne Passwort:   Spitze = 2,0 x Dateigröße + 4 MB
/// mit Passwort:    dasselbe, zusätzlich rund 250 MB für Argon2
/// ```
///
/// # Was die erste Messung falsch verstanden hatte
///
/// Ursprünglich stand hier ein einzelner Wert — 200 MB ergaben 460 MB, also
/// „Faktor 2,3". Diese Messung lief **mit Passwort**, und die 60 MB über dem
/// Doppelten waren nicht etwa Aufschlag der Dateigröße, sondern ein Rest von
/// **Argon2s Speicherkosten**. Ein Sockel wurde für einen Faktor gehalten.
///
/// Sichtbar wird der Fehler erst bei kleinen Dateien: 50 MB mit Passwort
/// ergeben 310 MB — Faktor **6,2**, nicht 2,3. Und bei 400 MB fällt der
/// Unterschied ganz weg, weil Argon2 seinen Speicher freigibt, **bevor** die
/// großen Puffer ihre Spitze erreichen. Die beiden Anteile stapeln sich
/// nicht.
///
/// Für die Grenze selbst ändert das nichts: Bei 2 GB sind rund 4,2 GB nötig,
/// und der hinterlegte Faktor liegt mit Absicht etwas darüber. Falsch war
/// nicht die Zahl, sondern ihre Begründung.
///
/// **Warum überhaupt eine Grenze:** Ohne sie endet eine zu große Datei in
/// einem Speicherfehler — für den Nutzer ein Absturz ohne Erklärung. Eine
/// Grenze macht daraus eine verständliche Auskunft, die er obendrein
/// aufheben kann.
pub const MAX_DATEI_VOREINSTELLUNG: u64 = 2 * 1024 * 1024 * 1024;

/// Was das Verarbeiten an Arbeitsspeicher kostet, als Vielfaches der
/// Dateigröße.
///
/// Gemessen wurden **2,08** als höchster Wert (siehe
/// [`MAX_DATEI_VOREINSTELLUNG`]). Der hinterlegte Wert liegt bewusst darüber:
/// Die Meldung soll den Bedarf lieber leicht überschätzen als jemanden mit
/// einer zu knappen Zusage in einen Speicherfehler laufen lassen.
const SPEICHERFAKTOR: f64 = 2.3;

/// Was die Schlüsselableitung zusätzlich braucht, in Bytes.
///
/// Argon2 belegt nach `KdfParams::recommended` 256 MiB. Bei kleinen Dateien
/// ist das der **beherrschende** Anteil; bei großen fällt er nicht ins
/// Gewicht, weil er vor deren Spitze schon wieder frei ist.
const KDF_SPEICHER: u64 = 256 * 1024 * 1024;

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

    // Der größere der beiden Anteile bestimmt die Spitze — sie stapeln sich
    // nicht, weil Argon2 seinen Speicher freigibt, bevor die großen Puffer
    // ihren Höchststand erreichen. Bei dieser Dateigröße gewinnt ohnehin
    // immer der erste; die Fallunterscheidung steht da, damit die Formel
    // auch stimmt, wenn jemand die Grenze herabsetzt.
    #[expect(
        clippy::cast_precision_loss,
        reason = "nur zur Anzeige einer Groessenordnung"
    )]
    let bedarf_bytes = (groesse as f64 * SPEICHERFAKTOR).max(KDF_SPEICHER as f64);
    let bedarf = bedarf_bytes / (1024.0 * 1024.0 * 1024.0);
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
/// # Warum das Ergebnis in `Zeroizing` steckt
///
/// Beim Verschlüsseln ist dieser Puffer **der Klartext**. Ihn nach getaner
/// Arbeit im Speicher liegen zu lassen, während der Kern jeden Schlüssel
/// sorgfältig überschreibt, wäre inkonsequent — dieselbe Überlegung wie bei
/// [`cabrik_core::Opened`].
///
/// Beim Entschlüsseln ist es nur Chiffrat und damit unbedenklich. Es
/// trotzdem zu überschreiben kostet nichts Nennenswertes und erspart die
/// Frage, welcher Aufrufer welchen Fall hat.
///
/// # Fehler
///
/// Dateizugriff.
pub fn lies_eingabe(pfad: &Path) -> Ergebnis<Zeroizing<Vec<u8>>> {
    use std::io::Read as _;

    if pfad.as_os_str() == "-" {
        let mut puffer = Zeroizing::new(Vec::new());
        std::io::stdin()
            .read_to_end(&mut puffer)
            .map_err(|e| Fehler::datei("<stdin>", e))?;
        return Ok(puffer);
    }
    std::fs::read(pfad)
        .map(Zeroizing::new)
        .map_err(|e| Fehler::datei(pfad, e))
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

/// Prüft ein **neu gewähltes** Passwort gegen eine Untergrenze.
///
/// Die Zahlen stehen in `cabrik_core::passwort` — hier steht nur der Satz,
/// den ein Mensch an der Kommandozeile lesen soll.
///
/// # Fehler
///
/// [`Fehler::bedienung`], wenn es zu kurz ist.
pub fn pruefe_passwortlaenge(passwort: &[u8], mindest: usize, wofuer: &str) -> Ergebnis<()> {
    cabrik_core::passwort::pruefe(passwort, mindest).map_err(|_| {
        Fehler::bedienung(format!(
            "{wofuer}\n\
             Es muss mindestens {mindest} Zeichen haben. Kürzer lässt sich ein\n\
             reines Durchprobieren nicht ausschließen — gegen ein erratbares\n\
             Passwort hilft die Länge allerdings nicht."
        ))
    })
}
