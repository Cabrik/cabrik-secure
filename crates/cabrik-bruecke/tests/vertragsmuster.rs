//! Erzeugt die Prüfmuster, gegen die das Frontend seinen Vertrag hält.
//!
//! # Warum als Test und nicht als Programm
//!
//! Ein Erzeugungsprogramm läuft, wenn jemand daran denkt. Ein Test läuft in
//! der CI. Und weil er die erzeugten Dateien mit den eingecheckten
//! **vergleicht**, statt sie stillschweigend zu überschreiben, schlägt er
//! fehl, sobald sich der Vertrag ändert — genau dann, wenn das Frontend
//! nachziehen muss.
//!
//! Neu erzeugen mit:
//!
//! ```text
//! MUSTER_SCHREIBEN=1 cargo test -p cabrik-bruecke
//! ```
//!
//! # Warum jede Variante vorkommen muss
//!
//! Ein Muster, das nur den Normalfall abbildet, prüft den Normalfall. Die
//! Fälle, an denen sich dieses Projekt entscheidet, sind aber die anderen:
//! `Unbekannt` ohne Formathinweis, ein verifizierter Absender **ohne**
//! vermerkten Weg, ein Kontakt ohne Post-Quantum-Schlüssel. Der Test unten
//! zählt deshalb ab, dass jede Variante genau einmal auftaucht.

// Ein Test, der seine Vorbedingung nicht herstellen kann, hat kein
// Ergebnis, sondern einen kaputten Test. Im Programm selbst gelten die
// Regeln unveraendert weiter.
#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "Fehlschlag soll den Test abbrechen"
)]

use cabrik_bruecke::{
    Absender, Aussenansicht, Bereinigung, Fassung, Fund, Fundart, Geoeffnet, Identitaet,
    Inhaltsart, KdfStufe, Kontakt, Loeschbeurteilung, Loeschergebnis, Loeschfaehigkeit,
    Loeschvorbehalt, Schwere, Sendedatei, Sitzungsstand, Sperrfrist, Verifikationsweg,
    Vertrauen,
};
use std::path::PathBuf;


/// Wohin die Muster gehören: neben den Vertrag, den sie prüfen.
fn ordner() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../app/oberflaeche/src/lib/kern/vertrag")
}

/// Schreibt oder vergleicht — je nach Umgebungsvariable.
fn muster(name: &str, wert: &impl serde::Serialize) {
    let json = format!("{}\n", serde_json::to_string_pretty(wert).expect("JSON"));
    let pfad = ordner().join(format!("{name}.json"));

    if std::env::var_os("MUSTER_SCHREIBEN").is_some() {
        std::fs::create_dir_all(ordner()).expect("Ordner");
        std::fs::write(&pfad, &json).expect("schreiben");
        return;
    }

    let vorhanden = std::fs::read_to_string(&pfad).unwrap_or_else(|_| {
        panic!(
            "Prüfmuster {} fehlt.\n\
             Neu erzeugen mit: MUSTER_SCHREIBEN=1 cargo test -p cabrik-bruecke",
            pfad.display()
        )
    });

    // Zeilenenden vereinheitlichen: Git normalisiert sie unter Windows.
    assert_eq!(
        vorhanden.replace("\r\n", "\n"),
        json,
        "\nDer Vertrag hat sich geaendert: {name}\n\
         Das Frontend muss nachziehen (app/oberflaeche/src/lib/kern/typen.ts).\n\
         Danach neu erzeugen mit: MUSTER_SCHREIBEN=1 cargo test -p cabrik-bruecke\n"
    );
}

fn fund(art: Fundart, ort: &str, wert: Option<&str>, schwere: Schwere) -> Fund {
    Fund {
        art,
        ort: ort.to_owned(),
        wert: wert.map(str::to_owned),
        schwere,
    }
}

// ---------------------------------------------------------------------------

