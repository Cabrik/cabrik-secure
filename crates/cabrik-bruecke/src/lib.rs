//! Der Vertrag zwischen Kern und Oberfläche.
//!
//! # Warum es diese Schicht gibt
//!
//! Vor ihr gab es **drei** unabhängige Auffassungen desselben Sachverhalts:
//! die Typen in `cabrik-core` und `cabrik-metadata`, die von Hand gebauten
//! `json!`-Blöcke der CLI, und `kern/typen.ts` im Frontend. Keine zwei waren
//! aneinander geprüft, und der Kern trug nicht eine einzige
//! `Serialize`-Ableitung. Der Brückenvertrag war eine Vermutung.
//!
//! Diese Schicht macht ihn zu einer Tatsache. Sie ist bewusst **kein**
//! Bestandteil des Kerns: `cabrik-core` bleibt frei von serde und von jeder
//! Annahme darüber, wer die Daten anzeigt.
//!
//! # Was hier nie hineingehört
//!
//! Schlüsselmaterial. Kein Feld dieser Schicht trägt einen privaten
//! Schlüssel, ein Passwort oder abgeleitetes Geheimnis — und das ist der
//! Grund, warum die Umwandlung hier stattfindet und nicht im Kern: Was gar
//! nicht erst in einen serialisierbaren Typ gerät, kann nicht versehentlich
//! über die Brücke gehen (`spec/anzeige.md` §6).
//!
//! # Die Namensregel
//!
//! Rust schreibt `snake_case`, TypeScript `camelCase`. Statt das dem Zufall
//! zu überlassen, steht auf jedem Typ `#[serde(rename_all = "camelCase")]`.
//! Aufzählungen werden **intern getaggt** (`tag = "fall"`), weil die
//! Oberfläche danach unterscheidet und ein externes Tag dort umständliche
//! Sonderfälle erzwänge.

#![forbid(unsafe_code)]

use cabrik_core::trust::{Authenticity, Contact, TrustState, VerifiedVia};
use cabrik_metadata::model::{Finding, FindingKind, Severity, StripResult};
use cabrik_metadata::pdf;
use serde::Serialize;

/// Rohe Bytes als Hexziffern.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut s, b| {
        use core::fmt::Write as _;
        let _ = write!(s, "{b:02X}");
        s
    })
}

// ---------------------------------------------------------------------------
// Funde
// ---------------------------------------------------------------------------

/// Wie schwer ein einzelner Fund wiegt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Schwere {
    /// Farbprofil, Aufloesung, Orientierung.
    Gering,
    /// Kameramodell, Software, Bearbeitungszeit.
    Beachtlich,
    /// GPS, Klarname, Geraetenummer -- und Zweitkopien des Inhalts.
    Kritisch,
}

impl From<Severity> for Schwere {
    fn from(s: Severity) -> Self {
        match s {
            Severity::Minor => Self::Gering,
            Severity::Notable => Self::Beachtlich,
            Severity::Critical => Self::Kritisch,
        }
    }
}

/// Art eines Fundes.
///
/// **`FindingKind` ist im Kern `#[non_exhaustive]`.** Eine geschlossene
/// Aufzählung hier hinzuschreiben wäre deshalb eine Falle: Käme im Kern eine
/// Art hinzu, träfe die Oberfläche auf einen Wert, den sie nicht kennt — und
/// zeigte im besten Fall nichts an, im schlechteren etwas Falsches.
///
/// Deshalb gibt es [`Fundart::Unbekannt`]. Es ist kein Verlegenheitswert,
/// sondern derselbe Gedanke wie beim vierten Anzeigezustand: Lieber „ich
/// weiß nicht, was das ist" als eine plausible Einordnung ohne Grundlage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Fundart {
    /// Ortsangabe.
    Ortsangabe,
    /// Personenname.
    Personenname,
    /// Geraet oder Seriennummer.
    Geraet,
    /// Erzeugende Software.
    Software,
    /// Zeitangabe.
    Zeitangabe,
    /// Firmen- oder Organisationsname.
    Organisation,
    /// Eingebettetes Vorschaubild -- eine zweite Kopie des Inhalts.
    Vorschaubild,
    /// Zugeschnittenes Bild in einem Office-Dokument; das Original faehrt mit.
    ZugeschnittenesBild,
    /// Nachverfolgte Aenderung -- geloeschter Text ist noch enthalten.
    NachverfolgteAenderung,
    /// Farbprofil.
    Farbprofil,
    /// Anmerkung.
    Kommentar,
    /// Bearbeitungssitzung, etwa die Gesamtbearbeitungszeit.
    Bearbeitungssitzung,
    /// Urspruenglicher Dateiname.
    Dateiname,
    /// Unbekannte Erweiterung des Formats.
    UnbekannteErweiterung,
    /// Eine Art, die dieser Vertrag noch nicht kennt.
    Unbekannt,
}

