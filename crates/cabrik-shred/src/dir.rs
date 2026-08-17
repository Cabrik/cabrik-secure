//! Rekursives Löschen von Verzeichnissen (`spec/shredding.md` §5.2).
//!
//! Rekursion wird unterstützt, weil Nutzer sonst Dateien einzeln auswählen
//! und dabei welche übersehen — das ist schlechter als eine gut abgesicherte
//! rekursive Funktion.
//!
//! Weil ein Fehlgriff **unwiderruflich** ist, gelten harte Leitplanken:
//!
//! 1. Vorschau vor der Ausführung: [`preview`] liefert Pfad, Dateizahl und
//!    Gesamtgröße.
//! 2. Bestätigung durch **Eintippen des Verzeichnisnamens**. Ein Klick auf
//!    „OK" genügt bei einer unumkehrbaren Aktion nicht.
//! 3. Kategorische Verweigerung bei Laufwerkswurzeln, Benutzerprofil,
//!    Systemverzeichnissen und allem, was ein `.git` enthält.
//! 4. **Symlinks und Junctions werden niemals verfolgt.** Ein Link im Baum
//!    darf nicht dazu führen, dass außerhalb gelöscht wird.
//! 5. Von innen nach außen.
//! 6. Verzeichnisnamen werden ebenfalls überschrieben — auch sie stehen im
//!    MFT.
//! 7. Ein Fehler bei einer Datei bricht den Vorgang nicht ab.

use crate::capability::Warning;
use crate::file::{ShredOptions, ShredOutcome, shred_file};

use std::fs;
use std::path::{Path, PathBuf};

/// Warum ein Verzeichnis nicht gelöscht werden darf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Laufwerkswurzel.
    DriveRoot,
    /// Benutzerprofil oder Heimatverzeichnis.
    HomeDirectory,
    /// Systemverzeichnis.
    SystemDirectory,
    /// Enthält ein Git-Repository.
    ContainsRepository,
    /// Ist ein Symlink oder eine Junction.
    IsLink,
    /// Existiert nicht oder ist kein Verzeichnis.
    NotADirectory,
    /// Der eingetippte Name stimmt nicht.
    NameMismatch {
        /// Was hätte eingetippt werden müssen.
        expected: String,
    },
}

impl Refusal {
    /// Meldung für die Oberfläche.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::DriveRoot => "Laufwerkswurzeln werden nicht gelöscht".to_owned(),
            Self::HomeDirectory => "Das Benutzerprofil wird nicht gelöscht".to_owned(),
            Self::SystemDirectory => "Systemverzeichnisse werden nicht gelöscht".to_owned(),
            Self::ContainsRepository => {
                "Das Verzeichnis enthält ein Git-Repository — zu gefährlich".to_owned()
            }
            Self::IsLink => {
                "Verknüpfungen werden nicht verfolgt, sonst würde außerhalb gelöscht".to_owned()
            }
            Self::NotADirectory => "Kein Verzeichnis".to_owned(),
            Self::NameMismatch { expected } => {
                format!("Zur Bestätigung muss der Verzeichnisname eingetippt werden: {expected}")
            }
        }
    }
}

/// Was gelöscht würde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    /// Vollständiger Pfad.
    pub path: PathBuf,
    /// Zahl der Dateien.
    pub file_count: usize,
    /// Gesamtgröße in Bytes.
    pub total_bytes: u64,
    /// Zahl der Unterverzeichnisse.
    pub dir_count: usize,
    /// Gefundene Verknüpfungen. Sie werden **nicht** verfolgt.
    pub links_skipped: usize,
    /// Zur Bestätigung einzutippender Name.
    pub confirmation_word: String,
}

/// Ergebnis eines rekursiven Löschvorgangs.
#[derive(Debug, Clone)]
pub struct DirOutcome {
    /// Ergebnis je Datei.
    pub files: Vec<ShredOutcome>,
    /// Ob das Verzeichnis selbst verschwunden ist.
    pub removed: bool,
    /// Nicht verfolgte Verknüpfungen.
    pub links_skipped: usize,
    /// Fehler, die den Vorgang nicht abgebrochen haben.
    pub errors: Vec<String>,
}

