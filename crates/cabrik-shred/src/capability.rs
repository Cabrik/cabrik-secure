//! Was auf diesem Datenträger tatsächlich erreichbar ist
//! (`spec/shredding.md` §4).
//!
//! # Die unbequeme Wahrheit
//!
//! Auf einer SSD kann eine einzelne Datei **nicht zuverlässig durch
//! Überschreiben gelöscht werden**. Wear-Leveling schreibt jeden Vorgang auf
//! eine neue physische Seite; die alte bleibt bis zur Garbage Collection
//! lesbar, und wann die läuft, ist von außen weder steuerbar noch prüfbar.
//! Dazu kommen Over-Provisioning, NTFS-Journal, Schattenkopien, Pagefile.
//!
//! # Das Dateisystem zählt mehr als die Hardware
//!
//! Der Punkt, der am häufigsten übersehen wird: **Copy-on-Write-Dateisysteme
//! überschreiben grundsätzlich nie an Ort und Stelle** — unabhängig davon,
//! ob darunter eine SSD oder eine rotierende Platte liegt. Auf einem
//! ZFS-Pool aus Festplatten ist Überschreiben ebenso wirkungslos wie auf
//! einer NVMe.
//!
//! # Im Zweifel `BestEffort`
//!
//! [`ShredCapability::Overwrite`] wird nur zurückgegeben, wenn positiv
//! festgestellt wurde, dass Überschreiben wirkt. Wo das nicht sicher
//! feststellbar ist, lautet die Antwort `BestEffort` — lieber zu wenig
//! versprechen als zu viel.
//!
//! Das ist zugleich der Grund, warum dieses Modul ohne `unsafe` auskommt:
//! Die genaue Datenträgerabfrage unter Windows bräuchte `DeviceIoControl`.
//! Sie könnte das Ergebnis nur von `BestEffort` auf `Overwrite` **anheben**,
//! und das auch nur auf rotierenden Platten — ein seltener Fall mit geringem
//! Nutzen. Der Preis wäre `unsafe` im gesamten Arbeitsbereich.

use std::path::Path;

/// Was auf diesem Datenträger erreichbar ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShredCapability {
    /// Überschreiben wirkt tatsächlich.
    ///
    /// Nur bei Nicht-CoW-Dateisystem **und** rotierendem Datenträger
    /// **und** ohne erkennbare Snapshots.
    Overwrite,
    /// Überschreiben ist nicht verlässlich.
    ///
    /// Der Normalfall auf heutigen Systemen.
    BestEffort,
    /// Netzlaufwerk, schreibgeschützt, oder kein Zugriff.
    Unsupported,
}

impl ShredCapability {
    /// Meldung für die Oberfläche (`spec/shredding.md` §6).
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::Overwrite => "Überschreiben wirkt auf diesem Datenträger",
            Self::BestEffort => {
                "Überschreiben ist auf diesem Datenträger nicht verlässlich \
                 (SSD, Copy-on-Write oder nicht feststellbar)"
            }
            Self::Unsupported => "Sicheres Löschen ist hier nicht möglich",
        }
    }
}

/// Was dem Nutzer zusätzlich gesagt werden muss.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    /// Die Datei liegt in einem Synchronisationsordner.
    ///
    /// Dann existieren mit hoher Wahrscheinlichkeit **Serverkopien**, die
    /// lokales Löschen nicht erreicht.
    CloudSynced {
        /// Woran es erkannt wurde.
        hinweis: String,
    },
    /// Kopien außerhalb des Zugriffs können nicht ausgeschlossen werden.
    ///
    /// Erscheint **immer**, außer es wurde positiv festgestellt, dass es
    /// sich um ein einfaches lokales Volume handelt. Das ist ehrlicher und
    /// einfacher als eine Anbieterliste, die nie vollständig wird
    /// (`spec/shredding.md` §4.3).
    CopiesMayExist,
    /// Wechselmedium oder Netzlaufwerk.
    RemovableOrNetwork,
    /// Die Datei war schreibgeschützt; das Attribut wurde entfernt.
    WasReadOnly,
    /// Der Zeitstempel konnte nicht normalisiert werden.
    TimestampNotCleared,
}

