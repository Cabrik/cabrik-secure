//! Fehlertypen nach `spec/test-vectors.md` §7.
//!
//! Die Fehlerkodes sind Teil der Spezifikation: Alle Implementierungen —
//! Desktop, iOS, Android — müssen dieselben Unterscheidungen treffen, damit
//! Negativvektoren prüfen können, ob eine Implementierung aus dem *richtigen*
//! Grund fehlschlägt.

use core::fmt;

/// Fehler des Krypto-Kerns.
///
/// Der Wortlaut von [`Display`](fmt::Display) ist die **nutzerseitige**
/// Meldung; [`Error::code`] liefert den maschinenlesbaren Kode für Tests und
/// Diagnose. Die beiden dürfen sich unterscheiden — siehe [`Error::code`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Formatversion unbekannt.
    UnsupportedVersion,
    /// Ciphersuite unbekannt oder nicht erlaubt.
    UnsupportedSuite,
    /// Struktur nicht lesbar.
    Malformed(&'static str),
    /// AEAD-Prüfung fehlgeschlagen (Manipulation oder falscher Schlüssel).
    AuthFailed,
    /// Keine Kapsel ließ sich mit diesem Schlüssel öffnen.
    NoMatchingRecipient,
    /// Stream endet ohne Abschluss-Chunk.
    Truncated,
    /// Chunk-Position stimmt nicht.
    ChunkOrder,
    /// Signatur vorhanden, Prüfung fehlgeschlagen.
    SignatureInvalid,
    /// Signatur gefordert, keine vorhanden.
    SignatureMissing,
    /// Passwort falsch oder Keyfile manipuliert.
    KeyfileAuthFailed,
    /// Der öffentliche Schlüssel eines Empfängers ist unbrauchbar.
    ///
    /// Tritt beim **Verschlüsseln** auf, nicht beim Entschlüsseln — etwa bei
    /// einem Punkt niedriger Ordnung oder einem Schlüssel aus lauter Nullen.
    /// Vorher lief das in [`Error::AuthFailed`] und meldete dem Nutzer
    /// „konnte nicht entschlüsselt werden", obwohl gerade verschlüsselt
    /// wurde. Der Fall kam beim Verdrahten der CLI heraus.
    InvalidRecipientKey,
}

impl Error {
    /// Maschinenlesbarer Kode nach `spec/test-vectors.md` §7.
    ///
    /// Anders als [`Display`](fmt::Display) unterscheidet dieser Wert
    /// [`Error::AuthFailed`] von [`Error::NoMatchingRecipient`]. Die
    /// Spezifikation verlangt, dass die beiden **nach außen ununterscheidbar**
    /// bleiben — ein Angreifer soll aus der Fehlermeldung nicht ablesen
    /// können, ob eine Kapsel für ihn bestimmt war. Für Testvektoren und
    /// Fehlersuche wird die Unterscheidung dennoch gebraucht.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "UNSUPPORTED_VERSION",
            Self::UnsupportedSuite => "UNSUPPORTED_SUITE",
            Self::Malformed(_) => "MALFORMED",
            Self::AuthFailed => "AUTH_FAILED",
            Self::NoMatchingRecipient => "NO_MATCHING_RECIPIENT",
            Self::Truncated => "TRUNCATED",
            Self::ChunkOrder => "CHUNK_ORDER",
            Self::SignatureInvalid => "SIGNATURE_INVALID",
            Self::SignatureMissing => "SIGNATURE_MISSING",
            Self::KeyfileAuthFailed => "KEYFILE_AUTH_FAILED",
            Self::InvalidRecipientKey => "INVALID_RECIPIENT_KEY",
        }
    }

    /// Zusatzangabe bei [`Error::Malformed`], sonst `None`.
    ///
    /// Nur für Fehlersuche gedacht und **nicht** Teil der nutzerseitigen
    /// Meldung.
    #[must_use]
    pub const fn detail(&self) -> Option<&'static str> {
        match self {
            Self::Malformed(d) => Some(d),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::UnsupportedVersion => {
                "Diese Datei wurde mit einer neueren Programmversion erstellt."
            }
            Self::UnsupportedSuite => {
                "Diese Datei verwendet ein Verschlüsselungsverfahren, das dieses Programm nicht kennt."
            }
            Self::Malformed(_) => "Die Datei ist beschädigt oder kein gültiger Envelope.",

            // Bewusst identischer Wortlaut: siehe Error::code.
            Self::AuthFailed | Self::NoMatchingRecipient => {
                "Die Datei konnte nicht entschlüsselt werden. Passt der Schlüssel?"
            }

            Self::Truncated => "Die Datei ist unvollständig.",
            Self::ChunkOrder => "Die Datei ist beschädigt.",
            Self::SignatureInvalid => "Die Signatur ist ungültig.",
            Self::SignatureMissing => "Die Nachricht ist nicht signiert.",
            Self::KeyfileAuthFailed => {
                "Der Schlüssel konnte nicht geöffnet werden. Ist das Passwort richtig?"
            }
            Self::InvalidRecipientKey => {
                "Der Schlüssel eines Empfängers ist unbrauchbar. Bitte die Identität neu austauschen."
            }
        };
        f.write_str(msg)
    }
}

/// Kurzform für Ergebnisse des Kerns.
pub type Result<T> = core::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_failed_und_no_matching_recipient_sind_nach_aussen_gleich() {
        // spec/test-vectors.md §7: die Unterscheidung darf einem Angreifer
        // nicht zugänglich sein.
        assert_eq!(
            Error::AuthFailed.to_string(),
            Error::NoMatchingRecipient.to_string()
        );
        // Für Tests bleibt sie erhalten.
        assert_ne!(Error::AuthFailed.code(), Error::NoMatchingRecipient.code());
    }

    #[test]
    fn malformed_detail_erscheint_nicht_in_der_nutzermeldung() {
        let e = Error::Malformed("stanza length 70000 exceeds 4096");
        assert_eq!(e.detail(), Some("stanza length 70000 exceeds 4096"));
        assert!(!e.to_string().contains("70000"));
    }

    #[test]
    fn alle_kodes_aus_der_spezifikation_sind_abgedeckt() {
        let erwartet = [
            "UNSUPPORTED_VERSION",
            "UNSUPPORTED_SUITE",
            "MALFORMED",
            "AUTH_FAILED",
            "NO_MATCHING_RECIPIENT",
            "TRUNCATED",
            "CHUNK_ORDER",
            "SIGNATURE_INVALID",
            "SIGNATURE_MISSING",
            "KEYFILE_AUTH_FAILED",
            "INVALID_RECIPIENT_KEY",
        ];
        let vorhanden = [
            Error::UnsupportedVersion,
            Error::UnsupportedSuite,
            Error::Malformed(""),
            Error::AuthFailed,
            Error::NoMatchingRecipient,
            Error::Truncated,
            Error::ChunkOrder,
            Error::SignatureInvalid,
            Error::SignatureMissing,
            Error::KeyfileAuthFailed,
            Error::InvalidRecipientKey,
        ]
        .map(|e| e.code());
        assert_eq!(vorhanden.as_slice(), erwartet.as_slice());
    }

    /// Ein Fehler beim Verschlüsseln darf nicht vom Entschlüsseln reden.
    /// Genau das tat `AuthFailed` an dieser Stelle.
    #[test]
    fn unbrauchbarer_empfaengerschluessel_redet_nicht_vom_entschluesseln() {
        let m = Error::InvalidRecipientKey.to_string();
        assert!(!m.contains("entschlüsselt"), "irrefuehrende Meldung: {m}");
        assert!(m.contains("Empfängers"));
    }
}
