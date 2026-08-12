//! MP3 mit ID3v2, ID3v1 und APEv2 (`spec/metadata.md` §4).
//!
//! # Warum hier zum ersten Mal wirklich entfernt wird
//!
//! Bei TIFF, HEIC und allen drei Videobehältern durfte sich kein Byte
//! bewegen. Die Regel dahinter lautete aber nie „nichts verschieben", sondern
//! **„nichts verschieben, worauf etwas zeigt"**. Ein MP3 enthält keine
//! einzige Tabelle mit Byte-Positionen: Der Datenstrom besteht aus Rahmen,
//! die sich selbst synchronisieren — jeder beginnt mit elf gesetzten Bits,
//! und ein Abspielprogramm findet den nächsten, indem es danach sucht.
//!
//! Deshalb werden die Marken hier **abgeschnitten**, nicht überschrieben. Die
//! Datei wird kleiner, und das ist richtig so: Ein leergeräumter, aber noch
//! vorhandener Tag verriete weiterhin, dass es einmal einen gab.
//!
//! Der `Xing`- beziehungsweise `Info`-Kopf für die Sprungtabelle liegt
//! **innerhalb des ersten Audiorahmens** und zählt Versätze ab genau diesem
//! Rahmen. Da der Rahmen der erste bleibt, stimmt die Tabelle weiterhin.
//!
//! # Wo ein MP3 Marken trägt
//!
//! - **ID3v2** am Anfang — der Regelfall. Titel, Interpret, Kommentar, und
//!   dazu Dinge, die niemand erwartet: `APIC` ist ein **eingebettetes Bild**
//!   mit eigenen Metadaten, `GEOB` eine **beliebige zweite Datei**, `UFID`
//!   und `PRIV` sind **Kennungen**, mit denen Händler ihre Käufer
//!   wiedererkennen.
//! - **ID3v1** als letzte 128 Bytes — das alte Format, oft zusätzlich da.
//! - **APEv2** am Ende, mit einem Fußteil, der rückwärts gelesen wird.
//! - **Lyrics3v2** am Ende, aus derselben Zeit.

use crate::model::{Finding, FindingKind, Inspection, Severity, StripResult};

use cabrik_core::{Error, Result};

/// Höchstzahl der ID3v2-Rahmen, die verfolgt werden.
const MAX_RAHMEN: usize = 10_000;

/// Ob die Bytes wie ein MP3 aussehen.
///
/// Entweder steht ein ID3v2-Tag davor, oder die Datei beginnt unmittelbar mit
/// einem gültigen Rahmenkopf. Auf einen bloßen `0xFF` zu prüfen genügt nicht —
/// damit fingen zu viele fremde Dateien an.
#[must_use]
pub fn looks_like_mp3(daten: &[u8]) -> bool {
    if daten.starts_with(b"ID3") {
        // Die Fassung muss plausibel sein, sonst ist es Zufall.
        return daten.get(3).is_some_and(|v| *v < 5);
    }
    ist_rahmenkopf(daten, 0)
}

/// Ob an dieser Stelle ein gültiger MPEG-Audio-Rahmenkopf steht.
fn ist_rahmenkopf(daten: &[u8], p: usize) -> bool {
    let (Some(a), Some(b), Some(c)) = (
        daten.get(p),
        daten.get(p.saturating_add(1)),
        daten.get(p.saturating_add(2)),
    ) else {
        return false;
    };
    // Elf gesetzte Bits als Synchronisationswort.
    if *a != 0xFF || (b & 0xE0) != 0xE0 {
        return false;
    }
    // Fassung 01 und Schicht 00 sind reserviert.
    if (b & 0x18) == 0x08 || (b & 0x06) == 0x00 {
        return false;
    }
    // Bitrate „frei" und „ungültig", Abtastrate „reserviert".
    if (c & 0xF0) == 0xF0 || (c & 0x0C) == 0x0C {
        return false;
    }
    true
}

// ---------------------------------------------------------------------------
// Die Grenzen des Tonstroms
// ---------------------------------------------------------------------------

/// Wo die Marken liegen und wo der Ton beginnt und endet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Aufteilung {
    /// Länge des ID3v2-Tags am Anfang, null wenn keiner da ist.
    id3v2: usize,
    /// Erstes Byte des Tonstroms.
    ton_von: usize,
    /// Erstes Byte hinter dem Tonstrom.
    ton_bis: usize,
    /// Länge des ID3v1-Tags am Ende.
    id3v1: usize,
    /// Länge eines APEv2-Tags am Ende.
    ape: usize,
    /// Länge eines Lyrics3v2-Tags am Ende.
    lyrics: usize,
}

/// Liest eine „syncsafe"-Zahl: sieben nutzbare Bits je Byte.
///
/// ID3v2 speichert Längen so, damit in ihnen nie ein Synchronisationswort
/// entsteht, das ein Abspielprogramm für den Anfang eines Rahmens hielte.
fn syncsafe(b: &[u8]) -> Option<usize> {
    let mut wert = 0usize;
    for x in b.get(..4)? {
        if *x & 0x80 != 0 {
            return None;
        }
        wert = wert.checked_mul(128)?.checked_add(usize::from(*x))?;
    }
    Some(wert)
}

fn u32_be(b: &[u8]) -> Option<usize> {
    Some(u32::from_be_bytes(b.get(..4)?.try_into().ok()?) as usize)
}