#[test]
fn bereinigung_alle_vier_faelle() {
    let faelle = vec![
        Bereinigung::Vollstaendig {
            entfernt: vec![
                fund(
                    Fundart::Ortsangabe,
                    "EXIF:GPSLatitude",
                    Some("52.5163, 13.3777"),
                    Schwere::Kritisch,
                ),
                fund(
                    Fundart::Farbprofil,
                    "JPEG:APP2/ICC",
                    None,
                    Schwere::Gering,
                ),
            ],
            format: "JPEG".to_owned(),
        },
        Bereinigung::Teilweise {
            entfernt: vec![fund(
                Fundart::Personenname,
                "MP3:ID3v2/TPE1",
                Some("Dr. Anna Beispiel"),
                Schwere::Kritisch,
            )],
            geblieben: vec![fund(
                Fundart::Software,
                "MP3:Tonrahmen",
                Some("LAME"),
                Schwere::Beachtlich,
            )],
            grund: "Der Name des Kodierers steckt in den Zusatzdaten der Tonrahmen."
                .to_owned(),
            format: "MP3".to_owned(),
        },
        Bereinigung::Unbekannt {
            formathinweis: Some("Photoshop-Dokument (PSD)".to_owned()),
        },
        // Ohne Hinweis: Der Fall, in dem nicht einmal das Format erkennbar
        // war. Die Oberflaeche muss auch dafuer einen Satz haben.
        Bereinigung::Unbekannt {
            formathinweis: None,
        },
        Bereinigung::Fehler {
            grund: "Die Datei liess sich nicht lesen.".to_owned(),
        },
    ];
    muster("bereinigung", &faelle);
}

#[test]
fn absender_alle_sechs_faelle() {
    let faelle = vec![
        Absender::Unsigniert,
        Absender::Unbekannt {
            signierschluessel: "3F8A1C2B4D5E6F70".to_owned(),
        },
        Absender::Bekannt {
            fingerprint: "C9KY J9RH P88Z 1BQ4 M76W".to_owned(),
            name: "Bert Muster".to_owned(),
        },
        Absender::Verifiziert {
            fingerprint: "8F3B 1C2A 4D5E 4F60 9A7B".to_owned(),
            name: "Dr. Anna Beispiel".to_owned(),
            verifiziert_am: Some(1_770_000_000),
            verifiziert_ueber: Some(Verifikationsweg::SafetyNumber),
        },
        // Der Fall, den es seit heute erst gibt: verifiziert, aber der Weg
        // ist nicht vermerkt. Bei aus v1 uebernommenen Kontakten der
        // Normalfall -- v1 kannte die Unterscheidung nicht.
        Absender::Verifiziert {
            fingerprint: "W9VZ KAZQ 3QNH HBM3 6AQ6".to_owned(),
            name: "Archiv".to_owned(),
            verifiziert_am: None,
            verifiziert_ueber: None,
        },
        Absender::Gewechselt {
            fingerprint: "DVKQ G1JC 05M3 MKPN 825Q".to_owned(),
            name: "Cora Steinbach".to_owned(),
            vorheriger_fingerprint: Some("AAAA BBBB CCCC DDDD EEEE".to_owned()),
            vorher_verifiziert: true,
        },
        Absender::Widerrufen {
            fingerprint: "29WN 92PP 1JH8 7P1M 10C5".to_owned(),
            name: "Unbekannter Zutraeger".to_owned(),
        },
    ];
    muster("absender", &faelle);
}

#[test]
fn kontakt_alle_vier_zustaende() {
    let bau = |name: &str, vertrauen, pq: bool| Kontakt {
        name: name.to_owned(),
        fingerprint: "8F3B 1C2A 4D5E 4F60 9A7B 1C2D 3E4F 5061 8F3B 1C2A".to_owned(),
        vertrauen,
        seit: 1_762_000_000,
        verifiziert_am: matches!(vertrauen, Vertrauen::Verifiziert)
            .then_some(1_770_000_000),
        verifiziert_ueber: matches!(vertrauen, Vertrauen::Verifiziert)
            .then_some(Verifikationsweg::Qr),
        notiz: None,
        hat_post_quantum: pq,
        safety_number: "38472 91053 66218 40397 15884 72609".to_owned(),
    };
    let faelle = vec![
        bau("Dr. Anna Beispiel", Vertrauen::Verifiziert, true),
        bau("Bert Muster", Vertrauen::Gesehen, true),
        bau("Cora Steinbach", Vertrauen::Gewechselt, true),
        bau("Unbekannter Zutraeger", Vertrauen::Widerrufen, true),
        // Ohne Post-Quantum: aus v1 uebernommen.
        bau("Archiv", Vertrauen::Verifiziert, false),
    ];
    muster("kontakt", &faelle);
}

