//! Referenz-CLI für Cabrik Secure.
//!
//! # Wozu sie da ist
//!
//! Sie ist nicht nur ein Werkzeug, sondern der **erste echte Aufrufer** des
//! Kerns. Tests rufen Funktionen so auf, wie die Autorin sie gedacht hat; eine
//! Bedienoberfläche muss sie so aufrufen, wie ein Mensch sie braucht. Jeder
//! bisherige Entwurfsfehler des Projekts kam an genau dieser Naht heraus —
//! zuletzt das Austauschformat, dem der Post-Quantum-Schlüssel fehlte
//! (`spec/trust-store.md` §5.1).
//!
//! Deshalb kommt sie vor der Oberfläche und deckt den Kern **vollständig** ab.
//!
//! # Aufbau
//!
//! - [`ausgabe`] trennt Ergebnis von Darstellung: Jeder Befehl liefert Daten,
//!   `--json` entscheidet über die Form.
//! - [`geheimnis`] liest Passwörter — nie aus einem Argument, siehe dort.
//! - [`ablage`] kennt als Einzige Pfade und Dateien.
//! - [`befehl`] enthält je Befehl ein Modul.

// In einem Programm ohne Bibliotheksziel ist jedes `pub` per Definition nur
// kistenintern erreichbar. `pub(crate)` überall hinzuschreiben würde das
// Gegenteil von Klarheit bewirken: viel Rauschen ohne eine einzige zusätzliche
// Aussage. Im Kern gilt die Regel weiter, dort trägt sie.
#![expect(
    unreachable_pub,
    reason = "Binärkiste ohne Bibliotheksziel — nichts ist von außen erreichbar"
)]

mod ablage;
mod ausgabe;
mod befehl;
mod fehler;
mod geheimnis;

use ausgabe::Schreiber;
use clap::{Args, Parser, Subcommand, ValueEnum};
use fehler::Ergebnis;
use std::path::PathBuf;

/// Verschlüsselung mit ehrlichen Aussagen.
#[derive(Debug, Parser)]
#[command(name = "cabrik", version, about, long_about = None)]
struct Aufruf {
    #[command(subcommand)]
    befehl: Befehle,

    #[command(flatten)]
    global: Global,
}

/// Schalter, die für alle Befehle gelten.
#[derive(Debug, Args, Clone)]
struct Global {
    /// Ergebnis als JSON ausgeben.
    #[arg(long, global = true)]
    json: bool,

    /// Hinweise unterdrücken.
    #[arg(long, short, global = true)]
    quiet: bool,

    /// Pfad zum Keyfile.
    #[arg(long, global = true, value_name = "DATEI")]
    keyfile: Option<PathBuf>,

    /// Pfad zum Kontaktspeicher.
    #[arg(long, global = true, value_name = "DATEI")]
    contacts: Option<PathBuf>,

    /// Passwort aus einer Datei lesen statt es abzufragen.
    #[arg(long, global = true, value_name = "DATEI")]
    password_file: Option<PathBuf>,

    /// Passwort von der Standardeingabe lesen.
    #[arg(long, global = true)]
    password_stdin: bool,

    /// Grenze für die Dateigröße in Bytes.
    ///
    /// Voreingestellt sind 2 GB. Das Programm verarbeitet Dateien im
    /// Arbeitsspeicher und braucht dafür rund das 2,3-fache ihrer Größe.
    #[arg(long, global = true, value_name = "BYTES")]
    max_size: Option<u64>,
}

impl Global {
    fn schreiber(&self) -> Schreiber {
        Schreiber {
            json: self.json,
            still: self.quiet,
            stdout_belegt: false,
        }
    }

    fn passwortquelle(&self) -> Ergebnis<geheimnis::Quelle> {
        geheimnis::Quelle::waehle(self.password_file.as_deref(), self.password_stdin)
    }
}

