//! Läuft die Regelliste dieser Kiste noch mit der des Arbeitsbereichs?
//!
//! # Warum es diesen Test gibt
//!
//! Alle anderen acht Kisten schreiben `[lints] workspace = true` und erben
//! die Regeln damit. Diese eine kann das nicht: Cargo lässt kein Erben mit
//! einer Ausnahme zu und sagt das auch deutlich —
//!
//! > cannot override `workspace.lints` in `lints`, either remove the
//! > overrides or `lints.workspace = true` and manually specify the lints
//!
//! Also steht die Liste hier ein zweites Mal. Zwei Abschriften derselben
//! Entscheidung gehen irgendwann auseinander, und zwar still: Wer künftig
//! im Arbeitsbereich eine Regel ergänzt, hat sie überall — nur nicht in der
//! einen Kiste, in der `unsafe` erlaubt ist. Das wäre die schlechteste
//! Stelle für eine Lücke.
//!
//! Der Test vergleicht deshalb beide Listen und lässt genau einen
//! Unterschied durch: `unsafe_code`.
//!
//! # Und die anderen Listen im selben Haus
//!
//! Inzwischen steht hier mehr als die Regelliste. Es ist der Ort für
//! Aufzählungen geworden, die von Hand geführt werden und deshalb still
//! veralten: welche Kiste die Regeln erbt, welche quelloffen ist, welche im
//! Pfadfilter der Gegenprobe steht. Jede einzelne davon ist an einem Tag
//! danebengegangen, und keine hätte sich beim Lesen von selbst gemeldet.

#![expect(clippy::unwrap_used, reason = "Fehlschlag soll den Test abbrechen")]

use std::collections::BTreeMap;
use std::path::PathBuf;

/// Liest eine TOML-Tabelle als Paare, bis die nächste Überschrift kommt.
///
/// Ein vollwertiger TOML-Leser wäre hier eine Abhängigkeit für zwei
/// Dutzend Zeilen aus einer Datei, die wir selbst schreiben. Was dieser
/// Leser nicht versteht, fällt sofort auf: Dann fehlt ein Eintrag, und der
/// Vergleich unten schlägt an.
fn tabelle(inhalt: &str, ueberschrift: &str) -> BTreeMap<String, String> {
    let mut paare = BTreeMap::new();
    let mut drin = false;
    for zeile in inhalt.lines() {
        let z = zeile.trim();
        if z.starts_with('[') {
            drin = z == ueberschrift;
            continue;
        }
        if !drin || z.is_empty() || z.starts_with('#') {
            continue;
        }
        if let Some((schluessel, wert)) = z.split_once('=') {
            paare.insert(
                schluessel.trim().to_owned(),
                wert.trim().trim_matches('"').to_owned(),
            );
        }
    }
    paare
}

fn wurzel() -> PathBuf {
    let hier = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    hier.parent()
        .and_then(std::path::Path::parent)
        .map(PathBuf::from)
        .unwrap()
}

#[test]
fn dieselben_regeln_bis_auf_unsafe_code() {
    let wurzel_toml = std::fs::read_to_string(wurzel().join("Cargo.toml")).unwrap();
    let eigene_toml =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
            .unwrap();

    for (dort, hier) in [
        ("[workspace.lints.rust]", "[lints.rust]"),
        ("[workspace.lints.clippy]", "[lints.clippy]"),
    ] {
        let mut arbeitsbereich = tabelle(&wurzel_toml, dort);
        let mut kiste = tabelle(&eigene_toml, hier);

        assert!(
            !arbeitsbereich.is_empty(),
            "{dort} nicht gefunden oder leer -- der Test prueft sonst nichts"
        );

        // Der eine erlaubte Unterschied, und er wird einzeln geprueft.
        let dort_unsafe = arbeitsbereich.remove("unsafe_code");
        let hier_unsafe = kiste.remove("unsafe_code");
        if dort.contains("rust") {
            assert_eq!(
                dort_unsafe.as_deref(),
                Some("forbid"),
                "der Arbeitsbereich verbietet `unsafe` nicht mehr -- dann ist diese Kiste keine Ausnahme mehr, sondern die Regel"
            );
            assert_eq!(
                hier_unsafe.as_deref(),
                Some("deny"),
                "diese Kiste steht nicht mehr auf `deny`"
            );
        }

        assert_eq!(
            arbeitsbereich, kiste,
            "die Regellisten laufen auseinander ({dort} gegen {hier}). \
             Was im Arbeitsbereich ergaenzt wurde, gehoert auch hierher."
        );
    }
}

