//! Erzeugt, was Tauri zur Bauzeit braucht — und wacht über die Oberfläche.
//!
//! Liest `tauri.conf.json` und legt die Symbole sowie die
//! Windows-Ressourcen ab. Ohne diese Datei findet `generate_context!` seine
//! Angaben nicht.
//!
//! Dazu kommt eine Wache, die mit Tauri selbst nichts zu tun hat: Sie
//! verhindert, dass ein Release-Bau eine veraltete Oberfläche einbackt.
//! Siehe [`pruefe_dass_die_oberflaeche_frisch_ist`].

use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn main() {
    tauri_build::build();
    pruefe_dass_die_oberflaeche_frisch_ist();
}

/// Bricht den Release-Bau ab, wenn `dist/` älter ist als die Quellen.
///
/// # Die Falle, gegen die das steht
///
/// Im Entwicklungsbau holt sich das Fenster die Oberfläche über `devUrl`
/// vom laufenden Vite-Server — dort ist immer frisch, was auf der Platte
/// steht. Im Release-Bau nimmt Tauri stattdessen `frontendDist`, also
/// einen **gebauten** Ordner.
///
/// Gefüllt wird der von `beforeBuildCommand`. Das führt aber nur die
/// Tauri-Befehlszeile aus. Wer `cargo build --release -p cabrik-fenster`
/// tippt — und das ist der naheliegende Befehl —, bekommt genau das, was
/// zufällig in `dist/` liegt. Beim Fund dieser Wache war das acht Tage
/// alt.
///
/// Es gäbe keine Warnung. Der Bau liefe durch, der Installer entstünde,
/// und im Programm stünde eine Oberfläche, die niemand mehr so geschrieben
/// hat. Bei einem Verschlüsselungsprogramm ist das kein Schönheitsfehler:
/// Es sind die Sätze über Schlüssel, Befunde und Löschzusagen, die dann
/// nicht mehr dem entsprechen, was der Kern tut.
///
/// # Warum ein Abbruch und keine Warnung
///
/// Weil `cargo:warning=` im Rauschen eines Release-Baus untergeht und die
/// Folge unbemerkt ausgeliefert würde. Der Ausweg steht in der Meldung und
/// ist ein einziger Befehl.
///
/// # Warum nichts im Entwicklungsbau geschieht
///
/// Dort gilt `devUrl`, `dist/` wird gar nicht gelesen. Eine Wache, die
/// beim täglichen `cargo run` anschlüge, wäre eine Wache, die abgeschaltet
/// wird.
fn pruefe_dass_die_oberflaeche_frisch_ist() {
    if std::env::var("PROFILE").as_deref() != Ok("release") {
        return;
    }

    let Some(dist) = gebauter_ordner() else {
        // Ohne lesbare Angabe gibt es nichts zu bewachen. Das ist kein
        // Grund, den Bau anzuhalten — `tauri_build::build()` oben hätte
        // eine kaputte Konfiguration längst bemängelt.
        return;
    };

    // `app/oberflaeche/dist` -> `app/oberflaeche/src`. Aus der Angabe
    // abgeleitet statt ein zweites Mal hingeschrieben: Zöge jemand die
    // Oberfläche um, wanderte die Wache mit.
    let Some(wurzel) = dist.parent() else {
        return;
    };
    let quellen = wurzel.join("src");
    if !quellen.is_dir() {
        return;
    }

    // Ein neuer Quelltext soll die Wache erneut anstoßen. Ohne das hielte
    // Cargo das Ergebnis des letzten Laufs für weiterhin gültig.
    println!("cargo:rerun-if-changed={}", quellen.display());
    println!("cargo:rerun-if-changed={}", dist.display());

    let einstieg = dist.join("index.html");
    let Some(gebaut) = geaendert_am(&einstieg) else {
        abbrechen(&format!(
            "Es gibt keine gebaute Oberflaeche unter {}.",
            dist.display()
        ))
    };

    let Some(geschrieben) = neuste_aenderung(&quellen) else {
        return;
    };

    if geschrieben > gebaut {
        abbrechen(&format!(
            "Die gebaute Oberflaeche unter {} ist aelter als der Quelltext \
             unter {}.",
            dist.display(),
            quellen.display()
        ));
    }
}

/// Der Ordner aus `frontendDist`, absolut gemacht.
///
/// Aus `tauri.conf.json` gelesen und nicht hier hineingeschrieben — sonst
/// stünde derselbe Pfad an zwei Stellen und die zweite veraltete still.
fn gebauter_ordner() -> Option<PathBuf> {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").ok()?);
    let inhalt = std::fs::read_to_string(manifest.join("tauri.conf.json")).ok()?;
    let angaben: serde_json::Value = serde_json::from_str(&inhalt).ok()?;
    let angabe = angaben.get("build")?.get("frontendDist")?.as_str()?;
    // Eine URL statt eines Ordners ist erlaubt; dann gibt es nichts zu
    // pruefen.
    if angabe.contains("://") {
        return None;
    }
    Some(manifest.join(angabe))
}

/// Wann die Datei zuletzt geschrieben wurde — `None`, wenn es sie nicht gibt.
fn geaendert_am(pfad: &Path) -> Option<SystemTime> {
    std::fs::metadata(pfad).ok()?.modified().ok()
}

/// Die jüngste Änderung irgendwo unterhalb des Verzeichnisses.
///
/// Ohne Rekursion, damit ein tief verschachtelter Baum den Bau nicht über
/// den Stapel kippt.
fn neuste_aenderung(verzeichnis: &Path) -> Option<SystemTime> {
    let mut neuste: Option<SystemTime> = None;
    let mut offen = vec![verzeichnis.to_path_buf()];

    while let Some(pfad) = offen.pop() {
        let Ok(eintraege) = std::fs::read_dir(&pfad) else {
            continue;
        };
        for eintrag in eintraege.flatten() {
            let kind = eintrag.path();
            if kind.is_dir() {
                offen.push(kind);
                continue;
            }
            let Some(zeit) = geaendert_am(&kind) else {
                continue;
            };
            let juenger = match neuste {
                None => true,
                Some(bisher) => zeit > bisher,
            };
            if juenger {
                neuste = Some(zeit);
            }
        }
    }

    neuste
}

/// Hält den Bau an und sagt, was zu tun ist.
///
/// Über `exit` statt `panic!`: Ein Panik-Abbruch im Bauskript druckt einen
/// Rueckverfolgungsstapel, in dem die eigentliche Aussage untergeht — und
/// `clippy::panic` ist in diesem Arbeitsbereich ohnehin ein Fehler.
fn abbrechen(grund: &str) -> ! {
    eprintln!();
    eprintln!("cabrik-fenster: {grund}");
    eprintln!();
    eprintln!("  Ein Release-Bau backt die Oberflaeche aus diesem Ordner ein.");
    eprintln!("  Sie ist nicht auf dem Stand des Quelltexts.");
    eprintln!();
    eprintln!("  Zuerst bauen:");
    eprintln!("      npm --prefix app/oberflaeche run build");
    eprintln!();
    eprintln!("  Oder gleich ueber Tauri, das erledigt es selbst:");
    eprintln!("      npm --prefix app/oberflaeche run tauri build");
    eprintln!();
    std::process::exit(1)
}
