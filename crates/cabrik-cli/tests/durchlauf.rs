//! Vollständige Durchläufe gegen das gebaute Programm.
//!
//! # Warum gegen das Programm und nicht gegen die Funktionen
//!
//! Die Modultests prüfen einzelne Bausteine. Diese Tests prüfen, was ein
//! Mensch tatsächlich tippt — inklusive Argumentauswertung, Dateizugriff,
//! Passwortquellen und Ausgabe. Jeder schwere Entwurfsfehler dieses Projekts
//! kam an genau dieser Naht heraus, nicht in den Bausteinen.
//!
//! Der wichtigste Test ist [`beide_seiten_sehen_dieselbe_safety_number`]:
//! Er prüft die Eigenschaft, auf der das ganze Vertrauensmodell ruht, und
//! genau die war vor Phase 2.11 verletzt.

// In einer Testdatei ist der Abbruch das gewünschte Verhalten: Wer eine
// Vorbedingung nicht herstellen kann, hat kein Ergebnis, sondern einen
// kaputten Test. Im Programm selbst gelten die Regeln unverändert weiter.
#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "Fehlschlag soll den Test abbrechen"
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Pfad zum gebauten Programm.
fn programm() -> PathBuf {
    // Cargo legt Integrationstests neben das Programm.
    let mut p = std::env::current_exe().expect("Testprogramm hat keinen Pfad");
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    p.join(format!("cabrik{}", std::env::consts::EXE_SUFFIX))
}

/// Ein eigenes Arbeitsverzeichnis je Test.
struct Werkstatt(PathBuf);

impl Werkstatt {
    fn neu(name: &str) -> Self {
        let mut zufall = [0u8; 8];
        getrandom::fill(&mut zufall).expect("Zufallsquelle");
        let suffix: String = zufall.iter().map(|b| format!("{b:02x}")).collect();
        let p = std::env::temp_dir().join(format!("cabrik-cli-{name}-{suffix}"));
        std::fs::create_dir_all(&p).expect("Arbeitsverzeichnis");
        Self(p)
    }