#[test]
fn nur_diese_eine_kiste_ist_ausgenommen() {
    // GEGENSTUECK: Der Test oben achtet auf die Liste, dieser auf ihre
    // Alleinstellung. Erbte eine zweite Kiste die Regeln nicht mehr, faende
    // es sonst niemand.
    let crates = wurzel().join("crates");
    let mut ohne_erbe = Vec::new();

    for eintrag in std::fs::read_dir(&crates).unwrap() {
        let pfad = eintrag.unwrap().path();
        let manifest = pfad.join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        let inhalt = std::fs::read_to_string(&manifest).unwrap();
        // Ueber die Tabelle und nicht ueber Textsuche. Der erste Anlauf
        // suchte schlicht nach „[lints]" und „workspace = true" -- und fand
        // beides im KOMMENTAR dieser Kiste, der erklaert, dass sie genau das
        // nicht tut. Der Test hielt sich daraufhin selbst zum Narren.
        if tabelle(&inhalt, "[lints]")
            .get("workspace")
            .map(String::as_str)
            != Some("true")
        {
            let name = pfad
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap_or("?")
                .to_owned();
            ohne_erbe.push(name);
        }
    }

    ohne_erbe.sort();
    assert_eq!(
        ohne_erbe,
        vec!["cabrik-speicher".to_owned()],
        "diese Kisten erben die Regeln des Arbeitsbereichs nicht"
    );
}

#[test]
fn die_offenen_kisten_haengen_an_nichts_proprietaerem() {
    // WARUM DAS HIER STEHT. Die Roadmap führt unter 5.0 eine Handprüfung:
    // „geprüft, dass die offenen einen geschlossenen Teilgraphen bilden".
    // Sie galt für vier Kisten. Mit `cabrik-speicher` sind es fünf, und
    // eine Handprüfung, die bei jeder neuen Kiste wiederholt werden müsste,
    // wird irgendwann nicht wiederholt.
    //
    // Die Zusage dahinter ist keine Förmlichkeit: Hinge eine offene Kiste
    // an einer proprietären, liesse sie sich nicht für sich bauen -- und
    // damit waere ihre Prüfbarkeit, der ganze Grund für die Offenlegung,
    // eine leere Behauptung.
    let crates = wurzel().join("crates");
    let mut lizenz = BTreeMap::new();
    let mut inhalte = BTreeMap::new();

    for eintrag in std::fs::read_dir(&crates).unwrap() {
        let pfad = eintrag.unwrap().path();
        let manifest = pfad.join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        let inhalt = std::fs::read_to_string(&manifest).unwrap();
        let name = tabelle(&inhalt, "[package]")
            .get("name")
            .cloned()
            .unwrap_or_default();
        assert!(!name.is_empty(), "Kiste ohne Namen in {}", pfad.display());
        let offen =
            tabelle(&inhalt, "[package]").get("license").cloned() == Some("Apache-2.0".into());
        lizenz.insert(name.clone(), offen);
        inhalte.insert(name, inhalt);
    }

    let offene: Vec<&String> = lizenz
        .iter()
        .filter(|(_, offen)| **offen)
        .map(|(name, _)| name)
        .collect();
    assert!(
        offene.len() >= 5,
        "es sollten mindestens fuenf offene Kisten sein, gefunden: {offene:?}"
    );

    for name in &offene {
        let inhalt = inhalte.get(*name).unwrap();
        for (andere, offen) in &lizenz {
            if *offen || andere == *name {
                continue;
            }
            assert!(
                !inhalt.contains(andere.as_str()),
                "die offene Kiste `{name}` nennt die proprietaere `{andere}` \
                 in ihrem Manifest -- dann laesst sie sich nicht fuer sich bauen"
            );
        }
    }
}

