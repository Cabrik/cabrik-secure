//! Prüft WebP, GIF und BMP an **echten**, von Pillow erzeugten Dateien.
//!
//! Die Modultests bauen die Formate von Hand — das prüft die Struktur, aber
//! nicht die Wirklichkeit. Erst eine echte Datei zeigt, wie ein Erzeuger die
//! Chunks tatsächlich anordnet.
//!
//! Der wichtigste Fund dieser Runde kam von hier: Die BMP-Erkennung verlangte,
//! dass die im Kopf angegebene Dateigröße genau der Länge entspricht — und
//! wies damit ausgerechnet die Datei mit Anhängsel ab, also genau den Fall,
//! für den das Modul gebaut wurde.
//!
//! Vorlagen erzeugen mit:
//!
//! ```text
//! python testvectors/tools/gen_metadata_fixtures.py
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use cabrik_metadata::{FindingKind, Severity, StripResult, inspect, strip};
use std::path::PathBuf;

fn lade(name: &str) -> Option<Vec<u8>> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testvectors/metadata")
        .join(name);
    std::fs::read(p).ok()
}

fn enthaelt(daten: &[u8], spur: &[u8]) -> bool {
    daten.windows(spur.len()).any(|f| f == spur)
}

/// WebP trägt seine Metadaten in RIFF-Chunks.
#[test]
fn ein_echtes_webp_wird_bereinigt() {
    let Some(daten) = lade("bild_mit_metadaten.webp") else {
        eprintln!("uebersprungen: gen_metadata_fixtures.py wurde nicht ausgefuehrt");
        return;
    };

    let vorher = inspect(&daten).unwrap();
    assert_eq!(vorher.format.as_deref(), Some("WebP"));
    assert!(
        vorher
            .findings
            .iter()
            .any(|f| f.severity == Severity::Critical),
        "der XMP-Block mit dem Namen fehlt: {:?}",
        vorher.findings
    );
    assert!(enthaelt(&daten, b"Dr. Anna Beispiel"), "Vorlage ohne Namen");

    let (sauber, ergebnis) = strip(&daten).unwrap();
    assert!(ergebnis.may_show_clean(), "{ergebnis:?}");

    for spur in [
        &b"Dr. Anna Beispiel"[..],
        b"Kamerahersteller",
        b"XY-2000",
        b"FAKE-ICC-PROFIL",
    ] {
        assert!(!enthaelt(&sauber, spur), "Spur blieb: {spur:?}");
    }

    // Es muss ein WebP bleiben.
    assert!(sauber.starts_with(b"RIFF"));
    assert_eq!(sauber.get(8..12), Some(&b"WEBP"[..]));
    assert!(inspect(&sauber).unwrap().findings.is_empty());
}

/// GIF trägt Kommentare in Erweiterungsblöcken.
#[test]
fn ein_echtes_gif_wird_bereinigt() {
    let Some(daten) = lade("bild_mit_metadaten.gif") else {
        return;
    };

    let vorher = inspect(&daten).unwrap();
    assert_eq!(vorher.format.as_deref(), Some("GIF"));
    let kommentar = vorher
        .findings
        .iter()
        .find(|f| f.kind == FindingKind::Comment)
        .expect("Kommentar nicht gefunden");
    assert!(
        kommentar
            .value
            .as_deref()
            .unwrap_or_default()
            .contains("XY-2000"),
        "{kommentar:?}"
    );

    let (sauber, ergebnis) = strip(&daten).unwrap();
    assert!(ergebnis.may_show_clean());
    assert!(!enthaelt(&sauber, b"XY-2000"));
    assert!(!enthaelt(&sauber, b"Anna Beispiel"));

    assert!(sauber.starts_with(b"GIF8"), "kein GIF mehr");
    assert_eq!(sauber.last(), Some(&0x3B), "Abschlussbyte fehlt");
    assert!(inspect(&sauber).unwrap().findings.is_empty());
}

