//! Passwörter einlesen.
//!
//! # Warum es kein `--password <text>` gibt
//!
//! Ein Passwort als Befehlszeilenargument ist auf jedem Mehrbenutzersystem
//! für **alle** sichtbar: Es steht in der Prozessliste (`ps`, Task-Manager)
//! und landet zusätzlich in der Shell-History, wo es dauerhaft auf der Platte
//! bleibt. Das ist kein theoretisches Problem — es ist der häufigste Weg, auf
//! dem Passwörter aus Skripten entweichen.
//!
//! Deshalb gibt es hier nur drei Wege, und keiner davon ist ein Argument:
//!
//! 1. **Abfrage im Terminal** — der Regelfall, verdeckte Eingabe.
//! 2. `--password-file <pfad>` — für Skripte; die Rechte der Datei sind
//!    kontrollierbar, die Prozessliste ist es nicht.
//! 3. `--password-stdin` — für Pipelines und Passwortmanager.
//!
//! `gpg` und `age` treffen dieselbe Unterscheidung.

use crate::fehler::{Ergebnis, Fehler};

use std::io::Read as _;
use std::path::Path;
use zeroize::Zeroizing;

/// Woher das Passwort kommt.
#[derive(Debug, Clone)]
pub enum Quelle {
    /// Verdeckte Abfrage im Terminal.
    Abfrage,
    /// Aus einer Datei.
    Datei(std::path::PathBuf),
    /// Von der Standardeingabe.
    Stdin,
}

impl Quelle {
    /// Wählt die Quelle aus den Schaltern.
    ///
    /// # Fehler
    ///
    /// [`Fehler::Bedienung`], wenn beide Schalter gesetzt sind.
    pub fn waehle(datei: Option<&Path>, stdin: bool) -> Ergebnis<Self> {
        match (datei, stdin) {
            (Some(_), true) => Err(Fehler::bedienung(
                "--password-file und --password-stdin schließen einander aus",
            )),
            (Some(p), false) => Ok(Self::Datei(p.to_path_buf())),
            (None, true) => Ok(Self::Stdin),
            (None, false) => Ok(Self::Abfrage),
        }
    }

    /// Ob die Quelle ohne Zutun des Nutzers liest.
    #[must_use]
    pub const fn ist_automatisch(&self) -> bool {
        !matches!(self, Self::Abfrage)
    }
}

/// Liest ein Passwort zum **Öffnen** — einmalige Eingabe.
///
/// # Fehler
///
/// Lesefehler der jeweiligen Quelle.
pub fn lies(quelle: &Quelle, anzeige: &str) -> Ergebnis<Zeroizing<Vec<u8>>> {
    match quelle {
        Quelle::Abfrage => {
            let s = Zeroizing::new(
                rpassword::prompt_password(format!("{anzeige}: "))
                    .map_err(|e| Fehler::datei("<terminal>", e))?,
            );
            Ok(Zeroizing::new(s.as_bytes().to_vec()))
        }
        Quelle::Datei(p) => {
            let roh = std::fs::read(p).map_err(|e| Fehler::datei(p, e))?;
            Ok(Zeroizing::new(trimme(&roh)))
        }
        Quelle::Stdin => {
            let mut roh = Vec::new();
            std::io::stdin()
                .read_to_end(&mut roh)
                .map_err(|e| Fehler::datei("<stdin>", e))?;
            let g = Zeroizing::new(roh);
            Ok(Zeroizing::new(trimme(&g)))
        }
    }
}

/// Liest ein Passwort zum **Festlegen** — mit Wiederholung.
///
/// Die Wiederholung entfällt bei Datei und Standardeingabe: Dort gäbe es
/// nichts zu vertippen, und ein zweites Lesen von `stdin` liefert nichts.
///
/// # Fehler
///
/// - Lesefehler der Quelle
/// - [`Fehler::Bedienung`], wenn die Eingaben nicht übereinstimmen oder das
///   Passwort leer ist
pub fn lies_neu(quelle: &Quelle, anzeige: &str) -> Ergebnis<Zeroizing<Vec<u8>>> {
    if quelle.ist_automatisch() {
        let p = lies(quelle, anzeige)?;
        pruefe_nicht_leer(&p)?;
        return Ok(p);
    }

    let erst = lies(quelle, anzeige)?;
    pruefe_nicht_leer(&erst)?;
    let zweit = lies(quelle, "Zur Bestätigung wiederholen")?;

    if erst.as_slice() != zweit.as_slice() {
        return Err(Fehler::bedienung(
            "Die Eingaben stimmen nicht überein — nichts wurde geschrieben",
        ));
    }
    Ok(erst)
}

fn pruefe_nicht_leer(p: &[u8]) -> Ergebnis<()> {
    if p.is_empty() {
        return Err(Fehler::bedienung("Ein leeres Passwort schützt nichts"));
    }
    Ok(())
}

/// Entfernt genau **einen** abschließenden Zeilenumbruch.
///
/// `echo geheim > pw.txt` hängt ein `\n` an. Würde es mitgelesen, wäre das
/// Passwort ein anderes als das eingetippte — ein Fehler, den niemand findet.
/// Weiter wird nicht getrimmt: Leerzeichen am Anfang oder Ende können
/// beabsichtigt sein, und stillschweigend Zeichen zu verwerfen wäre
/// schlimmer als der Zeilenumbruch.
fn trimme(roh: &[u8]) -> Vec<u8> {
    let ohne_lf = roh.strip_suffix(b"\n").unwrap_or(roh);
    let ohne_crlf = ohne_lf.strip_suffix(b"\r").unwrap_or(ohne_lf);
    ohne_crlf.to_vec()
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Fehlschlag soll den Test abbrechen")]
mod tests {
    use super::*;

    #[test]
    fn genau_ein_zeilenumbruch_faellt_weg() {
        assert_eq!(trimme(b"geheim\n"), b"geheim");
        assert_eq!(trimme(b"geheim\r\n"), b"geheim");
        assert_eq!(trimme(b"geheim"), b"geheim");
    }

    /// Zwei Umbrueche bedeuten: der zweite gehoert zum Passwort. Wer mehr
    /// wegwirft, aendert das Passwort hinter dem Ruecken des Nutzers.
    #[test]
    fn weitere_zeichen_bleiben_erhalten() {
        assert_eq!(trimme(b"geheim\n\n"), b"geheim\n");
        assert_eq!(trimme(b" geheim "), b" geheim ");
    }

    #[test]
    fn beide_schalter_zugleich_sind_ein_bedienfehler() {
        let e = Quelle::waehle(Some(Path::new("pw.txt")), true).unwrap_err();
        assert_eq!(e.code(), "USAGE");
    }

    #[test]
    fn ohne_schalter_wird_gefragt() {
        assert!(!Quelle::waehle(None, false).unwrap().ist_automatisch());
        assert!(Quelle::waehle(None, true).unwrap().ist_automatisch());
    }

    #[test]
    fn leeres_passwort_wird_abgelehnt() {
        assert!(pruefe_nicht_leer(b"").is_err());
        assert!(pruefe_nicht_leer(b"x").is_ok());
    }
}
