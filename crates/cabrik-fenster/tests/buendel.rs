//! Das macOS-Bündel ist versiegelt — ohne Zertifikat, aber versiegelt.
//!
//! # Warum das ein Test ist
//!
//! Auf Rechnern mit Apple-Chip hält macOS ein Programm aus dem Netz ohne
//! Signatur für beschädigt und bietet keinen Weg, es trotzdem zu öffnen.
//! Die Ad-hoc-Signatur versiegelt das Bündel ohne Zertifikat und ohne
//! Herausgeber; dann bleibt der Umweg über „Dennoch öffnen“ in den
//! Systemeinstellungen.
//!
//! Der Eintrag dafür ist ein einzelner Strich und sieht aus wie eine
//! Signatur, die jemand auszufüllen vergessen hat. Wer ihn für einen
//! Platzhalter hält und löscht, erfährt es sonst erst von einem Tester.
//! Ob das Siegel im fertigen Bündel steckt, prüft `auslieferung.yml` am
//! Ergebnis.

/// Die Konfiguration, aus der Tauri die Bündel baut.
const KONFIGURATION: &str = include_str!("../tauri.conf.json");

#[test]
fn das_macos_buendel_wird_ad_hoc_versiegelt() {
    let konfiguration: serde_json::Value =
        serde_json::from_str(KONFIGURATION).unwrap_or(serde_json::Value::Null);
    assert_eq!(
        konfiguration
            .pointer("/bundle/macOS/signingIdentity")
            .and_then(serde_json::Value::as_str),
        Some("-"),
        "`tauri.conf.json` versiegelt das macOS-Bündel nicht mehr ad hoc"
    );
}