#[derive(Debug, Subcommand)]
enum Befehle {
    /// Neue Identität erzeugen.
    Keygen(KeygenArgs),
    /// Eigene Identität anzeigen oder weitergeben.
    #[command(subcommand)]
    Identity(IdentityBefehl),
    /// Datei oder Text verschlüsseln.
    Encrypt(EncryptArgs),
    /// Envelope entschlüsseln.
    Decrypt(DecryptArgs),
    /// Zeigen, was ohne Schlüssel sichtbar ist.
    Inspect(InspectArgs),
    /// Kontakte verwalten.
    #[command(subcommand)]
    Contacts(ContactsBefehl),
    /// Safety Number mit einem Kontakt zum Vorlesen.
    SafetyNumber(SafetyNumberArgs),
    /// Metadaten prüfen und entfernen.
    #[command(subcommand)]
    Metadata(MetadataBefehl),
    /// Dateien sicher löschen.
    Shred(ShredArgs),
    /// Schlüssel aus Version 1 übernehmen.
    Migrate(MigrateArgs),
}

/// Stärke der Passwortableitung im Keyfile.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum KdfStufe {
    /// Untergrenze der Spezifikation: 64 MiB. Nur für schwache Geräte.
    Min,
    /// Empfohlen: 256 MiB, spürbar aber erträglich.
    Recommended,
    /// 1 GiB. Deutlich langsam, auch beim eigenen Entsperren.
    Strong,
}

/// Welches Verfahren die Verschlüsselung nutzt.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum SuiteWahl {
    /// Post-Quantum, wenn alle Empfänger es können; sonst klassisch.
    Auto,
    /// X25519 mit ChaCha20-Poly1305.
    Classical,
    /// X-Wing: X25519 **und** ML-KEM-768.
    Hybrid,
}

#[derive(Debug, Args)]
struct KeygenArgs {
    /// Wohin der Schlüssel geschrieben wird.
    #[arg(long, short, value_name = "DATEI")]
    out: Option<PathBuf>,

    /// Bezeichnung, nur lokal sichtbar.
    #[arg(long)]
    label: Option<String>,

    /// Ohne Signierschlüssel — Nachrichten sind dann nie einem Absender
    /// zuzuordnen, auch nicht dem eigenen.
    #[arg(long)]
    no_signing: bool,

    /// Stärke der Passwortableitung.
    #[arg(long, value_enum, default_value = "recommended")]
    kdf: KdfStufe,
}

#[derive(Debug, Subcommand)]
enum IdentityBefehl {
    /// Fingerprint und Schlüssel anzeigen.
    Show,
    /// Austausch-Nutzlast ausgeben, zum Weitergeben an andere.
    Export {
        /// In eine Datei schreiben statt auf die Standardausgabe.
        #[arg(long, short, value_name = "DATEI")]
        out: Option<PathBuf>,
    },
}

#[derive(Debug, Args)]
struct EncryptArgs {
    /// Zu verschlüsselnde Datei. `-` liest von der Standardeingabe.
    datei: PathBuf,

    /// Empfänger aus dem Kontaktspeicher, mehrfach möglich.
    #[arg(long = "to", value_name = "NAME")]
    an: Vec<String>,

    /// Empfänger als Austausch-Nutzlast, mehrfach möglich.
    #[arg(long = "to-key", value_name = "NUTZLAST")]
    an_schluessel: Vec<String>,

    /// Zusätzlich mit einem Passwort öffenbar machen.
    #[arg(long)]
    password: bool,

    /// Verfahren.
    #[arg(long, value_enum, default_value = "auto")]
    suite: SuiteWahl,

    /// Ohne Signatur senden, auch wenn ein Schlüssel geladen ist.
    #[arg(long)]
    anonymous: bool,

    /// Signieren, auch ohne Kontaktauflösung.
    #[arg(long, conflicts_with = "anonymous")]
    sign: bool,

    /// Länge auffüllen. Voreinstellung: an bei Text, aus bei Dateien.
    #[arg(long)]
    pad: bool,

    /// Länge nicht auffüllen.
    #[arg(long, conflicts_with = "pad")]
    no_pad: bool,