#[test]
fn fassungen_mit_entferntem_text() {
    let faelle = vec![
        Fassung {
            nummer: 1,
            bytes: 96_112,
            seiten: 4,
            wird_angezeigt: false,
            auszug: "Vermerk zur Sitzung.".to_owned(),
            nur_hier: vec!["Hinweisgeber: Martin Kessler".to_owned()],
        },
        Fassung {
            nummer: 2,
            bytes: 184_320,
            seiten: 4,
            wird_angezeigt: true,
            auszug: "Vermerk zur Sitzung.".to_owned(),
            nur_hier: vec![],
        },
    ];
    muster("fassung", &faelle);
}

/// Jede Fundart einmal — auch die, die es im Kern noch nicht gibt.
#[test]
fn fundart_vollstaendig() {
    let alle = [
        Fundart::Ortsangabe,
        Fundart::Personenname,
        Fundart::Geraet,
        Fundart::Software,
        Fundart::Zeitangabe,
        Fundart::Organisation,
        Fundart::Vorschaubild,
        Fundart::ZugeschnittenesBild,
        Fundart::NachverfolgteAenderung,
        Fundart::Farbprofil,
        Fundart::Kommentar,
        Fundart::Bearbeitungssitzung,
        Fundart::Dateiname,
        Fundart::UnbekannteErweiterung,
        Fundart::Unbekannt,
    ];
    muster("fundart", &alle);
}

/// Die Umwandlung deckt jede heute bekannte Fundart des Kerns ab.
///
/// Schlägt fehl, sobald im Kern eine hinzukommt — dann fällt sie auf
/// `Unbekannt`, und dieser Test sagt, dass der Vertrag nachziehen sollte.
#[test]
fn jede_bekannte_fundart_hat_eine_entsprechung() {
    use cabrik_metadata::model::FindingKind;
    let bekannt = [
        FindingKind::Gps,
        FindingKind::Author,
        FindingKind::Device,
        FindingKind::Software,
        FindingKind::Timestamp,
        FindingKind::Organization,
        FindingKind::EmbeddedPreview,
        FindingKind::CroppedImage,
        FindingKind::TrackedChange,
        FindingKind::ColorProfile,
        FindingKind::Comment,
        FindingKind::EditingSession,
        FindingKind::FileName,
        FindingKind::UnknownExtension,
    ];
    for k in bekannt {
        assert_ne!(
            Fundart::from(k),
            Fundart::Unbekannt,
            "{k:?} faellt auf Unbekannt -- der Vertrag kennt sie nicht"
        );
    }
}

// ---------------------------------------------------------------------------
// Öffnen, Außenansicht, Löschen
// ---------------------------------------------------------------------------

#[test]
fn geoeffnet_text_und_datei() {
    let faelle = vec![
        // Eine Textnachricht: Der Text IST der Inhalt und wird
        // durchgereicht. Ihn zurückzuhalten hieße, sie nicht zu zeigen.
        Geoeffnet {
            art: Inhaltsart::Text,
            text: Some("Treffen verschoben auf Donnerstag.".to_owned()),
            dateiname: None,
            groesse_bytes: 34,
            zeitpunkt: Some(1_772_000_000),
            absender: Absender::Verifiziert {
                fingerprint: "8F3B 1C2A 4D5E 4F60 9A7B".to_owned(),
                name: "Dr. Anna Beispiel".to_owned(),
                verifiziert_am: Some(1_770_000_000),
                verifiziert_ueber: Some(Verifikationsweg::SafetyNumber),
            },
            metadaten: None,
        },
        // Eine Datei: nur Name und Größe. Die Bytes bleiben in Rust.
        Geoeffnet {
            art: Inhaltsart::Datei,
            text: None,
            dateiname: Some("Protokoll.pdf".to_owned()),
            groesse_bytes: 184_320,
            zeitpunkt: None,
            absender: Absender::Unsigniert,
            metadaten: Some(Bereinigung::Vollstaendig {
                entfernt: vec![],
                format: "PDF".to_owned(),
            }),
        },
    ];
    muster("geoeffnet", &faelle);
}