#[test]
fn die_gegenprobe_bewacht_jede_kiste_die_sie_uebersetzt() {
    // Schritt 3 der Gegenprobe ruft `cargo test --workspace --exclude
    // cabrik-fenster`. Ihr Pfadfilter kannte aber nur vier Kisten -- und
    // das ging so aus: Ein Fehler in `cabrik-speicher` liess die Gegenprobe
    // scheitern, die Behebung fasste dieselbe Kiste an, der Filter loeste
    // nicht aus, und der rote Lauf blieb stehen. Niemand haette ihn je neu
    // angestossen.
    //
    // Ein Filter, der weniger abdeckt als der Lauf, versteckt genau die
    // Fehler, die der Lauf finden soll.
    let ablauf =
        std::fs::read_to_string(wurzel().join(".github/workflows/gegenprobe.yml")).unwrap();

    // Womit die Gegenprobe testet, steht in ihr selbst -- nicht hier noch
    // einmal. Sonst waere dieser Test die naechste Abschrift, die veraltet.
    assert!(
        ablauf.contains("cargo test --workspace --exclude cabrik-fenster"),
        "die Gegenprobe testet nicht mehr die ganze Werkbank -- dann stimmt \
         die Erwartung dieses Tests nicht mehr, und beides gehoert angesehen"
    );

    let mut fehlend = Vec::new();
    for eintrag in std::fs::read_dir(wurzel().join("crates")).unwrap() {
        let pfad = eintrag.unwrap().path();
        if !pfad.join("Cargo.toml").is_file() {
            continue;
        }
        let name = pfad
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or("?")
            .to_owned();
        // Die eine, die der Lauf ausdruecklich auslaesst.
        if name == "cabrik-fenster" {
            assert!(
                !ablauf.contains(&format!("\"crates/{name}/**\"")),
                "`{name}` wird von der Gegenprobe ausgeschlossen, steht aber \
                 in ihrem Pfadfilter -- dann startet ein 23-Minuten-Lauf fuer \
                 eine Kiste, die er gar nicht uebersetzt"
            );
            continue;
        }
        if !ablauf.contains(&format!("\"crates/{name}/**\"")) {
            fehlend.push(name);
        }
    }

    fehlend.sort();
    assert!(
        fehlend.is_empty(),
        "diese Kisten uebersetzt die Gegenprobe, ohne dass eine Aenderung \
         daran sie ausloest: {fehlend:?}"
    );
}

/// So viele Stellen heben `unsafe_code` auf — im **Text**, nicht im Bau.
///
/// Je nach Betriebssystem wird nur ein Teil davon übersetzt: Der
/// Windows-Zweig und der POSIX-Zweig stehen beide da, aber nie beide
/// zugleich im Ergebnis.
const AUFHEBUNGEN: usize = 26;

#[test]
fn es_gibt_nicht_mehr_unsafe_als_gezaehlt() {
    // KEIN SELBSTZWECK. In der einen Kiste, in der `unsafe` erlaubt ist,
    // soll jede neue Stelle jemandem auffallen -- auch dem, der sie
    // schreibt. Eine Zahl, die man beim Hinzufuegen anfassen muss, ist die
    // billigste Bremse, die es dafuer gibt.
    //
    // Sie ist ausdruecklich keine Obergrenze und kein Urteil. Wer eine
    // sechzehnte braucht, erhoeht sie -- und hat dabei einen Augenblick
    // darueber nachgedacht.
    let quelle = wurzel().join("crates/cabrik-speicher/src");
    let mut gefunden = 0_usize;
    let mut dateien = Vec::new();

    for eintrag in std::fs::read_dir(&quelle).unwrap() {
        let pfad = eintrag.unwrap().path();
        if pfad.extension().and_then(std::ffi::OsStr::to_str) != Some("rs") {
            continue;
        }
        let inhalt = std::fs::read_to_string(&pfad).unwrap();
        let hier = inhalt.matches("#[allow(unsafe_code)]").count();
        if hier > 0 {
            dateien.push(format!(
                "{}: {hier}",
                pfad.file_name()
                    .and_then(std::ffi::OsStr::to_str)
                    .unwrap_or("?")
            ));
        }
        gefunden += hier;
    }

    dateien.sort();
    assert_eq!(
        gefunden, AUFHEBUNGEN,
        "die Zahl der `unsafe`-Aufhebungen hat sich geaendert ({dateien:?}). \
         Ist das Absicht, wird `AUFHEBUNGEN` hier und die Zahl im Kopf von \
         `src/lib.rs` nachgezogen."
    );

    // Und die Zahl im Kopf der Kiste muss dasselbe sagen. Zwei Stellen
    // fuer dieselbe Zahl gehen sonst auseinander -- schon wieder.
    let lib = std::fs::read_to_string(quelle.join("lib.rs")).unwrap();
    assert!(
        lib.contains("sechsundzwanzig Stellen"),
        "die Zahl im Kopf von `src/lib.rs` passt nicht mehr zu {AUFHEBUNGEN}"
    );
}