/// Gesamtlänge eines ID3v2-Tags am Dateianfang, einschließlich Kopf und
/// etwaigem Fußteil. `None`, wenn dort keiner steht.
///
/// Auch [`crate::flac`] braucht das: FLAC kennt kein ID3, trotzdem schreiben
/// manche Programme einen davor.
pub(crate) fn id3v2_laenge(daten: &[u8]) -> Option<usize> {
    if !daten.starts_with(b"ID3") {
        return None;
    }
    let groesse = syncsafe(daten.get(6..10)?)?;
    let mut laenge = groesse.checked_add(10)?;
    // Merkmalsbit 4 in v2.4: ein Fußteil von zehn Bytes folgt.
    if daten.get(3).is_some_and(|v| *v >= 4) && daten.get(5).is_some_and(|f| f & 0x10 != 0) {
        laenge = laenge.checked_add(10)?;
    }
    (laenge <= daten.len()).then_some(laenge)
}

fn teile_auf(daten: &[u8]) -> Result<Aufteilung> {
    let gesamt = daten.len();

    // --- Vorne: ID3v2 -----------------------------------------------------
    let id3v2 = if daten.starts_with(b"ID3") {
        id3v2_laenge(daten).ok_or(Error::Malformed("mp3: ID3v2 reicht ueber das Dateiende"))?
    } else {
        0
    };

    // --- Hinten: rückwärts abtragen ---------------------------------------
    let mut ende = gesamt;
    let mut id3v1 = 0usize;
    let mut ape = 0usize;
    let mut lyrics = 0usize;

    // ID3v1 ist immer genau die letzten 128 Bytes.
    if ende
        .checked_sub(128)
        .is_some_and(|a| daten.get(a..a.saturating_add(3)) == Some(b"TAG"))
    {
        id3v1 = 128;
        ende = ende.saturating_sub(128);
    }

    // Lyrics3v2 endet auf „LYRICS200" mit sechs Ziffern Länge davor.
    if let Some(a) = ende.checked_sub(15)
        && daten.get(a.saturating_add(6)..ende) == Some(b"LYRICS200")
        && let Some(ziffern) = daten.get(a..a.saturating_add(6))
        && let Ok(text) = core::str::from_utf8(ziffern)
        && let Ok(n) = text.parse::<usize>()
    {
        // Die Zahl zählt ohne den eigenen Abschluss von 15 Bytes.
        let ganz = n.saturating_add(15);
        if ganz <= ende {
            lyrics = ganz;
            ende = ende.saturating_sub(ganz);
        }
    }

    // APEv2 hat einen Fußteil von 32 Bytes, der rückwärts gefunden wird.
    if let Some(a) = ende.checked_sub(32)
        && daten.get(a..a.saturating_add(8)) == Some(b"APETAGEX")
    {
        let groesse = daten
            .get(a.saturating_add(12)..a.saturating_add(16))
            .and_then(|b| Some(u32::from_le_bytes(b.try_into().ok()?) as usize))
            .unwrap_or(0);
        // Die Angabe zählt den Fußteil mit, den Kopf aber nur, wenn es ihn
        // gibt — deshalb wird beides geprüft.
        let mit_kopf = groesse.checked_add(32).unwrap_or(groesse);
        let ganz = if ende
            .checked_sub(mit_kopf)
            .is_some_and(|k| daten.get(k..k.saturating_add(8)) == Some(b"APETAGEX"))
        {
            mit_kopf
        } else {
            groesse
        };
        if ganz > 0 && ganz <= ende {
            ape = ganz;
            ende = ende.saturating_sub(ganz);
        }
    }

    if ende < id3v2 {
        return Err(Error::Malformed("mp3: Marken ueberlappen den Tonstrom"));
    }

    Ok(Aufteilung {
        id3v2,
        ton_von: id3v2,
        ton_bis: ende,
        id3v1,
        ape,
        lyrics,
    })
}

// ---------------------------------------------------------------------------
// ID3v2 lesen
// ---------------------------------------------------------------------------

/// Entschlüsselt eine ID3v2-Zeichenkette nach ihrem Kodierungsbyte.
fn id3_text(roh: &[u8]) -> String {
    let (Some(art), Some(rest)) = (roh.first(), roh.get(1..)) else {
        return String::new();
    };
    let aufraeumen = |s: String| s.replace('\u{0}', " ").trim().to_owned();

    match art {
        // ISO-8859-1: jedes Byte ist unmittelbar ein Zeichen.
        0 => aufraeumen(rest.iter().map(|b| char::from(*b)).collect()),
        1 | 2 => {
            let (gross, nutz) = match (art, rest.get(..2)) {
                (1, Some([0xFF, 0xFE])) => (false, rest.get(2..).unwrap_or(&[])),
                (1, Some([0xFE, 0xFF])) => (true, rest.get(2..).unwrap_or(&[])),
                // Ohne Kennzeichen gilt bei Art 2 die große Reihenfolge.
                _ => (*art == 2, rest),
            };
            let paare: Vec<u16> = nutz
                .chunks_exact(2)
                .filter_map(|p| {
                    let (a, b) = (*p.first()?, *p.get(1)?);
                    Some(if gross {
                        u16::from_be_bytes([a, b])
                    } else {
                        u16::from_le_bytes([a, b])
                    })
                })
                .collect();
            aufraeumen(String::from_utf16_lossy(&paare))
        }
        // 3 und alles Unbekannte: UTF-8 versuchen.
        _ => aufraeumen(String::from_utf8_lossy(rest).into_owned()),
    }
}