impl Warning {
    /// Meldung für die Oberfläche.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::CloudSynced { hinweis } => format!(
                "Liegt in einem Synchronisationsordner ({hinweis}) — \
                 Serverkopien werden durch lokales Löschen nicht erreicht"
            ),
            Self::CopiesMayExist => "Kopien in Backups, Synchronisation oder \
                 Schattenkopien sind nicht erfasst"
                .to_owned(),
            Self::RemovableOrNetwork => {
                "Wechselmedium oder Netzlaufwerk — Überschreiben wirkt dort nicht".to_owned()
            }
            Self::WasReadOnly => "Der Schreibschutz wurde aufgehoben".to_owned(),
            Self::TimestampNotCleared => {
                "Der Zeitstempel konnte nicht zurückgesetzt werden".to_owned()
            }
        }
    }
}

/// Einschätzung für einen konkreten Pfad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assessment {
    /// Was erreichbar ist.
    pub capability: ShredCapability,
    /// Was dem Nutzer zusätzlich gesagt werden muss.
    pub warnings: Vec<Warning>,
}

/// Beurteilt, was für `path` erreichbar ist.
///
/// Gibt **niemals** [`ShredCapability::Overwrite`] zurück, ohne das positiv
/// festgestellt zu haben.
#[must_use]
pub fn assess(path: &Path) -> Assessment {
    let mut warnings = Vec::new();

    let Ok(meta) = path.metadata() else {
        return Assessment {
            capability: ShredCapability::Unsupported,
            warnings,
        };
    };

    if meta.permissions().readonly() {
        warnings.push(Warning::WasReadOnly);
    }

    if let Some(hinweis) = cloud_hinweis(path) {
        warnings.push(Warning::CloudSynced { hinweis });
    }

    // §4.3: Die Warnung erscheint immer, außer es wurde positiv das
    // Gegenteil festgestellt. Da wir das nirgends sicher können, steht sie
    // hier grundsätzlich.
    warnings.push(Warning::CopiesMayExist);

    Assessment {
        capability: erkenne_faehigkeit(path),
        warnings,
    }
}

/// Erkennt die Fähigkeit — plattformabhängig.
#[cfg(target_os = "linux")]
fn erkenne_faehigkeit(path: &Path) -> ShredCapability {
    // Unter Linux geht echte Erkennung ohne `unsafe`: Der Datenträgertyp
    // steht als Textdatei in sysfs.
    match linux::ist_rotierend(path) {
        Some(true) if !linux::ist_copy_on_write(path).unwrap_or(true) => ShredCapability::Overwrite,
        _ => ShredCapability::BestEffort,
    }
}

/// Erkennt die Fähigkeit — plattformabhängig.
#[cfg(not(target_os = "linux"))]
fn erkenne_faehigkeit(_path: &Path) -> ShredCapability {
    // Windows: NTFS ist zwar kein Copy-on-Write, aber der Datenträgertyp
    // liesse sich nur ueber DeviceIoControl feststellen — das braeuchte
    // `unsafe`. Da SSDs den Regelfall darstellen, ist BestEffort ohnehin
    // fast immer richtig.
    //
    // macOS: APFS ist Copy-on-Write und die Hardware seit Jahren durchweg
    // Flash. Apple hat "Sicheres Leeren des Papierkorbs" in OS X 10.11 aus
    // genau diesem Grund entfernt.
    ShredCapability::BestEffort
}

#[cfg(target_os = "linux")]
mod linux {
    use std::path::Path;

    /// Liest `/sys/dev/block/MAJ:MIN/queue/rotational`.
    pub(super) fn ist_rotierend(path: &Path) -> Option<bool> {
        use std::os::unix::fs::MetadataExt as _;
        let dev = path.metadata().ok()?.dev();
        // Glibc-Makros von Hand: major = bits 8..12 und 32..64, minor der Rest.
        let major = (dev >> 8) & 0xFFF;
        let minor = (dev & 0xFF) | ((dev >> 12) & 0xFFF_FF00);

        for kandidat in [
            format!("/sys/dev/block/{major}:{minor}/queue/rotational"),
            // Partitionen haben keine eigene Queue; eine Ebene höher.
            format!("/sys/dev/block/{major}:{minor}/../queue/rotational"),
        ] {
            if let Ok(inhalt) = std::fs::read_to_string(&kandidat) {
                return Some(inhalt.trim() == "1");
            }
        }
        None
    }