impl From<FindingKind> for Fundart {
    fn from(k: FindingKind) -> Self {
        match k {
            FindingKind::Gps => Self::Ortsangabe,
            FindingKind::Author => Self::Personenname,
            FindingKind::Device => Self::Geraet,
            FindingKind::Software => Self::Software,
            FindingKind::Timestamp => Self::Zeitangabe,
            FindingKind::Organization => Self::Organisation,
            FindingKind::EmbeddedPreview => Self::Vorschaubild,
            FindingKind::CroppedImage => Self::ZugeschnittenesBild,
            FindingKind::TrackedChange => Self::NachverfolgteAenderung,
            FindingKind::ColorProfile => Self::Farbprofil,
            FindingKind::Comment => Self::Kommentar,
            FindingKind::EditingSession => Self::Bearbeitungssitzung,
            FindingKind::FileName => Self::Dateiname,
            FindingKind::UnknownExtension => Self::UnbekannteErweiterung,
            // Kein `unreachable!()`: `FindingKind` ist non_exhaustive, und
            // eine Panik in der Anzeigeschicht wäre der schlechteste
            // denkbare Umgang mit einer neuen Fundart.
            _ => Self::Unbekannt,
        }
    }
}

/// Ein einzelner Fund.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fund {
    /// Art des Fundes.
    pub art: Fundart,
    /// Wo er saß, etwa `"EXIF:GPSLatitude"`.
    pub ort: String,
    /// Der Wert, sofern darstellbar.
    pub wert: Option<String>,
    /// Wie schwer er wiegt.
    pub schwere: Schwere,
}