/// Ordnet einen ID3v2-Rahmen ein.
///
/// Die vierbuchstabigen Namen gelten ab v2.3; v2.2 benutzt drei Buchstaben,
/// die hier auf dieselben Fälle abgebildet werden.
fn rahmen_einordnung(kennung: &[u8]) -> Option<(&'static str, FindingKind, Severity)> {
    Some(match kennung {
        b"TPE1" | b"TP1" | b"TPE2" | b"TP2" | b"TCOM" | b"TCM" | b"TEXT" | b"TOPE" => (
            "Interpret oder Verfasser",
            FindingKind::Author,
            Severity::Critical,
        ),
        b"COMM" | b"COM" => ("Kommentar", FindingKind::Comment, Severity::Critical),
        b"USLT" | b"ULT" => ("Liedtext", FindingKind::Comment, Severity::Notable),
        b"TXXX" | b"TXX" => ("freie Angabe", FindingKind::Comment, Severity::Critical),
        b"TALB" | b"TAL" => ("Album", FindingKind::Comment, Severity::Notable),
        b"TIT2" | b"TT2" | b"TIT1" | b"TIT3" => ("Titel", FindingKind::Comment, Severity::Notable),
        b"TSSE" | b"TSS" | b"TENC" | b"TEN" => (
            "erzeugendes Programm",
            FindingKind::Software,
            Severity::Notable,
        ),
        b"TDRC" | b"TYER" | b"TYE" | b"TDAT" | b"TIME" | b"TDEN" | b"TDTG" => {
            ("Zeitangabe", FindingKind::Timestamp, Severity::Notable)
        }
        b"TPUB" | b"TPB" | b"TCOP" | b"TCR" | b"TOWN" => (
            "Herausgeber oder Eigentümer",
            FindingKind::Organization,
            Severity::Notable,
        ),
        // Die drei, die niemand erwartet.
        b"APIC" | b"PIC" => (
            "eingebettetes Bild — es trägt eigene Metadaten",
            FindingKind::EmbeddedPreview,
            Severity::Critical,
        ),
        b"GEOB" | b"GEO" => (
            "eingebettete Datei — eine vollständige zweite Datei",
            FindingKind::UnknownExtension,
            Severity::Critical,
        ),
        b"UFID" | b"UFI" | b"PRIV" => (
            "Kennung — damit erkennt ein Händler seinen Käufer wieder",
            FindingKind::Device,
            Severity::Critical,
        ),
        b"WXXX" | b"WXX" | b"WOAF" | b"WOAR" | b"WPUB" => {
            ("Web-Adresse", FindingKind::Comment, Severity::Notable)
        }
        _ => return None,
    })
}

/// Läuft die Rahmen eines ID3v2-Tags ab.
///
/// Auch von [`crate::flac`] benutzt, wenn dort ein ID3-Tag vorgefunden wird.
pub(crate) fn id3v2_funde(daten: &[u8], laenge: usize) -> Vec<Finding> {
    let mut aus = Vec::new();
    let Some(fassung) = daten.get(3).copied() else {
        return aus;
    };
    // v2.2 hat kürzere Rahmenköpfe und dreibuchstabige Namen.
    let (kopf, kennung_len) = if fassung <= 2 {
        (6usize, 3usize)
    } else {
        (10usize, 4usize)
    };

    let mut p = 10usize;
    while p.checked_add(kopf).is_some_and(|e| e <= laenge) && aus.len() < MAX_RAHMEN {
        let Some(kennung) = daten.get(p..p.saturating_add(kennung_len)) else {
            break;
        };
        // Ein Nullbyte heißt: ab hier ist nur noch Füllung.
        if kennung.first() == Some(&0) {
            break;
        }
        let feld = daten
            .get(
                p.saturating_add(kennung_len)
                    ..p.saturating_add(kopf.saturating_sub(2).max(kennung_len)),
            )
            .unwrap_or(&[]);
        let groesse = match (fassung, kennung_len) {
            // Ab v2.4 sind auch die Rahmenlängen syncsafe.
            (4.., _) => syncsafe(feld),
            // v2.2: drei Bytes, ganz gewöhnlich gezählt.
            (_, 3) => feld.get(..3).map(|b| {
                b.iter()
                    .fold(0usize, |a, x| a.wrapping_shl(8) | usize::from(*x))
            }),
            _ => u32_be(feld),
        }
        .unwrap_or(0);

        let inhalt_von = p.saturating_add(kopf);
        let inhalt_bis = inhalt_von.saturating_add(groesse).min(laenge);
        if groesse == 0 || inhalt_bis <= inhalt_von {
            break;
        }

        if let Some((was, art, schwere)) = rahmen_einordnung(kennung) {
            let roh = daten.get(inhalt_von..inhalt_bis).unwrap_or(&[]);
            let name = String::from_utf8_lossy(kennung).into_owned();
            // Bei Bildern und Dateien steht im Inhalt keine lesbare Angabe,
            // sondern der Rohinhalt — dann wird die Größe genannt.
            let wert = if matches!(
                art,
                FindingKind::EmbeddedPreview | FindingKind::UnknownExtension
            ) {
                format!("{was} ({} Bytes)", inhalt_bis.saturating_sub(inhalt_von))
            } else if matches!(kennung, b"PRIV" | b"UFID" | b"UFI") {
                // **Diese beiden haben kein Kodierungsbyte.** Ihr Inhalt ist
                // `Eigentümer\0Daten`, wobei die Daten binär sein dürfen.
                // Sie wie eine Textangabe zu lesen verschluckt das erste
                // Zeichen des Eigentümers — aus „WM/…" wurde „M/…".
                let eigentuemer = roh
                    .split(|b| *b == 0)
                    .next()
                    .map(|s| String::from_utf8_lossy(s).into_owned())
                    .unwrap_or_default();
                let rest = roh.len().saturating_sub(eigentuemer.len());
                format!("{was}: {eigentuemer} ({rest} Bytes Nutzlast)")
            } else {
                let t = id3_text(roh);
                if t.is_empty() {
                    was.to_owned()
                } else {
                    format!("{was}: {t}")
                }
            };
            aus.push(Finding {
                kind: art,
                location: format!("MP3:ID3v2/{name}"),
                value: Some(wert),
                severity: schwere,
            });
        }

        p = inhalt_bis;
    }
    aus
}

