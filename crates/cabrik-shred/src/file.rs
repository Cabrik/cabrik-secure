//! Löschen einzelner Dateien (`spec/shredding.md` §5, §6).

use crate::capability::{Assessment, ShredCapability, Warning, assess};
use crate::{DEFAULT_PASSES, GROW_BELOW, MAX_PASSES};

use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Einstellungen.
#[derive(Debug, Clone, Copy)]
pub struct ShredOptions {
    /// Zahl der Überschreibdurchgänge mit Zufall.
    ///
    /// Wird auf [`MAX_PASSES`] begrenzt. Siehe [`DEFAULT_PASSES`] zur Frage,
    /// warum mehr als einer nichts bringt.
    pub passes: u8,
    /// Ob die Datei vor dem Löschen mehrfach umbenannt wird.
    ///
    /// Der Dateiname bleibt sonst im MFT stehen — und er allein kann
    /// verräterisch genug sein.
    pub rename: bool,
}

impl Default for ShredOptions {
    fn default() -> Self {
        Self {
            passes: DEFAULT_PASSES,
            rename: true,
        }
    }
}

/// Was tatsächlich geschehen ist (`spec/shredding.md` §6).
///
/// Jeder Schritt wird **einzeln** gemeldet. Ein pauschales „Gelöscht" wie in
/// v1 gibt es nicht mehr.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShredOutcome {
    /// Der Pfad, wie er übergeben wurde.
    pub path: PathBuf,
    /// Was auf diesem Datenträger erreichbar war.
    pub capability: ShredCapability,
    /// Ob tatsächlich geschrieben wurde.
    pub overwritten: bool,
    /// Ob der Name überschrieben wurde.
    pub renamed: bool,
    /// Ob der Verzeichniseintrag verschwunden ist.
    pub removed: bool,
    /// Was dem Nutzer zusätzlich gesagt werden muss.
    pub warnings: Vec<Warning>,
    /// Warum es fehlschlug.
    pub error: Option<String>,
}

impl ShredOutcome {
    /// Ob alles gelungen ist.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.removed && self.error.is_none()
    }

    /// Meldung für die Oberfläche.
    ///
    /// Unterscheidet die vier Fälle aus `spec/shredding.md` §6. Ein
    /// pauschales „Gelöscht" kommt darin nicht vor.
    #[must_use]
    pub fn message(&self) -> String {
        if let Some(e) = &self.error {
            return format!("Fehlgeschlagen: {e}");
        }
        if !self.removed {
            return "Fehlgeschlagen: die Datei existiert weiterhin".to_owned();
        }
        if self
            .warnings
            .iter()
            .any(|w| matches!(w, Warning::CloudSynced { .. }))
        {
            return "Gelöscht, aber Kopien sind wahrscheinlich vorhanden".to_owned();
        }
        match self.capability {
            ShredCapability::Overwrite if self.overwritten => {
                "Überschrieben und gelöscht".to_owned()
            }
            ShredCapability::Overwrite | ShredCapability::BestEffort => {
                "Gelöscht — Überschreiben ist auf diesem Datenträger nicht verlässlich".to_owned()
            }
            ShredCapability::Unsupported => "Gelöscht, ohne Überschreiben".to_owned(),
        }
    }
}

/// Löscht eine einzelne Datei.
///
/// Ablauf nach `spec/shredding.md` §5. Gibt **immer** ein [`ShredOutcome`]
/// zurück — auch im Fehlerfall, damit der Aufrufer sieht, wie weit es kam.
///
/// # Panics
///
/// Nie.
#[must_use]
pub fn shred_file(path: &Path, opts: &ShredOptions) -> ShredOutcome {
    let Assessment {
        capability,
        mut warnings,
    } = assess(path);

    let mut outcome = ShredOutcome {
        path: path.to_path_buf(),
        capability,
        overwritten: false,
        renamed: false,
        removed: false,
        warnings: Vec::new(),
        error: None,
    };

    if !path.is_file() {
        outcome.warnings = warnings;
        outcome.error = Some("kein regulärer Dateipfad".to_owned());
        return outcome;
    }

    // §5 Schritt 3: Schreibschutz aufheben, sonst scheitert alles Weitere.
    if let Ok(meta) = path.metadata()
        && meta.permissions().readonly()
    {
        let mut p = meta.permissions();
        // Clippy warnt hier zu Recht im Allgemeinen: unter Unix setzt das die
        // Rechte auf 0o777. Hier ist es dennoch richtig — die Datei wird in
        // den nächsten Zeilen überschrieben und gelöscht, ein zu weites
        // Rechtebit überlebt den Aufruf also nicht.
        #[expect(
            clippy::permissions_set_readonly_false,
            reason = "Datei wird unmittelbar danach überschrieben und gelöscht"
        )]
        p.set_readonly(false);
        if fs::set_permissions(path, p).is_err() {
            outcome.warnings = warnings;
            outcome.error = Some("Schreibschutz ließ sich nicht aufheben".to_owned());
            return outcome;
        }
    }

    match ueberschreiben(path, opts.passes.clamp(1, MAX_PASSES)) {
        Ok(()) => outcome.overwritten = true,
        Err(e) => {
            // Nicht abbrechen: Löschen ohne Überschreiben ist immer noch
            // besser als gar nichts — aber es wird ehrlich gemeldet.
            outcome.error = Some(format!("Überschreiben fehlgeschlagen: {e}"));
        }
    }

    let mut aktuell = path.to_path_buf();
    if opts.rename {
        match umbenennen(&aktuell) {
            Ok(neu) => {
                outcome.renamed = true;
                aktuell = neu;
            }
            Err(_) => warnings.push(Warning::TimestampNotCleared),
        }
    }

    match fs::remove_file(&aktuell) {
        Ok(()) => outcome.removed = !aktuell.exists(),
        Err(e) => {
            outcome.error = Some(match &outcome.error {
                Some(vorher) => format!("{vorher}; Löschen fehlgeschlagen: {e}"),
                None => format!("Löschen fehlgeschlagen: {e}"),
            });
        }
    }

    outcome.warnings = warnings;
    outcome
}