impl DirOutcome {
    /// Zahl der erfolgreich gelöschten Dateien.
    #[must_use]
    pub fn succeeded(&self) -> usize {
        self.files.iter().filter(|f| f.is_success()).count()
    }

    /// Zahl der fehlgeschlagenen Dateien.
    #[must_use]
    pub fn failed(&self) -> usize {
        self.files.iter().filter(|f| !f.is_success()).count()
    }

    /// Ob alles gelungen ist.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.removed && self.failed() == 0 && self.errors.is_empty()
    }

    /// Sammelt die Warnungen aller Dateien, ohne Wiederholungen.
    #[must_use]
    pub fn warnings(&self) -> Vec<Warning> {
        let mut out: Vec<Warning> = Vec::new();
        for f in &self.files {
            for w in &f.warnings {
                if !out.contains(w) {
                    out.push(w.clone());
                }
            }
        }
        out
    }
}

/// Prüft, ob das Verzeichnis überhaupt gelöscht werden darf.
///
/// # Fehler
///
/// [`Refusal`] mit dem Grund.
pub fn check(path: &Path) -> Result<(), Refusal> {
    let meta = fs::symlink_metadata(path).map_err(|_| Refusal::NotADirectory)?;
    if meta.file_type().is_symlink() {
        return Err(Refusal::IsLink);
    }
    if !meta.is_dir() {
        return Err(Refusal::NotADirectory);
    }

    // Alle folgenden Prüfungen brauchen einen absoluten Pfad.
    //
    // `shred --dir ordner` galt vorher als Laufwerkswurzel, weil ein
    // relativer Pfad mit einer Komponente wie `/` aussieht. Der Fehler blieb
    // unentdeckt, weil sämtliche Tests hier absolute Temp-Pfade übergaben —
    // er kam erst heraus, als die CLI zum ersten Mal einen echten Aufruf
    // machte.
    //
    // Bewusst `absolute` und **nicht** `canonicalize`: Letzteres löst
    // Verknüpfungen auf. Ein Symlink darf hier aber niemals verfolgt werden
    // (§5.2 Nr. 4) — er wurde oben abgewiesen, und das soll so bleiben.
    let absolut = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());

    // Wurzel eines Laufwerks oder des Dateisystems. Bei einem absoluten Pfad
    // ist das genau dann der Fall, wenn es kein übergeordnetes Verzeichnis
    // gibt — `C:\` und `/` haben keines.
    if absolut.parent().is_none() {
        return Err(Refusal::DriveRoot);
    }

    // Benutzerprofil.
    for var in ["USERPROFILE", "HOME"] {
        if let Ok(heim) = std::env::var(var)
            && !heim.is_empty()
            && Path::new(&heim) == absolut
        {
            return Err(Refusal::HomeDirectory);
        }
    }

    if let Some(grund) = systemverzeichnis(&absolut) {
        return Err(grund);
    }

    if path.join(".git").exists() {
        return Err(Refusal::ContainsRepository);
    }

    Ok(())
}