    fn pfad(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn schreib(&self, name: &str, inhalt: &[u8]) -> PathBuf {
        let p = self.pfad(name);
        std::fs::write(&p, inhalt).expect("schreiben");
        p
    }

    /// Ruft das Programm auf und erwartet Erfolg.
    fn ruf(&self, args: &[&str]) -> String {
        let aus = self.roh(args);
        assert!(
            aus.status.success(),
            "Aufruf {args:?} schlug fehl:\n{}\n{}",
            String::from_utf8_lossy(&aus.stdout),
            String::from_utf8_lossy(&aus.stderr)
        );
        String::from_utf8_lossy(&aus.stdout).into_owned()
    }

    /// Ruft das Programm auf und erwartet einen Fehlschlag.
    fn ruf_fehler(&self, args: &[&str]) -> String {
        let aus = self.roh(args);
        assert!(
            !aus.status.success(),
            "Aufruf {args:?} haette scheitern muessen:\n{}",
            String::from_utf8_lossy(&aus.stdout)
        );
        format!(
            "{}{}",
            String::from_utf8_lossy(&aus.stdout),
            String::from_utf8_lossy(&aus.stderr)
        )
    }

    fn roh(&self, args: &[&str]) -> Output {
        Command::new(programm())
            .args(args)
            .current_dir(&self.0)
            .output()
            .expect("Programm liess sich nicht starten")
    }
}

impl Drop for Werkstatt {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Legt eine Identität an und gibt die Argumente zurück, die sie ansprechen.
fn identitaet(w: &Werkstatt, name: &str, passwort: &str) {
    w.schreib(&format!("{name}.pw"), passwort.as_bytes());
    w.ruf(&[
        "keygen",
        "--keyfile",
        &format!("{name}.key"),
        "--label",
        name,
        // Untergrenze der Spezifikation: Die Tests sollen den Ablauf prüfen,
        // nicht Argon2 vermessen.
        "--kdf",
        "min",
        "--password-file",
        &format!("{name}.pw"),
        "-q",
    ]);
}

fn wer(name: &str) -> [String; 6] {
    [
        "--keyfile".to_owned(),
        format!("{name}.key"),
        "--contacts".to_owned(),
        format!("{name}.contacts"),
        "--password-file".to_owned(),
        format!("{name}.pw"),
    ]
}

fn mit<'a>(basis: &'a [String; 6], weiteres: &[&'a str]) -> Vec<&'a str> {
    let mut v: Vec<&str> = weiteres.to_vec();
    v.extend(basis.iter().map(String::as_str));
    v.push("-q");
    v
}

/// Zieht die Ziffern der Safety Number aus der Ausgabe.
fn ziffern(text: &str) -> String {
    text.chars().filter(char::is_ascii_digit).collect()
}

// ---------------------------------------------------------------------------

/// **Der wichtigste Test des Projekts.**
///
/// Alice und Bob müssen dieselbe Zeichenfolge sehen — sonst kann sich niemand
/// verifizieren, und der gesamte Trust Store ist wertlos.
///
/// Vor Phase 2.11 schlug das fehl: Die Austausch-Nutzlast trug den
/// Post-Quantum-Schlüssel nicht, der Fingerprint aber schon. Beide Seiten
/// rechneten über verschiedene Schlüsselsätze.
#[test]
fn beide_seiten_sehen_dieselbe_safety_number() {
    let w = Werkstatt::neu("safety");
    identitaet(&w, "alice", "alice-geheim");
    identitaet(&w, "bob", "bob-geheim");
    let (a, b) = (wer("alice"), wer("bob"));

    w.ruf(&mit(&a, &["identity", "export", "--out", "alice.contact"]));
    w.ruf(&mit(&b, &["identity", "export", "--out", "bob.contact"]));
    w.ruf(&mit(
        &a,
        &["contacts", "add", "bob.contact", "--name", "Bob"],
    ));
    w.ruf(&mit(
        &b,
        &["contacts", "add", "alice.contact", "--name", "Alice"],
    ));

    let von_alice = ziffern(&w.ruf(&mit(&a, &["safety-number", "Bob"])));
    let von_bob = ziffern(&w.ruf(&mit(&b, &["safety-number", "Alice"])));

    assert_eq!(
        von_alice, von_bob,
        "Alice und Bob sehen verschiedene Safety Numbers — Verifikation unmoeglich"
    );
    assert_eq!(
        von_alice.len(),
        60,
        "60 Ziffern nach spec/trust-store.md §3"
    );
}

/// Was Bob beim Aufnehmen sieht, muss Alices eigener Anzeige entsprechen.
#[test]
fn der_aufgenommene_fingerprint_ist_der_angezeigte() {
    let w = Werkstatt::neu("fingerprint");
    identitaet(&w, "alice", "alice-geheim");
    identitaet(&w, "bob", "bob-geheim");
    let (a, b) = (wer("alice"), wer("bob"));

    let eigene = w.ruf(&mit(&a, &["identity", "show", "--json"]));
    let eigener_fp = feld(&eigene, "fingerprint");

    w.ruf(&mit(&a, &["identity", "export", "--out", "alice.contact"]));
    let aufgenommen = w.ruf(&mit(
        &b,
        &[
            "contacts",
            "add",
            "alice.contact",
            "--name",
            "Alice",
            "--json",
        ],
    ));

    assert_eq!(
        eigener_fp,
        feld(&aufgenommen, "fingerprint"),
        "Alice zeigt einen anderen Fingerprint an, als Bob berechnet"
    );
}

/// Liest ein Zeichenkettenfeld aus der JSON-Ausgabe.
fn feld(json: &str, name: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(json).expect("keine gueltige JSON-Ausgabe");
    v.get(name)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("Feld {name} fehlt in {json}"))
        .to_owned()
}

/// Der vollständige Weg: verschlüsseln, prüfen, entschlüsseln.
#[test]
fn nachricht_kommt_unveraendert_an_und_ist_post_quantum() {
    let w = Werkstatt::neu("rundlauf");
    identitaet(&w, "alice", "alice-geheim");
    identitaet(&w, "bob", "bob-geheim");
    let (a, b) = (wer("alice"), wer("bob"));

    w.ruf(&mit(&b, &["identity", "export", "--out", "bob.contact"]));
    w.ruf(&mit(
        &a,
        &["contacts", "add", "bob.contact", "--name", "Bob"],
    ));

    let inhalt = "Angebot: 240.000 Euro. Umlaute: äöüß.";
    w.schreib("geheim.txt", inhalt.as_bytes());

    let bericht = w.ruf(&mit(
        &a,
        &["encrypt", "geheim.txt", "--to", "Bob", "--json"],
    ));
    assert!(
        feld(&bericht, "suite").contains("ML-KEM"),
        "Suite 0x0002 haette gewaehlt werden muessen: {bericht}"
    );

    w.ruf(&mit(
        &b,
        &["decrypt", "geheim.txt.cab", "--out", "klar.txt"],
    ));
    let zurueck = std::fs::read_to_string(w.pfad("klar.txt")).expect("Ausgabe lesen");
    assert_eq!(zurueck, inhalt);
}

/// Ohne Kontakt kein Kontakt: Aus einer Signatur allein lässt sich keiner
/// bilden, und es wird auch keiner erfunden.
#[test]
fn ein_unbekannter_absender_wird_nicht_zum_kontakt_erfunden() {
    let w = Werkstatt::neu("unbekannt");
    identitaet(&w, "alice", "alice-geheim");
    identitaet(&w, "bob", "bob-geheim");
    let (a, b) = (wer("alice"), wer("bob"));

    w.ruf(&mit(&b, &["identity", "export", "--out", "bob.contact"]));
    w.ruf(&mit(
        &a,
        &["contacts", "add", "bob.contact", "--name", "Bob"],
    ));
    w.schreib("m.txt", b"hallo");
    w.ruf(&mit(&a, &["encrypt", "m.txt", "--to", "Bob"]));

    let bericht = w.ruf(&mit(
        &b,
        &["decrypt", "m.txt.cab", "--out", "k.txt", "--json"],
    ));
    let v: serde_json::Value = serde_json::from_str(&bericht).expect("JSON");
    assert!(
        !v["darf_gruen_zeigen"].as_bool().unwrap_or(true),
        "unbekannter Absender darf nie gruen sein"
    );
    assert!(
        v["unbekannter_signierschluessel"].is_string(),
        "der Signierschluessel gehoert in die Ausgabe"
    );

    let liste = w.ruf(&mit(&b, &["contacts", "list", "--json"]));
    let v: serde_json::Value = serde_json::from_str(&liste).expect("JSON");
    assert_eq!(
        v["kontakte"].as_array().map(Vec::len),
        Some(0),
        "es wurde ein Kontakt erfunden, der keiner sein kann"
    );
}

/// Widerruf ist der eine Zustand, der das Verschlüsseln **verhindern** muss.
#[test]
fn an_einen_widerrufenen_kontakt_wird_nicht_verschluesselt() {
    let w = Werkstatt::neu("widerruf");
    identitaet(&w, "alice", "alice-geheim");
    identitaet(&w, "bob", "bob-geheim");
    let (a, b) = (wer("alice"), wer("bob"));

    w.ruf(&mit(&b, &["identity", "export", "--out", "bob.contact"]));
    w.ruf(&mit(
        &a,
        &["contacts", "add", "bob.contact", "--name", "Bob"],
    ));
    w.schreib("m.txt", b"hallo");
    w.ruf(&mit(&a, &["contacts", "revoke", "Bob"]));

    let meldung = w.ruf_fehler(&mit(&a, &["encrypt", "m.txt", "--to", "Bob"]));
    assert!(
        meldung.contains("kompromittiert"),
        "die Verweigerung wurde nicht begruendet: {meldung}"
    );
    assert!(!w.pfad("m.txt.cab").exists(), "es wurde doch geschrieben");
}

/// Der Passwortmodus kommt ganz ohne Schlüssel aus.
#[test]
fn passwortmodus_funktioniert_ohne_jeden_schluessel() {
    let w = Werkstatt::neu("passwort");
    w.schreib("env.pw", b"envelope-geheim");
    w.schreib("falsch.pw", b"daneben");
    w.schreib("m.txt", b"nur mit Passwort");

    w.ruf(&[
        "encrypt",
        "m.txt",
        "--password",
        "--password-file",
        "env.pw",
        "-q",
    ]);
    w.ruf(&[
        "decrypt",
        "m.txt.cab",
        "--password",
        "--password-file",
        "env.pw",
        "--out",
        "k.txt",
        "-q",
    ]);
    assert_eq!(
        std::fs::read(w.pfad("k.txt")).expect("lesen"),
        b"nur mit Passwort"
    );

    let meldung = w.ruf_fehler(&[
        "decrypt",
        "m.txt.cab",
        "--password",
        "--password-file",
        "falsch.pw",
        "--out",
        "nie.txt",
        "-q",
    ]);
    assert!(!w.pfad("nie.txt").exists());
    assert!(meldung.contains("entschlüsselt"), "{meldung}");
}

/// Bestehende Dateien werden nie stillschweigend überschrieben.
#[test]
fn nichts_wird_stillschweigend_ueberschrieben() {
    let w = Werkstatt::neu("ueberschreiben");
    w.schreib("env.pw", b"geheim");
    w.schreib("m.txt", b"inhalt");
    w.schreib("m.txt.cab", b"WICHTIGE ALTE DATEI");

    let meldung = w.ruf_fehler(&[
        "encrypt",
        "m.txt",
        "--password",
        "--password-file",
        "env.pw",
        "-q",
    ]);
    assert!(meldung.contains("existiert bereits"), "{meldung}");
    assert_eq!(
        std::fs::read(w.pfad("m.txt.cab")).expect("lesen"),
        b"WICHTIGE ALTE DATEI",
        "die alte Datei wurde vernichtet"
    );
}

/// Ein Keyfile darf niemals überschrieben werden — der Verlust ist endgültig.
#[test]
fn ein_bestehendes_keyfile_wird_geschuetzt() {
    let w = Werkstatt::neu("keyfile-schutz");
    identitaet(&w, "alice", "alice-geheim");
    let vorher = std::fs::read(w.pfad("alice.key")).expect("lesen");

    let meldung = w.ruf_fehler(&[
        "keygen",
        "--keyfile",
        "alice.key",
        "--kdf",
        "min",
        "--password-file",
        "alice.pw",
        "-q",
    ]);
    assert!(meldung.contains("unwiederbringlich"), "{meldung}");
    assert_eq!(
        std::fs::read(w.pfad("alice.key")).expect("lesen"),
        vorher,
        "der Schluessel wurde ueberschrieben"
    );
}

/// Der Kontaktspeicher darf mit keinem fremden Schlüssel lesbar sein.
#[test]
fn der_kontaktspeicher_ist_an_die_identitaet_gebunden() {
    let w = Werkstatt::neu("speicher");
    identitaet(&w, "alice", "alice-geheim");
    identitaet(&w, "bob", "bob-geheim");
    identitaet(&w, "mallory", "mallory-geheim");
    let (a, b) = (wer("alice"), wer("bob"));

    w.ruf(&mit(&b, &["identity", "export", "--out", "bob.contact"]));
    w.ruf(&mit(
        &a,
        &["contacts", "add", "bob.contact", "--name", "Rechtsanwalt"],
    ));

    let roh = std::fs::read(w.pfad("alice.contacts")).expect("lesen");
    assert!(
        !roh.windows(12).any(|f| f == b"Rechtsanwalt"),
        "der Kontaktname stand lesbar in der Datei"
    );

    // Mallory hat die Datei, aber nicht den Schlüssel.
    let meldung = w.ruf_fehler(&[
        "contacts",
        "list",
        "--keyfile",
        "mallory.key",
        "--contacts",
        "alice.contacts",
        "--password-file",
        "mallory.pw",
        "-q",
    ]);
    assert!(!meldung.contains("Rechtsanwalt"), "{meldung}");
}

/// Ein Verzeichnis wird nur nach getippter Bestätigung gelöscht.
#[test]
fn ein_verzeichnis_verschwindet_nur_nach_bestaetigung() {
    let w = Werkstatt::neu("shred-dir");
    let ziel = w.pfad("ordner");
    std::fs::create_dir_all(ziel.join("unter")).expect("anlegen");
    std::fs::write(ziel.join("a.txt"), b"eins").expect("schreiben");
    std::fs::write(ziel.join("unter/b.txt"), b"zwei").expect("schreiben");

    // Ohne --confirm nur eine Vorschau.
    let vorschau = w.ruf(&["shred", "--dir", "ordner", "--json", "-q"]);
    let v: serde_json::Value = serde_json::from_str(&vorschau).expect("JSON");
    assert_eq!(v["ausgefuehrt"], serde_json::json!(false));
    assert_eq!(v["dateien"], serde_json::json!(2));
    assert!(ziel.exists(), "die Vorschau hat geloescht");

    // Falsches Wort.
    w.ruf_fehler(&["shred", "--dir", "ordner", "--confirm", "falsch", "-q"]);
    assert!(ziel.exists(), "falsches Wort hat trotzdem geloescht");

    // Richtiges Wort.
    w.ruf(&["shred", "--dir", "ordner", "--confirm", "ordner", "-q"]);
    assert!(!ziel.exists(), "das Verzeichnis blieb stehen");
}

/// Ein Verzeichnis mit `.git` wird kategorisch verweigert.
#[test]
fn ein_repository_wird_niemals_geloescht() {
    let w = Werkstatt::neu("shred-repo");
    let ziel = w.pfad("projekt");
    std::fs::create_dir_all(ziel.join(".git")).expect("anlegen");
    std::fs::write(ziel.join("wichtig.rs"), b"code").expect("schreiben");

    let meldung = w.ruf_fehler(&["shred", "--dir", "projekt", "--confirm", "projekt", "-q"]);
    assert!(meldung.contains("Git-Repository"), "{meldung}");
    assert!(ziel.join("wichtig.rs").exists(), "der Code ist weg");
}

/// Zwei verschiedene Passwörter lassen sich nicht aus einer Quelle lesen.
/// Stillschweigend dasselbe zu nehmen wäre schlimmer als der Abbruch: Das
/// Envelope-Passwort wäre dann das Keyfile-Passwort.
#[test]
fn zwei_passwoerter_aus_einer_quelle_werden_abgelehnt() {
    let w = Werkstatt::neu("zwei-passwoerter");
    identitaet(&w, "alice", "alice-geheim");
    identitaet(&w, "bob", "bob-geheim");
    let (a, b) = (wer("alice"), wer("bob"));

    w.ruf(&mit(&b, &["identity", "export", "--out", "bob.contact"]));
    w.ruf(&mit(
        &a,
        &["contacts", "add", "bob.contact", "--name", "Bob"],
    ));
    w.schreib("m.txt", b"hallo");

    let meldung = w.ruf_fehler(&mit(&a, &["encrypt", "m.txt", "--to", "Bob", "--password"]));
    assert!(
        meldung.contains("zwei verschiedene Passwörter"),
        "{meldung}"
    );
}

/// Metadaten werden erkannt, entfernt — und die Aussage bleibt ehrlich.
#[test]
fn metadaten_werden_entfernt_ohne_zu_viel_zu_versprechen() {
    let quelle =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testvectors/metadata/foto_mit_exif.jpg");
    if !quelle.exists() {
        // Ohne Testvektoren wird still übersprungen statt falsch bestanden.
        return;
    }

    let w = Werkstatt::neu("metadaten");
    let daten = std::fs::read(&quelle).expect("Testvektor lesen");
    w.schreib("foto.jpg", &daten);

    let vorher = w.ruf(&["metadata", "inspect", "foto.jpg", "--json", "-q"]);
    let v: serde_json::Value = serde_json::from_str(&vorher).expect("JSON");
    let funde = v["funde"].as_array().expect("Fundliste");
    assert!(!funde.is_empty(), "im Testbild steckt EXIF mit GPS");
    assert!(
        funde
            .iter()
            .any(|f| f["schwere"] == serde_json::json!("critical")),
        "GPS und Vorschaubild sind kritisch"
    );

    w.ruf(&["metadata", "strip", "foto.jpg", "--out", "sauber.jpg", "-q"]);

    let nachher = w.ruf(&["metadata", "inspect", "sauber.jpg", "--json", "-q"]);
    let v: serde_json::Value = serde_json::from_str(&nachher).expect("JSON");
    assert_eq!(
        v["funde"].as_array().map(Vec::len),
        Some(0),
        "es blieb etwas stehen"
    );

    // Das Bild muss noch ein Bild sein.
    let sauber = std::fs::read(w.pfad("sauber.jpg")).expect("lesen");
    assert_eq!(sauber.get(..2), Some(&[0xFF, 0xD8][..]), "kein JPEG mehr");
}

/// Ein v1-Keyfile wird erkannt und der Weg genannt, statt „beschädigt" zu
/// melden.
#[test]
fn ein_alter_schluessel_bekommt_einen_weg_gewiesen() {
    let w = Werkstatt::neu("v1");
    // Ein v1-Keyfile ist JSON mit diesen Feldern.
    w.schreib(
        "alt.key",
        br#"{"version":1,"kdf":"argon2id","salt":"AAAA","nonce":"AAAA","ct":"AAAA"}"#,
    );
    w.schreib("pw", b"egal");

    let meldung = w.ruf_fehler(&[
        "identity",
        "show",
        "--keyfile",
        "alt.key",
        "--password-file",
        "pw",
        "-q",
    ]);
    assert!(
        meldung.contains("migrate"),
        "der Weg zur Uebernahme fehlt: {meldung}"
    );
}
