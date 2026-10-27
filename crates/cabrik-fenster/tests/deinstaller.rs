//! Der Deinstaller löscht, was er zusagt — und nur, wenn er soll.
//!
//! # Warum das ein Test ist
//!
//! `nsis/haken.nsh` ist die einzige Stelle, an der dieses Programm
//! Schlüssel löscht, ohne dass der Nutzer es im Programm selbst bestätigt
//! hätte. Sie läuft nur im fertigen Setup, auf einem Rechner mit
//! Schlüsseln — kein anderer Test kommt dorthin, und ein Fehler kostet dort
//! alles, was an die Identität gerichtet war.
//!
//! Prüfen lässt sich, was im Text stehen muss: derselbe Ordner wie in
//! `cabrik-ablage`, nichts sonst, und die drei Sperren, die Behalten
//! voreinstellen. Ob das Fenster dann wirklich so aussieht, zeigt nur ein
//! Durchlauf in der Windows-Sandbox.

/// Der Haken, wie er ins Setup geht.
const HAKEN: &str = include_str!("../nsis/haken.nsh");

/// Die Konfiguration, die ihn einbindet.
const KONFIGURATION: &str = include_str!("../tauri.conf.json");

/// Die Zeilen ohne Kommentare — was dort steht, tut nichts.
fn befehle() -> String {
    HAKEN
        .lines()
        .filter(|z| !z.trim_start().starts_with(';'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Wo `teil` in den Befehlen beginnt — `usize::MAX`, wenn nirgends.
fn stelle(teil: &str) -> usize {
    befehle().find(teil).unwrap_or(usize::MAX)
}

#[test]
fn der_haken_ist_eingetragen() {
    // Ohne den Eintrag baut Tauri das Setup ohne ihn — still. Das Häkchen
    // löschte dann nur die Daten der Webansicht, und Schlüssel und
    // Kontakte blieben liegen, obwohl jemand sie loswerden wollte.
    assert!(
        KONFIGURATION.contains(r#""installerHooks": "nsis/haken.nsh""#),
        "`tauri.conf.json` bindet `nsis/haken.nsh` nicht ein"
    );
}

#[test]
fn geloescht_wird_der_ordner_aus_cabrik_ablage_und_nichts_sonst() {
    let ziel = format!(r#"RMDir /r "$APPDATA\{}""#, cabrik_ablage::ORDNER_WINDOWS);
    let b = befehle();
    assert!(
        b.contains(&ziel),
        "der Haken löscht nicht `{ziel}` — `cabrik_ablage::ORDNER_WINDOWS` \
         und `nsis/haken.nsh` sind auseinandergelaufen"
    );
    // Die Befehle am Zeilenanfang, nicht jedes Vorkommen des Wortes:
    // `$DeleteAppDataCheckboxState` ist eine Variable, kein Löschen.
    let befehl = |name: &str| {
        b.lines()
            .filter(|z| z.trim_start().starts_with(&format!("{name} ")))
            .count()
    };
    assert_eq!(befehl("RMDir"), 1, "mehr als ein RMDir");
    assert_eq!(befehl("Delete"), 0, "ein Delete im Haken");
}

#[test]
fn behalten_ist_dreifach_voreingestellt() {
    let geloescht = stelle("RMDir");
    assert_ne!(geloescht, usize::MAX);

    // 1. Nur mit Häkchen, und nie beim Update.
    assert!(stelle("${If} $DeleteAppDataCheckboxState = 1") < geloescht);
    assert!(stelle("${AndIf} $UpdateMode <> 1") < geloescht);

    // 2. Die Rückfrage, mit „Nein“ vorgewählt — gelöscht wird nur nach
    //    „Ja“.
    let frage = stelle("MessageBox MB_YESNO|MB_ICONEXCLAMATION|MB_DEFBUTTON2");
    assert!(frage < geloescht);
    assert!(stelle("IDYES`") < geloescht);

    // 3. Ohne Fenster gilt „Nein“.
    assert!(stelle("/SD IDNO") < geloescht);
}

#[test]
fn die_rueckfrage_nennt_die_folge() {
    // In beiden Sprachen — mit der Folge, nicht mit der Regel.
    assert!(HAKEN.contains("Danach lässt sich nichts mehr öffnen"));
    assert!(HAKEN.contains("nothing that was encrypted to you can be opened again"));
    // Und dass eine Sicherung anderswo bleibt: Wer eine hat, soll nicht
    // glauben, sie sei mit fort.
    assert!(HAKEN.contains("Sicherung der Schlüsseldatei an einem anderen Ort"));
}