    /// Empfängerzahl mit Attrappen verschleiern.
    #[arg(long)]
    dummies: bool,

    /// Sendezeitpunkt mitschicken. Voreinstellung: nein.
    #[arg(long)]
    timestamp: bool,

    /// Metadaten der Nutzdatei vorher entfernen.
    #[arg(long)]
    strip_metadata: bool,

    /// Ausgabedatei. Voreinstellung: Eingabename mit `.cab`.
    #[arg(long, short, value_name = "DATEI")]
    out: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct DecryptArgs {
    /// Zu entschlüsselnde Datei.
    datei: PathBuf,

    /// Ausgabedatei. Voreinstellung: der im Envelope hinterlegte Name.
    #[arg(long, short, value_name = "DATEI")]
    out: Option<PathBuf>,

    /// Mit Passwort öffnen statt mit dem Keyfile.
    #[arg(long)]
    password: bool,

    /// Abbrechen, wenn die Nachricht nicht signiert ist.
    #[arg(long)]
    require_signature: bool,
}

#[derive(Debug, Args)]
struct InspectArgs {
    /// Zu prüfende Datei.
    datei: PathBuf,
}

#[derive(Debug, Subcommand)]
enum ContactsBefehl {
    /// Alle Kontakte auflisten.
    List,
    /// Kontakt aus einer Austausch-Nutzlast aufnehmen.
    Add {
        /// Die Nutzlast, oder `-` für die Standardeingabe.
        nutzlast: String,
        /// Anzeigename.
        #[arg(long)]
        name: String,
    },
    /// Einen Kontakt im Einzelnen anzeigen.
    Show {
        /// Name des Kontakts.
        name: String,
    },
    /// Kontakt als verifiziert markieren.
    Verify {
        /// Name des Kontakts.
        name: String,
        /// Auf welchem Weg verifiziert wurde.
        #[arg(long, value_enum, default_value = "safety-number")]
        via: VerifikationsWeg,
    },
    /// Schlüssel eines Kontakts lokal für ungültig erklären.
    Revoke {
        /// Name des Kontakts.
        name: String,
        /// Begründung.
        #[arg(long)]
        note: Option<String>,
    },
    /// Kontakt umbenennen — etwa einen automatisch aufgenommenen Absender.
    Rename {
        /// Bisheriger Name.
        name: String,
        /// Neuer Name.
        neu: String,
    },
    /// Kontakt entfernen. Verwirft auch die Schlüsselhistorie.
    Remove {
        /// Name des Kontakts.
        name: String,
    },
}

/// Weg der Verifikation.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum VerifikationsWeg {
    /// QR-Code, erfordert physische Nähe.
    QrCode,
    /// Safety Number vorgelesen.
    SafetyNumber,
    /// Fingerprint abgetippt.
    Fingerprint,
}

#[derive(Debug, Args)]
struct SafetyNumberArgs {
    /// Name des Kontakts.
    name: String,
}

#[derive(Debug, Subcommand)]
enum MetadataBefehl {
    /// Zeigen, welche Metadaten in einer Datei stecken.
    Inspect {
        /// Zu prüfende Datei.
        datei: PathBuf,
    },
    /// Metadaten entfernen.
    Strip {
        /// Zu bereinigende Datei.
        datei: PathBuf,
        /// Ausgabedatei.
        #[arg(long, short, value_name = "DATEI")]
        out: PathBuf,

        /// Zusätzlich Kommentare aus Office-Dokumenten entfernen.
        ///
        /// Betrifft nur die Anmerkungen — der Text bleibt Zeichen für
        /// Zeichen erhalten.
        #[arg(long)]
        remove_comments: bool,

        /// Zusätzlich nachverfolgte Änderungen annehmen.
        ///
        /// Wie „Alle Änderungen annehmen" in Word: Einfügungen bleiben,
        /// Löschungen verschwinden samt Text. **Das verändert den Inhalt.**
        #[arg(long)]
        accept_changes: bool,

        /// Bei PDF: welche Fassung eingeflacht wird, gezählt ab eins.
        ///
        /// Voreinstellung ist die zuletzt bearbeitete — also das, was ein
        /// Leser anzeigt. Vorher ansehen mit `metadata revisions`.
        #[arg(long, value_name = "N")]
        revision: Option<usize>,

        /// Bei PDF: die Änderungshistorie **nicht** entfernen.
        ///
        /// Für Fälle, in denen das Dokument nicht verändert werden darf —
        /// Beweismittel, Archivierung. Frühere Fassungen bleiben dann
        /// wiederherstellbar.
        #[arg(long, conflicts_with = "revision")]
        keep_history: bool,
    },
    /// Frühere Fassungen eines PDF anzeigen, ohne etwas zu verändern.
    Revisions {
        /// Zu prüfende Datei.
        datei: PathBuf,
    },
}