#[test]
fn aussenansicht_v1_und_v2() {
    let faelle = vec![
        // v2: nichts als die Kapselzahl.
        Aussenansicht {
            fassung: "v2".to_owned(),
            suite: Some("Post-Quantum-Hybrid (0x0002)".to_owned()),
            kapseln: Some(3),
            groesse_bytes: 190_112,
            offengelegt: vec![],
        },
        // v1: der Kopf stand im Klartext. Die Sätze kommen aus dem Kern --
        // die Oberfläche zählt sie auf, statt sie zu deuten.
        Aussenansicht {
            fassung: "v1".to_owned(),
            suite: Some("klassisch (v1)".to_owned()),
            kapseln: Some(1),
            groesse_bytes: 188_204,
            offengelegt: vec![
                "Dateiname: Kuendigung-Mueller.pdf".to_owned(),
                "Klartextgröße: 184320 Bytes".to_owned(),
                "Signierschlüssel des Absenders".to_owned(),
            ],
        },
    ];
    muster("aussenansicht", &faelle);
}

#[test]
fn loeschen_beurteilung_und_ergebnis() {
    let beurteilungen = vec![
        Loeschbeurteilung {
            faehigkeit: Loeschfaehigkeit::Ueberschreiben,
            vorbehalte: vec![Loeschvorbehalt::WarSchreibgeschuetzt],
        },
        // Der Normalfall auf heutigen Systemen.
        Loeschbeurteilung {
            faehigkeit: Loeschfaehigkeit::BestEffort,
            vorbehalte: vec![Loeschvorbehalt::KopienMoeglich],
        },
        Loeschbeurteilung {
            faehigkeit: Loeschfaehigkeit::NichtMoeglich,
            vorbehalte: vec![
                Loeschvorbehalt::WechselOderNetz,
                Loeschvorbehalt::KopienMoeglich,
            ],
        },
        // Jeder Vorbehalt kommt mindestens einmal vor.
        Loeschbeurteilung {
            faehigkeit: Loeschfaehigkeit::BestEffort,
            vorbehalte: vec![
                Loeschvorbehalt::CloudOrdner {
                    hinweis: "Ordnername „OneDrive“ und Reparse-Punkt".to_owned(),
                },
                Loeschvorbehalt::KopienMoeglich,
                Loeschvorbehalt::ZeitstempelBlieb,
            ],
        },
    ];
    muster("loeschbeurteilung", &beurteilungen);

    let ergebnisse = vec![
        Loeschergebnis {
            pfad: "D:\\Archiv\\Protokoll-2019.pdf".to_owned(),
            faehigkeit: Loeschfaehigkeit::Ueberschreiben,
            ueberschrieben: true,
            umbenannt: true,
            entfernt: true,
            vorbehalte: vec![Loeschvorbehalt::WarSchreibgeschuetzt],
            fehler: None,
        },
        // Fehlgeschlagen: Der Grund gehört in die Anzeige, nicht in ein Log.
        Loeschergebnis {
            pfad: "\\\\server\\freigabe\\Liste.xlsx".to_owned(),
            faehigkeit: Loeschfaehigkeit::NichtMoeglich,
            ueberschrieben: false,
            umbenannt: false,
            entfernt: false,
            vorbehalte: vec![Loeschvorbehalt::WechselOderNetz],
            fehler: Some("Zugriff verweigert".to_owned()),
        },
    ];
    muster("loeschergebnis", &ergebnisse);
}

/// Die Sitzung — der Vertrag, den der Sperrbildschirm liest.
///
/// Die drei Lagen unterscheiden sich in genau einem Feld, und die
/// Oberfläche muss alle drei auseinanderhalten:
///
/// - **offen mit Frist**: `restsekunden` trägt eine Zahl
/// - **offen ohne Frist**: `restsekunden` ist `null` — nicht `0`. Null
///   hieße „gleich", `null` heißt „es läuft nichts"
/// - **gesperrt**: ebenfalls `null`, denn es gibt nichts abzuwarten
#[test]
fn sitzung_alle_lagen() {
    muster(
        "sperrfrist",
        &vec![
            Sperrfrist::EineMinute,
            Sperrfrist::FuenfMinuten,
            Sperrfrist::FuenfzehnMinuten,
            Sperrfrist::DreissigMinuten,
            Sperrfrist::EineStunde,
            Sperrfrist::BisZumSchliessen,
        ],
    );

    let lagen = vec![
        Sitzungsstand {
            gesperrt: false,
            frist: Sperrfrist::FuenfzehnMinuten,
            restsekunden: Some(842),
        },
        Sitzungsstand {
            gesperrt: false,
            frist: Sperrfrist::BisZumSchliessen,
            restsekunden: None,
        },
        Sitzungsstand {
            gesperrt: true,
            frist: Sperrfrist::FuenfzehnMinuten,
            restsekunden: None,
        },
    ];
    muster("sitzungsstand", &lagen);
}