/// Liest die festen Felder eines ID3v1-Tags.
fn id3v1_funde(daten: &[u8], von: usize) -> Vec<Finding> {
    let feld = |a: usize, b: usize| -> String {
        daten
            .get(von.saturating_add(a)..von.saturating_add(b))
            .map(|s| {
                s.iter()
                    .take_while(|c| **c != 0)
                    .map(|c| char::from(*c))
                    .collect::<String>()
                    .trim()
                    .to_owned()
            })
            .unwrap_or_default()
    };
    let mut aus = Vec::new();
    for (a, b, was, art, schwere) in [
        (
            3usize,
            33usize,
            "Titel",
            FindingKind::Comment,
            Severity::Notable,
        ),
        (33, 63, "Interpret", FindingKind::Author, Severity::Critical),
        (63, 93, "Album", FindingKind::Comment, Severity::Notable),
        (93, 97, "Jahr", FindingKind::Timestamp, Severity::Notable),
        (
            97,
            127,
            "Kommentar",
            FindingKind::Comment,
            Severity::Critical,
        ),
    ] {
        let t = feld(a, b);
        if !t.is_empty() {
            aus.push(Finding {
                kind: art,
                location: "MP3:ID3v1".to_owned(),
                value: Some(format!("{was}: {t}")),
                severity: schwere,
            });
        }
    }
    aus
}

// ---------------------------------------------------------------------------
// Der Kodierername — zweimal in derselben Datei, zweimal anders zu behandeln
// ---------------------------------------------------------------------------
//
// Ein von ffmpeg erzeugtes MP3 nennt sein Werkzeug an **zwei** Stellen, und
// der Unterschied zwischen ihnen ist der Unterschied zwischen „entfernbar"
// und „nicht entfernbar":
//
//   1. Im `Xing`- beziehungsweise `Info`-Kopf steht ein neun Byte breites
//      Feld mit dem Namen. Dieser Kopf sitzt zwar in einem MPEG-Rahmen, aber
//      in einem, der **keinen Ton enthält** — er dient allein der
//      Sprungtabelle. Das Feld hat feste Breite. Es zu nullen verschiebt
//      nichts und ist nicht zu hören.
//
//   2. In den **Zusatzdaten der eigentlichen Tonrahmen**. LAME schreibt
//      seinen Namen dorthin, wo im Rahmen Platz übrig ist — bei leisen
//      Stellen also in fast jeden. Das ist Tondatenstrom. Ihn zu entfernen
//      hieße, den Ton neu zu berechnen, und damit wäre es nicht mehr
//      dieselbe Aufnahme.
//
// Fall 2 ist der Grund, warum ein MP3 aus einem Schnittprogramm in aller
// Regel `Partial` bleibt. Das zu verschweigen wäre bequemer und falsch.

/// Kennungen, unter denen sich Kodierer in den Zusatzdaten zu erkennen geben.
const KODIERER_SPUREN: [&[u8]; 3] = [b"LAME", b"Lavc", b"Lavf"];

/// Länge der Seiteninformation, die dem `Xing`-Kopf vorausgeht.
///
/// Sie hängt von der MPEG-Fassung und davon ab, ob die Aufnahme einkanalig
/// ist — die Norm sieht vier verschiedene Werte vor.
fn seiten_info(ton: &[u8]) -> Option<usize> {
    let mpeg1 = (*ton.get(1)? >> 3) & 0b11 == 0b11;
    let mono = (*ton.get(3)? >> 6) & 0b11 == 0b11;
    Some(match (mpeg1, mono) {
        (true, false) => 32,
        (true, true) | (false, false) => 17,
        (false, true) => 9,
    })
}

/// Lage des Kodierernamens im `Xing`/`Info`-Kopf, als Bereich in `ton`.
fn xing_kodierer(ton: &[u8]) -> Option<(usize, usize)> {
    let xing = 4usize.checked_add(seiten_info(ton)?)?;
    let marke = ton.get(xing..xing.checked_add(4)?)?;
    if marke != b"Xing" && marke != b"Info" {
        return None;
    }
    let merkmale = u32_be(ton.get(xing.checked_add(4)?..)?)?;

    // Die vier wahlfreien Felder stehen in fester Reihenfolge, jedes nur
    // dann, wenn sein Merkbit gesetzt ist.
    let mut p = xing.checked_add(8)?;
    for (bit, laenge) in [(0x1usize, 4usize), (0x2, 4), (0x4, 100), (0x8, 4)] {
        if merkmale & bit != 0 {
            p = p.checked_add(laenge)?;
        }
    }

    let ende = p.checked_add(9)?;
    let feld = ton.get(p..ende)?;
    // Steht dort kein lesbarer Text, ist die Rechnung nicht aufgegangen —
    // dann lieber nichts anfassen.
    feld.iter().any(u8::is_ascii_graphic).then_some((p, ende))
}

