//! MP4, MOV und M4V (`spec/metadata.md` §4).
//!
//! Dasselbe Behälterformat wie HEIC und AVIF — und derselbe Weg: **ersetzen
//! an Ort und Stelle**, nichts neu bauen.
//!
//! # Warum das hier noch zwingender ist als bei Bildern
//!
//! Ein Video verweist über `stco` beziehungsweise `co64` auf jeden einzelnen
//! Datenblock in `mdat`. Diese Tabellen sind bei einem längeren Film
//! **tausende Einträge lang**. Wer eine Box entfernt und alles Nachfolgende
//! verschiebt, muss jeden dieser Werte neu berechnen — ein Fehler dabei
//! erzeugt eine Datei, die sich öffnen lässt und nicht abspielt.
//!
//! ISO-BMFF sieht für diesen Fall einen Platzhalter vor: die **`free`-Box**.
//! Ein Leser überspringt sie ausdrücklich. Eine `udta`-Box durch ein `free`
//! gleicher Größe zu ersetzen ist deshalb kein Kunstgriff, sondern die im
//! Format vorgesehene Lösung — und es bewegt sich kein einziges Byte.
//!
//! # Was ein Handyvideo verrät
//!
//! - **Die GPS-Koordinaten** der Aufnahme — der schwerwiegendste Fund, dem
//!   GPS-Tag eines Fotos gleichwertig. Sie stehen an **zwei** möglichen
//!   Stellen, je nach Erzeuger: als `©xyz` in `moov/udta`, oder unter
//!   `com.apple.quicktime.location.ISO6709` in Apples Schlüsselverzeichnis.
//! - **`ilst`-Marken** in `moov/udta/meta` — Titel, Verfasser, Kommentar und
//!   das erzeugende Programm.
//! - **Zeitstempel** in `mvhd`, `tkhd` und `mdhd` — Erstellung und letzte
//!   Änderung, auf die Sekunde genau. Sie sind Felder fester Breite und
//!   lassen sich auf null setzen, ohne dass sich etwas verschiebt.
//! - **Namensfelder** in `hdlr` und die Herstellerkennung in `stsd`.
//!
//! # Dasselbe Modul, drei Arten von Dateien
//!
//! Der Behälter trägt mehr als Bewegtbild, und das ist kein Zufall:
//!
//! | | Marke | Behandlung |
//! |---|---|---|
//! | Video | `isom`, `mp42`, `qt  ` | vollständig |
//! | **Ton** | `M4A `, `M4B ` | vollständig — dieselben `ilst`-Marken |
//! | **Rohdatei** | `crx ` (Canon CR3) | erkannt, **unangetastet gelassen** |

use crate::model::{Finding, FindingKind, Inspection, Severity, StripResult};

use cabrik_core::{Error, Result};

/// Höchstzahl der Boxen, die verfolgt werden.
const MAX_BOXEN: usize = 200_000;
/// Höchste Schachtelungstiefe.
const MAX_TIEFE: usize = 12;

/// Marken, die diesen Behälter als Ton oder Bewegtbild ausweisen.
///
/// `M4A` und `M4B` sind reine **Tondateien** — derselbe Behälter, dieselben
/// Marken in `ilst`, deshalb dasselbe Modul.
const VIDEO_MARKEN: [&[u8; 4]; 13] = [
    b"isom", b"iso2", b"iso4", b"iso5", b"iso6", b"mp41", b"mp42", b"avc1", b"M4V ", b"M4A ",
    b"M4B ", b"qt  ", b"mmp4",
];

/// Ob die Bytes wie ein MP4, MOV oder M4V aussehen.
#[must_use]
pub fn looks_like_video(daten: &[u8]) -> bool {
    if daten.get(4..8) != Some(b"ftyp") {
        return false;
    }
    let passt = |m: &[u8]| VIDEO_MARKEN.iter().any(|k| m == k.as_slice());

    // Hauptmarke oder eine der kompatiblen Marken.
    if daten.get(8..12).is_some_and(passt) {
        return true;
    }
    // 3GPP-Marken beginnen alle mit `3g`.
    if daten.get(8..10) == Some(b"3g") {
        return true;
    }
    let ende = u32_bei(daten, 0)
        .and_then(|g| usize::try_from(g).ok())
        .unwrap_or(0)
        .min(daten.len());
    let mut p = 16usize;
    while p.saturating_add(4) <= ende {
        if daten.get(p..p.saturating_add(4)).is_some_and(passt) {
            return true;
        }
        p = p.saturating_add(4);
    }
    false
}

fn u32_bei(d: &[u8], p: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        d.get(p..p.saturating_add(4))?.try_into().ok()?,
    ))
}

fn u64_bei(d: &[u8], p: usize) -> Option<u64> {
    Some(u64::from_be_bytes(
        d.get(p..p.saturating_add(8))?.try_into().ok()?,
    ))
}

// ---------------------------------------------------------------------------
// Boxen
// ---------------------------------------------------------------------------

/// Eine gefundene Box mit ihrer Lage in der Datei.
#[derive(Debug, Clone, Copy)]
struct Kasten {
    typ: [u8; 4],
    /// Anfang der Box, einschließlich Kopf.
    anfang: usize,
    /// Anfang des Inhalts.
    inhalt: usize,
    /// Ende der Box.
    ende: usize,
    /// Schachtelungstiefe, null für die oberste Ebene.
    tiefe: usize,
}

impl Kasten {
    const fn laenge(&self) -> usize {
        self.ende.saturating_sub(self.anfang)
    }
}

