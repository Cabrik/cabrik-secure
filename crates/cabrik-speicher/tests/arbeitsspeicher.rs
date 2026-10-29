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

#[test]
fn die_spitze_steigt_mit_dem_was_belegt_wird() {
    // Der Speichertest (`spec/grosse-dateien.md` §9) verlässt sich auf
    // diese Zahl. Ob sie überhaupt anschlägt, zeigt nur eine Belegung, die
    // man selbst vornimmt: 128 MiB, jede Seite beschrieben.
    let vorher = cabrik_speicher::spitze_arbeitsspeicher();
    let belegt = vec![1u8; 128 << 20];
    let summe: u64 = belegt.iter().step_by(4096).map(|&b| u64::from(b)).sum();
    let nachher = cabrik_speicher::spitze_arbeitsspeicher();
    drop(belegt);
    eprintln!(
        "Spitze vorher: {} MiB, nachher: {} MiB ({summe} Seiten beschrieben)",
        vorher.map_or_else(|| "?".to_owned(), |b| (b >> 20).to_string()),
        nachher.map_or_else(|| "?".to_owned(), |b| (b >> 20).to_string()),
    );

    if cfg!(any(windows, target_os = "linux", target_os = "macos")) {
        let vorher = vorher.expect("das System nennt keine Spitze");
        let nachher = nachher.expect("das System nennt keine Spitze");
        assert!(
            nachher >= vorher + (100 << 20),
            "128 MiB belegt, die Spitze stieg nur von {vorher} auf {nachher}"
        );
    }
}