/// §5 Schritte 4 bis 6.
fn ueberschreiben(path: &Path, passes: u8) -> std::io::Result<()> {
    let laenge = path.metadata()?.len();

    // §5.1: Kleine Dateien liegen resident im MFT-Eintrag. Vergrößern, damit
    // der Inhalt ausgelagert wird.
    let arbeitslaenge = laenge.max(GROW_BELOW);

    let mut datei = OpenOptions::new().write(true).open(path)?;
    datei.set_len(arbeitslaenge)?;

    let mut puffer = vec![0u8; 64 * 1024];

    for durchgang in 0..=passes {
        datei.seek(SeekFrom::Start(0))?;

        // Letzter Durchgang mit Nullen, davor mit Zufall (§5 Schritt 5).
        let mit_zufall = durchgang < passes;
        if !mit_zufall {
            puffer.fill(0);
        }

        let mut rest = arbeitslaenge;
        while rest > 0 {
            let n = usize::try_from(rest.min(puffer.len() as u64)).unwrap_or(puffer.len());
            let stueck = puffer
                .get_mut(..n)
                .ok_or_else(|| std::io::Error::other("Pufferlänge"))?;
            if mit_zufall {
                getrandom::fill(stueck).map_err(|_| std::io::Error::other("Zufallsquelle"))?;
            }
            datei.write_all(stueck)?;
            rest = rest.saturating_sub(n as u64);
        }

        datei.flush()?;
        datei.sync_all()?;
    }

    // §5 Schritt 6.
    datei.set_len(0)?;
    datei.sync_all()?;
    Ok(())
}