impl From<&Finding> for Fund {
    fn from(f: &Finding) -> Self {
        Self {
            art: f.kind.into(),
            ort: f.location.clone(),
            wert: f.value.clone(),
            schwere: f.severity.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Bereinigung
// ---------------------------------------------------------------------------

/// Ergebnis des Bereinigens.
///
/// **Vier Fälle, obwohl `StripResult` nur drei hat.** Der vierte,
/// [`Bereinigung::Fehler`], entsteht nicht aus einem `StripResult`, sondern
/// aus einem `Err` — die Datei ließ sich gar nicht lesen. Die Oberfläche
/// muss beides unterscheiden können: „Format nicht verstanden" ist keine
/// Aussage über die Datei, „nicht lesbar" ist eine.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "fall", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Bereinigung {
    /// Alle **bekannten** Träger des Formats wurden behandelt.
    #[serde(rename = "vollstaendig")]
    Vollstaendig {
        /// Was entfernt wurde.
        entfernt: Vec<Fund>,
        /// Das erkannte Format — gehört zwingend in die Anzeige.
        format: String,
    },
    /// Bereinigt, aber Reste sind benannt.
    Teilweise {
        /// Was entfernt wurde.
        entfernt: Vec<Fund>,
        /// Was bleiben musste.
        geblieben: Vec<Fund>,
        /// Warum.
        grund: String,
        /// Das erkannte Format -- gehoert zwingend in die Anzeige.
        format: String,
    },
    /// Format nicht verstanden. **Keine Aussage über Sauberkeit.**
    Unbekannt {
        /// Was das Format vermutlich war, soweit erkennbar.
        formathinweis: Option<String>,
    },
    /// Die Datei ließ sich nicht lesen.
    Fehler {
        /// Warum sie sich nicht lesen ließ.
        grund: String,
    },
}

impl Bereinigung {
    /// Aus einem Ergebnis des Kerns, mit dem Format daneben.
    ///
    /// Das Format steht nicht in `StripResult::Complete` — es kommt aus der
    /// Erkennung. Die Oberfläche braucht es aber zwingend: „Alle bekannten
    /// Metadaten entfernt" ohne Formatangabe wäre eine stärkere Aussage,
    /// als der Kern deckt (`spec/anzeige.md` §4.1).
    #[must_use]
    pub fn aus(ergebnis: &StripResult, format: &str) -> Self {
        match ergebnis {
            StripResult::Complete { removed } => Self::Vollstaendig {
                entfernt: removed.iter().map(Fund::from).collect(),
                format: format.to_owned(),
            },
            StripResult::Partial {
                removed,
                remaining,
                reason,
            } => Self::Teilweise {
                entfernt: removed.iter().map(Fund::from).collect(),
                geblieben: remaining.iter().map(Fund::from).collect(),
                grund: reason.clone(),
                format: format.to_owned(),
            },
            StripResult::Unknown { format_hint } => Self::Unbekannt {
                formathinweis: format_hint.clone(),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Absender
// ---------------------------------------------------------------------------

/// Wie das Vertrauen zustande kam.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Verifikationsweg {
    /// QR-Code gescannt -- erfordert physische Naehe.
    Qr,
    /// Safety Number vorgelesen und verglichen.
    SafetyNumber,
    /// Fingerprint abgeglichen.
    Fingerprint,
}

impl From<VerifiedVia> for Verifikationsweg {
    fn from(v: VerifiedVia) -> Self {
        match v {
            VerifiedVia::QrCode => Self::Qr,
            VerifiedVia::SafetyNumber => Self::SafetyNumber,
            VerifiedVia::Fingerprint => Self::Fingerprint,
        }
    }
}

/// Wer die Nachricht geschickt hat — und wie sicher das ist.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "fall", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Absender {
    /// Nicht signiert. **Ein legitimer Modus, kein Mangel.**
    Unsigniert,
    /// Gültige Signatur eines Schlüssels, den niemand kennt.
    Unbekannt {
        /// Der Ed25519-Signierschlüssel als Hexziffern.
        signierschluessel: String,
    },
    /// Bekannter Kontakt, aber **nie verifiziert**.
    Bekannt {
        /// Fingerprint des Kontakts.
        fingerprint: String,
        /// Anzeigename — eine Notiz des Nutzers, keine Zusicherung.
        name: String,
    },
    /// Verifizierter Kontakt. Der einzige Fall, der Grün verdient.
    Verifiziert {
        /// Fingerprint des Kontakts.
        fingerprint: String,
        /// Anzeigename — eine Notiz des Nutzers, keine Zusicherung.
        name: String,
        /// **Optional** -- der Kern kennt den Zeitpunkt nicht immer.
        verifiziert_am: Option<u64>,
        /// Auf welchem Weg — die Wege sind nicht gleichwertig
        /// (`spec/trust-store.md` §5).
        verifiziert_ueber: Option<Verifikationsweg>,
    },
    /// Der Schlüssel ist nicht der aktuelle des Kontakts.
    Gewechselt {
        /// Fingerprint des Kontakts.
        fingerprint: String,
        /// Anzeigename — eine Notiz des Nutzers, keine Zusicherung.
        name: String,
        /// Der abgeloeste Schluesselsatz, sofern bekannt.
        vorheriger_fingerprint: Option<String>,
        /// Ob er damals verifiziert war -- wiegt schwerer.
        vorher_verifiziert: bool,
    },
    /// Lokal als kompromittiert markiert.
    Widerrufen {
        /// Fingerprint des Kontakts.
        fingerprint: String,
        /// Anzeigename.
        name: String,
    },
}

// ---------------------------------------------------------------------------
// Kontakte
// ---------------------------------------------------------------------------

/// Vertrauenszustand eines Kontakts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Vertrauen {
    /// Trust on First Use — bekannt, aber nie geprüft.
    Gesehen,
    /// Aussenhalb des Kanals geprueft.
    Verifiziert,
    /// Der Kontakt tritt mit einem anderen Schluessel auf.
    Gewechselt,
    /// Lokal als kompromittiert markiert.
    Widerrufen,
}

impl From<TrustState> for Vertrauen {
    fn from(z: TrustState) -> Self {
        match z {
            TrustState::Seen => Self::Gesehen,
            TrustState::Verified => Self::Verifiziert,
            TrustState::Changed => Self::Gewechselt,
            TrustState::Revoked => Self::Widerrufen,
        }
    }
}

/// Ein Eintrag im Kontaktspeicher.
///
/// **Ohne die Schlüssel selbst.** Die Oberfläche zeigt den Fingerprint; das
/// Schlüsselmaterial bleibt in Rust.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Kontakt {
    /// Anzeigename -- eine Notiz des Nutzers, keine Zusicherung.
    pub name: String,
    /// Der Fingerprint, in voller Laenge.
    pub fingerprint: String,
    /// Vertrauenszustand.
    pub vertrauen: Vertrauen,
    /// Erstkontakt, Unix-Sekunden.
    pub seit: u64,
    /// Wann verifiziert wurde, sofern überhaupt.
    pub verifiziert_am: Option<u64>,
    /// Auf welchem Weg — die Wege sind nicht gleichwertig.
    pub verifiziert_ueber: Option<Verifikationsweg>,
    /// Freie Notiz des Nutzers.
    pub notiz: Option<String>,
    /// Ob der Kontakt einen Post-Quantum-Schlüssel führt.
    pub hat_post_quantum: bool,
    /// Die Safety Number gegenüber der eigenen Identität.
    pub safety_number: String,
}

impl Kontakt {
    /// Aus einem Kontakt des Speichers, mit der Safety Number daneben.
    ///
    /// Die Nummer ist eine **paarweise** Ableitung und hängt an der eigenen
    /// Identität — sie kann deshalb nicht aus dem Kontakt allein entstehen.
    #[must_use]
    pub fn aus(k: &Contact, safety_number: String) -> Self {
        Self {
            name: k.name.clone(),
            fingerprint: k.fingerprint().display_full(),
            vertrauen: k.state.into(),
            seit: k.first_seen,
            verifiziert_am: k.verified_at,
            verifiziert_ueber: k.verified_via.map(Into::into),
            notiz: k.note.clone(),
            hat_post_quantum: k.supports_post_quantum(),
            safety_number,
        }
    }
}

// ---------------------------------------------------------------------------
// Frühere PDF-Fassungen
// ---------------------------------------------------------------------------

/// Eine frühere Fassung eines PDF.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fassung {
    /// Zaehlung ab eins, aelteste zuerst.
    pub nummer: usize,
    /// Laenge der Datei bis zum Ende dieser Fassung.
    pub bytes: usize,
    /// Zahl der Seiten.
    pub seiten: usize,
    /// Ob dies die Fassung ist, die ein Leser anzeigt.
    pub wird_angezeigt: bool,
    /// Anfang des Textes, gekuerzt.
    pub auszug: String,
    /// Zeilen, die es **nur hier** gibt — also später entfernt wurden.
    pub nur_hier: Vec<String>,
}

impl From<&pdf::Fassung> for Fassung {
    fn from(f: &pdf::Fassung) -> Self {
        Self {
            nummer: f.nummer,
            bytes: f.bytes,
            seiten: f.seiten,
            wird_angezeigt: f.ist_aktuell,
            auszug: f.auszug.clone(),
            nur_hier: f.nur_hier.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Authentizität
// ---------------------------------------------------------------------------

impl Absender {
    /// Aus dem Urteil des Kerns.
    #[must_use]
    pub fn aus(a: &Authenticity) -> Self {
        match a {
            Authenticity::Unsigned => Self::Unsigniert,
            // Der Signierschluessel sind rohe Bytes -- eine Anzeigeform
            // daraus zu machen ist Aufgabe dieser Schicht, nicht des Kerns.
            Authenticity::SignedUnknown { sig_pub } => Self::Unbekannt {
                signierschluessel: hex(sig_pub),
            },
            Authenticity::SignedSeen { name, fingerprint } => Self::Bekannt {
                fingerprint: fingerprint.display_full(),
                name: name.clone(),
            },
            Authenticity::SignedVerified {
                name,
                fingerprint,
                verified_at,
                verified_via,
            } => Self::Verifiziert {
                fingerprint: fingerprint.display_full(),
                name: name.clone(),
                verifiziert_am: *verified_at,
                verifiziert_ueber: verified_via.map(Into::into),
            },
            Authenticity::SignedChanged {
                name,
                fingerprint,
                previous_fingerprint,
                previous_was_verified,
            } => Self::Gewechselt {
                fingerprint: fingerprint.display_full(),
                name: name.clone(),
                vorheriger_fingerprint: previous_fingerprint.map(|f| f.display_full()),
                vorher_verifiziert: *previous_was_verified,
            },
            Authenticity::SignedRevoked { name, fingerprint } => Self::Widerrufen {
                fingerprint: fingerprint.display_full(),
                name: name.clone(),
            },
        }
    }
}