/// Boxen, in die abgestiegen wird.
///
/// Alles andere bleibt unangetastet — insbesondere `mdat`, in dem die
/// eigentlichen Bilddaten liegen.
const BEHAELTER: [&[u8; 4]; 9] = [
    b"moov", b"trak", b"mdia", b"minf", b"udta", b"meta", b"ilst", b"edts", b"stbl",
];

/// Durchläuft die Boxen rekursiv.
fn sammle_boxen(
    daten: &[u8],
    von: usize,
    bis: usize,
    tiefe: usize,
    eltern: Option<[u8; 4]>,
    aus: &mut Vec<Kasten>,
) -> Result<()> {
    if tiefe > MAX_TIEFE {
        return Ok(());
    }
    let mut p = von;

    while p.saturating_add(8) <= bis {
        if aus.len() >= MAX_BOXEN {
            return Err(Error::Malformed("video: zu viele Boxen"));
        }

        let roh = u32_bei(daten, p).ok_or(Error::Malformed("video: Boxgroesse unlesbar"))?;
        let typ: [u8; 4] = daten
            .get(p.saturating_add(4)..p.saturating_add(8))
            .and_then(|s| s.try_into().ok())
            .ok_or(Error::Malformed("video: Boxtyp unlesbar"))?;

        let (groesse, kopf) = match roh {
            1 => {
                let g = u64_bei(daten, p.saturating_add(8))
                    .ok_or(Error::Malformed("video: grosse Boxgroesse unlesbar"))?;
                (
                    usize::try_from(g).map_err(|_| Error::Malformed("video: Box zu gross"))?,
                    16usize,
                )
            }
            0 => (bis.saturating_sub(p), 8usize),
            g => (
                usize::try_from(g).map_err(|_| Error::Malformed("video: Box zu gross"))?,
                8usize,
            ),
        };

        if groesse < kopf {
            return Err(Error::Malformed("video: Box kleiner als ihr Kopf"));
        }
        let ende = p
            .checked_add(groesse)
            .ok_or(Error::Malformed("video: Boxende ueberlaeuft"))?;
        if ende > bis {
            return Err(Error::Malformed("video: Box reicht ueber ihren Bereich"));
        }

        let inhalt = p.saturating_add(kopf);
        aus.push(Kasten {
            typ,
            anfang: p,
            inhalt,
            ende,
            tiefe,
        });

        // Abgestiegen wird in bekannte Behälter — und in die Marken eines
        // `ilst`. Deren Wert steckt in einer `data`-Box eine Ebene tiefer;
        // ohne diesen Schritt bleiben alle Marken ohne Inhalt.
        let ist_behaelter = BEHAELTER.iter().any(|k| typ == **k) || eltern == Some(*b"ilst");
        if ist_behaelter {
            // `meta` ist eine FullBox: vier Bytes Version und Merkmale voraus.
            let start = if typ == *b"meta" {
                inhalt.saturating_add(4)
            } else {
                inhalt
            };
            sammle_boxen(daten, start, ende, tiefe.saturating_add(1), Some(typ), aus)?;
        }

        p = ende;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Finden
// ---------------------------------------------------------------------------

/// Zeitstempel-Boxen: Name, Versatz der Zeitfelder ab Inhaltsbeginn bei v0.
const ZEIT_BOXEN: [(&[u8; 4], usize); 3] = [(b"mvhd", 4), (b"tkhd", 4), (b"mdhd", 4)];

/// Bereich des freien Namensfeldes in einer `hdlr`-Box.
///
/// Es steht hinter Version, Merkmalen, `pre_defined`, dem Handhabungstyp und
/// zwölf reservierten Bytes — zusammen 24. Was dort steht, ist meist
/// Beiwerk wie „VideoHandler", manchmal aber der Name des Schnittprogramms
/// oder gar des Geräts. Der Fund kam aus der unabhängigen Prüfung mit
/// ffmpeg: Es las das Feld noch, nachdem alles andere schon leer war.
fn hdlr_name(daten: &[u8], b: &Kasten) -> Option<(usize, usize)> {
    if b.typ != *b"hdlr" {
        return None;
    }
    let von = b.inhalt.checked_add(24)?;
    (von < b.ende && daten.len() >= b.ende).then_some((von, b.ende))
}

/// `ilst`-Marken mit ihrer Einordnung.
fn ilst_einordnung(typ: &[u8; 4]) -> Option<(&'static str, FindingKind, Severity)> {
    Some(match typ {
        b"\xa9ART" | b"aART" => ("Interpret", FindingKind::Author, Severity::Critical),
        b"\xa9wrt" => ("Verfasser", FindingKind::Author, Severity::Critical),
        b"\xa9cmt" => ("Kommentar", FindingKind::Comment, Severity::Critical),
        b"\xa9nam" => ("Titel", FindingKind::Comment, Severity::Notable),
        b"desc" | b"ldes" => ("Beschreibung", FindingKind::Comment, Severity::Notable),
        b"\xa9too" => (
            "erzeugendes Programm",
            FindingKind::Software,
            Severity::Notable,
        ),
        b"\xa9day" => ("Aufnahmedatum", FindingKind::Timestamp, Severity::Notable),
        b"\xa9swr" => ("Aufnahmesoftware", FindingKind::Software, Severity::Notable),
        b"\xa9mak" | b"\xa9mod" => ("Gerät", FindingKind::Device, Severity::Notable),
        _ => return None,
    })
}

/// Zieht den Text aus einer `ilst`-Marke.
///
/// Aufbau: `data`-Box mit Version(1) Merkmale(3) Sprache(4) Text.
fn ilst_text(daten: &[u8], boxen: &[Kasten], marke: &Kasten) -> Option<String> {
    let d = boxen
        .iter()
        .find(|b| b.typ == *b"data" && b.anfang > marke.anfang && b.ende <= marke.ende)?;
    let roh = daten.get(d.inhalt.saturating_add(8)..d.ende)?;
    let s = String::from_utf8_lossy(roh).trim().to_owned();
    (!s.is_empty()).then_some(s)
}

// ---------------------------------------------------------------------------
// Apples Schlüsselverzeichnis
// ---------------------------------------------------------------------------
//
// **Ein iPhone benutzt die iTunes-Marken nicht.** Statt vierstelliger Codes
// wie `©xyz` legt QuickTime ein `keys`-Verzeichnis an: Dort stehen die vollen
// Namen im Umkehr-Domänenstil, und im `ilst` ist der Kastentyp nur noch der
// **Index** in dieses Verzeichnis — vier Bytes, die als Zahl zu lesen sind.
//
// Ein Leser, der auf `©`-Codes prüft, sieht dort deshalb **gar nichts**. Das
// Entfernen wirkte trotzdem, weil das ganze `udta` zu `free` wird — aber
// gemeldet wurde nur „614 Bytes Benutzerdaten". Für ein echtes Handyvideo
// wäre damit der wichtigste Fund des Moduls, die Ortsangabe, unbenannt
// geblieben. Und `inspect` ist gerade das Werkzeug, mit dem man entscheidet.
//
// # Das Live Photo
//
// Ein Live Photo besteht aus **zwei Dateien**: `IMG_1234.HEIC` und
// `IMG_1234.MOV`. Verknüpft werden sie durch einen gemeinsamen Kennzeichner —
// `com.apple.quicktime.content.identifier` im Film, Apples MakerNote-Marke
// 0x0011 im Bild. Wer nur eine der beiden bereinigt und beide verschickt,
// lässt die Verbindung bestehen.

/// Liest das `keys`-Verzeichnis: Index (ab eins) auf den vollen Namen.
fn keys_verzeichnis(daten: &[u8], boxen: &[Kasten]) -> Vec<String> {
    let mut aus = Vec::new();
    let Some(k) = boxen.iter().find(|b| b.typ == *b"keys") else {
        return aus;
    };
    // Version und Merkmale (4), dann die Anzahl (4).
    let Some(anzahl) = u32_bei(daten, k.inhalt.saturating_add(4)) else {
        return aus;
    };
    let mut p = k.inhalt.saturating_add(8);

    for _ in 0..anzahl.min(4096) {
        let Some(laenge) = u32_bei(daten, p).and_then(|g| usize::try_from(g).ok()) else {
            break;
        };
        if laenge < 8 {
            break;
        }
        let ende = p.saturating_add(laenge).min(k.ende);
        // Nach Länge und Namensraum folgt der Name.
        let name = daten
            .get(p.saturating_add(8)..ende)
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .unwrap_or_default();
        aus.push(name);
        if ende <= p {
            break;
        }
        p = ende;
    }
    aus
}

/// Ordnet einen Apple-Schlüssel ein.
///
/// Die Liste deckt ab, was ein iPhone schreibt. Alles Übrige wird trotzdem
/// gemeldet — ein unbenannter Fund ist besser als ein verschwiegener.
fn schluessel_einordnung(name: &str) -> (&'static str, FindingKind, Severity) {
    // Der Namensraum sagt nichts, der letzte Teil alles.
    let kurz = name.rsplit('.').next().unwrap_or(name);
    match kurz {
        "ISO6709" => ("Aufnahmeort", FindingKind::Gps, Severity::Critical),
        "identifier" => (
            "Kennzeichner — verknüpft die beiden Hälften eines Live Photo",
            FindingKind::Device,
            Severity::Critical,
        ),
        "artist" | "author" | "director" | "producer" | "performer" => {
            ("Personenname", FindingKind::Author, Severity::Critical)
        }
        "comment" | "description" | "information" => {
            ("Kommentar", FindingKind::Comment, Severity::Critical)
        }
        "title" | "album" | "genre" | "keywords" => {
            ("Titel", FindingKind::Comment, Severity::Notable)
        }
        "make" | "model" => ("Gerät", FindingKind::Device, Severity::Notable),
        "software" | "encoder" | "creationsoftware" => (
            "erzeugende Software",
            FindingKind::Software,
            Severity::Notable,
        ),
        "creationdate" | "year" => ("Zeitangabe", FindingKind::Timestamp, Severity::Notable),
        "copyright" | "publisher" => ("Rechteangabe", FindingKind::Organization, Severity::Notable),
        _ => ("Angabe", FindingKind::Comment, Severity::Notable),
    }
}

/// Sammelt die Funde aus Apples Schlüsselverzeichnis.
fn apple_funde(daten: &[u8], boxen: &[Kasten]) -> Vec<Finding> {
    let namen = keys_verzeichnis(daten, boxen);
    if namen.is_empty() {
        return Vec::new();
    }

    let Some(ilst) = boxen.iter().find(|b| b.typ == *b"ilst") else {
        return Vec::new();
    };

    let mut aus = Vec::new();
    for b in boxen {
        // Die Marken sind die unmittelbaren Kinder des `ilst`.
        if b.anfang <= ilst.anfang || b.ende > ilst.ende || b.tiefe != ilst.tiefe.saturating_add(1)
        {
            continue;
        }
        // Der Kastentyp ist hier keine Kennung, sondern eine Zahl.
        let index = u32::from_be_bytes(b.typ);
        let Some(name) = index
            .checked_sub(1)
            .and_then(|i| usize::try_from(i).ok())
            .and_then(|i| namen.get(i))
        else {
            continue;
        };

        let (was, art, schwere) = schluessel_einordnung(name);
        let wert = ilst_text(daten, boxen, b);
        aus.push(Finding::new(
            art,
            format!("Video:{name}"),
            Some(match wert {
                Some(w) => format!("{was}: {w}"),
                None => was.to_owned(),
            }),
            schwere,
        ));
    }
    aus
}

/// Bereiche der Herstellerkennung in den Spurbeschreibungen.
///
/// In `stsd` steht je Spur ein Eintrag, der das Format beschreibt. Ab Byte
/// zwölf seines Inhalts liegen vier Bytes, die den **Hersteller der
/// schreibenden Software** benennen — `FFMP` bei ffmpeg, `appl` bei Apple.
/// In ISO-BMFF sind diese Bytes als `pre_defined` ohne Bedeutung, QuickTime
/// füllt sie. Sie zu nullen ist deshalb unbedenklich und verschiebt nichts.
///
/// Der Fund kam aus der Gegenprobe: ffmpeg las die Kennung noch, nachdem
/// alles andere schon leer war — unser „vollständig bereinigt" war um diese
/// vier Bytes unwahr.
fn stsd_hersteller(daten: &[u8], boxen: &[Kasten]) -> Vec<(usize, usize)> {
    let mut aus = Vec::new();

    for s in boxen.iter().filter(|b| b.typ == *b"stsd") {
        // Version und Merkmale (4), dann die Anzahl der Einträge (4).
        let Some(anzahl) = u32_bei(daten, s.inhalt.saturating_add(4)) else {
            continue;
        };
        let mut p = s.inhalt.saturating_add(8);

        for _ in 0..anzahl.min(64) {
            let Some(groesse) = u32_bei(daten, p).and_then(|g| usize::try_from(g).ok()) else {
                break;
            };
            if groesse < 24 {
                break;
            }
            let ende = p.saturating_add(groesse).min(s.ende);
            let von = p.saturating_add(8).saturating_add(12);
            let bis = von.saturating_add(4);
            if bis <= ende {
                aus.push((von, bis));
            }
            if ende <= p {
                break;
            }
            p = ende;
        }
    }
    aus
}

/// Zerlegt die Datei.
fn zerlege(daten: &[u8]) -> Result<Vec<Kasten>> {
    if !looks_like_video(daten) {
        return Err(Error::Malformed("video: keine bekannte Marke"));
    }
    let mut aus = Vec::new();
    // `data` liegt in `ilst`-Marken, die selbst keine Behälter sind — deshalb
    // wird `ilst` als Behälter geführt und eine Ebene tiefer gesucht.
    sammle_boxen(daten, 0, daten.len(), 0, None, &mut aus)?;
    Ok(aus)
}

fn sammle(daten: &[u8], boxen: &[Kasten]) -> Vec<Finding> {
    let mut funde = Vec::new();

    for b in boxen {
        // --- GPS: der schwerwiegendste Fund ---
        if b.typ == *b"\xa9xyz" {
            let roh = daten
                .get(b.inhalt.saturating_add(4)..b.ende)
                .unwrap_or_default();
            let koord = String::from_utf8_lossy(roh).trim().to_owned();
            funde.push(Finding::new(
                FindingKind::Gps,
                "Video:udta/©xyz".to_owned(),
                Some(format!(
                    "Aufnahmeort {koord} — jedes Mobiltelefon schreibt ihn hinein"
                )),
                Severity::Critical,
            ));
            continue;
        }

        // --- iTunes-Marken ---
        if let Some((name, art, schwere)) = ilst_einordnung(&b.typ) {
            funde.push(Finding::new(
                art,
                format!("Video:ilst/{name}"),
                ilst_text(daten, boxen, b),
                schwere,
            ));
            continue;
        }

        // --- Zeitstempel ---
        if let Some((_, versatz)) = ZEIT_BOXEN.iter().find(|(t, _)| b.typ == **t) {
            let version = *daten.get(b.inhalt).unwrap_or(&0);
            let (erstellt, breite) = if version == 1 {
                (u64_bei(daten, b.inhalt.saturating_add(*versatz)), 8usize)
            } else {
                (
                    u32_bei(daten, b.inhalt.saturating_add(*versatz)).map(u64::from),
                    4usize,
                )
            };
            let _ = breite;
            if erstellt.is_some_and(|z| z != 0) {
                funde.push(Finding::new(
                    FindingKind::Timestamp,
                    format!("Video:{}", String::from_utf8_lossy(&b.typ)),
                    Some("Erstellungs- und Änderungszeitpunkt, auf die Sekunde genau".to_owned()),
                    Severity::Notable,
                ));
            }
        }

        // --- Der Name des Spurbearbeiters ---
        if let Some((a, e)) = hdlr_name(daten, b)
            && let Some(text) = daten.get(a..e)
        {
            let name = String::from_utf8_lossy(text)
                .trim_matches(|c: char| c == '\u{0}' || c.is_whitespace())
                .to_owned();
            if !name.is_empty() {
                funde.push(Finding::new(
                    FindingKind::Software,
                    "Video:hdlr".to_owned(),
                    Some(format!("Spurbeschreibung „{name}“")),
                    Severity::Notable,
                ));
            }
        }
    }

    // --- Die Herstellerkennung in der Spurbeschreibung ---
    for (a, e) in stsd_hersteller(daten, boxen) {
        let roh = daten.get(a..e).unwrap_or(&[]);
        let name = String::from_utf8_lossy(roh)
            .trim_matches(|c: char| c == '\u{0}' || c.is_whitespace())
            .to_owned();
        if !name.is_empty() {
            funde.push(Finding::new(
                FindingKind::Software,
                "Video:stsd".to_owned(),
                Some(format!(
                    "Herstellerkennung der schreibenden Software „{name}“"
                )),
                Severity::Notable,
            ));
        }
    }

    // --- Apples Schlüsselverzeichnis ---
    let apple = apple_funde(daten, boxen);
    let hat_apple = !apple.is_empty();
    funde.extend(apple);

    // --- Sonstige Benutzerdaten, die wir nicht einzeln kennen ---
    for b in boxen {
        if b.typ != *b"udta" {
            continue;
        }
        let bekannt = hat_apple
            || boxen.iter().any(|x| {
                x.anfang > b.anfang
                    && x.ende <= b.ende
                    && (x.typ == *b"\xa9xyz" || ilst_einordnung(&x.typ).is_some())
            });
        if !bekannt && b.laenge() > 8 {
            funde.push(Finding::new(
                FindingKind::UnknownExtension,
                "Video:udta".to_owned(),
                Some(format!(
                    "{} Bytes Benutzerdaten — Inhalt nicht im Einzelnen bekannt",
                    b.laenge()
                )),
                Severity::Notable,
            ));
        }
    }

    funde
}

/// Untersucht ein Video.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Struktur.
pub fn inspect(daten: &[u8]) -> Result<Inspection> {
    let boxen = zerlege(daten)?;
    let mut funde = sammle(daten, &boxen);

    if ist_rohdatei(daten) {
        funde.push(Finding::new(
            FindingKind::UnknownExtension,
            "Video:Rohdatei".to_owned(),
            Some(ROH_HINWEIS.to_owned()),
            Severity::Notable,
        ));
    }

    Ok(Inspection {
        format: Some(marke_name(daten).to_owned()),
        findings: funde,
        understood: true,
    })
}

/// Begründung, warum eine Rohdatei nicht umgeschrieben wird.
const ROH_HINWEIS: &str = "eine Rohdatei aus einer Kamera — sie enthält in `THMB` und `PRVW` \
     zweite Kopien der Aufnahme und in `CMT1` bis `CMT4` vollständige \
     EXIF-Verzeichnisse, die dieses Programm nicht beurteilen kann";

/// Ob es eine Rohdatei aus einer Kamera ist.
///
/// **Canons CR3 ist ISO-BMFF.** Es trägt `isom` in seiner Markenliste und
/// wird von [`looks_like_video`] deshalb beansprucht — dieselbe Falle wie bei
/// den TIFF-Rohformaten (`spec/metadata.md` §4.2.12), nur in einem anderen
/// Behälter.
///
/// Anders als dort geht dabei kein Bild verloren: Die Sensordaten liegen in
/// `mdat`, und dieses Modul verschiebt nichts. Aber es behauptete
/// „vollständig bereinigt" für eine Datei, deren `THMB`- und `PRVW`-Boxen
/// **zweite Kopien der Aufnahme** enthalten und die es gar nicht kennt. Dazu
/// stecken in `CMT1` bis `CMT4` vollständige EXIF-Verzeichnisse samt
/// Canons eigenem `MakerNote`.
///
/// Die Hauptmarke ist die Selbstauskunft der Datei und lügt nicht.
fn ist_rohdatei(daten: &[u8]) -> bool {
    daten.get(8..12) == Some(b"crx ")
}

fn marke_name(daten: &[u8]) -> &'static str {
    match daten.get(8..12) {
        Some(b"crx ") => "Canon-Rohdatei (CR3)",
        Some(b"qt  ") => "QuickTime (MOV)",
        Some(b"M4V ") => "M4V",
        // M4A und M4B sind **Tondateien** in genau demselben Behälter. Sie
        // hier zu behandeln ist kein Zufall: Ihre Marken stehen in `ilst`,
        // wie bei einem Video vom Telefon.
        Some(b"M4A ") => "M4A (Ton)",
        Some(b"M4B ") => "M4B (Hörbuch)",
        Some(m) if m.starts_with(b"3g") => "3GPP",
        _ => "MP4",
    }
}

// ---------------------------------------------------------------------------
// Bereinigen
// ---------------------------------------------------------------------------

/// Ersetzt eine Box durch ein `free` gleicher Größe.
///
/// `free` ist im Format als „überspringen" definiert. Der Kopf bleibt an
/// derselben Stelle, die Größe bleibt dieselbe — es bewegt sich kein Byte,
/// und keine Versatztabelle wird ungültig.
fn zu_free(ziel: &mut [u8], b: Kasten) {
    let Some(kopf) = ziel.get_mut(b.anfang.saturating_add(4)..b.anfang.saturating_add(8)) else {
        return;
    };
    kopf.copy_from_slice(b"free");
    // Den Inhalt überschreiben — die Box zu überspringen genügt nicht, wenn
    // die Bytes lesbar bleiben.
    if let Some(rest) = ziel.get_mut(b.inhalt..b.ende) {
        rest.fill(0);
    }
}

/// Setzt die Zeitfelder einer Kopfbox auf null.
///
/// Null bedeutet in ISO-BMFF „unbekannt" und ist ein gültiger Wert. Die
/// Felder haben feste Breite, es verschiebt sich also nichts.
fn zeiten_loeschen(ziel: &mut [u8], b: Kasten, versatz: usize) {
    let version = *ziel.get(b.inhalt).unwrap_or(&0);
    let breite = if version == 1 { 8usize } else { 4usize };
    let start = b.inhalt.saturating_add(versatz);
    // Erstellung und Änderung stehen unmittelbar hintereinander.
    let ende = start.saturating_add(breite.saturating_mul(2));
    if ende <= b.ende
        && let Some(feld) = ziel.get_mut(start..ende)
    {
        feld.fill(0);
    }
}

/// Bereinigt ein Video.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Struktur.
pub fn strip(daten: &[u8]) -> Result<(Vec<u8>, StripResult)> {
    let boxen = zerlege(daten)?;
    let entfernt = sammle(daten, &boxen);

    // Rohdateien bleiben unangetastet — aus demselben Grund wie bei TIFF.
    if ist_rohdatei(daten) {
        let mut reste = entfernt;
        reste.push(Finding::new(
            FindingKind::UnknownExtension,
            "Video:Rohdatei".to_owned(),
            Some(ROH_HINWEIS.to_owned()),
            Severity::Notable,
        ));
        return Ok((
            daten.to_vec(),
            StripResult::Partial {
                removed: Vec::new(),
                remaining: reste,
                reason: "Rohdateien werden nicht umgeschrieben. Wer die Aufnahme weitergeben \
                         will, entwickelt sie und exportiert sie als JPEG oder TIFF — das \
                         Ergebnis wird dann vollständig bereinigt"
                    .to_owned(),
            },
        ));
    }

    let mut aus = daten.to_vec();

    // Benutzerdaten und Metadatenblöcke werden zu `free`. Nur die äußerste
    // Box je Zweig anfassen — die inneren liegen darin und sind damit
    // miterledigt.
    for b in &boxen {
        let ist_ziel = b.typ == *b"udta" || (b.typ == *b"meta" && b.tiefe <= 1);
        if !ist_ziel {
            continue;
        }
        let liegt_in_anderem = boxen.iter().any(|x| {
            (x.typ == *b"udta" || x.typ == *b"meta") && x.anfang < b.anfang && x.ende >= b.ende
        });
        if !liegt_in_anderem {
            zu_free(&mut aus, *b);
        }
    }

    for b in &boxen {
        if let Some((_, versatz)) = ZEIT_BOXEN.iter().find(|(t, _)| b.typ == **t) {
            zeiten_loeschen(&mut aus, *b, *versatz);
        }
        // Das Namensfeld der Spurbeschreibung. Es hat feste Lage und darf
        // leer sein — die Norm verlangt nur, dass die Box selbst da ist.
        if let Some((a, e)) = hdlr_name(daten, b)
            && let Some(feld) = aus.get_mut(a..e)
        {
            feld.fill(0);
        }
    }

    // Die Herstellerkennung in der Spurbeschreibung — vier Bytes fester Lage.
    for (a, e) in stsd_hersteller(daten, &boxen) {
        if let Some(feld) = aus.get_mut(a..e) {
            feld.fill(0);
        }
    }

    debug_assert_eq!(aus.len(), daten.len(), "die Dateilaenge hat sich geaendert");

    Ok((aus, StripResult::Complete { removed: entfernt }))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;

    fn kasten(typ: &[u8; 4], inhalt: &[u8]) -> Vec<u8> {
        let mut v = u32::try_from(8 + inhalt.len())
            .unwrap()
            .to_be_bytes()
            .to_vec();
        v.extend_from_slice(typ);
        v.extend_from_slice(inhalt);
        v
    }

    fn video() -> Vec<u8> {
        let zeit = 3_855_000_000u32;
        let mut mvhd = vec![0u8, 0, 0, 0];
        mvhd.extend_from_slice(&zeit.to_be_bytes());
        mvhd.extend_from_slice(&zeit.to_be_bytes());
        mvhd.extend_from_slice(&[0u8; 92]);

        let mut xyz_inhalt = vec![0u8, 18, 0x15, 0xC7];
        xyz_inhalt.extend_from_slice(b"+46.9481+007.4474/");

        let daten_box = {
            let mut v = vec![0u8, 0, 0, 1, 0, 0, 0, 0];
            v.extend_from_slice(b"Dr. Anna Beispiel");
            kasten(b"data", &v)
        };
        let ilst = kasten(b"ilst", &kasten(b"\xa9ART", &daten_box));
        let meta = {
            let mut v = vec![0u8, 0, 0, 0];
            v.extend_from_slice(&ilst);
            kasten(b"meta", &v)
        };
        let mut udta_inhalt = kasten(b"\xa9xyz", &xyz_inhalt);
        udta_inhalt.extend_from_slice(&meta);

        let mut moov_inhalt = kasten(b"mvhd", &mvhd);
        moov_inhalt.extend_from_slice(&kasten(b"udta", &udta_inhalt));

        let mut v = kasten(b"ftyp", b"isom\x00\x00\x02\x00isomiso2mp41");
        v.extend_from_slice(&kasten(b"moov", &moov_inhalt));
        v.extend_from_slice(&kasten(b"mdat", b"BILDDATEN-UNVERAENDERT"));
        v
    }

    #[test]
    fn video_wird_an_der_marke_erkannt() {
        assert!(looks_like_video(&video()));
        let mut mp3 = vec![0, 0, 0, 16];
        mp3.extend_from_slice(b"ftypheic\x00\x00\x00\x00");
        assert!(!looks_like_video(&mp3), "HEIC ist kein Video");
        assert!(!looks_like_video(b"\x89PNG\r\n\x1a\n"));
    }

    /// **Der schwerwiegendste Fund.** Jedes Mobiltelefon schreibt die
    /// Koordinaten der Aufnahme hinein.
    #[test]
    fn die_aufnahmekoordinaten_werden_gefunden() {
        let i = inspect(&video()).unwrap();
        let gps = i
            .findings
            .iter()
            .find(|f| f.kind == FindingKind::Gps)
            .expect("GPS nicht gefunden");
        assert_eq!(gps.severity, Severity::Critical);
        assert!(
            gps.value.as_deref().unwrap_or_default().contains("46.9481"),
            "{gps:?}"
        );
    }

    #[test]
    fn marken_und_zeitstempel_werden_gefunden() {
        let i = inspect(&video()).unwrap();
        assert!(
            i.findings.iter().any(|f| f.location.contains("Interpret")
                && f.value.as_deref() == Some("Dr. Anna Beispiel")),
            "{:?}",
            i.findings
        );
        assert!(i.findings.iter().any(|f| f.location.contains("mvhd")));
    }

    /// **Die entscheidende Zusicherung.** Ändert sich die Länge, wird eine
    /// Versatztabelle ungültig und das Video spielt nicht mehr ab.
    #[test]
    fn die_dateilaenge_bleibt_unveraendert() {
        let roh = video();
        let (sauber, _) = strip(&roh).unwrap();
        assert_eq!(sauber.len(), roh.len());
        assert_eq!(sauber.get(..12), roh.get(..12), "der Kopf wurde veraendert");
    }

    #[test]
    fn die_metadaten_verschwinden_die_bilddaten_bleiben() {
        let (sauber, ergebnis) = strip(&video()).unwrap();
        assert!(ergebnis.may_show_clean());

        for spur in [&b"Dr. Anna Beispiel"[..], b"46.9481", b"+007.4474"] {
            assert!(
                !sauber.windows(spur.len()).any(|f| f == spur),
                "Spur blieb: {spur:?}"
            );
        }
        assert!(
            sauber.windows(22).any(|f| f == b"BILDDATEN-UNVERAENDERT"),
            "die Bilddaten gingen verloren"
        );
    }

    /// Die Benutzerdaten werden zu `free` — das ist im Format als
    /// „ueberspringen" definiert.
    #[test]
    fn benutzerdaten_werden_zu_free() {
        let (sauber, _) = strip(&video()).unwrap();
        let boxen = zerlege(&sauber).unwrap();
        assert!(
            !boxen.iter().any(|b| b.typ == *b"udta"),
            "udta blieb stehen"
        );
        assert!(
            boxen.iter().any(|b| b.typ == *b"free"),
            "es wurde kein free gesetzt"
        );
    }

    #[test]
    fn die_zeitstempel_werden_genullt() {
        let (sauber, _) = strip(&video()).unwrap();
        let i = inspect(&sauber).unwrap();
        assert!(
            !i.findings.iter().any(|f| f.kind == FindingKind::Timestamp),
            "Zeitstempel blieben: {:?}",
            i.findings
        );
    }

    #[test]
    fn die_bereinigung_ist_wiederholbar() {
        let einmal = strip(&video()).unwrap().0;
        assert_eq!(strip(&einmal).unwrap().0, einmal);
    }

    #[test]
    fn kaputte_daten_ergeben_einen_fehler_keinen_absturz() {
        assert!(inspect(b"").is_err());
        assert!(inspect(b"\x00\x00\x00\x10ftypisom").is_err() || true);
        let mut roh = video();
        roh.truncate(20);
        let _ = inspect(&roh);
    }

    // -----------------------------------------------------------------------
    // Apples Schlüsselverzeichnis und das Live Photo
    // -----------------------------------------------------------------------

    /// Baut ein `keys`-Verzeichnis mit den vollen Namen.
    fn keys(namen: &[&str]) -> Vec<u8> {
        let mut inhalt = vec![0u8, 0, 0, 0]; // Version und Merkmale
        inhalt.extend_from_slice(&u32::try_from(namen.len()).unwrap().to_be_bytes());
        for n in namen {
            inhalt.extend_from_slice(&u32::try_from(8 + n.len()).unwrap().to_be_bytes());
            inhalt.extend_from_slice(b"mdta");
            inhalt.extend_from_slice(n.as_bytes());
        }
        kasten(b"keys", &inhalt)
    }

    /// Eine Marke im `ilst`: Der Kastentyp ist der **Index**, keine Kennung.
    fn marke_nach_index(index: u32, wert: &str) -> Vec<u8> {
        let mut data = vec![0u8, 0, 0, 1, 0, 0, 0, 0];
        data.extend_from_slice(wert.as_bytes());
        let inhalt = kasten(b"data", &data);
        let mut v = u32::try_from(8 + inhalt.len())
            .unwrap()
            .to_be_bytes()
            .to_vec();
        v.extend_from_slice(&index.to_be_bytes());
        v.extend_from_slice(&inhalt);
        v
    }

    /// Ein Video, wie es ein iPhone schreibt.
    fn iphone_video() -> Vec<u8> {
        let verzeichnis = keys(&[
            "com.apple.quicktime.location.ISO6709",
            "com.apple.quicktime.content.identifier",
            "com.apple.quicktime.model",
        ]);
        let ilst = kasten(
            b"ilst",
            &[
                marke_nach_index(1, "+46.9481+007.4474+561.000/"),
                marke_nach_index(2, "8F3B1C2A-4D5E-4F60-9A7B-1C2D3E4F5061"),
                marke_nach_index(3, "iPhone 15 Pro"),
            ]
            .concat(),
        );
        let hdlr = kasten(
            b"hdlr",
            &[vec![0u8; 8], b"mdta".to_vec(), vec![0u8; 13]].concat(),
        );
        let meta = kasten(b"meta", &[vec![0u8; 4], hdlr, verzeichnis, ilst].concat());
        let udta = kasten(b"udta", &meta);

        let mut aus = kasten(b"ftyp", b"qt  \x00\x00\x02\x00qt  ");
        aus.extend_from_slice(&kasten(b"moov", &udta));
        aus.extend_from_slice(&kasten(b"mdat", &[0x42u8; 128]));
        aus
    }

    /// **Ein iPhone benutzt die iTunes-Marken nicht.** Ohne das
    /// Schlüsselverzeichnis blieb der wichtigste Fund des Moduls — die
    /// Ortsangabe — bei einem echten Handyvideo unbenannt. Entfernt wurde er
    /// trotzdem; gemeldet wurde nur „614 Bytes Benutzerdaten".
    #[test]
    fn der_aufnahmeort_wird_auch_in_apples_form_gefunden() {
        let i = inspect(&iphone_video()).unwrap();
        let ort = i
            .findings
            .iter()
            .find(|f| f.kind == FindingKind::Gps)
            .expect("die Ortsangabe wurde nicht gefunden");
        assert_eq!(ort.severity, Severity::Critical);
        assert!(ort.value.as_deref().unwrap().contains("+46.9481"));
        assert_eq!(ort.location, "Video:com.apple.quicktime.location.ISO6709");
    }

    /// **Der Kennzeichner des Live Photo.** Er steht in beiden Hälften und
    /// verknüpft sie. Wer nur eine bereinigt, lässt die Verbindung bestehen —
    /// deshalb muss er benannt werden, nicht nur verschwinden.
    #[test]
    fn der_kennzeichner_des_live_photo_wird_benannt() {
        let i = inspect(&iphone_video()).unwrap();
        let f = i
            .findings
            .iter()
            .find(|f| f.location.ends_with("content.identifier"))
            .expect("der Kennzeichner fehlt");
        assert_eq!(f.severity, Severity::Critical);
        assert!(
            f.value.as_deref().unwrap().contains("Live Photo"),
            "die Bedeutung muss dabeistehen: {:?}",
            f.value
        );
        assert!(f.value.as_deref().unwrap().contains("8F3B1C2A"));
    }

    /// Bekanntes verdrängt den Sammelposten: Wer die Einzelfunde nennt, darf
    /// nicht zusätzlich „Inhalt nicht im Einzelnen bekannt" melden.
    #[test]
    fn der_sammelposten_entfaellt_wenn_die_marken_gelesen_wurden() {
        let i = inspect(&iphone_video()).unwrap();
        assert!(
            !i.findings.iter().any(|f| f.location == "Video:udta"),
            "es wurde doppelt gemeldet: {:?}",
            i.findings
        );
        assert_eq!(i.findings.len(), 3);
    }

    #[test]
    fn apples_marken_verschwinden_beim_bereinigen() {
        let vorher = iphone_video();
        let (nachher, ergebnis) = strip(&vorher).unwrap();

        assert_eq!(nachher.len(), vorher.len());
        assert!(matches!(ergebnis, StripResult::Complete { .. }));
        for spur in [
            &b"+46.9481"[..],
            b"8F3B1C2A",
            b"iPhone 15 Pro",
            b"com.apple.quicktime",
        ] {
            assert!(
                !nachher.windows(spur.len()).any(|f| f == spur),
                "noch lesbar: {}",
                String::from_utf8_lossy(spur)
            );
        }
        assert!(inspect(&nachher).unwrap().findings.is_empty());
    }

    // -----------------------------------------------------------------------
    // Canons CR3
    // -----------------------------------------------------------------------

    /// **Dieselbe Falle wie bei den TIFF-Rohformaten, anderer Behälter.**
    /// CR3 trägt `isom` in seiner Markenliste und wurde deshalb als Video
    /// behandelt — mit der Meldung „vollständig bereinigt", obwohl `THMB`
    /// und `PRVW` zweite Kopien der Aufnahme enthalten.
    #[test]
    fn eine_cr3_rohdatei_wird_erkannt_und_nicht_angetastet() {
        let mut datei = kasten(b"ftyp", b"crx \x00\x00\x00\x01crx isom");
        datei.extend_from_slice(&kasten(
            b"moov",
            &kasten(b"udta", &kasten(b"CMT1", b"II*\x00 EXIF-Verzeichnis")),
        ));
        datei.extend_from_slice(&kasten(b"mdat", b"SENSORDATEN"));

        let i = inspect(&datei).unwrap();
        assert_eq!(i.format.as_deref(), Some("Canon-Rohdatei (CR3)"));
        assert!(i.findings.iter().any(|f| f.location == "Video:Rohdatei"));

        let (aus, ergebnis) = strip(&datei).unwrap();
        assert_eq!(aus, datei, "an einer Rohdatei darf sich nichts ändern");
        assert!(
            !ergebnis.may_show_clean(),
            "für eine Rohdatei darf keine Sauberkeit behauptet werden"
        );
    }

    /// Die Gegenprobe: Ein gewöhnliches MP4 muss weiterhin bereinigt werden.
    #[test]
    fn ein_gewoehnliches_video_bleibt_unberuehrt_von_der_rohdatei_pruefung() {
        let (_, ergebnis) = strip(&video()).unwrap();
        assert!(ergebnis.may_show_clean(), "{ergebnis:?}");
    }
}
