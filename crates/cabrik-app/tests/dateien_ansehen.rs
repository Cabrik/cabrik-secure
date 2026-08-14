//! Was der Bildschirm über eine Datei erfährt, bevor etwas geschieht.
//!
//! # Die Eigenschaft, um die es geht
//!
//! **Der Befund ist der Vorgang, nicht eine Einschätzung davon.**
//! `datei_pruefen` ruft dieselbe Bereinigung auf, die beim Senden läuft,
//! und wirft das Ergebnis weg. Eine zweite Umsetzung derselben Frage wäre
//! bequemer und liefe beim nächsten Formatzusatz auseinander -- dann zeigte
//! der Bildschirm etwas anderes an, als danach passiert.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "Fehlschlag soll den Test abbrechen"
)]

use cabrik_bruecke::Bereinigung;

/// Ein PNG mit einem Textblock -- also mit etwas zu finden.
fn png_mit_text() -> Vec<u8> {
    let mut d = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let chunk = |typ: &[u8; 4], inhalt: &[u8]| {
        let mut c = Vec::new();
        c.extend_from_slice(&u32::try_from(inhalt.len()).expect("Laenge").to_be_bytes());
        c.extend_from_slice(typ);
        c.extend_from_slice(inhalt);
        let mut roh = typ.to_vec();
        roh.extend_from_slice(inhalt);
        c.extend_from_slice(&crc32(&roh).to_be_bytes());
        c
    };
    // IHDR: 1x1, 8 Bit, Graustufen.
    d.extend(chunk(b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 0, 0, 0, 0]));
    d.extend(chunk(b"tEXt", b"Author\0Wer das liest"));
    d.extend(chunk(b"IEND", b""));
    d
}

fn crc32(daten: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for &b in daten {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[test]
fn der_pfad_ist_die_kennung_und_der_name_bleibt_daneben() {
    // Zwei Dateien koennen gleich heissen. Wer den Namen als Kennung
    // benutzt, trifft mit jeder Ausnahme beide oder keine.
    let d = cabrik_app::datei_pruefen(
        "C:\\Arbeit\\Rechnung.pdf",
        "Rechnung.pdf",
        b"kein bekanntes Format",
    );

    assert_eq!(d.pfad, "C:\\Arbeit\\Rechnung.pdf");
    assert_eq!(d.name, "Rechnung.pdf");
}

#[test]
fn ein_unbekanntes_format_ergibt_keine_aussage() {
    // NICHT „sauber". Eine leere Fundliste sagt nur, dass nichts gefunden
    // wurde -- und wer das Format nicht versteht, findet nichts.
    let d = cabrik_app::datei_pruefen("/tmp/x.dat", "x.dat", b"weder Bild noch Dokument");

    match d.befund {
        Bereinigung::Unbekannt { .. } => {}
        anderer => panic!("erwartet: keine Aussage, bekommen: {anderer:?}"),
    }
}

#[test]
fn ein_png_mit_text_wird_verstanden_und_der_fund_benannt() {
    let daten = png_mit_text();
    let d = cabrik_app::datei_pruefen("/tmp/bild.png", "bild.png", &daten);

    match d.befund {
        Bereinigung::Vollstaendig { entfernt, format } => {
            assert!(format.to_lowercase().contains("png"), "Format: {format}");
            assert!(!entfernt.is_empty(), "der tEXt-Block muss auftauchen");
        }
        anderer => panic!("erwartet: vollstaendig, bekommen: {anderer:?}"),
    }
}

#[test]
fn die_groesse_ist_die_echte() {
    let daten = png_mit_text();
    let d = cabrik_app::datei_pruefen("/tmp/bild.png", "bild.png", &daten);

    assert_eq!(d.groesse_bytes, daten.len());
}

#[test]
fn ohne_pdf_gibt_es_keine_fassungen() {
    // Die leere Liste ist hier die richtige Aussage, kein fehlendes
    // Ergebnis: Nur PDF traegt einen Aenderungsverlauf.
    let d = cabrik_app::datei_pruefen("/tmp/bild.png", "bild.png", &png_mit_text());

    assert!(d.fassungen.is_empty());
}

#[test]
fn eine_leere_datei_bringt_nichts_zum_absturz() {
    // Der Fall, den ein Dateidialog jederzeit liefert.
    let d = cabrik_app::datei_pruefen("/tmp/leer.bin", "leer.bin", b"");

    assert_eq!(d.groesse_bytes, 0);
    match d.befund {
        Bereinigung::Unbekannt { .. } | Bereinigung::Fehler { .. } => {}
        anderer => panic!("erwartet: keine Aussage oder Fehler, bekommen: {anderer:?}"),
    }
}

#[test]
fn ein_abgeschnittenes_png_wird_als_fehler_gemeldet_nicht_als_sauber() {
    // Der gefaehrlichste Fehlgriff waere, eine kaputte Datei als bereinigt
    // auszugeben. „Liess sich nicht lesen" ist eine Aussage, „Format nicht
    // verstanden" ist keine -- beides ist besser als eine falsche.
    let mut daten = png_mit_text();
    daten.truncate(20);
    let d = cabrik_app::datei_pruefen("/tmp/kaputt.png", "kaputt.png", &daten);

    match d.befund {
        Bereinigung::Fehler { .. } | Bereinigung::Unbekannt { .. } => {}
        anderer => panic!("eine kaputte Datei darf nicht sauber heissen: {anderer:?}"),
    }
}

#[test]
fn der_befund_ist_derselbe_wie_die_spaetere_bereinigung() {
    // Die eigentliche Zusicherung. Waere das eine zweite Einschaetzung
    // statt des Vorgangs, koennte sie ihm davonlaufen -- und der
    // Bildschirm zeigte etwas anderes, als danach geschieht.
    let daten = png_mit_text();
    let d = cabrik_app::datei_pruefen("/tmp/bild.png", "bild.png", &daten);
    let (_sauber, wirklich) = cabrik_metadata::strip(&daten).expect("bereinigen");

    let gezeigt = match &d.befund {
        Bereinigung::Vollstaendig { entfernt, .. } => entfernt.len(),
        Bereinigung::Teilweise { entfernt, .. } => entfernt.len(),
        _ => 0,
    };
    let getan = match &wirklich {
        cabrik_metadata::StripResult::Complete { removed } => removed.len(),
        cabrik_metadata::StripResult::Partial { removed, .. } => removed.len(),
        cabrik_metadata::StripResult::Unknown { .. } => 0,
    };
    assert_eq!(gezeigt, getan);
}