/// Ob `absolut` in einem Systemverzeichnis liegt.
///
/// # Warum das eine eigene Funktion ist
///
/// Damit die **Regel** prüfbar ist, ohne die passende Umgebung zu haben.
/// Ein Test, der `std::env::temp_dir()` benutzt, prüft je Plattform etwas
/// anderes — und genau deshalb blieb der macOS-Fall unentdeckt, bis zum
/// ersten Mal ein macOS-Läufer lief. Diese Funktion nimmt einen Pfad und
/// gibt ein Urteil; das lässt sich überall nachstellen.
fn systemverzeichnis(absolut: &Path) -> Option<Refusal> {
    let unten = absolut.to_string_lossy().to_lowercase().replace('\\', "/");

    // Was UNTER einem gesperrten Pfad liegt und trotzdem dem Benutzer
    // gehört.
    //
    // # Der Fund, der das nötig machte
    //
    // macOS legt das Arbeitsverzeichnis des Benutzers unter
    // `/var/folders/<zwei>/<lang>/T/` an — `std::env::temp_dir()` gibt
    // genau das zurück. Da `/var` in der Sperrliste steht, verweigerte
    // Cabrik dort **jedes** sichere Löschen: im eigenen Zwischenspeicher
    // des Benutzers, in dem heruntergeladene Anhänge und entpackte
    // Archive landen.
    //
    // Aufgefallen ist es beim ersten CI-Lauf auf macOS: Sieben Tests in
    // dieser Datei scheiterten, weil sie ihr Prüfverzeichnis dort anlegen.
    // Auf Windows und Linux liegt es woanders, deshalb blieb es Jahre
    // unentdeckt.
    //
    // # Warum das keine Aufweichung ist
    //
    // `/var/folders` ist auf macOS kein Systemverzeichnis, sondern der
    // Benutzerbereich unter einem systemnahen Namen. Die Sperrliste soll
    // vor dem Löschen des **Betriebssystems** schützen, nicht vor dem
    // Löschen eigener Dateien. Wer sie so weit fasst, dass die Funktion
    // unbrauchbar wird, hat nichts geschützt — er hat sie abgeschafft.
    const AUSGENOMMEN: [&str; 2] = [
        // macOS: das Arbeitsverzeichnis je Benutzer.
        "/var/folders/",
        // Dasselbe über den nicht aufgelösten Pfad — `/var` ist auf macOS
        // eine Verknüpfung auf `/private/var`, und `absolute` löst sie
        // bewusst nicht auf (§5.2 Nr. 4).
        "/private/var/folders/",
    ];
    if !AUSGENOMMEN.iter().any(|a| unten.starts_with(a)) {
        const GESPERRT: [&str; 12] = [
            "c:/windows",
            "c:/program files",
            "c:/program files (x86)",
            "c:/programdata",
            "/usr",
            "/etc",
            "/bin",
            "/sbin",
            "/var",
            "/boot",
            // Auf macOS sind `/etc` und `/var` bloße Verknüpfungen auf
            // diese Pfade. Da hier bewusst `absolute` statt `canonicalize`
            // benutzt wird (§5.2 Nr. 4), kommt der aufgelöste Name nicht
            // von selbst an — wer `/private/var/db` übergibt, umginge die
            // Sperre sonst vollständig.
            //
            // Gefunden von der Gegenprobe zum `/var/folders`-Fall: Der
            // Test, der die Ausnahme eingrenzen sollte, deckte die
            // Lücke daneben auf.
            "/private/etc",
            "/private/var",
        ];
        for g in GESPERRT {
            if unten == g || unten.starts_with(&format!("{g}/")) {
                return Some(Refusal::SystemDirectory);
            }
        }
    }
    None
}

/// Zählt, was gelöscht würde, ohne etwas zu verändern.
///
/// # Fehler
///
/// [`Refusal`] aus [`check`].
pub fn preview(path: &Path) -> Result<Preview, Refusal> {
    check(path)?;

    let mut p = Preview {
        path: path.to_path_buf(),
        file_count: 0,
        total_bytes: 0,
        dir_count: 0,
        links_skipped: 0,
        confirmation_word: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    };
    zaehle(path, &mut p);
    Ok(p)
}

fn zaehle(dir: &Path, p: &mut Preview) {
    let Ok(eintraege) = fs::read_dir(dir) else {
        return;
    };
    for e in eintraege.flatten() {
        let pfad = e.path();
        let Ok(meta) = fs::symlink_metadata(&pfad) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            p.links_skipped = p.links_skipped.saturating_add(1);
        } else if meta.is_dir() {
            p.dir_count = p.dir_count.saturating_add(1);
            zaehle(&pfad, p);
        } else {
            p.file_count = p.file_count.saturating_add(1);
            p.total_bytes = p.total_bytes.saturating_add(meta.len());
        }
    }
}