/// Sucht Kodiererkennungen im Tonstrom, außerhalb des `Xing`-Feldes.
fn kodierer_in_rahmen(ton: &[u8], ausser: Option<(usize, usize)>) -> Option<String> {
    for spur in KODIERER_SPUREN {
        let mut p = 0usize;
        while let Some(rel) = ton
            .get(p..)
            .and_then(|s| s.windows(spur.len()).position(|f| f == spur))
        {
            let stelle = p.saturating_add(rel);
            let drin = ausser.is_some_and(|(a, b)| stelle >= a && stelle < b);
            if !drin {
                return Some(String::from_utf8_lossy(spur).into_owned());
            }
            p = stelle.saturating_add(1);
        }
    }
    None
}

fn funde(daten: &[u8], teil: &Aufteilung) -> Vec<Finding> {
    let mut aus = Vec::new();
    if teil.id3v2 > 0 {
        aus.extend(id3v2_funde(daten, teil.id3v2));
    }
    if teil.id3v1 > 0 {
        aus.extend(id3v1_funde(
            daten,
            teil.ton_bis
                .saturating_add(teil.ape)
                .saturating_add(teil.lyrics),
        ));
    }
    if teil.ape > 0 {
        aus.push(Finding {
            kind: FindingKind::UnknownExtension,
            location: "MP3:APEv2".to_owned(),
            value: Some(format!("APEv2-Marken am Dateiende ({} Bytes)", teil.ape)),
            severity: Severity::Critical,
        });
    }
    if teil.lyrics > 0 {
        aus.push(Finding {
            kind: FindingKind::Comment,
            location: "MP3:Lyrics3v2".to_owned(),
            value: Some(format!("Lyrics3v2 am Dateiende ({} Bytes)", teil.lyrics)),
            severity: Severity::Notable,
        });
    }

    // Der Kodierername im Xing-Kopf — feste Breite, entfernbar.
    if let Some(ton) = daten.get(teil.ton_von..teil.ton_bis)
        && let Some((a, b)) = xing_kodierer(ton)
        && let Some(feld) = ton.get(a..b)
    {
        let name = String::from_utf8_lossy(feld)
            .trim_matches(|c: char| c == '\u{0}' || c.is_whitespace())
            .to_owned();
        if !name.is_empty() {
            aus.push(Finding {
                kind: FindingKind::Software,
                location: "MP3:Xing/Kodierer".to_owned(),
                value: Some(name),
                severity: Severity::Notable,
            });
        }
    }
    aus
}

/// Der Kodierername in den Zusatzdaten der Tonrahmen — **nicht** entfernbar.
fn rest_im_tonstrom(ton: &[u8], ausser: Option<(usize, usize)>) -> Option<Finding> {
    let spur = kodierer_in_rahmen(ton, ausser)?;
    Some(Finding {
        kind: FindingKind::Software,
        location: "MP3:Tonrahmen".to_owned(),
        value: Some(format!(
            "der Name des Kodierers („{spur}“) steht in den Zusatzdaten der Tonrahmen selbst — \
             ihn zu entfernen hieße, den Ton neu zu berechnen"
        )),
        severity: Severity::Notable,
    })
}

/// Zeigt die Marken an, ohne die Datei zu verändern.
///
/// # Fehler
///
/// [`Error::Malformed`], wenn die Marken über den Tonstrom hinausreichen.
pub fn inspect(daten: &[u8]) -> Result<Inspection> {
    let teil = teile_auf(daten)?;
    let mut alle = funde(daten, &teil);

    if let Some(ton) = daten.get(teil.ton_von..teil.ton_bis)
        && let Some(f) = rest_im_tonstrom(ton, xing_kodierer(ton))
    {
        alle.push(f);
    }

    Ok(Inspection {
        format: Some("MP3".to_owned()),
        findings: alle,
        understood: true,
    })
}

/// Entfernt sämtliche Marken und behält nur den Tonstrom.
///
/// Der Kodierername im `Xing`-Kopf wird mitgenullt; er hat feste Breite und
/// steht in einem Rahmen ohne Ton. Steht derselbe Name auch in den
/// Zusatzdaten der Tonrahmen, **bleibt er dort** — und das Ergebnis ist
/// `Partial`.
///
/// # Fehler
///
/// [`Error::Malformed`], wenn hinter den Marken kein gültiger Rahmenkopf
/// steht. Dann stimmt die Aufteilung nicht, und lieber wird nichts geliefert
/// als eine beschädigte Datei.
pub fn strip(daten: &[u8]) -> Result<(Vec<u8>, StripResult)> {
    let teil = teile_auf(daten)?;
    let entfernt = funde(daten, &teil);

    let mut ton = daten
        .get(teil.ton_von..teil.ton_bis)
        .ok_or(Error::Malformed("mp3: Tonstrom nicht auffindbar"))?
        .to_vec();

    // Die Probe aufs Exempel: Am errechneten Anfang muss ein Rahmen stehen.
    // Ohne diese Prüfung lieferte eine falsch gelesene Länge stillschweigend
    // eine Datei, die sich nicht mehr abspielen lässt.
    if !ton.is_empty() && !ist_rahmenkopf(&ton, 0) {
        return Err(Error::Malformed(
            "mp3: hinter den Marken steht kein Rahmenkopf",
        ));
    }

    if let Some((a, b)) = xing_kodierer(&ton)
        && let Some(feld) = ton.get_mut(a..b)
    {
        feld.fill(0);
    }

    // Gesucht wird im **Ergebnis**: Was danach noch dasteht, steht wirklich
    // noch da. Eine Vorhersage wäre eine Behauptung.
    match rest_im_tonstrom(&ton, None) {
        None => Ok((ton, StripResult::Complete { removed: entfernt })),
        Some(rest) => Ok((
            ton,
            StripResult::Partial {
                removed: entfernt,
                remaining: vec![rest],
                reason: "der Name des Kodierers steckt in den Zusatzdaten der Tonrahmen; \
                         er ließe sich nur durch Neuberechnen des Tons entfernen"
                    .to_owned(),
            },
        )),
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "Tests duerfen laut werden"
)]
mod tests {
    use super::*;

