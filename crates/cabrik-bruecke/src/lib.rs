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

use cabrik_core::envelope::ContentType;
use cabrik_core::trust::{Authenticity, Contact, TrustState, VerifiedVia};
use cabrik_metadata::model::{Finding, FindingKind, Severity, StripResult};
use cabrik_metadata::pdf;
use cabrik_shred::{Assessment, ShredCapability, ShredOutcome, Warning};
use serde::{Deserialize, Serialize};

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
///
/// **Der einzige Typ dieser Schicht, der auch hereinkommt.** Alle anderen
/// sind `Serialize` und sonst nichts, und das ist Absicht: Was nur
/// hinausgeht, kann die Oberfläche nicht erfinden. Wäre `Kontakt`
/// `Deserialize`, könnte ein Aufruf dem Kern einen ausgedachten Kontakt
/// reichen — mit Vertrauenszustand und Verifikationsdatum. Der Kern nimmt
/// stattdessen nur entgegen, was ein Mensch wirklich entschieden hat, und
/// bildet den Rest selbst.
///
/// Hier ist das Hereinkommen richtig: Auf welchem Weg jemand verglichen
/// hat, weiß nur er.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

// ---------------------------------------------------------------------------
// Das Ergebnis des Öffnens
// ---------------------------------------------------------------------------

/// Art der Nutzdaten.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Inhaltsart {
    /// Textnachricht.
    Text,
    /// Einzelne Datei.
    Datei,
}

impl From<ContentType> for Inhaltsart {
    fn from(a: ContentType) -> Self {
        match a {
            ContentType::Text => Self::Text,
            ContentType::File => Self::Datei,
        }
    }
}

/// Was beim Öffnen herauskommt.
///
/// # Was hier fehlt, fehlt mit Absicht
///
/// `Opened::plaintext` ist ein `Zeroizing<Vec<u8>>` — der eigentliche
/// Klartext. Dieser Typ trägt ihn **nicht**, und das ist der Grund, warum
/// die Umwandlung in dieser Schicht stattfindet und nicht im Kern: Was gar
/// nicht erst in einen serialisierbaren Typ gerät, kann nicht versehentlich
/// über die Brücke gehen.
///
/// Eine Ausnahme gibt es, und sie ist keine: Bei einer **Textnachricht**
/// ist der Text der Inhalt und zugleich das, was angezeigt werden soll.
/// Ihn zurückzuhalten hieße, die Nachricht nicht zu zeigen. Bei einer Datei
/// bekommt die Oberfläche nur Name und Größe; die Bytes gehen auf die
/// Platte, ohne dass die Anzeige sie berührt.
///
/// # Warum `absender` nicht aus `Opened` allein entsteht
///
/// `Opened::signer` sagt nur, **ob** signiert wurde und mit welchem
/// Schlüssel — nicht, wem er gehört. Die Zuordnung entsteht erst am
/// Kontaktspeicher. Der Kern hält beides getrennt, und diese Schicht setzt
/// es zusammen.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Geoeffnet {
    /// Art der Nutzdaten.
    pub art: Inhaltsart,
    /// Nur bei [`Inhaltsart::Text`] — dort ist der Text der Inhalt.
    pub text: Option<String>,
    /// Der mitgeschickte Dateiname, bereits auf Unbedenklichkeit geprüft.
    pub dateiname: Option<String>,
    /// Größe der Nutzdaten.
    pub groesse_bytes: usize,
    /// Sendezeitpunkt, sofern der Absender ihn mitgeschickt hat.
    pub zeitpunkt: Option<u64>,
    /// Wer geschickt hat — aus `signer` **und** Kontaktspeicher.
    pub absender: Absender,
    /// Ergebnis der Metadatenprüfung, sofern eine stattfand.
    pub metadaten: Option<Bereinigung>,
}

// ---------------------------------------------------------------------------
// Was ohne Schlüssel sichtbar ist
// ---------------------------------------------------------------------------

/// Was ein Mitleser **ohne** Schlüssel erkennen kann.
///
/// **Die Liste ist frei, nicht aufgezählt.** Ein früherer Entwurf führte
/// hier feste Felder für Dateiname und Größe, weil das die Lecks von
/// Version 1 sind. Das ist zu eng: Was ein Format preisgibt, hängt am
/// Format, und eine künftige Fassung leckte womöglich etwas anderes. Der
/// Kern gibt deshalb Sätze aus, keine Felder — und die Oberfläche zählt sie
/// auf, statt sie zu deuten.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Aussenansicht {
    /// Fassung des Formats, etwa `"v2"`.
    pub fassung: String,
    /// Verfahren, sofern erkennbar.
    pub suite: Option<String>,
    /// Zahl der Kapseln, sofern erkennbar.
    pub kapseln: Option<usize>,
    /// Größe der Datei.
    pub groesse_bytes: usize,
    /// Was ohne Schlüssel erkennbar ist. Leer heißt: nichts.
    pub offengelegt: Vec<String>,
}

