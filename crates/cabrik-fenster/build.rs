//! Erzeugt, was Tauri zur Bauzeit braucht.
//!
//! Liest `tauri.conf.json` und legt die Symbole sowie die
//! Windows-Ressourcen ab. Ohne diese Datei findet `generate_context!` seine
//! Angaben nicht.

fn main() {
    tauri_build::build();
}