    /// Ein gültiger MPEG-1 Layer III Rahmenkopf, 128 kbit/s, 44,1 kHz.
    const RAHMEN: [u8; 4] = [0xFF, 0xFB, 0x90, 0x00];

    fn ton() -> Vec<u8> {
        let mut v = RAHMEN.to_vec();
        v.extend_from_slice(&[0x55u8; 200]);
        v
    }

    /// Baut einen ID3v2.3-Rahmen.
    fn rahmen(kennung: &[u8; 4], inhalt: &[u8]) -> Vec<u8> {
        let mut v = kennung.to_vec();
        v.extend_from_slice(&u32::try_from(inhalt.len()).unwrap().to_be_bytes());
        v.extend_from_slice(&[0, 0]);
        v.extend_from_slice(inhalt);
        v
    }

    fn id3v2(rahmen: &[Vec<u8>], fuellung: usize) -> Vec<u8> {
        let inhalt: Vec<u8> = rahmen.iter().flatten().copied().collect();
        let groesse = inhalt.len() + fuellung;
        let mut v = b"ID3".to_vec();
        v.extend_from_slice(&[3, 0, 0]);
        // Syncsafe: sieben Bits je Byte.
        v.extend_from_slice(&[
            ((groesse >> 21) & 0x7F) as u8,
            ((groesse >> 14) & 0x7F) as u8,
            ((groesse >> 7) & 0x7F) as u8,
            (groesse & 0x7F) as u8,
        ]);
        v.extend_from_slice(&inhalt);
        v.extend_from_slice(&vec![0u8; fuellung]);
        v
    }

    fn beispiel() -> Vec<u8> {
        let tag = id3v2(
            &[
                rahmen(b"TIT2", b"\x00Angebot Nordstern"),
                rahmen(b"TPE1", b"\x00Dr. Anna Beispiel"),
                rahmen(b"COMM", b"\x00engNicht an den Kunden geben"),
                rahmen(b"TSSE", b"\x00Bearbeitungsprogramm 3.1"),
                rahmen(
                    b"APIC",
                    b"\x00image/jpeg\x00\x03\x00\xFF\xD8\xFF\xE0BILDDATEN",
                ),
                rahmen(b"PRIV", b"WM/Kennung\x00\x11\x22\x33\x44"),
            ],
            16,
        );
        [tag, ton()].concat()
    }

    #[test]
    fn mp3_wird_erkannt() {
        assert!(looks_like_mp3(&beispiel()));
        assert!(looks_like_mp3(&ton()), "auch ohne ID3v2");
        assert!(!looks_like_mp3(b"ID3 aber falsche Fassung\xFF"));
        assert!(!looks_like_mp3(&[0xFF, 0x00, 0x00, 0x00]));
        assert!(!looks_like_mp3(b""));
    }

    #[test]
    fn die_rahmen_werden_gelesen() {
        let i = inspect(&beispiel()).unwrap();
        assert!(i.understood);
        assert_eq!(i.format.as_deref(), Some("MP3"));

        let hole = |ort: &str| {
            i.findings
                .iter()
                .find(|f| f.location == ort)
                .unwrap_or_else(|| panic!("{ort} fehlt: {:?}", i.findings))
                .clone()
        };
        assert!(hole("MP3:ID3v2/TPE1").value.unwrap().contains("Dr. Anna"));
        assert_eq!(hole("MP3:ID3v2/TPE1").severity, Severity::Critical);
        assert!(
            hole("MP3:ID3v2/COMM")
                .value
                .unwrap()
                .contains("Nicht an den Kunden")
        );
    }

    /// **Die drei, die niemand erwartet.** Ein eingebettetes Bild bringt seine
    /// eigenen Metadaten mit, und `PRIV` trägt Kennungen von Händlern.
    #[test]
    fn bild_und_kennung_sind_kritische_funde() {
        let i = inspect(&beispiel()).unwrap();

        let bild = i
            .findings
            .iter()
            .find(|f| f.kind == FindingKind::EmbeddedPreview)
            .expect("das eingebettete Bild fehlt");
        assert_eq!(bild.severity, Severity::Critical);

        let kennung = i
            .findings
            .iter()
            .find(|f| f.location == "MP3:ID3v2/PRIV")
            .expect("PRIV fehlt");
        assert_eq!(kennung.severity, Severity::Critical);
        // `PRIV` hat **kein** Kodierungsbyte. Wer es als Text liest,
        // verschluckt das erste Zeichen des Eigentümers.
        assert!(
            kennung.value.as_deref().unwrap().contains("WM/Kennung"),
            "das erste Zeichen ging verloren: {:?}",
            kennung.value
        );
    }