/// **Der Fund dieser Runde.** Eine Datei mit Anhängsel ist länger als im Kopf
/// angegeben. Eine Erkennung, die auf Gleichheit prüft, übersieht genau das.
#[test]
fn ein_bmp_mit_anhaengsel_wird_erkannt_und_gekuerzt() {
    let Some(daten) = lade("bild_schlicht.bmp") else {
        return;
    };

    let vorher = inspect(&daten).unwrap();
    assert!(
        vorher.understood,
        "die Erkennung uebersieht die Datei, die sie finden soll"
    );
    assert_eq!(vorher.format.as_deref(), Some("BMP"));

    let fund = vorher
        .findings
        .iter()
        .find(|f| f.location == "BMP:Anhängsel")
        .expect("das Anhaengsel wurde nicht gefunden");
    assert!(
        fund.value.as_deref().unwrap_or_default().contains("26"),
        "{fund:?}"
    );

    let (sauber, ergebnis) = strip(&daten).unwrap();
    assert!(
        !enthaelt(&sauber, b"HEIMLICHE-NUTZLAST"),
        "Anhaengsel blieb"
    );
    assert!(sauber.len() < daten.len());
    assert!(sauber.starts_with(b"BM"));

    match ergebnis {
        StripResult::Complete { removed } => assert_eq!(removed.len(), 1),
        other => panic!("erwartete Complete, bekam {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// TIFF
//
// Der schwierigste Fall: Die Bilddaten hängen an Versätzen, die beim Entfernen
// eines Eintrags **alle** neu vergeben werden müssen. Ein Fehler dabei erzeugt
// eine Datei, die keinen Fehler meldet und trotzdem Müll anzeigt.
// ---------------------------------------------------------------------------

/// Zählt die Verzeichnisse einer TIFF-Datei, ohne unser eigenes Modul zu
/// benutzen — ein Prüfmittel, das vom Prüfling unabhängig ist.
fn tiff_verzeichnisse(daten: &[u8]) -> usize {
    let u16le = |o: usize| -> usize {
        usize::from(u16::from_le_bytes([
            *daten.get(o).unwrap_or(&0),
            *daten.get(o + 1).unwrap_or(&0),
        ]))
    };
    let u32le = |o: usize| -> usize {
        u32::from_le_bytes([
            *daten.get(o).unwrap_or(&0),
            *daten.get(o + 1).unwrap_or(&0),
            *daten.get(o + 2).unwrap_or(&0),
            *daten.get(o + 3).unwrap_or(&0),
        ]) as usize
    };

    let mut versatz = u32le(4);
    let mut n = 0usize;
    while versatz != 0 && versatz < daten.len() && n < 64 {
        n += 1;
        let anzahl = u16le(versatz);
        versatz = u32le(versatz + 2 + anzahl * 12);
    }
    n
}

#[test]
fn ein_echtes_tiff_wird_bereinigt_und_bleibt_lesbar() {
    let Some(daten) = lade("bild_mit_exif.tiff") else {
        eprintln!("uebersprungen: gen_metadata_fixtures.py wurde nicht ausgefuehrt");
        return;
    };

    let vorher = inspect(&daten).unwrap();
    assert_eq!(vorher.format.as_deref(), Some("TIFF"));
    assert!(
        vorher.findings.len() >= 8,
        "in der Vorlage stecken acht Marken: {:?}",
        vorher.findings
    );

    let (sauber, ergebnis) = strip(&daten).unwrap();
    assert!(ergebnis.may_show_clean(), "{ergebnis:?}");

    for spur in [
        &b"Dr. Anna Beispiel"[..],
        b"ARBEITSPLATZ-DANIW",
        b"XY-2000",
        b"Kanzlei Muster",
        b"nicht weitergeben",
    ] {
        assert!(!enthaelt(&sauber, spur), "Spur blieb: {spur:?}");
    }

    // Es muss ein TIFF bleiben, und zwar mit derselben Byte-Reihenfolge.
    assert_eq!(
        sauber.get(..2),
        daten.get(..2),
        "Byte-Reihenfolge gewechselt"
    );
    assert_eq!(tiff_verzeichnisse(&sauber), 1);
    assert!(inspect(&sauber).unwrap().findings.is_empty());
}

/// **Die zentrale Entscheidung, Seite eins.** Ein mehrseitiger Scan darf
/// keine Seite verlieren — sie sind Inhalt, keine Vorschaubilder.
#[test]
fn ein_mehrseitiger_scan_behaelt_alle_seiten() {
    let Some(daten) = lade("scan_mehrseitig.tiff") else {
        return;
    };

    assert_eq!(
        tiff_verzeichnisse(&daten),
        2,
        "Vorlage hat keine zwei Seiten"
    );

    let vorher = inspect(&daten).unwrap();
    assert!(
        vorher.format.as_deref().unwrap_or_default().contains('2'),
        "die Zahl der Verzeichnisse gehoert in die Meldung: {:?}",
        vorher.format
    );

    let (sauber, _) = strip(&daten).unwrap();
    assert_eq!(
        tiff_verzeichnisse(&sauber),
        2,
        "eine Seite des Scans ging verloren"
    );
    assert!(!enthaelt(&sauber, b"Dr. Anna Beispiel"));
}

/// **Die zentrale Entscheidung, Seite zwei.** Ein Verzeichnis, das sich als
/// verkleinerte Fassung ausweist, ist ein Vorschaubild — eine zweite Kopie
/// des Inhalts — und verschwindet.
#[test]
fn ein_vorschau_verzeichnis_verschwindet() {
    let Some(daten) = lade("bild_mit_vorschau.tiff") else {
        return;
    };

    assert_eq!(tiff_verzeichnisse(&daten), 2, "Vorlage ohne Vorschau");

    let vorher = inspect(&daten).unwrap();
    assert!(
        vorher
            .findings
            .iter()
            .any(|f| f.kind == FindingKind::EmbeddedPreview),
        "das Vorschau-Verzeichnis wurde nicht erkannt: {:?}",
        vorher.findings
    );

    let (sauber, _) = strip(&daten).unwrap();
    assert_eq!(
        tiff_verzeichnisse(&sauber),
        1,
        "das Vorschaubild blieb in der Datei"
    );
}

// ---------------------------------------------------------------------------
// HEIC und AVIF
//
// Hier wird nicht neu gebaut, sondern an Ort und Stelle ersetzt. Daraus folgt
// eine ungewöhnlich klare Prüfbedingung: Die Dateilänge muss danach auf das
// Byte genau dieselbe sein. Ändert sie sich, ist ein Versatz ungültig
// geworden — und genau das wollte dieses Vorgehen ausschließen.
// ---------------------------------------------------------------------------

#[test]
fn heic_und_avif_werden_geleert_ohne_die_laenge_zu_aendern() {
    for name in ["bild_mit_exif.avif", "bild_mit_exif.heic"] {
        let Some(daten) = lade(name) else {
            eprintln!("uebersprungen: {name} fehlt (pillow-heif nicht installiert?)");
            continue;
        };

        let vorher = inspect(&daten).unwrap();
        assert!(vorher.understood, "{name} wurde nicht erkannt");
        assert!(
            vorher
                .findings
                .iter()
                .any(|f| f.location.contains("Exif") && f.severity == Severity::Critical),
            "{name}: der Name im Exif fehlt: {:?}",
            vorher.findings
        );
        assert!(
            vorher.findings.iter().any(|f| f.location.contains("XMP")),
            "{name}: der XMP-Block fehlt"
        );

        let (sauber, _) = strip(&daten).unwrap();

        // **Die entscheidende Zusicherung.**
        assert_eq!(
            sauber.len(),
            daten.len(),
            "{name}: die Dateilaenge hat sich geaendert — ein Versatz ist jetzt falsch"
        );

        for spur in [
            &b"Dr. Anna Beispiel"[..],
            b"Kamerahersteller",
            b"XY-2000",
            b"Bearbeitungsprogramm",
            b"2026:03:01",
        ] {
            assert!(!enthaelt(&sauber, spur), "{name}: Spur blieb: {spur:?}");
        }

        // Kopf und Marke bleiben unangetastet.
        assert_eq!(
            sauber.get(..12),
            daten.get(..12),
            "{name}: der Kopf wurde veraendert"
        );

        let nachher = inspect(&sauber).unwrap();
        assert!(
            !nachher
                .findings
                .iter()
                .any(|f| f.severity == Severity::Critical),
            "{name}: es blieb etwas Kritisches: {:?}",
            nachher.findings
        );
    }
}

// ---------------------------------------------------------------------------
// SVG
//
// Der ungewöhnlichste Fall: SVG ist beliebiges XML und kann Programmcode,
// Verweise auf fremde Rechner und ganze Rasterbilder tragen. Zwei Dinge müssen
// gleichzeitig gelten — nichts Gefährliches bleibt, und die Darstellung geht
// nicht kaputt.
// ---------------------------------------------------------------------------

#[test]
fn ein_echtes_svg_wird_bereinigt_und_bleibt_darstellbar() {
    let Some(daten) = lade("zeichnung_mit_metadaten.svg") else {
        eprintln!("uebersprungen: gen_metadata_fixtures.py wurde nicht ausgefuehrt");
        return;
    };

    let vorher = inspect(&daten).unwrap();
    assert_eq!(vorher.format.as_deref(), Some("SVG"));
    assert!(
        vorher.findings.len() >= 15,
        "in der Vorlage steckt mehr: {:?}",
        vorher.findings
    );

    // Der schwerwiegendste Fund: das Zählpixel.
    let zaehlpixel = vorher
        .findings
        .iter()
        .find(|f| {
            f.value
                .as_deref()
                .unwrap_or_default()
                .contains("IP-Adresse")
        })
        .expect("der Verweis nach aussen wurde nicht als Zaehlpixel benannt");
    assert_eq!(zaehlpixel.severity, Severity::Critical);

    // Und die Rekursion ins eingebettete Foto.
    assert!(
        vorher
            .findings
            .iter()
            .any(|f| f.kind == FindingKind::Gps && f.location.contains("eingebettetes Bild")),
        "GPS im eingebetteten Foto nicht gefunden"
    );

    let (sauber, ergebnis) = strip(&daten).unwrap();
    assert!(
        !ergebnis.may_show_clean(),
        "fuer SVG darf keine Vollstaendigkeit behauptet werden"
    );

    let text = String::from_utf8(sauber.clone()).expect("kein UTF-8 mehr");

    for spur in [
        "Anna Beispiel",
        "Kanzlei",
        "daniw",
        "tracker.example",
        "fremd.example",
        "inkscape",
        "sodipodi",
        "alert",
        "Nicht an den Kunden",
    ] {
        assert!(!text.contains(spur), "„{spur}\" blieb im SVG");
    }

    // Die Darstellung muss unangetastet bleiben.
    for noetig in [
        "viewBox",
        "translate(10,10)",
        "#c81e1e",
        "stroke-width",
        "Sichtbarer Text",
        "font-family",
    ] {
        assert!(text.contains(noetig), "„{noetig}\" ging verloren");
    }

    // Das eingebettete Foto bleibt ein Foto — ohne sein EXIF.
    let anfang = text
        .find("base64,")
        .expect("das eingebettete Bild verschwand");
    let rest = text.get(anfang + 7..).unwrap_or_default();
    let ende = rest.find('"').unwrap_or(rest.len());
    let kodiert = rest.get(..ende).unwrap_or_default();
    assert!(!kodiert.is_empty(), "das Bild ist leer");

    let inner = inspect(&sauber).unwrap();
    assert!(
        !inner.findings.iter().any(|f| f.kind == FindingKind::Gps),
        "GPS blieb im eingebetteten Foto: {:?}",
        inner.findings
    );
}

/// Zweimal bereinigen muss zweimal dasselbe ergeben — für jedes Format.
#[test]
fn die_bereinigung_ist_bei_allen_formaten_wiederholbar() {
    for name in [
        "bild_mit_metadaten.webp",
        "bild_mit_metadaten.gif",
        "bild_schlicht.bmp",
        "bild_mit_exif.tiff",
        "scan_mehrseitig.tiff",
        "bild_mit_vorschau.tiff",
        "bild_mit_exif.avif",
        "bild_mit_exif.heic",
        "zeichnung_mit_metadaten.svg",
    ] {
        let Some(daten) = lade(name) else { continue };
        let einmal = strip(&daten).unwrap().0;
        let zweimal = strip(&einmal).unwrap().0;
        assert_eq!(einmal, zweimal, "{name} ist nicht stabil");
    }
}

// ---------------------------------------------------------------------------
// PDF
//
// Der folgenreichste Fall des ganzen Moduls: Eine „geschwärzte" Stelle steht
// vollständig lesbar in der Datei, und kein Leser zeigt sie an.
// ---------------------------------------------------------------------------

/// Zählt die Fassungen einer PDF-Datei, ohne unser eigenes Modul zu benutzen.
fn pdf_fassungen(daten: &[u8]) -> usize {
    daten.windows(5).filter(|f| *f == b"%%EOF").count()
}

/// **Der Fund, um den es geht.** Die Meldung muss nicht nur sagen, *dass* es
/// frühere Fassungen gibt, sondern *was* nur dort steht.
#[test]
fn eine_geschwaerzte_stelle_wird_wortwoertlich_benannt() {
    let Some(daten) = lade("dokument_mit_verlauf.pdf") else {
        eprintln!("uebersprungen: gen_metadata_fixtures.py wurde nicht ausgefuehrt");
        return;
    };

    assert_eq!(pdf_fassungen(&daten), 2, "Vorlage ohne Aenderungshistorie");

    let vorher = cabrik_metadata::inspect(&daten).unwrap();
    assert_eq!(vorher.format.as_deref(), Some("PDF (2 Fassungen)"));

    let historie = vorher
        .findings
        .iter()
        .find(|f| f.kind == FindingKind::TrackedChange)
        .expect("die Aenderungshistorie wurde nicht gemeldet");
    assert_eq!(historie.severity, Severity::Critical);
    assert!(
        historie
            .value
            .as_deref()
            .unwrap_or_default()
            .contains("38 Prozent"),
        "die Meldung nennt nicht, was versteckt ist: {historie:?}"
    );
}

/// Die Vorschau zeigt je Fassung genau das, was nur dort steht.
#[test]
fn die_vorschau_zeigt_was_entfernt_wurde() {
    let Some(daten) = lade("dokument_mit_verlauf.pdf") else {
        return;
    };
    let f = cabrik_metadata::pdf::fassungen(&daten, None).unwrap();
    assert_eq!(f.len(), 2);

    assert!(!f[0].ist_aktuell);
    assert!(f[1].ist_aktuell);
    assert!(
        f[0].nur_hier.iter().any(|z| z.contains("38 Prozent")),
        "die alte Zeile fehlt in der Vorschau: {:?}",
        f[0].nur_hier
    );
    assert!(
        f[1].nur_hier.is_empty(),
        "die angezeigte Fassung kann nichts Verstecktes enthalten"
    );
}

/// Die Voreinstellung flacht die **angezeigte** Fassung ein.
#[test]
fn die_voreinstellung_beseitigt_die_historie() {
    let Some(daten) = lade("dokument_mit_verlauf.pdf") else {
        return;
    };
    let (sauber, ergebnis) = cabrik_metadata::pdf::strip(&daten).unwrap();

    assert!(
        !ergebnis.may_show_clean(),
        "PDF darf nie Vollstaendigkeit behaupten"
    );
    assert_eq!(pdf_fassungen(&sauber), 1, "die Historie blieb");
    assert!(!enthaelt(&sauber, b"38 Prozent"), "die alte Fassung blieb");
    assert!(
        !enthaelt(&sauber, b"Dr. Anna Beispiel"),
        "die Dokumenteigenschaften blieben"
    );
}

/// Auf ausdrueckliche Wahl wird die **aeltere** Fassung eingeflacht — der
/// Fall, den ein Journalist braucht, um zu sehen, was geschwaerzt wurde.
#[test]
fn eine_gewaehlte_fassung_wird_eingeflacht() {
    use cabrik_metadata::pdf::Verlauf;
    let Some(daten) = lade("dokument_mit_verlauf.pdf") else {
        return;
    };

    let (sauber, _) = cabrik_metadata::pdf::strip_mit(&daten, Verlauf::Fassung(1), None).unwrap();
    assert_eq!(pdf_fassungen(&sauber), 1);
    assert!(
        !enthaelt(&sauber, b"Dr. Anna Beispiel"),
        "auch hier muessen die Eigenschaften weg"
    );

    // Diese Fassung zeigt die ungeschwaerzte Stelle — das ist gewollt.
    let i = cabrik_metadata::pdf::fassungen(&sauber, None).unwrap();
    assert_eq!(i.len(), 1);
    assert!(
        i[0].auszug.contains("38 Prozent"),
        "die gewaehlte Fassung wurde nicht eingeflacht: {}",
        i[0].auszug
    );
}

/// Eine unbekannte Fassungsnummer ist ein Fehler, kein stiller Rueckfall.
#[test]
fn eine_unbekannte_fassung_wird_abgelehnt() {
    use cabrik_metadata::pdf::Verlauf;
    let Some(daten) = lade("dokument_mit_verlauf.pdf") else {
        return;
    };
    assert!(cabrik_metadata::pdf::strip_mit(&daten, Verlauf::Fassung(99), None).is_err());
}

/// `Behalten` veraendert nichts — fuer Beweismittel und Archivierung.
#[test]
fn mit_behalten_bleibt_die_datei_unveraendert() {
    use cabrik_metadata::pdf::Verlauf;
    let Some(daten) = lade("dokument_mit_verlauf.pdf") else {
        return;
    };
    let (aus, ergebnis) = cabrik_metadata::pdf::strip_mit(&daten, Verlauf::Behalten, None).unwrap();

    assert_eq!(aus, daten, "es wurde doch etwas veraendert");
    assert!(!ergebnis.may_show_clean());
}

// ---------------------------------------------------------------------------
// MP4
// ---------------------------------------------------------------------------

/// Der Aufnahmeort ist bei einem Handyvideo der schwerwiegendste Fund — und
/// der einzige, der sich nicht aus dem Bild ablesen lässt.
#[test]
fn ein_echtes_mp4_verliert_ort_marken_und_zeiten() {
    let Some(daten) = lade("video_mit_ortsangabe.mp4") else {
        eprintln!("uebersprungen: gen_metadata_fixtures.py wurde nicht ausgefuehrt");
        return;
    };

    let vorher = inspect(&daten).unwrap();
    // Die Anzeige nennt die Marke, nicht die Formatfamilie: Ein MOV heißt
    // hier „QuickTime (MOV)", auch wenn beides derselbe Behälter ist.
    assert_eq!(vorher.format.as_deref(), Some("MP4"));
    assert!(
        vorher
            .findings
            .iter()
            .any(|f| f.kind == FindingKind::Gps && f.severity == Severity::Critical),
        "der Aufnahmeort wurde nicht gefunden"
    );

    let (sauber, ergebnis) = strip(&daten).unwrap();
    assert!(
        matches!(ergebnis, StripResult::Complete { .. }),
        "unerwartet: {ergebnis:?}"
    );

    for spur in [
        &b"+46.9481"[..],
        b"Dr. Anna Beispiel",
        b"Nicht an den Kunden geben",
        b"Angebot Nordstern",
        b"Bearbeitungsprogramm",
    ] {
        assert!(!enthaelt(&sauber, spur), "noch lesbar: {spur:?}");
    }
    assert!(inspect(&sauber).unwrap().findings.is_empty());
}

/// **Der Kern des Verfahrens.** Ein Video verweist über `stco` auf jeden
/// Datenblock in `mdat`. Verschiebt sich auch nur ein Byte, zeigen alle diese
/// Verweise ins Leere — die Datei öffnet sich und spielt nicht ab.
///
/// Deshalb wird nichts entfernt, sondern durch ein `free` gleicher Größe
/// ersetzt. Dieser Test hält fest, dass das eingehalten wird.
#[test]
fn im_mp4_verschiebt_sich_kein_einziges_byte() {
    let Some(daten) = lade("video_mit_ortsangabe.mp4") else {
        return;
    };
    let (sauber, _) = strip(&daten).unwrap();

    assert_eq!(sauber.len(), daten.len(), "die Länge hat sich geändert");

    // Die Bilddaten selbst bleiben Byte für Byte gleich.
    let mdat = daten
        .windows(4)
        .position(|f| f == b"mdat")
        .expect("keine mdat-Box in der Vorlage");
    assert_eq!(
        sauber.get(mdat..),
        daten.get(mdat..),
        "die Bilddaten wurden angetastet"
    );
}

/// Ein zweiter Durchlauf über eine bereits bereinigte Datei muss sie
/// unverändert lassen. Andernfalls würde jede Wiederholung weiter am
/// Boxbaum nagen.
#[test]
fn ein_zweiter_durchlauf_am_mp4_aendert_nichts() {
    let Some(daten) = lade("video_mit_ortsangabe.mp4") else {
        return;
    };
    let (einmal, _) = strip(&daten).unwrap();
    let (zweimal, ergebnis) = strip(&einmal).unwrap();

    assert_eq!(einmal, zweimal);
    assert!(matches!(ergebnis, StripResult::Complete { .. }));
}

// ---------------------------------------------------------------------------
// Matroska, WebM und AVI
//
// Diese drei Vorlagen erzeugt ffmpeg, nicht dieses Projekt. Das ist der
// Unterschied, auf den es ankommt: Eine selbstgebaute Datei prüft nur, ob der
// Leser zum eigenen Schreiber passt. Bei Matroska stand in der handgebauten
// Vorlage zunächst ein falsches Byte in der Kennung des `Info`-Elements —
// Leser und Datei waren sich einig, und beide lagen daneben.
// ---------------------------------------------------------------------------

/// Was ffmpeg in eine Matroska schreibt, muss vollständig gefunden werden.
#[test]
fn eine_echte_matroska_verliert_alle_marken() {
    let Some(daten) = lade("video_mit_marken.mkv") else {
        eprintln!("uebersprungen: gen_metadata_fixtures.py wurde nicht ausgefuehrt");
        return;
    };

    let vorher = inspect(&daten).unwrap();
    assert_eq!(vorher.format.as_deref(), Some("Matroska (MKV)"));
    assert!(
        vorher
            .findings
            .iter()
            .any(|f| f.kind == FindingKind::Author && f.severity == Severity::Critical),
        "der Verfasser wurde nicht gefunden: {:?}",
        vorher.findings
    );

    let (sauber, _) = strip(&daten).unwrap();
    assert_eq!(sauber.len(), daten.len(), "es hat sich etwas verschoben");

    for spur in [
        &b"Dr. Anna Beispiel"[..],
        b"Nicht an den Kunden geben",
        b"Angebot Nordstern",
        b"Interner Rohschnitt",
        b"Kameraspur A",
    ] {
        assert!(!enthaelt(&sauber, spur), "noch lesbar: {spur:?}");
    }
}

/// WebM ist dasselbe EBML mit anderem `DocType` — und muss deshalb ohne
/// jede Sonderbehandlung durchlaufen.
#[test]
fn webm_wird_wie_matroska_behandelt() {
    let Some(daten) = lade("video_mit_marken.webm") else {
        return;
    };
    let vorher = inspect(&daten).unwrap();
    assert_eq!(vorher.format.as_deref(), Some("WebM"));

    let (sauber, _) = strip(&daten).unwrap();
    assert_eq!(sauber.len(), daten.len());
    assert!(!enthaelt(&sauber, b"Dr. Anna Beispiel"));
}

/// Die Pflichtelemente bleiben stehen, sonst wäre die Datei formal fehlerhaft.
/// Ihr Inhalt ist danach leer — das genügt.
#[test]
fn matroska_behaelt_seine_pflichtelemente() {
    let Some(daten) = lade("video_mit_marken.mkv") else {
        return;
    };
    let (sauber, _) = strip(&daten).unwrap();

    // MuxingApp (0x4D80) und WritingApp (0x5741) müssen weiterhin vorkommen.
    for kennung in [&[0x4Du8, 0x80][..], &[0x57, 0x41]] {
        assert!(
            enthaelt(&sauber, kennung),
            "ein Pflichtelement wurde entfernt: {kennung:?}"
        );
    }
    assert!(!enthaelt(&sauber, b"Lavf"), "der Muxer ist noch lesbar");
}

/// AVI legt seine Angaben in eine `LIST INFO`, die zu `JUNK` wird.
#[test]
fn ein_echtes_avi_verliert_seine_info_liste() {
    let Some(daten) = lade("video_mit_marken.avi") else {
        return;
    };

    let vorher = inspect(&daten).unwrap();
    assert_eq!(vorher.format.as_deref(), Some("AVI"));
    assert!(
        vorher
            .findings
            .iter()
            .any(|f| f.location == "AVI:INFO/IART")
    );
    assert!(vorher.findings.iter().any(|f| f.location == "AVI:strn"));

    let (sauber, _) = strip(&daten).unwrap();
    assert_eq!(
        sauber.len(),
        daten.len(),
        "der idx1-Index wäre jetzt falsch"
    );

    for spur in [
        &b"Dr. Anna Beispiel"[..],
        b"Nicht an den Kunden geben",
        b"Angebot Nordstern",
        b"Kameraspur A",
    ] {
        assert!(!enthaelt(&sauber, spur), "noch lesbar: {spur:?}");
    }
    assert!(inspect(&sauber).unwrap().findings.is_empty());
}