#[derive(Debug, Args)]
struct ShredArgs {
    /// Zu löschende Dateien.
    pfade: Vec<PathBuf>,

    /// Verzeichnis rekursiv löschen.
    #[arg(long, value_name = "VERZEICHNIS")]
    dir: Option<PathBuf>,

    /// Verzeichnisname zur Bestätigung — ohne ihn passiert nichts.
    #[arg(long, value_name = "NAME")]
    confirm: Option<String>,

    /// Zahl der Überschreibdurchgänge mit Zufall.
    #[arg(long, default_value_t = cabrik_shred::DEFAULT_PASSES)]
    passes: u8,

    /// Dateinamen vor dem Löschen nicht überschreiben.
    #[arg(long)]
    keep_name: bool,
}

#[derive(Debug, Args)]
struct MigrateArgs {
    /// Das alte Keyfile.
    datei: PathBuf,

    /// Wohin der übernommene Schlüssel geschrieben wird.
    #[arg(long, short, value_name = "DATEI")]
    out: PathBuf,

    /// Stärke der Passwortableitung im neuen Keyfile.
    #[arg(long, value_enum, default_value = "recommended")]
    kdf: KdfStufe,
}

fn main() -> std::process::ExitCode {
    let aufruf = Aufruf::parse();
    let schreiber = aufruf.global.schreiber();

    match fuehre_aus(&aufruf) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            schreiber.fehler(&e);
            std::process::ExitCode::FAILURE
        }
    }
}

fn fuehre_aus(aufruf: &Aufruf) -> Ergebnis<()> {
    let g = &aufruf.global;
    match &aufruf.befehl {
        Befehle::Keygen(a) => befehl::schluessel::keygen(g, a),
        Befehle::Identity(b) => befehl::schluessel::identity(g, b),
        Befehle::Migrate(a) => befehl::schluessel::migrate(g, a),
        Befehle::Encrypt(a) => befehl::krypto::encrypt(g, a),
        Befehle::Decrypt(a) => befehl::krypto::decrypt(g, a),
        Befehle::Inspect(a) => befehl::krypto::inspect(g, a),
        Befehle::Contacts(b) => befehl::kontakte::fuehre_aus(g, b),
        Befehle::SafetyNumber(a) => befehl::kontakte::safety_number(g, a),
        Befehle::Metadata(b) => befehl::werkzeuge::metadata(g, b),
        Befehle::Shred(a) => befehl::werkzeuge::shred(g, a),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory as _;

    /// clap prüft die Definition selbst — doppelte Kurzformen, unmögliche
    /// Kombinationen und fehlende Hilfetexte fallen hier auf.
    #[test]
    fn die_befehlsdefinition_ist_in_sich_stimmig() {
        Aufruf::command().debug_assert();
    }

    /// Ein Passwort als Argument wäre in der Prozessliste sichtbar. Es darf
    /// deshalb keinen solchen Schalter geben — siehe `geheimnis`.
    #[test]
    fn es_gibt_kein_passwort_argument() {
        let hilfe = Aufruf::command().render_long_help().to_string();
        assert!(
            !hilfe.contains("--password <"),
            "ein --password mit Wert waere in der Prozessliste sichtbar"
        );
    }
}