/// Löscht ein Verzeichnis rekursiv.
///
/// `confirmation` muss dem Verzeichnisnamen entsprechen — bei einer
/// unumkehrbaren Aktion genügt ein Klick nicht.
///
/// # Fehler
///
/// [`Refusal`], wenn die Leitplanken greifen oder die Bestätigung nicht passt.
pub fn shred_dir(
    path: &Path,
    confirmation: &str,
    opts: &ShredOptions,
) -> Result<DirOutcome, Refusal> {
    let vorschau = preview(path)?;
    if confirmation != vorschau.confirmation_word {
        return Err(Refusal::NameMismatch {
            expected: vorschau.confirmation_word,
        });
    }

    let mut outcome = DirOutcome {
        files: Vec::new(),
        removed: false,
        links_skipped: 0,
        errors: Vec::new(),
    };
    loesche_rekursiv(path, opts, &mut outcome);

    // §5.2 Punkt 6: auch der Verzeichnisname steht im MFT.
    match benenne_und_entferne(path) {
        Ok(()) => outcome.removed = !path.exists(),
        Err(e) => outcome.errors.push(format!("{}: {e}", path.display())),
    }

    Ok(outcome)
}

fn loesche_rekursiv(dir: &Path, opts: &ShredOptions, outcome: &mut DirOutcome) {
    let Ok(eintraege) = fs::read_dir(dir) else {
        outcome
            .errors
            .push(format!("{}: nicht lesbar", dir.display()));
        return;
    };

    for e in eintraege.flatten() {
        let pfad = e.path();
        let Ok(meta) = fs::symlink_metadata(&pfad) else {
            continue;
        };

        if meta.file_type().is_symlink() {
            // §5.2 Punkt 4: niemals verfolgen. Nur die Verknüpfung selbst
            // entfernen, nie ihr Ziel.
            outcome.links_skipped = outcome.links_skipped.saturating_add(1);
            let _ = fs::remove_file(&pfad).or_else(|_| fs::remove_dir(&pfad));
        } else if meta.is_dir() {
            // §5.2 Punkt 5: von innen nach außen.
            loesche_rekursiv(&pfad, opts, outcome);
            if let Err(e) = benenne_und_entferne(&pfad) {
                outcome.errors.push(format!("{}: {e}", pfad.display()));
            }
        } else {
            // §5.2 Punkt 7: ein Fehler bricht nicht ab.
            outcome.files.push(shred_file(&pfad, opts));
        }
    }
}

