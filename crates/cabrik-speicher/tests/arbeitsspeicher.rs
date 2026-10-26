//! Fragt das **echte** System, wie viel Arbeitsspeicher frei ist.
//!
//! # Warum gegen das echte System
//!
//! Weil es nichts anderes zu prüfen gibt: Die Funktion ist ein
//! Systemaufruf und eine Rechnung darüber. Eine Attrappe bestätigte nur,
//! dass die Rechnung zur Attrappe passt.
//!
//! In der CI läuft der Test auf allen drei Systemen mit `--nocapture`,
//! damit die Zahlen im Protokoll stehen. Für macOS ist das der einzige Weg,
//! die Herleitung über `kern.memorystatus_level` an einem wirklichen
//! Rechner zu sehen — hier gibt es keinen.

#![expect(clippy::expect_used, reason = "Fehlschlag soll den Test abbrechen")]

#[test]
fn das_system_nennt_eine_glaubhafte_zahl() {
    let frei = cabrik_speicher::freier_arbeitsspeicher();
    let grenze = cabrik_speicher::ableitungsgrenze();
    eprintln!(
        "frei: {} MiB, Grenze fuer die Ableitung: {} MiB",
        frei.map_or_else(|| "?".to_owned(), |b| (b >> 20).to_string()),
        grenze.map_or_else(|| "?".to_owned(), |b| (b >> 20).to_string()),
    );

    // Auf den drei Zielsystemen MUSS es eine Antwort geben. Anderswo ist
    // „unbekannt“ eine zulässige Antwort und kein Fehler.
    if cfg!(any(windows, target_os = "linux", target_os = "macos")) {
        let frei = frei.expect("das System nennt keine Zahl");
        // Ein laufender Prüfrechner hat mehr als 16 MiB frei und weniger
        // als ein Pebibyte. Was ausserhalb liegt, ist eine falsche Einheit.
        assert!(frei > 16 << 20, "unglaubhaft wenig: {frei} Bytes");
        assert!(frei < 1 << 50, "unglaubhaft viel: {frei} Bytes");

        let grenze = grenze.expect("ohne Grenze trotz bekanntem Speicher");
        assert!(grenze < frei, "die Grenze laesst kein Viertel uebrig");
        assert!(grenze > frei / 2, "die Grenze ist kleiner als gedacht");
    }
}