// ---------------------------------------------------------------------------
// Sicheres Löschen
// ---------------------------------------------------------------------------

/// Was Überschreiben auf diesem Datenträger ausrichtet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Loeschfaehigkeit {
    /// Überschreiben wirkt tatsächlich.
    Ueberschreiben,
    /// Nicht verlässlich — der **Normalfall** auf heutigen Systemen.
    BestEffort,
    /// Netzlaufwerk, schreibgeschützt, oder kein Zugriff.
    NichtMoeglich,
}

impl From<ShredCapability> for Loeschfaehigkeit {
    fn from(c: ShredCapability) -> Self {
        match c {
            ShredCapability::Overwrite => Self::Ueberschreiben,
            ShredCapability::BestEffort => Self::BestEffort,
            ShredCapability::Unsupported => Self::NichtMoeglich,
        }
    }
}

/// Ein Vorbehalt beim Löschen.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "art", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Loeschvorbehalt {
    /// Die Datei liegt in einem Synchronisationsordner.
    CloudOrdner {
        /// Woran es erkannt wurde.
        hinweis: String,
    },
    /// Kopien außerhalb des Zugriffs sind nicht auszuschließen.
    ///
    /// Erscheint **immer**, außer es wurde positiv festgestellt, dass es
    /// sich um ein einfaches lokales Volume handelt.
    KopienMoeglich,
    /// Wechselmedium oder Netzlaufwerk.
    WechselOderNetz,
    /// Die Datei war schreibgeschützt; das Attribut wurde entfernt.
    WarSchreibgeschuetzt,
    /// Der Zeitstempel konnte nicht normalisiert werden.
    ZeitstempelBlieb,
}

impl From<&Warning> for Loeschvorbehalt {
    fn from(w: &Warning) -> Self {
        match w {
            Warning::CloudSynced { hinweis } => Self::CloudOrdner {
                hinweis: hinweis.clone(),
            },
            Warning::CopiesMayExist => Self::KopienMoeglich,
            Warning::RemovableOrNetwork => Self::WechselOderNetz,
            Warning::WasReadOnly => Self::WarSchreibgeschuetzt,
            Warning::TimestampNotCleared => Self::ZeitstempelBlieb,
            // Kein Auffangzweig: `Warning` ist -- anders als `FindingKind`
            // -- nicht `non_exhaustive`. Ein neuer Vorbehalt im Kern bricht
            // hier die Übersetzung, und das ist die bessere Nachricht: Ein
            // stiller Auffangzweig verschlucke ihn, und die Oberfläche
            // zeigte eine Warnung an, die nicht die gemeinte ist.
        }
    }
}

/// Was sich über eine Datei sagen lässt, **bevor** gelöscht wird.
///
/// Entspricht `cabrik_shred::Assessment`. Bewusst **ohne** Begründung im
/// Klartext: Ein früherer Entwurf führte hier ein Feld `grundlage` mit
/// Sätzen wie „NTFS auf rotierender Platte, keine Schattenkopien“. Der Kern
/// liefert so etwas nicht, und es zu erfinden hieße, der Oberfläche eine
/// Gewissheit zu geben, die niemand geprüft hat.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Loeschbeurteilung {
    /// Was erreichbar ist.
    pub faehigkeit: Loeschfaehigkeit,
    /// Was zusätzlich gesagt werden muss.
    pub vorbehalte: Vec<Loeschvorbehalt>,
}

impl From<&Assessment> for Loeschbeurteilung {
    fn from(a: &Assessment) -> Self {
        Self {
            faehigkeit: a.capability.into(),
            vorbehalte: a.warnings.iter().map(Loeschvorbehalt::from).collect(),
        }
    }
}

/// Was tatsächlich geschehen ist.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Loeschergebnis {
    /// Der Pfad, wie er übergeben wurde.
    pub pfad: String,
    /// Was auf diesem Datenträger erreichbar war.
    pub faehigkeit: Loeschfaehigkeit,
    /// Ob tatsächlich überschrieben wurde.
    pub ueberschrieben: bool,
    /// Ob der Name überschrieben wurde.
    pub umbenannt: bool,
    /// Ob der Verzeichniseintrag verschwunden ist.
    pub entfernt: bool,
    /// Was zusätzlich gesagt werden muss.
    pub vorbehalte: Vec<Loeschvorbehalt>,
    /// Warum es fehlschlug, sofern es das tat.
    pub fehler: Option<String>,
}