    /// **Hier wird wirklich entfernt.** Ein MP3 führt keine Tabelle mit
    /// Byte-Positionen, also darf der Tonstrom nach vorn rücken.
    #[test]
    fn der_tonstrom_bleibt_uebrig_und_ist_bytegleich() {
        let vorher = beispiel();
        let (nachher, ergebnis) = strip(&vorher).unwrap();

        assert!(nachher.len() < vorher.len(), "es wurde nichts entfernt");
        assert_eq!(nachher, ton(), "der Tonstrom wurde veraendert");
        assert!(matches!(ergebnis, StripResult::Complete { .. }));
        assert!(inspect(&nachher).unwrap().findings.is_empty());
    }

    #[test]
    fn id3v1_am_ende_wird_gefunden_und_entfernt() {
        let mut v1 = b"TAG".to_vec();
        v1.extend_from_slice(b"Angebot Nordstern             "); // 30
        v1.extend_from_slice(b"Dr. Anna Beispiel             "); // 30
        v1.extend_from_slice(b"Interner Rohschnitt           "); // 30
        v1.extend_from_slice(b"2026"); // 4
        v1.extend_from_slice(b"Nicht an den Kunden geben     "); // 30
        v1.push(0);
        assert_eq!(v1.len(), 128);

        let datei = [ton(), v1].concat();
        let i = inspect(&datei).unwrap();
        let f = i
            .findings
            .iter()
            .find(|f| f.location == "MP3:ID3v1" && f.kind == FindingKind::Author)
            .expect("der Interpret aus ID3v1 fehlt");
        assert!(f.value.as_deref().unwrap().contains("Dr. Anna Beispiel"));

        let (sauber, _) = strip(&datei).unwrap();
        assert_eq!(sauber, ton());
    }

    #[test]
    fn ein_apev2_am_ende_wird_gefunden_und_entfernt() {
        // Nur der Fußteil, ohne Kopf — auch das kommt vor.
        let mut fuss = b"APETAGEX".to_vec();
        fuss.extend_from_slice(&2000u32.to_le_bytes()); // Fassung
        fuss.extend_from_slice(&64u32.to_le_bytes()); // Groesse mit Fussteil
        fuss.extend_from_slice(&1u32.to_le_bytes()); // Anzahl
        fuss.extend_from_slice(&0u32.to_le_bytes()); // Merkmale
        fuss.extend_from_slice(&[0u8; 8]);
        assert_eq!(fuss.len(), 32);

        let datei = [ton(), vec![0xAA; 32], fuss].concat();
        let i = inspect(&datei).unwrap();
        assert!(i.findings.iter().any(|f| f.location == "MP3:APEv2"));

        let (sauber, _) = strip(&datei).unwrap();
        assert_eq!(sauber, ton());
    }

    /// Alle drei Anhängsel gleichzeitig — sie treten wirklich gemeinsam auf.
    #[test]
    fn mehrere_marken_am_ende_werden_alle_abgetragen() {
        let mut v1 = b"TAG".to_vec();
        v1.extend_from_slice(&[b' '; 125]);

        let mut fuss = b"APETAGEX".to_vec();
        fuss.extend_from_slice(&2000u32.to_le_bytes());
        fuss.extend_from_slice(&48u32.to_le_bytes());
        fuss.extend_from_slice(&1u32.to_le_bytes());
        fuss.extend_from_slice(&0u32.to_le_bytes());
        fuss.extend_from_slice(&[0u8; 8]);

        let datei = [
            id3v2(&[rahmen(b"TIT2", b"\x00Titel")], 0),
            ton(),
            vec![0xAA; 16],
            fuss,
            v1,
        ]
        .concat();

        let (sauber, _) = strip(&datei).unwrap();
        assert_eq!(sauber, ton(), "nicht alle Anhaengsel wurden abgetragen");
    }

    /// **Die Sicherung.** Eine falsch gelesene Länge lieferte sonst
    /// stillschweigend eine Datei, die sich nicht abspielen lässt.
    #[test]
    fn eine_falsche_laenge_wird_bemerkt_statt_verschluckt() {
        let mut datei = beispiel();
        // Die Tag-Länge um vier Bytes verkleinern: Der Anfang des Tonstroms
        // liegt damit mitten in der Füllung.
        let alt = syncsafe(&datei[6..10]).unwrap();
        let neu = alt - 4;
        datei[6..10].copy_from_slice(&[
            ((neu >> 21) & 0x7F) as u8,
            ((neu >> 14) & 0x7F) as u8,
            ((neu >> 7) & 0x7F) as u8,
            (neu & 0x7F) as u8,
        ]);

        assert!(
            strip(&datei).is_err(),
            "die falsche Laenge wurde stillschweigend hingenommen"
        );
    }

    #[test]
    fn ein_zweiter_durchlauf_aendert_nichts() {
        let (einmal, _) = strip(&beispiel()).unwrap();
        let (zweimal, _) = strip(&einmal).unwrap();
        assert_eq!(einmal, zweimal);
    }