/// §5 Schritt 7: dreimal in gleichlange Zufallsnamen umbenennen.
///
/// Der Dateiname bleibt sonst im MFT stehen. Er allein kann verräterisch
/// genug sein — `Kuendigung_Arbeitgeber_vertraulich.pdf` sagt schon alles.
fn umbenennen(path: &Path) -> std::io::Result<PathBuf> {
    /// Genau 32 Zeichen, damit 5 Bits eines Zufallsbytes ohne Rest und damit
    /// ohne Restklassenverzerrung darauf abgebildet werden können.
    const ZEICHEN: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz012345";

    let verzeichnis = path
        .parent()
        .ok_or_else(|| std::io::Error::other("kein übergeordnetes Verzeichnis"))?;
    let laenge = path
        .file_name()
        .map_or(8, |n| n.to_string_lossy().chars().count())
        .max(8);

    let mut aktuell = path.to_path_buf();
    for _ in 0..3 {
        let mut zufall = vec![0u8; laenge];
        getrandom::fill(&mut zufall).map_err(|_| std::io::Error::other("Zufallsquelle"))?;

        let name: String = zufall
            .iter()
            .map(|b| {
                let idx = usize::from(*b & 0b0001_1111);
                char::from(*ZEICHEN.get(idx).unwrap_or(&b'x'))
            })
            .collect();

        let neu = verzeichnis.join(&name);
        if neu.exists() {
            continue;
        }
        fs::rename(&aktuell, &neu)?;
        aktuell = neu;
    }
    Ok(aktuell)
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Fehlschlag soll den Test abbrechen")]
mod tests {
    use super::*;

    /// Ein eigenes Verzeichnis je Test, ohne zusätzliche Abhängigkeit.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let mut zufall = [0u8; 8];
            getrandom::fill(&mut zufall).unwrap();
            let suffix: String = zufall.iter().map(|b| format!("{b:02x}")).collect();
            let p = std::env::temp_dir().join(format!("cabrik-shred-{name}-{suffix}"));
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }

        fn datei(&self, name: &str, inhalt: &[u8]) -> PathBuf {
            let p = self.0.join(name);
            fs::write(&p, inhalt).unwrap();
            p
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn datei_verschwindet() {
        let d = TempDir::new("weg");
        let p = d.datei("geheim.txt", b"vertraulicher Inhalt");

        let r = shred_file(&p, &ShredOptions::default());
        assert!(r.removed, "Datei existiert noch: {:?}", r.error);
        assert!(r.overwritten);
        assert!(r.is_success());
        assert!(!p.exists());
    }

    #[test]
    fn der_name_wird_ueberschrieben() {
        // §5 Schritt 7: Der Name bleibt sonst im MFT stehen.
        let d = TempDir::new("name");
        let p = d.datei("Kuendigung_vertraulich.pdf", b"x");

        let r = shred_file(&p, &ShredOptions::default());
        assert!(r.renamed, "die Datei wurde nicht umbenannt");
        assert!(r.removed);
    }

    #[test]
    fn umbenennen_laesst_sich_abschalten() {
        let d = TempDir::new("kein-rename");
        let p = d.datei("a.txt", b"x");
        let r = shred_file(
            &p,
            &ShredOptions {
                rename: false,
                ..ShredOptions::default()
            },
        );
        assert!(!r.renamed);
        assert!(r.removed);
    }

    #[test]
    fn kleine_dateien_werden_vor_dem_ueberschreiben_vergroessert() {
        // §5.1: sonst bliebe der Inhalt resident im MFT-Eintrag.
        let d = TempDir::new("klein");
        let p = d.datei("winzig.txt", b"nur ein paar Bytes");
        assert!(p.metadata().unwrap().len() < GROW_BELOW);

        let r = shred_file(&p, &ShredOptions::default());
        assert!(r.overwritten);
        assert!(r.removed);
    }

    #[test]
    fn schreibgeschuetzte_dateien_werden_behandelt() {
        let d = TempDir::new("readonly");
        let p = d.datei("geschuetzt.txt", b"Inhalt");

        let mut perms = p.metadata().unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&p, perms).unwrap();

        let r = shred_file(&p, &ShredOptions::default());
        assert!(
            r.removed,
            "v1 waere hier gescheitert und haette trotzdem Erfolg gemeldet: {:?}",
            r.error
        );
        assert!(
            r.warnings.contains(&Warning::WasReadOnly),
            "der Schreibschutz haette gemeldet werden muessen"
        );
    }

    /// Der Kernfehler aus v1.
    #[test]
    fn ein_fehlschlag_wird_nicht_als_erfolg_gemeldet() {
        let p = std::env::temp_dir().join("cabrik-gibt-es-nicht-12345.txt");
        let r = shred_file(&p, &ShredOptions::default());

        assert!(!r.removed);
        assert!(!r.is_success());
        assert!(r.error.is_some());
        assert!(
            r.message().starts_with("Fehlgeschlagen"),
            "v1 meldete hier 'Geloescht'"
        );
    }

    #[test]
    fn verzeichnisse_werden_hier_nicht_angenommen() {
        let d = TempDir::new("verzeichnis");
        let r = shred_file(&d.0, &ShredOptions::default());
        assert!(!r.is_success());
        assert!(r.error.is_some());
        assert!(d.0.exists(), "das Verzeichnis wurde angetastet");
    }

    #[test]
    fn die_meldung_verspricht_nie_zu_viel() {
        let d = TempDir::new("meldung");
        let p = d.datei("a.txt", b"x");
        let r = shred_file(&p, &ShredOptions::default());

        // Auf jedem Testsystem ist BestEffort das erwartete Ergebnis.
        if r.capability == ShredCapability::BestEffort {
            assert!(
                r.message().contains("nicht verlässlich"),
                "die Meldung verschweigt die Einschraenkung: {}",
                r.message()
            );
        }
        assert_ne!(
            r.message(),
            "Gelöscht",
            "ein pauschales 'Geloescht' gibt es nicht"
        );
    }

    #[test]
    fn kopien_werden_immer_erwaehnt() {
        // §4.3: die Warnung erscheint grundsaetzlich.
        let d = TempDir::new("kopien");
        let p = d.datei("a.txt", b"x");
        let r = shred_file(&p, &ShredOptions::default());
        assert!(
            r.warnings.contains(&Warning::CopiesMayExist),
            "Backups und Schattenkopien wurden nicht erwaehnt"
        );
    }

    #[test]
    fn mehrere_durchgaenge_funktionieren() {
        let d = TempDir::new("passes");
        let p = d.datei("a.txt", &vec![0xAAu8; 100 * 1024]);
        let r = shred_file(
            &p,
            &ShredOptions {
                passes: 3,
                rename: true,
            },
        );
        assert!(r.overwritten);
        assert!(r.removed);
    }

    #[test]
    fn null_durchgaenge_werden_auf_einen_angehoben() {
        let d = TempDir::new("null");
        let p = d.datei("a.txt", b"x");
        let r = shred_file(
            &p,
            &ShredOptions {
                passes: 0,
                rename: true,
            },
        );
        assert!(
            r.overwritten,
            "es muss mindestens einmal ueberschrieben werden"
        );
    }

    #[test]
    fn grosse_dateien_funktionieren() {
        let d = TempDir::new("gross");
        let p = d.datei("gross.bin", &vec![0x42u8; 300 * 1024]);
        let r = shred_file(&p, &ShredOptions::default());
        assert!(r.is_success());
    }
}