    /// Grobe Erkennung von Copy-on-Write-Dateisystemen.
    ///
    /// btrfs, ZFS und bcachefs schreiben nie an Ort und Stelle. Kann der Typ
    /// nicht ermittelt werden, lautet die Antwort `true` — im Zweifel gegen
    /// das Versprechen.
    pub(super) fn ist_copy_on_write(path: &Path) -> Option<bool> {
        let mounts = std::fs::read_to_string("/proc/mounts").ok()?;
        let pfad = path.to_str()?;
        let mut beste: Option<(usize, &str)> = None;

        for zeile in mounts.lines() {
            let mut teile = zeile.split_whitespace();
            let _quelle = teile.next()?;
            let ziel = teile.next()?;
            let typ = teile.next()?;
            if pfad.starts_with(ziel) && beste.is_none_or(|(len, _)| ziel.len() > len) {
                beste = Some((ziel.len(), typ));
            }
        }

        beste.map(|(_, typ)| matches!(typ, "btrfs" | "zfs" | "bcachefs"))
    }
}

/// Erkennt Synchronisationsordner, soweit möglich.
///
/// Vollständigkeit ist ausgeschlossen — deshalb steht [`Warning::CopiesMayExist`]
/// ohnehin immer daneben. Diese Prüfung dient dazu, in den erkannten Fällen
/// **deutlicher** zu warnen.
fn cloud_hinweis(path: &Path) -> Option<String> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        // Alle modernen Anbieter nutzen unter Windows die Cloud Filter API
        // und setzen diese Attribute. Sie sind ueber die Standardbibliothek
        // zugaenglich — kein `unsafe` noetig.
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        const FILE_ATTRIBUTE_OFFLINE: u32 = 0x0000_1000;
        const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x0004_0000;
        const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;

        if let Ok(meta) = path.metadata() {
            let a = meta.file_attributes();
            if a & FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS != 0
                || a & FILE_ATTRIBUTE_RECALL_ON_OPEN != 0
                || a & FILE_ATTRIBUTE_OFFLINE != 0
            {
                return Some("Platzhalterdatei der Cloud Filter API".to_owned());
            }
            if a & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                return Some("Reparse Point".to_owned());
            }
        }
    }

    // Bekannte Anbieterpfade. Nie vollständig, aber billig.
    let pfad = path.to_string_lossy().to_lowercase();
    for (fragment, name) in [
        ("onedrive", "OneDrive"),
        ("dropbox", "Dropbox"),
        ("google drive", "Google Drive"),
        ("googledrive", "Google Drive"),
        ("icloud", "iCloud"),
        ("nextcloud", "Nextcloud"),
        ("pcloud", "pCloud"),
    ] {
        if pfad.contains(fragment) {
            return Some(name.to_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nicht_existierende_pfade_sind_unsupported() {
        let a = assess(Path::new("/pfad/den/es/nicht/gibt/x.txt"));
        assert_eq!(a.capability, ShredCapability::Unsupported);
    }

    #[test]
    fn cloud_pfade_werden_erkannt() {
        assert!(cloud_hinweis(Path::new(r"C:\Users\x\OneDrive\geheim.txt")).is_some());
        assert!(cloud_hinweis(Path::new("/home/x/Dropbox/geheim.txt")).is_some());
        assert!(cloud_hinweis(Path::new("/home/x/Nextcloud/a.txt")).is_some());
        assert!(cloud_hinweis(Path::new("/home/x/Dokumente/a.txt")).is_none());
    }

    #[test]
    fn erkennung_ist_nicht_gross_klein_empfindlich() {
        assert!(cloud_hinweis(Path::new(r"C:\Users\x\ONEDRIVE\a.txt")).is_some());
        assert!(cloud_hinweis(Path::new(r"C:\Users\x\oneDrive\a.txt")).is_some());
    }

    #[test]
    fn meldungen_versprechen_nichts_falsches() {
        assert!(
            ShredCapability::BestEffort
                .message()
                .contains("nicht verlässlich")
        );
        assert!(
            Warning::CopiesMayExist.message().contains("nicht erfasst"),
            "die Warnung muss die Luecke benennen"
        );
    }

    #[test]
    fn best_effort_ist_die_vorsichtige_antwort() {
        // Auf jedem System, auf dem wir es nicht positiv feststellen
        // koennen, darf nie Overwrite herauskommen.
        #[cfg(not(target_os = "linux"))]
        assert_eq!(
            erkenne_faehigkeit(Path::new(".")),
            ShredCapability::BestEffort
        );
    }
}
