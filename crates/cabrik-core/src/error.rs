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
    /// Die Passwortableitung braucht mehr Arbeitsspeicher, als hier zur
    /// Verfügung steht (`spec/keyfile-v2.md` §4.2).
    ///
    /// **Keine Eigenschaft der Datei, sondern des Rechners.** Dieselbe
    /// Datei geht auf einem anderen auf — oder auf diesem, sobald andere
    /// Programme Speicher freigeben. Deshalb darf der Fall nie als falsches
    /// Passwort erscheinen: Wer das liest, tippt sein richtiges Passwort
    /// wieder und wieder.
    InsufficientMemory {
        /// Was die Ableitung belegen würde, in Bytes.
        needed: u64,
        /// Was ihr zugestanden war, in Bytes — `None`, wenn keine Grenze
        /// bekannt war und der Rechner das Anlegen schlicht verweigert hat.
        limit: Option<u64>,
    },
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
            Self::InsufficientMemory { .. } => "INSUFFICIENT_MEMORY",
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
            // Die einzige Meldung mit Zahlen darin. Ohne sie wüsste niemand,
            // ob es um ein paar Megabyte geht oder um mehr, als der Rechner
            // überhaupt hat.
            Self::InsufficientMemory { needed, limit } => {
                return match limit {
                    Some(grenze) => write!(
                        f,
                        "Die Schlüsselableitung braucht {} Arbeitsspeicher. Auf \
                         diesem Rechner stehen dafür gerade {} zur Verfügung — \
                         andere Programme zu schließen kann helfen.",
                        Groesse::aufgerundet(*needed),
                        Groesse::abgerundet(*grenze)
                    ),
                    None => write!(
                        f,
                        "Die Schlüsselableitung braucht {} Arbeitsspeicher, und \
                         der Rechner konnte ihn gerade nicht bereitstellen. \
                         Andere Programme zu schließen kann helfen.",
                        Groesse::aufgerundet(*needed)
                    ),
                };
            }
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

/// Eine Speichergröße, so geschrieben wie überall in der Oberfläche:
/// „3.5 GiB“, „768.0 MiB“.
///
/// # GiB, nicht GB
///
/// Gerechnet wird in Zweierpotenzen, also heißt es auch so. Die Oberfläche
/// hat das ausdrücklich entschieden (`anzeige/zustand.ts`, `groesse`): Wer
/// „GB“ liest, rechnet mit 10⁹ Bytes, und „die Zahl soll die Wahrheit
/// sein“. Diese Meldung erscheint im Fenster neben deren Zahlen — sie
/// schreibt sie deshalb genauso, eine Nachkommastelle samt Punkt.
///
/// # Warum die Rundungsrichtung mitgegeben wird
///
/// Weil der Satz sonst lügt, ohne dass eine Zahl falsch wäre. Bedarf und
/// Grenze beide zum Nächsten gerundet, wird aus 1,04 gegen 1,00 GiB:
/// „braucht 1.0 GiB, zur Verfügung stehen 1.0 GiB“. Der **Bedarf wird
/// aufgerundet, die Grenze abgerundet** — dann steht der Abstand, den es
/// gibt, auch da.
struct Groesse {
    bytes: u64,
    aufrunden: bool,
}

impl Groesse {
    const fn aufgerundet(bytes: u64) -> Self {
        Self {
            bytes,
            aufrunden: true,
        }
    }

    const fn abgerundet(bytes: u64) -> Self {
        Self {
            bytes,
            aufrunden: false,
        }
    }

    /// Teilt und rundet in die gewählte Richtung.
    fn geteilt(&self, wert: u64, durch: u64) -> u64 {
        if self.aufrunden {
            wert.div_ceil(durch)
        } else {
            wert.checked_div(durch).unwrap_or(0)
        }
    }
}

impl fmt::Display for Groesse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const MIB: u64 = 1 << 20;
        const GIB: u64 = 1 << 30;
        let (einheit, name) = if self.bytes < GIB {
            (MIB, "MiB")
        } else {
            (GIB, "GiB")
        };
        // In Zehnteln, gerechnet als `bytes · 10 / einheit` und nicht als
        // `bytes / (einheit / 10)`: Der zweite Teiler geht nicht auf, und
        // genau 1 GiB -- die Stufe „Stark“ -- stuende aufgerundet als
        // „1.1 GiB“ da.
        let zehntel = self.geteilt(self.bytes.saturating_mul(10), einheit);
        write!(f, "{}.{} {name}", zehntel / 10, zehntel % 10)
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
            "INSUFFICIENT_MEMORY",
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
            Error::InsufficientMemory {
                needed: 0,
                limit: None,
            },
        ]
        .map(|e| e.code());
        assert_eq!(vorhanden.as_slice(), erwartet.as_slice());
    }

    /// Der Fall, für den es die Meldung gibt: Wer sie liest, darf nicht
    /// glauben, sein Passwort sei falsch.
    #[test]
    fn speichermangel_redet_nicht_vom_passwort() {
        for limit in [Some(2 << 30), None] {
            let m = Error::InsufficientMemory {
                needed: 3 << 30,
                limit,
            }
            .to_string();
            assert!(!m.contains("Passwort"), "irrefuehrende Meldung: {m}");
            assert!(m.contains("Arbeitsspeicher"), "{m}");
        }
    }

    /// Bedarf aufgerundet, Grenze abgerundet — sonst stünde zweimal
    /// dieselbe Zahl im Satz, und der Fehler sähe aus wie keiner.
    #[test]
    fn bedarf_und_grenze_stehen_nie_gleich_da() {
        const MIB: u64 = 1 << 20;
        const GIB: u64 = 1 << 30;
        let fall = |needed: u64, grenze: u64| {
            Error::InsufficientMemory {
                needed,
                limit: Some(grenze),
            }
            .to_string()
        };

        // Genau 1 GiB ist „1.0 GiB“ und nicht „1.1 GiB“: Der Teiler GIB / 10
        // geht nicht auf, und die Stufe „Stark“ läge genau darauf.
        let m = fall(GIB, 900 * MIB);
        assert!(m.contains("braucht 1.0 GiB "), "{m}");
        assert!(m.contains("gerade 900.0 MiB "), "{m}");

        // Knapp darüber gegen knapp darunter: zwei verschiedene Zahlen.
        let m = fall(GIB + 50 * MIB, GIB);
        assert!(m.contains("braucht 1.1 GiB "), "{m}");
        assert!(m.contains("gerade 1.0 GiB "), "{m}");

        let m = fall(3 * GIB + GIB / 2, 2 * GIB + 150 * MIB);
        assert!(m.contains("braucht 3.5 GiB "), "{m}");
        assert!(m.contains("gerade 2.1 GiB "), "{m}");
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