impl From<&ShredOutcome> for Loeschergebnis {
    fn from(o: &ShredOutcome) -> Self {
        Self {
            pfad: o.path.display().to_string(),
            faehigkeit: o.capability.into(),
            ueberschrieben: o.overwritten,
            umbenannt: o.renamed,
            entfernt: o.removed,
            vorbehalte: o.warnings.iter().map(Loeschvorbehalt::from).collect(),
            fehler: o.error.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Austausch-Nutzlast
// ---------------------------------------------------------------------------

/// Was beim Einlesen einer Austausch-Nutzlast herauskommt.
///
/// **Ohne Namen.** Die Nutzlast trägt keinen — der Empfänger vergibt ihn
/// selbst. Ein Name, der mitgeliefert würde, sähe wie eine Angabe des
/// Absenders aus und wäre doch nur eine Behauptung.
///
/// Der Fingerprint ist **neu berechnet**. Die acht Byte, die die Nutzlast
/// mitführt, sind ausdrücklich nur eine Prüfsumme gegen
/// Übertragungsfehler; ihnen zu vertrauen verbietet `spec/trust-store.md`
/// §5.1.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "fall", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Nutzlastbefund {
    /// Gelesen und brauchbar.
    Gelesen {
        /// Neu berechnet, nicht übernommen.
        fingerprint: String,
        /// Ohne ihn kann der Kontakt empfangen, aber nie signieren.
        hat_signierschluessel: bool,
        /// Ohne ihn ist nur die klassische Suite möglich.
        hat_post_quantum: bool,
        /// Ob dieser Fingerprint bereits im Speicher steht.
        schon_bekannt: Option<Bekannt>,
    },
    /// Erkennbar eine Cabrik-Nutzlast, aber unbrauchbar angekommen.
    ///
    /// **Ein Übertragungsfehler, kein Angriff.** Die Prüfsumme schützt
    /// gegen Verstümmelung, nicht gegen Fälschung — wer die Nutzlast
    /// austauscht, rechnet sie neu.
    Beschaedigt {
        /// Was dem Nutzer gesagt wird.
        grund: String,
    },
    /// Keine Cabrik-Austausch-Nutzlast.
    Unlesbar {
        /// Was dem Nutzer gesagt wird.
        grund: String,
    },
}

/// Ein Kontakt, den es schon gibt.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bekannt {
    /// Unter welchem Namen er im Verzeichnis steht.
    pub name: String,
    /// `false` heißt: derselbe Kontakt, anderer Schlüssel — der ernste Fall.
    pub gleicher_schluessel: bool,
}

// ---------------------------------------------------------------------------
// Sitzung (`spec/entsperrung.md`)
// ---------------------------------------------------------------------------

/// Nach welcher Untätigkeit gesperrt wird.
///
/// **Eine feste Liste und keine freie Zahl.** Freie Eingabe lädt zu „0" oder
/// „999999" ein — und das heißt „nie sperren", ohne dass jemand
/// *entschieden* hat, nie zu sperren. Jeder Eintrag einer Liste kann
/// dagegen seinen Preis danebenschreiben.
///
/// **Keine Werte über 60 Minuten.** Zwei oder vier Stunden sind keine eigene
/// Entscheidung, sondern dieselbe wie [`Sperrfrist::BisZumSchliessen`] — nur
/// als Vorsicht verkleidet.
///
/// Kommt aus der Oberfläche herein und ist deshalb `Deserialize`. Es ist
/// eine Einstellung des Nutzers, kein Zustand des Programms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Sperrfrist {
    /// Fremde Umgebung, Café, geteilter Arbeitsplatz.
    EineMinute,
    /// Fünf Minuten.
    FuenfMinuten,
    /// **Voreinstellung.**
    #[default]
    FuenfzehnMinuten,
    /// Dreißig Minuten.
    DreissigMinuten,
    /// Eine Stunde.
    EineStunde,
    /// Bis das Fenster geschlossen wird.
    ///
    /// Heißt, was es tut. Ein offener, unbeaufsichtigter Rechner bleibt
    /// dann offen — das gehört danebengeschrieben.
    BisZumSchliessen,
}

impl Sperrfrist {
    /// Wie viele Sekunden Untätigkeit erlaubt sind.
    ///
    /// `None` bei [`Sperrfrist::BisZumSchliessen`] — dort gibt es keine
    /// Frist, sondern nur das Schließen des Fensters.
    #[must_use]
    pub const fn sekunden(self) -> Option<u64> {
        match self {
            Self::EineMinute => Some(60),
            Self::FuenfMinuten => Some(300),
            Self::FuenfzehnMinuten => Some(900),
            Self::DreissigMinuten => Some(1_800),
            Self::EineStunde => Some(3_600),
            Self::BisZumSchliessen => None,
        }
    }
}