/// Die eigene Identität.
///
/// Zwei Fälle, die beide vorkommen und verschieden aussehen müssen:
///
/// 1. Eine frisch angelegte mit benannter Stufe und Signierschlüssel.
/// 2. Eine ohne Bezeichnung, ohne Signierschlüssel und mit **eigenen**
///    KDF-Werten. `kdf: null` heißt dort nicht „unbekannt", sondern „zu
///    keiner der drei Stufen gehörend" — die Zahl daneben ist die Aussage.
///
/// Der zweite Fall ist der, an dem sich die Anzeige entscheidet: Wer nur
/// den ersten baut, zeigt später `null` als leeres Feld an, wo eine Zahl
/// stehen müsste.
#[test]
fn identitaet_beide_faelle() {
    muster(
        "kdf_stufe",
        &vec![KdfStufe::Min, KdfStufe::Empfohlen, KdfStufe::Stark],
    );

    let faelle = vec![
        Identitaet {
            bezeichnung: Some("Arbeit".to_owned()),
            fingerprint: "XMSW-CE1Q-0RKZ-VB6C-WGSS-F01G-TMV4-699J-N238-4C3F-HXJT-4NQ9-7YZ0"
                .to_owned(),
            fingerprint_kurz: "XMSWCE1Q".to_owned(),
            erzeugt_am: 1_732_000_000,
            kdf: Some(KdfStufe::Empfohlen),
            kdf_speicher_mib: 256,
            hat_signierschluessel: true,
            hat_post_quantum: true,
            pfad: "C:\\Users\\name\\AppData\\Roaming\\CabrikSecure\\identity.cabrik-key"
                .to_owned(),
        },
        Identitaet {
            bezeichnung: None,
            fingerprint: "7YZ0-4NQ9-HXJT-4C3F-N238-699J-TMV4-F01G-WGSS-VB6C-0RKZ-CE1Q-XMSW"
                .to_owned(),
            fingerprint_kurz: "7YZ04NQ9".to_owned(),
            erzeugt_am: 1_700_000_000,
            kdf: None,
            kdf_speicher_mib: 195,
            hat_signierschluessel: false,
            hat_post_quantum: true,
            pfad: "/home/name/.config/cabrik/identity.cabrik-key".to_owned(),
        },
    ];
    muster("identitaet", &faelle);
}

/// Eine Datei, die verschickt werden soll.
///
/// Zwei Fälle, und der zweite ist der, an dem sich die Anzeige entscheidet:
/// **derselbe Name in zwei Ordnern.** Wer den Namen als Kennung benutzt,
/// trifft mit jeder Ausnahme beide oder keine — und merkt es erst, wenn
/// jemand versehentlich etwas mitschickt.
#[test]
fn sendedatei_zweimal_derselbe_name() {
    let faelle = vec![
        Sendedatei {
            pfad: "C:\\Arbeit\\Rechnung.pdf".to_owned(),
            name: "Rechnung.pdf".to_owned(),
            groesse_bytes: 184_320,
            befund: Bereinigung::Teilweise {
                entfernt: vec![fund(
                    Fundart::Software,
                    "PDF:Producer",
                    Some("Microsoft Word"),
                    Schwere::Gering,
                )],
                geblieben: vec![fund(
                    Fundart::NachverfolgteAenderung,
                    "PDF:Revisions",
                    None,
                    Schwere::Kritisch,
                )],
                grund: "Frühere Fassungen bleiben erhalten.".to_owned(),
                format: "PDF".to_owned(),
            },
            fassungen: vec![Fassung {
                nummer: 1,
                bytes: 120_000,
                seiten: 3,
                wird_angezeigt: false,
                auszug: "Angebot über 12.000 EUR".to_owned(),
                nur_hier: vec!["Rabatt intern: 30 %".to_owned()],
            }],
        },
        Sendedatei {
            pfad: "C:\\Privat\\Rechnung.pdf".to_owned(),
            name: "Rechnung.pdf".to_owned(),
            groesse_bytes: 22_105,
            befund: Bereinigung::Unbekannt {
                formathinweis: None,
            },
            fassungen: vec![],
        },
    ];
    muster("sendedatei", &faelle);
}