    /// Baut den ersten Rahmen mit `Xing`-Kopf, wie ihn jeder Kodierer
    /// voranstellt: vier Byte Rahmenkopf, 32 Byte Seiteninformation, dann
    /// `Xing` mit allen vier wahlfreien Feldern und dem Namensfeld.
    fn xing_rahmen(name: &[u8; 9]) -> Vec<u8> {
        let mut v = RAHMEN.to_vec();
        v.extend_from_slice(&[0u8; 32]);
        v.extend_from_slice(b"Xing");
        v.extend_from_slice(&0x0Fu32.to_be_bytes()); // alle vier Felder
        v.extend_from_slice(&20u32.to_be_bytes()); // Rahmenzahl
        v.extend_from_slice(&8000u32.to_be_bytes()); // Bytes
        v.extend_from_slice(&[0u8; 100]); // Sprungtabelle
        v.extend_from_slice(&100u32.to_be_bytes()); // Güte
        v.extend_from_slice(name);
        v.extend_from_slice(&[0x55u8; 64]);
        v
    }

    /// **Der Kodierername steht zweimal in derselben Datei**, und nur einer
    /// der beiden lässt sich entfernen.
    ///
    /// Im `Xing`-Kopf hat er feste Breite und sitzt in einem Rahmen ohne Ton
    /// — er wird genullt. In den Zusatzdaten der Tonrahmen ist er
    /// Tondatenstrom und bleibt; das Ergebnis ist dann `Partial`.
    ///
    /// Der Fall fiel an einer echten ffmpeg-Datei auf: Nach dem Bereinigen
    /// stand „LAME3.100" noch zwanzigmal in der Datei.
    #[test]
    fn der_kodierername_im_xing_kopf_faellt_weg_der_im_tonstrom_bleibt() {
        // Erst nur der Xing-Kopf, ohne Spur in den Tonrahmen.
        let sauber_moeglich = [
            id3v2(&[rahmen(b"TIT2", b"\x00Titel")], 0),
            xing_rahmen(b"Lavf62.12"),
        ]
        .concat();

        let i = inspect(&sauber_moeglich).unwrap();
        let f = i
            .findings
            .iter()
            .find(|f| f.location == "MP3:Xing/Kodierer")
            .expect("der Kodierer im Xing-Kopf wurde nicht gefunden");
        assert_eq!(f.value.as_deref(), Some("Lavf62.12"));

        let (aus, ergebnis) = strip(&sauber_moeglich).unwrap();
        assert!(
            !aus.windows(4).any(|w| w == b"Lavf"),
            "der Name im Xing-Kopf blieb stehen"
        );
        assert!(
            matches!(ergebnis, StripResult::Complete { .. }),
            "ohne Spur im Tonstrom muss es Complete sein: {ergebnis:?}"
        );

        // Jetzt dasselbe, aber mit dem Namen in den Zusatzdaten eines
        // weiteren Rahmens — so schreibt LAME ihn bei leisen Stellen.
        let mut zweiter = RAHMEN.to_vec();
        zweiter.extend_from_slice(&[0x55u8; 20]);
        zweiter.extend_from_slice(b"LAME3.100");
        zweiter.extend_from_slice(&[0x55u8; 20]);
        let mit_spur = [sauber_moeglich, zweiter].concat();

        let (aus, ergebnis) = strip(&mit_spur).unwrap();
        let StripResult::Partial { remaining, .. } = ergebnis else {
            panic!("mit Spur im Tonstrom muss es Partial sein");
        };
        assert!(remaining.iter().any(|f| f.location == "MP3:Tonrahmen"));
        assert!(
            !aus.windows(4).any(|w| w == b"Lavf"),
            "der Xing-Name muss trotzdem weg sein"
        );
        assert!(
            aus.windows(9).any(|w| w == b"LAME3.100"),
            "die Tondaten duerfen nicht angetastet werden"
        );
    }

    /// Die Seiteninformation ist unterschiedlich lang. Wird sie falsch
    /// berechnet, findet die Suche den `Xing`-Kopf nicht — und meldet
    /// stillschweigend nichts.
    #[test]
    fn die_seiteninformation_haengt_von_fassung_und_kanalzahl_ab() {
        // MPEG1, Stereo (Kanalmodus 00) -> 32
        assert_eq!(seiten_info(&[0xFF, 0xFB, 0x90, 0x00]), Some(32));
        // MPEG1, mono (Kanalmodus 11) -> 17
        assert_eq!(seiten_info(&[0xFF, 0xFB, 0x90, 0xC0]), Some(17));
        // MPEG2 (Fassung 10), Stereo -> 17
        assert_eq!(seiten_info(&[0xFF, 0xF3, 0x90, 0x00]), Some(17));
        // MPEG2, mono -> 9
        assert_eq!(seiten_info(&[0xFF, 0xF3, 0x90, 0xC0]), Some(9));
    }

    #[test]
    fn ein_id3v2_ueber_das_dateiende_hinaus_ist_ein_fehler() {
        let mut datei = id3v2(&[rahmen(b"TIT2", b"\x00Titel")], 0);
        datei[6..10].copy_from_slice(&[0x00, 0x00, 0x7F, 0x7F]);
        assert!(inspect(&datei).is_err());
    }

    /// UTF-16 mit Kennzeichen — ohne diesen Zweig wären die Werte unlesbar.
    #[test]
    fn utf16_wird_entschluesselt() {
        let mut inhalt = vec![0x01u8, 0xFF, 0xFE];
        for c in "Grüße".encode_utf16() {
            inhalt.extend_from_slice(&c.to_le_bytes());
        }
        assert_eq!(id3_text(&inhalt), "Grüße");

        assert_eq!(id3_text(b"\x00Caf\xE9"), "Café");
        assert_eq!(id3_text("\u{3}Grüße".as_bytes()), "Grüße");
    }
}