/// Benennt ein Verzeichnis um und entfernt es.
fn benenne_und_entferne(dir: &Path) -> std::io::Result<()> {
    let Some(eltern) = dir.parent() else {
        return fs::remove_dir(dir);
    };
    let mut zufall = [0u8; 12];
    getrandom::fill(&mut zufall).map_err(|_| std::io::Error::other("Zufallsquelle"))?;
    let name: String = zufall.iter().map(|b| format!("{:x}", b % 16)).collect();

    let neu = eltern.join(name);
    if fs::rename(dir, &neu).is_ok() {
        fs::remove_dir(&neu)
    } else {
        fs::remove_dir(dir)
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let mut zufall = [0u8; 8];
            getrandom::fill(&mut zufall).unwrap();
            let suffix: String = zufall.iter().map(|b| format!("{b:02x}")).collect();
            let p = std::env::temp_dir().join(format!("cabrik-dir-{name}-{suffix}"));
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn baum(wurzel: &Path) {
        fs::create_dir_all(wurzel.join("unter/tiefer")).unwrap();
        fs::write(wurzel.join("a.txt"), b"eins").unwrap();
        fs::write(wurzel.join("unter/b.txt"), b"zwei").unwrap();
        fs::write(wurzel.join("unter/tiefer/c.txt"), vec![0u8; 5000]).unwrap();
    }

    #[test]
    fn vorschau_zaehlt_richtig() {
        let d = TempDir::new("vorschau");
        baum(&d.0);

        let p = preview(&d.0).unwrap();
        assert_eq!(p.file_count, 3);
        assert_eq!(p.dir_count, 2);
        assert_eq!(p.total_bytes, 4 + 4 + 5000);
        assert_eq!(
            p.confirmation_word,
            d.0.file_name().unwrap().to_string_lossy()
        );
    }

    #[test]
    fn ohne_richtige_bestaetigung_passiert_nichts() {
        let d = TempDir::new("bestaetigung");
        baum(&d.0);

        let r = shred_dir(&d.0, "falscher name", &ShredOptions::default());
        match r {
            Err(Refusal::NameMismatch { expected }) => {
                assert_eq!(expected, d.0.file_name().unwrap().to_string_lossy());
            }
            other => panic!("erwartete NameMismatch, bekam {other:?}"),
        }
        assert!(d.0.join("a.txt").exists(), "es wurde trotzdem geloescht");
    }

    #[test]
    fn rekursives_loeschen_raeumt_auf() {
        let d = TempDir::new("rekursiv");
        baum(&d.0);
        let name = d.0.file_name().unwrap().to_string_lossy().into_owned();

        let r = shred_dir(&d.0, &name, &ShredOptions::default()).unwrap();
        assert_eq!(r.succeeded(), 3, "Fehler: {:?}", r.errors);
        assert_eq!(r.failed(), 0);
        assert!(r.removed);
        assert!(!d.0.exists());
    }

    /// Ein relativer Pfad mit einer Komponente sah aus wie `/` und wurde als
    /// Laufwerkswurzel verweigert. Saemtliche Tests hier uebergaben absolute
    /// Temp-Pfade, deshalb blieb es unentdeckt, bis die CLI zum ersten Mal
    /// `shred --dir ordner` aufrief.
    #[test]
    fn ein_relativer_pfad_ist_keine_laufwerkswurzel() {
        let d = TempDir::new("relativ");
        let unterordner = d.0.join("ziel");
        fs::create_dir_all(&unterordner).unwrap();
        fs::write(unterordner.join("a.txt"), b"x").unwrap();

        // In das Elternverzeichnis wechseln und relativ ansprechen.
        let vorher = std::env::current_dir().unwrap();
        std::env::set_current_dir(&d.0).unwrap();
        let ergebnis = check(Path::new("ziel"));
        std::env::set_current_dir(vorher).unwrap();

        assert!(
            ergebnis.is_ok(),
            "relativer Pfad wurde verweigert: {ergebnis:?}"
        );
    }

    #[test]
    fn laufwerkswurzeln_werden_verweigert() {
        #[cfg(windows)]
        let wurzeln = ["C:\\"];
        #[cfg(not(windows))]
        let wurzeln = ["/"];

        for w in wurzeln {
            let r = check(Path::new(w));
            assert!(
                matches!(r, Err(Refusal::DriveRoot) | Err(Refusal::SystemDirectory)),
                "{w} wurde nicht verweigert: {r:?}"
            );
        }
    }

    #[test]
    fn das_arbeitsverzeichnis_unter_var_folders_ist_erlaubt() {
        /*
         * DER macOS-FUND aus dem ersten CI-Lauf.
         *
         * macOS legt das Arbeitsverzeichnis des Benutzers unter
         * `/var/folders/<zwei>/<lang>/T/` an. Da `/var` in der Sperrliste
         * steht, verweigerte Cabrik dort JEDES sichere Loeschen -- im
         * eigenen Zwischenspeicher des Benutzers, in dem
         * heruntergeladene Anhaenge und entpackte Archive landen.
         *
         * Sieben Tests dieser Datei scheiterten daran, weil sie ihr
         * Pruefverzeichnis genau dort anlegen. Auf Windows und Linux liegt
         * es woanders -- deshalb blieb es unentdeckt, bis zum ersten Mal
         * ein macOS-Laeufer lief.
         *
         * Der Test prueft die REGEL, nicht die Umgebung: Er baut den Pfad
         * selbst und laeuft damit auf jeder Plattform. Ein Test, der
         * `temp_dir()` benutzt, pruefte diesen Fall nur auf macOS -- und
         * genau dort wurde er gebraucht.
         */
        for pfad in [
            "/var/folders/xy/abc123/T/cabrik-probe",
            // Ueber die nicht aufgeloeste Verknuepfung.
            "/private/var/folders/xy/abc123/T/cabrik-probe",
        ] {
            assert_ne!(
                systemverzeichnis(Path::new(pfad)),
                Some(Refusal::SystemDirectory),
                "{pfad} gehoert dem Benutzer, nicht dem System"
            );
        }
    }

    #[test]
    fn der_rest_von_var_bleibt_gesperrt() {
        // Die Gegenprobe. Eine Ausnahme, die zu weit greift, hebt die
        // Sperre auf, statt sie zu praezisieren.
        for pfad in ["/var", "/var/log", "/var/lib/wichtig", "/private/var/db"] {
            assert_eq!(
                systemverzeichnis(Path::new(pfad)),
                Some(Refusal::SystemDirectory),
                "{pfad} muss gesperrt bleiben"
            );
        }
    }

    #[test]
    fn systemverzeichnisse_werden_verweigert() {
        #[cfg(windows)]
        let pfade = ["C:\\Windows", "C:\\Windows\\System32", "C:\\Program Files"];
        #[cfg(not(windows))]
        let pfade = ["/etc", "/usr", "/usr/bin"];

        for p in pfade {
            if !Path::new(p).exists() {
                continue;
            }
            /*
             * Geprueft wird, DASS verweigert wird -- nicht mit welchem
             * Wortlaut.
             *
             * Auf macOS ist `/etc` selbst eine Verknuepfung auf
             * `/private/etc`. Sie wird deshalb schon eine Stufe frueher
             * abgewiesen, mit `IsLink` statt `SystemDirectory`, und das ist
             * genauso richtig: Verknuepfungen werden niemals verfolgt
             * (§5.2 Nr. 4). Der erste macOS-Lauf hat den Test hier
             * scheitern lassen, obwohl das Verhalten stimmte.
             *
             * Auf den Wortlaut zu pruefen hiesse, eine Reihenfolge
             * festzuschreiben, die nichts mit der Zusicherung zu tun hat.
             * Die REGEL selbst prueft `der_rest_von_var_bleibt_gesperrt`
             * ueber `systemverzeichnis` -- dort ohne Umgebung und auf jeder
             * Plattform gleich.
             */
            let urteil = check(Path::new(p));
            assert!(
                matches!(urteil, Err(Refusal::SystemDirectory) | Err(Refusal::IsLink)),
                "{p} wurde nicht verweigert, sondern: {urteil:?}"
            );
        }
    }

    #[test]
    fn benutzerprofil_wird_verweigert() {
        for var in ["USERPROFILE", "HOME"] {
            if let Ok(heim) = std::env::var(var)
                && !heim.is_empty()
                && Path::new(&heim).is_dir()
            {
                assert_eq!(
                    check(Path::new(&heim)),
                    Err(Refusal::HomeDirectory),
                    "{heim} wurde nicht verweigert"
                );
            }
        }
    }

    #[test]
    fn git_repositories_werden_verweigert() {
        let d = TempDir::new("git");
        fs::create_dir_all(d.0.join(".git")).unwrap();
        assert_eq!(check(&d.0), Err(Refusal::ContainsRepository));
    }

    #[test]
    fn nicht_existierende_pfade_werden_verweigert() {
        assert_eq!(
            check(Path::new("/gibt/es/nicht/12345")),
            Err(Refusal::NotADirectory)
        );
    }

    #[test]
    fn einzelne_dateien_werden_hier_verweigert() {
        let d = TempDir::new("datei");
        let f = d.0.join("a.txt");
        fs::write(&f, b"x").unwrap();
        assert_eq!(check(&f), Err(Refusal::NotADirectory));
    }

    #[test]
    fn leere_verzeichnisse_funktionieren() {
        let d = TempDir::new("leer");
        let unter = d.0.join("leer_drin");
        fs::create_dir(&unter).unwrap();

        let r = shred_dir(&unter, "leer_drin", &ShredOptions::default()).unwrap();
        assert!(r.removed);
        assert_eq!(r.succeeded(), 0);
        assert!(!unter.exists());
    }

    #[test]
    fn warnungen_werden_gesammelt_ohne_wiederholung() {
        let d = TempDir::new("warnungen");
        baum(&d.0);
        let name = d.0.file_name().unwrap().to_string_lossy().into_owned();

        let r = shred_dir(&d.0, &name, &ShredOptions::default()).unwrap();
        let w = r.warnings();
        assert!(w.contains(&Warning::CopiesMayExist));
        assert_eq!(
            w.iter().filter(|x| **x == Warning::CopiesMayExist).count(),
            1,
            "die Warnung darf nicht dreimal erscheinen"
        );
    }
}
