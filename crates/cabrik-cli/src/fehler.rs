//! Fehler der CLI.
//!
//! Der Kern kennt keine Dateien und keine Bedienoberfläche; hier kommen
//! Ein-/Ausgabefehler und Bedienfehler dazu.

use core::fmt;

/// Fehler eines CLI-Aufrufs.
#[derive(Debug)]
pub enum Fehler {
    /// Fehler aus dem Kryptokern.
    Kern(cabrik_core::Error),
    /// Dateizugriff.
    Datei {
        /// Betroffener Pfad.
        pfad: String,
        /// Ursache.
        ursache: std::io::Error,
    },
    /// Der Aufruf ergibt so keinen Sinn.
    Bedienung(String),
}

impl Fehler {
    /// Betrifft einen Pfad.
    pub fn datei(pfad: impl AsRef<std::path::Path>, ursache: std::io::Error) -> Self {
        Self::Datei {
            pfad: pfad.as_ref().display().to_string(),
            ursache,
        }
    }

    /// Bedienfehler mit Erläuterung.
    pub fn bedienung(text: impl Into<String>) -> Self {
        Self::Bedienung(text.into())
    }

    /// Maschinenlesbarer Kode.
    ///
    /// # Warum `NO_MATCHING_RECIPIENT` hier nicht vorkommt
    ///
    /// `spec/test-vectors.md` §7 verlangt, dass „falscher Schlüssel" und
    /// „nicht für dich bestimmt" **nach außen ununterscheidbar** bleiben.
    /// [`cabrik_core::Error`] trennt beide für Testvektoren und Fehlersuche;
    /// die CLI ist aber eine Außenschnittstelle. Sie meldet für beide Fälle
    /// denselben Kode, sonst wäre `--json` ein Orakel, das die Meldung
    /// bewusst nicht ist.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Kern(cabrik_core::Error::NoMatchingRecipient) => "AUTH_FAILED",
            Self::Kern(e) => e.code(),
            Self::Datei { .. } => "IO_ERROR",
            Self::Bedienung(_) => "USAGE",
        }
    }
}

impl fmt::Display for Fehler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kern(e) => write!(f, "{e}"),
            Self::Datei { pfad, ursache } => write!(f, "{pfad}: {ursache}"),
            Self::Bedienung(t) => f.write_str(t),
        }
    }
}

impl std::error::Error for Fehler {}

impl From<cabrik_core::Error> for Fehler {
    fn from(e: cabrik_core::Error) -> Self {
        Self::Kern(e)
    }
}

/// Kurzform für Ergebnisse der CLI.
pub type Ergebnis<T> = core::result::Result<T, Fehler>;

#[cfg(test)]
mod tests {
    use super::*;

    /// Die Unterscheidung, die der Kern für Testvektoren führt, darf über die
    /// CLI nicht nach außen dringen — weder im Text noch im Kode.
    #[test]
    fn entschluesselungsfehler_sind_nach_aussen_ununterscheidbar() {
        let a = Fehler::from(cabrik_core::Error::AuthFailed);
        let b = Fehler::from(cabrik_core::Error::NoMatchingRecipient);

        assert_eq!(a.to_string(), b.to_string());
        assert_eq!(
            a.code(),
            b.code(),
            "der Kode verriete, ob die Datei fuer diesen Schluessel bestimmt war"
        );
    }

    #[test]
    fn dateifehler_nennt_den_pfad() {
        let e = Fehler::datei(
            "C:\\fehlt.cab",
            std::io::Error::new(std::io::ErrorKind::NotFound, "nicht da"),
        );
        assert!(e.to_string().contains("fehlt.cab"));
        assert_eq!(e.code(), "IO_ERROR");
    }
}