/// Was die Oberfläche über die Sitzung wissen muss.
///
/// **Kein Schlüsselmaterial, keine Bezeichnung der Identität.** Wer auf
/// einen gesperrten Bildschirm sieht, soll nicht erfahren, wessen Rechner
/// das ist.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sitzungsstand {
    /// Ob gerade gesperrt ist.
    pub gesperrt: bool,
    /// Die eingestellte Frist.
    pub frist: Sperrfrist,
    /// Sekunden bis zur Sperre.
    ///
    /// `None`, wenn gesperrt ist oder keine Frist läuft. Die Oberfläche
    /// leitet daraus die Warnstufen ab (`spec/entsperrung.md` §9) — die
    /// Schwellen sind eine Anzeigefrage und stehen deshalb dort, nicht
    /// hier.
    pub restsekunden: Option<u64>,
}

// ---------------------------------------------------------------------------
// Die eigene Identität
// ---------------------------------------------------------------------------

/// Wie stark die Passwortableitung sein soll.
///
/// Kommt aus der Oberfläche herein und ist deshalb `Deserialize`.
///
/// **Die Zahlen stehen nicht hier**, sondern in `cabrik_core::keyfile`. Dies
/// ist nur die Übersetzung — sonst gäbe es zwei Auslegungen von
/// „empfohlen", und beim nächsten Anheben bliebe eine davon stehen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum KdfStufe {
    /// Untergrenze der Spezifikation. Nur für schwache Geräte.
    Min,
    /// Die Voreinstellung — spürbar, aber erträglich.
    #[default]
    Empfohlen,
    /// Deutlich langsam, auch beim eigenen Entsperren.
    Stark,
}

impl From<KdfStufe> for cabrik_core::keyfile::KdfStufe {
    fn from(s: KdfStufe) -> Self {
        match s {
            KdfStufe::Min => Self::Min,
            KdfStufe::Empfohlen => Self::Empfohlen,
            KdfStufe::Stark => Self::Stark,
        }
    }
}

impl From<cabrik_core::keyfile::KdfStufe> for KdfStufe {
    fn from(s: cabrik_core::keyfile::KdfStufe) -> Self {
        match s {
            cabrik_core::keyfile::KdfStufe::Min => Self::Min,
            cabrik_core::keyfile::KdfStufe::Empfohlen => Self::Empfohlen,
            cabrik_core::keyfile::KdfStufe::Stark => Self::Stark,
        }
    }
}

/// Die eigene Identität, wie die Oberfläche sie zeigen darf.
///
/// # Was hier fehlt und immer fehlen wird
///
/// Jedes Schlüsselmaterial. Der Typ hat kein Feld dafür, und das ist die
/// einfachste Art, die Architekturregel durchzusetzen: Was nicht existiert,
/// kann nicht versehentlich angezeigt, protokolliert oder abgeschickt
/// werden.
///
/// # Warum es diesen Typ nur im entsperrten Zustand gibt
///
/// Weil `bezeichnung` **im verschlüsselten Teil** der Schlüsseldatei steht.
/// Wer auf einen gesperrten Bildschirm sieht, kann sie deshalb nicht lesen —
/// nicht weil die Oberfläche sie verschweigt, sondern weil sie ohne das
/// Passwort niemandem vorliegt (`spec/entsperrung.md` §4.1).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Identitaet {
    /// Die freie Bezeichnung aus der Schlüsseldatei, sofern eine gesetzt ist.
    pub bezeichnung: Option<String>,
    /// Der volle Fingerprint, in Gruppen zu vier Zeichen.
    pub fingerprint: String,
    /// Die ersten drei Gruppen — für Listen und Überschriften.
    pub fingerprint_kurz: String,
    /// Erstellungszeitpunkt, Unix-Sekunden.
    pub erzeugt_am: u64,
    /// Welcher Stufe die Ableitung entspricht — falls einer.
    ///
    /// `None` heißt: eigene Werte. Kein Fehler, sondern eine Möglichkeit,
    /// die die Kommandozeile bietet. Ein Etikett danebenzusetzen, das
    /// „ungefähr" passt, wäre eine Falschaussage über die Stärke.
    pub kdf: Option<KdfStufe>,
    /// Der tatsächliche Speicherbedarf der Ableitung, in MiB.
    ///
    /// Steht immer da, auch wenn die Stufe einen Namen hat. Die Zahl ist
    /// die Aussage; der Name ist die Abkürzung dafür.
    pub kdf_speicher_mib: u32,
    /// Ob signiert werden kann.
    ///
    /// Ohne Signierschlüssel ist eine Nachricht **nie** einem Absender
    /// zuzuordnen, auch nicht dem eigenen. Das ist ein gewählter Modus, kein
    /// Mangel, und wird deshalb neutral angezeigt.
    pub hat_signierschluessel: bool,
    /// Ob ein Post-Quantum-Schlüssel geführt wird. Fehlt bei v1-Übernahmen.
    pub hat_post_quantum: bool,
    /// Wo die Schlüsseldatei liegt — damit man sie sichern kann.
    pub pfad: String,
}
