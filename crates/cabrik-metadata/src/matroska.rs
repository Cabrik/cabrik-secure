//! Matroska und WebM (`spec/metadata.md` §4).
//!
//! Dasselbe Vorgehen wie bei MP4, aus demselben Grund: **Ersetzen an Ort und
//! Stelle.** Ein Matroska-Segment enthält mit `SeekHead` und `Cues` zwei
//! Verzeichnisse voller **absoluter Byte-Positionen**. Wer ein Element
//! entfernt und alles Nachfolgende nach vorn rückt, macht jeden dieser
//! Einträge falsch.
//!
//! EBML sieht dafür — wie ISO-BMFF mit `free` — einen eigenen Platzhalter
//! vor: das **`Void`-Element**. Ein Leser überspringt es. Da die Längenangabe
//! in EBML absichtlich auch länger als nötig geschrieben werden darf
//! (RFC 8794 §4.4), lässt sich ein `Void` auf **jede** Größe ab zwei Bytes
//! bringen. Genau dafür ist es gedacht.
//!
//! # Drei Feinheiten, die ein naives Vorgehen übersieht
//!
//! 1. **`MuxingApp` und `WritingApp` sind Pflichtelemente** ohne Vorgabewert.
//!    Sie zu entfernen ergäbe eine formal fehlerhafte Datei. Sie werden
//!    deshalb nicht überschrieben, sondern **geleert** — die Zeichenkette
//!    wird leer, das Element bleibt stehen.
//!
//! 2. **Der `SeekHead` verrät die Entfernung.** Er ist ein Verzeichnis der
//!    Form „Tags stehen bei Byte 4711". Bleibt der Eintrag stehen, während
//!    die Tags zu `Void` geworden sind, steht dort weiterhin geschrieben,
//!    dass es einmal Tags gab. Die betroffenen `Seek`-Einträge werden
//!    deshalb ebenfalls zu `Void`.
//!
//! 3. **`CRC-32` prüft Geschwister.** Ändert sich etwas innerhalb eines
//!    Elternelements, das eine Prüfsumme führt, ist diese danach falsch.
//!    Sie ist optional und wird darum mit entfernt.
//!
//! # Was ein Video verrät
//!
//! - **`SegmentFilename`** — der **ursprüngliche Dateiname**. Derselbe Leck
//!   wie in v1, wo er im Klartext im Umschlag stand.
//! - **`SegmentUID`** — eine Zufallskennung, die Kopien derselben Datei
//!   miteinander verknüpft.
//! - **`Tags`** — Verfasser, Kommentar, Aufnahmedatum, erzeugendes Programm.
//! - **`Attachments`** — vollständige zweite Dateien im Video.
//! - **`DateUTC`**, `Title`, `WritingApp` und der Spurname.
//!
//! Kapitel bleiben **stehen**: Ihre Namen sind Navigation, also Inhalt, den
//! der Nutzer selbst angelegt hat. Sie werden gemeldet, nicht entfernt.

use crate::model::{Finding, FindingKind, Inspection, Severity, StripResult};

use cabrik_core::{Error, Result};

/// Höchstzahl der Elemente, die verfolgt werden.
const MAX_ELEMENTE: usize = 200_000;
/// Höchste Schachtelungstiefe.
const MAX_TIEFE: usize = 10;

// --- Kennungen, wie sie in der Datei stehen (mitsamt Markierungsbit) --------

const EBML_KOPF: u32 = 0x1A45_DFA3;
const DOC_TYPE: u32 = 0x4282;
const SEGMENT: u32 = 0x1853_8067;
const SEEK_HEAD: u32 = 0x114D_9B74;
const SEEK: u32 = 0x4DBB;
const SEEK_ID: u32 = 0x53AB;
const INFO: u32 = 0x1549_A966;
const SEGMENT_UID: u32 = 0x73A4;
const SEGMENT_DATEINAME: u32 = 0x7384;
const VORIGER_DATEINAME: u32 = 0x003C_83AB;
const NAECHSTER_DATEINAME: u32 = 0x003E_83BB;
const DATE_UTC: u32 = 0x4461;
const TITEL: u32 = 0x7BA9;
const MUXING_APP: u32 = 0x4D80;
const WRITING_APP: u32 = 0x5741;
const TRACKS: u32 = 0x1654_AE6B;
const TRACK_ENTRY: u32 = 0x00AE;
const TRACK_NAME: u32 = 0x536E;
const TAGS: u32 = 0x1254_C367;
const TAG: u32 = 0x7373;
const SIMPLE_TAG: u32 = 0x67C8;
const TAG_NAME: u32 = 0x45A3;
const TAG_TEXT: u32 = 0x4487;
const ATTACHMENTS: u32 = 0x1941_A469;
const ATTACHED_FILE: u32 = 0x61A7;
const DATEI_NAME: u32 = 0x466E;
const KAPITEL: u32 = 0x1043_A770;
const EDITION: u32 = 0x45B9;
const KAPITEL_ATOM: u32 = 0x00B6;
const KAPITEL_ANZEIGE: u32 = 0x0080;
const KAPITEL_TEXT: u32 = 0x0085;
const CLUSTER: u32 = 0x1F43_B675;
const CRC32: u32 = 0x00BF;
const VOID: u8 = 0xEC;

/// Elemente, die weitere Elemente enthalten.
///
/// `Cluster` fehlt bewusst: Dort liegen die Bilddaten, und dort hat dieses
/// Modul nichts verloren.
const MASTER: [u32; 16] = [
    EBML_KOPF,
    SEGMENT,
    SEEK_HEAD,
    SEEK,
    INFO,
    TRACKS,
    TRACK_ENTRY,
    TAGS,
    TAG,
    SIMPLE_TAG,
    ATTACHMENTS,
    ATTACHED_FILE,
    KAPITEL,
    EDITION,
    KAPITEL_ATOM,
    KAPITEL_ANZEIGE,
];

/// Ob die Bytes wie Matroska oder WebM aussehen.
#[must_use]
pub fn looks_like_matroska(daten: &[u8]) -> bool {
    daten.starts_with(&[0x1A, 0x45, 0xDF, 0xA3])
}

// ---------------------------------------------------------------------------
// EBML lesen
// ---------------------------------------------------------------------------

/// Liest eine Kennung mitsamt ihrem Markierungsbit.
///
/// Die Zahl der führenden Nullbits im ersten Byte gibt die Länge an. Anders
/// als bei der Größenangabe bleibt das Markierungsbit Teil des Werts — so
/// stehen die Kennungen in der Norm und so werden sie hier verglichen.
fn lies_kennung(daten: &[u8], p: usize) -> Option<(u32, usize)> {
    let erstes = *daten.get(p)?;
    if erstes == 0 {
        return None;
    }
    let laenge = usize::try_from(erstes.leading_zeros())
        .ok()?
        .checked_add(1)?;
    if laenge > 4 {
        return None;
    }
    let mut roh = [0u8; 4];
    let quelle = daten.get(p..p.checked_add(laenge)?)?;
    roh.get_mut(4usize.checked_sub(laenge)?..)?
        .copy_from_slice(quelle);
    Some((u32::from_be_bytes(roh), laenge))
}

/// Liest eine Längenangabe.
///
/// `None` als Länge heißt „unbekannt" — bei laufenden Aufnahmen zulässig.
fn lies_laenge(daten: &[u8], p: usize) -> Option<(Option<u64>, usize)> {
    let erstes = *daten.get(p)?;
    if erstes == 0 {
        return None;
    }
    let laenge = usize::try_from(erstes.leading_zeros())
        .ok()?
        .checked_add(1)?;
    if laenge > 8 {
        return None;
    }
    let mut roh = [0u8; 8];
    let quelle = daten.get(p..p.checked_add(laenge)?)?;
    roh.get_mut(8usize.checked_sub(laenge)?..)?
        .copy_from_slice(quelle);

    let maske = maske_fuer(laenge)?;
    let wert = u64::from_be_bytes(roh) & maske;
    // Lauter Einsen bedeutet: Länge unbekannt.
    Some((if wert == maske { None } else { Some(wert) }, laenge))
}

/// Größter Wert, der sich in einer Längenangabe aus `n` Bytes unterbringen
/// lässt — er selbst ist als „unbekannt" belegt.
fn maske_fuer(n: usize) -> Option<u64> {
    let bits = u32::try_from(n.checked_mul(7)?).ok()?;
    1u64.checked_shl(bits)?.checked_sub(1)
}

/// Ein gefundenes Element mit seiner Lage in der Datei.
#[derive(Debug, Clone, Copy)]
struct Element {
    kennung: u32,
    /// Beginn des Elements, einschließlich Kennung.
    anfang: usize,
    /// Beginn des Inhalts.
    inhalt: usize,
    /// Ende des Elements.
    ende: usize,
    /// Index des Elternelements in der Liste.
    eltern: Option<usize>,
}

/// Ergebnis des Durchlaufs.
struct Baum {
    elemente: Vec<Element>,
    /// Ob ein Element ohne Längenangabe auftrat. Dahinter ist der Aufbau
    /// nicht mehr sicher zu verfolgen.
    unklar: bool,
}

fn sammle(daten: &[u8]) -> Result<Baum> {
    let mut baum = Baum {
        elemente: Vec::new(),
        unklar: false,
    };
    lauf(daten, 0, daten.len(), 0, None, &mut baum)?;
    Ok(baum)
}

fn lauf(
    daten: &[u8],
    von: usize,
    bis: usize,
    tiefe: usize,
    eltern: Option<usize>,
    baum: &mut Baum,
) -> Result<()> {
    if tiefe > MAX_TIEFE {
        return Ok(());
    }
    let mut p = von;

    while p < bis {
        if baum.elemente.len() >= MAX_ELEMENTE {
            return Err(Error::Malformed("matroska: zu viele Elemente"));
        }
        let Some((kennung, k_len)) = lies_kennung(daten, p) else {
            // Kein lesbares Element mehr. Der Rest bleibt unangetastet —
            // das ist bei Füllbytes am Ende der Normalfall.
            return Ok(());
        };
        let nach_kennung = p
            .checked_add(k_len)
            .ok_or(Error::Malformed("matroska: Kennung ueberlaeuft"))?;
        let Some((laenge, l_len)) = lies_laenge(daten, nach_kennung) else {
            return Ok(());
        };
        let inhalt = nach_kennung
            .checked_add(l_len)
            .ok_or(Error::Malformed("matroska: Laengenangabe ueberlaeuft"))?;

        let ende = match laenge {
            Some(g) => {
                let g = usize::try_from(g)
                    .map_err(|_| Error::Malformed("matroska: Element zu gross"))?;
                let e = inhalt
                    .checked_add(g)
                    .ok_or(Error::Malformed("matroska: Elementende ueberlaeuft"))?;
                if e > bis {
                    return Err(Error::Malformed(
                        "matroska: Element reicht ueber seinen Bereich",
                    ));
                }
                e
            }
            None => {
                // Ohne Längenangabe reicht das Element bis zum Ende des
                // umgebenden Bereichs. Das stimmt für das Segment einer
                // laufenden Aufnahme; bei allem anderen ist danach nichts
                // mehr sicher zu erkennen, und das wird auch so gemeldet.
                if kennung != SEGMENT {
                    baum.unklar = true;
                }
                bis
            }
        };

        let hier = baum.elemente.len();
        baum.elemente.push(Element {
            kennung,
            anfang: p,
            inhalt,
            ende,
            eltern,
        });

        if kennung != CLUSTER && MASTER.contains(&kennung) {
            lauf(
                daten,
                inhalt,
                ende,
                tiefe.checked_add(1).unwrap_or(tiefe),
                Some(hier),
                baum,
            )?;
        }

        if ende <= p {
            return Err(Error::Malformed("matroska: Element ohne Fortschritt"));
        }
        p = ende;
    }
    Ok(())
}

/// Inhalt eines Elements als Zeichenkette, ohne die zulässige Nullfüllung.
fn text(daten: &[u8], e: &Element) -> String {
    let roh = daten.get(e.inhalt..e.ende).unwrap_or(&[]);
    let ohne_null = roh.split(|b| *b == 0).next().unwrap_or(&[]);
    String::from_utf8_lossy(ohne_null).trim().to_owned()
}

/// Sucht ein unmittelbares Kindelement.
fn kind(baum: &Baum, eltern: usize, kennung: u32) -> Option<Element> {
    baum.elemente
        .iter()
        .find(|e| e.eltern == Some(eltern) && e.kennung == kennung)
        .copied()
}

// ---------------------------------------------------------------------------
// Finden
// ---------------------------------------------------------------------------

fn fund(kind: FindingKind, ort: &str, wert: Option<String>, schwere: Severity) -> Finding {
    Finding {
        kind,
        location: format!("Matroska:{ort}"),
        value: wert,
        severity: schwere,
    }
}

/// Ordnet eine Marke aus `Tags` ein.
///
/// Matroska legt die Namen nicht fest — sie sind freie Zeichenketten. Die
/// gebräuchlichen werden erkannt, alles andere gilt vorsichtshalber als
/// Kommentar.
fn marke_einordnung(name: &str) -> (FindingKind, Severity) {
    match name.to_ascii_uppercase().as_str() {
        "ARTIST" | "AUTHOR" | "DIRECTOR" | "PRODUCER" | "WRITTEN_BY" | "LEAD_PERFORMER" => {
            (FindingKind::Author, Severity::Critical)
        }
        "COMMENT" | "COMMENTS" | "DESCRIPTION" | "SUMMARY" | "SYNOPSIS" => {
            (FindingKind::Comment, Severity::Critical)
        }
        "ENCODER" | "ENCODED_BY" | "APPLICATION" => (FindingKind::Software, Severity::Notable),
        "DATE_RECORDED" | "DATE_RELEASED" | "DATE_ENCODED" | "DATE_DIGITIZED" => {
            (FindingKind::Timestamp, Severity::Notable)
        }
        "PUBLISHER" | "DISTRIBUTED_BY" | "PRODUCTION_STUDIO" | "COPYRIGHT" => {
            (FindingKind::Organization, Severity::Notable)
        }
        _ => (FindingKind::Comment, Severity::Notable),
    }
}

fn funde(daten: &[u8], baum: &Baum) -> Vec<Finding> {
    let mut aus = Vec::new();

    for (i, e) in baum.elemente.iter().enumerate() {
        match e.kennung {
            SEGMENT_DATEINAME | VORIGER_DATEINAME | NAECHSTER_DATEINAME => {
                aus.push(fund(
                    FindingKind::FileName,
                    "Info/SegmentFilename",
                    Some(format!(
                        "ursprünglicher Dateiname „{}“ — er steht im Klartext in der Datei",
                        text(daten, e)
                    )),
                    Severity::Critical,
                ));
            }
            SEGMENT_UID => aus.push(fund(
                FindingKind::Device,
                "Info/SegmentUID",
                Some(
                    "Zufallskennung, die alle Kopien dieser Datei miteinander verknüpft".to_owned(),
                ),
                Severity::Notable,
            )),
            DATE_UTC => aus.push(fund(
                FindingKind::Timestamp,
                "Info/DateUTC",
                Some("Aufnahmezeitpunkt, auf die Sekunde genau".to_owned()),
                Severity::Notable,
            )),
            TITEL => aus.push(fund(
                FindingKind::Comment,
                "Info/Title",
                Some(text(daten, e)),
                Severity::Notable,
            )),
            MUXING_APP | WRITING_APP => {
                let t = text(daten, e);
                if !t.is_empty() {
                    let ort = if e.kennung == MUXING_APP {
                        "Info/MuxingApp"
                    } else {
                        "Info/WritingApp"
                    };
                    aus.push(fund(FindingKind::Software, ort, Some(t), Severity::Notable));
                }
            }
            TRACK_NAME => aus.push(fund(
                FindingKind::Comment,
                "Tracks/Name",
                Some(text(daten, e)),
                Severity::Notable,
            )),
            SIMPLE_TAG => {
                let name = kind(baum, i, TAG_NAME).map_or_else(String::new, |k| text(daten, &k));
                let wert = kind(baum, i, TAG_TEXT).map(|k| text(daten, &k));
                let (art, schwere) = marke_einordnung(&name);
                aus.push(fund(art, &format!("Tags/{name}"), wert, schwere));
            }
            ATTACHED_FILE => {
                let name = kind(baum, i, DATEI_NAME).map_or_else(String::new, |k| text(daten, &k));
                aus.push(fund(
                    FindingKind::UnknownExtension,
                    "Attachments",
                    Some(format!(
                        "angehängte Datei „{name}“ — eine vollständige zweite Datei im Video"
                    )),
                    Severity::Critical,
                ));
            }
            _ => {}
        }
    }
    aus
}

/// Kapitelnamen — gemeldet, aber nicht entfernt.
fn kapitel_funde(daten: &[u8], baum: &Baum) -> Vec<Finding> {
    baum.elemente
        .iter()
        .filter(|e| e.kennung == KAPITEL_TEXT)
        .map(|e| {
            fund(
                FindingKind::Comment,
                "Chapters",
                Some(format!("Kapitelname „{}“", text(daten, e))),
                Severity::Notable,
            )
        })
        .collect()
}

/// Zeigt die Metadaten an, ohne die Datei zu verändern.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputtem EBML-Aufbau.
pub fn inspect(daten: &[u8]) -> Result<Inspection> {
    let baum = sammle(daten)?;
    let mut alle = funde(daten, &baum);
    alle.extend(kapitel_funde(daten, &baum));

    Ok(Inspection {
        format: Some(art_name(&baum, daten).to_owned()),
        findings: alle,
        understood: true,
    })
}

fn art_name(baum: &Baum, daten: &[u8]) -> &'static str {
    let doc = baum
        .elemente
        .iter()
        .find(|e| e.kennung == DOC_TYPE)
        .map(|e| text(daten, e));
    match doc.as_deref() {
        Some("webm") => "WebM",
        _ => "Matroska (MKV)",
    }
}

// ---------------------------------------------------------------------------
// Bereinigen
// ---------------------------------------------------------------------------

/// Was mit einem Bereich geschehen soll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tat {
    /// Das ganze Element wird zu einem `Void` gleicher Größe.
    Leerraum,
    /// Nur der Inhalt wird genullt, das Element bleibt stehen.
    ///
    /// Für Pflichtelemente, deren Fehlen die Datei formal fehlerhaft machte.
    Entleeren,
}

/// Überschreibt einen Bereich mit einem `Void`-Element genau dieser Größe.
///
/// Gibt `false` zurück, wenn der Bereich zu klein ist — unter zwei Bytes
/// lässt sich kein Element unterbringen. Dann bleibt er unverändert.
fn schreib_void(ziel: &mut [u8]) -> bool {
    let gesamt = ziel.len();
    if gesamt < 2 {
        return false;
    }
    for n in 1..=8usize {
        let Some(nutz) = gesamt.checked_sub(1).and_then(|x| x.checked_sub(n)) else {
            continue;
        };
        let (Some(maske), Ok(nutz64)) = (maske_fuer(n), u64::try_from(nutz)) else {
            continue;
        };
        // Der Wert aus lauter Einsen ist als „Länge unbekannt" belegt.
        if nutz64 >= maske {
            continue;
        }
        let Some(marke) = maske.checked_add(1) else {
            continue;
        };
        ziel.fill(0);
        let Some(k) = ziel.first_mut() else {
            return false;
        };
        *k = VOID;

        let roh = (marke | nutz64).to_be_bytes();
        let (Some(quelle), Some(platz)) = (
            8usize.checked_sub(n).and_then(|a| roh.get(a..)),
            1usize.checked_add(n).and_then(|e| ziel.get_mut(1..e)),
        ) else {
            return false;
        };
        platz.copy_from_slice(quelle);
        return true;
    }
    false
}

/// Sammelt, was zu tun ist.
fn taten(daten: &[u8], baum: &Baum) -> Vec<(usize, usize, Tat)> {
    let mut aus: Vec<(usize, usize, Tat)> = Vec::new();
    let mut entfernte_kennungen: Vec<u32> = Vec::new();

    for e in &baum.elemente {
        match e.kennung {
            // Ganze Abschnitte, alle wahlfrei.
            TAGS | ATTACHMENTS => {
                aus.push((e.anfang, e.ende, Tat::Leerraum));
                entfernte_kennungen.push(e.kennung);
            }
            SEGMENT_UID | SEGMENT_DATEINAME | VORIGER_DATEINAME | NAECHSTER_DATEINAME
            | DATE_UTC | TITEL | TRACK_NAME => {
                aus.push((e.anfang, e.ende, Tat::Leerraum));
            }
            // Pflichtelemente: Sie bleiben stehen und werden geleert.
            MUXING_APP | WRITING_APP if e.ende > e.inhalt => {
                aus.push((e.inhalt, e.ende, Tat::Entleeren));
            }
            _ => {}
        }
    }

    // Der SeekHead wüsste sonst weiterhin zu berichten, dass hier einmal
    // Tags standen — samt Byte-Position.
    for (i, e) in baum.elemente.iter().enumerate() {
        if e.kennung != SEEK {
            continue;
        }
        let Some(ziel) = kind(baum, i, SEEK_ID) else {
            continue;
        };
        let roh = daten.get(ziel.inhalt..ziel.ende).unwrap_or(&[]);
        if let Some((kennung, _)) = lies_kennung(roh, 0)
            && entfernte_kennungen.contains(&kennung)
        {
            aus.push((e.anfang, e.ende, Tat::Leerraum));
        }
    }

    // Eine Prüfsumme über geänderte Geschwister ist danach falsch.
    let bisher = aus.clone();
    for (i, e) in baum.elemente.iter().enumerate() {
        if e.kennung != CRC32 {
            continue;
        }
        let Some(eltern) = e.eltern.and_then(|k| baum.elemente.get(k)) else {
            continue;
        };
        let innerhalb = |a: usize, b: usize| a >= eltern.inhalt && b <= eltern.ende;
        let betroffen = bisher.iter().any(|(a, b, _)| innerhalb(*a, *b));
        // Steckt die Prüfsumme selbst schon in einem Leerraum, ist nichts
        // mehr zu tun.
        let schon_weg = bisher
            .iter()
            .any(|(a, b, t)| *t == Tat::Leerraum && e.anfang >= *a && e.ende <= *b);
        if betroffen && !schon_weg && baum.elemente.get(i).is_some() {
            aus.push((e.anfang, e.ende, Tat::Leerraum));
        }
    }

    aus
}

/// Entfernt die Metadaten.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputtem EBML-Aufbau.
pub fn strip(daten: &[u8]) -> Result<(Vec<u8>, StripResult)> {
    let baum = sammle(daten)?;
    let entfernt = funde(daten, &baum);
    let kapitel = kapitel_funde(daten, &baum);

    let mut aus = daten.to_vec();
    for (a, b, tat) in taten(daten, &baum) {
        let Some(bereich) = aus.get_mut(a..b) else {
            continue;
        };
        match tat {
            Tat::Leerraum => {
                if !schreib_void(bereich) {
                    // Zu klein für ein Element — dann wenigstens genullt.
                    bereich.fill(0);
                }
            }
            Tat::Entleeren => bereich.fill(0),
        }
    }

    debug_assert_eq!(
        aus.len(),
        daten.len(),
        "in Matroska darf sich nichts verschieben"
    );

    let mut reste = kapitel;
    let mut gruende: Vec<&str> = Vec::new();
    if !reste.is_empty() {
        gruende.push(
            "Kapitelnamen sind Navigation, also Inhalt — sie werden gemeldet, nicht entfernt",
        );
    }
    if baum.unklar {
        gruende.push(
            "die Datei enthält Abschnitte ohne Längenangabe; dahinter ist der Aufbau \
             nicht mehr sicher zu verfolgen",
        );
        reste.push(fund(
            FindingKind::UnknownExtension,
            "EBML",
            Some("Abschnitt ohne Längenangabe — nicht weiter verfolgt".to_owned()),
            Severity::Notable,
        ));
    }

    if reste.is_empty() {
        Ok((aus, StripResult::Complete { removed: entfernt }))
    } else {
        Ok((
            aus,
            StripResult::Partial {
                removed: entfernt,
                remaining: reste,
                reason: gruende.join("; "),
            },
        ))
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

    /// Baut ein EBML-Element mit kürzestmöglicher Längenangabe.
    fn el(kennung: &[u8], inhalt: &[u8]) -> Vec<u8> {
        let mut v = kennung.to_vec();
        let n = inhalt.len() as u64;

        // So viele Bytes, wie die Länge braucht -- der Wert aus lauter
        // Einsen ist als "unbekannt" belegt und scheidet aus.
        let breite = (1..=8usize)
            .find(|b| n < maske_fuer(*b).unwrap())
            .expect("Testelement zu gross");
        let roh = ((maske_fuer(breite).unwrap() + 1) | n).to_be_bytes();
        v.extend_from_slice(&roh[8 - breite..]);
        v.extend_from_slice(inhalt);
        v
    }

    fn zusammen(teile: &[Vec<u8>]) -> Vec<u8> {
        teile.iter().flatten().copied().collect()
    }

    /// Eine kleine, aber vollständige Datei mit allem, worum es geht.
    fn beispiel() -> Vec<u8> {
        let kopf = el(&[0x1A, 0x45, 0xDF, 0xA3], &el(&[0x42, 0x82], b"matroska"));

        let info = el(
            &[0x15, 0x49, 0xA9, 0x66],
            &zusammen(&[
                el(&[0x2A, 0xD7, 0xB1], &[0x0F, 0x42, 0x40]),
                el(&[0x73, 0xA4], &[7u8; 16]),
                el(&[0x73, 0x84], b"Rohschnitt_Anna_final.mkv"),
                el(&[0x44, 0x61], &[0u8; 8]),
                el(&[0x7B, 0xA9], b"Angebot Nordstern"),
                el(&[0x4D, 0x80], b"libebml v1.4.4"),
                el(&[0x57, 0x41], b"Bearbeitungsprogramm 3.1"),
            ]),
        );

        let tags = el(
            &[0x12, 0x54, 0xC3, 0x67],
            &el(
                &[0x73, 0x73],
                &el(
                    &[0x67, 0xC8],
                    &zusammen(&[
                        el(&[0x45, 0xA3], b"ARTIST"),
                        el(&[0x44, 0x87], b"Dr. Anna Beispiel"),
                    ]),
                ),
            ),
        );

        let segment = el(&[0x18, 0x53, 0x80, 0x67], &zusammen(&[info, tags]));
        zusammen(&[kopf, segment])
    }

    #[test]
    fn matroska_wird_erkannt() {
        assert!(looks_like_matroska(&beispiel()));
        assert!(!looks_like_matroska(b"nicht matroska"));
        assert!(!looks_like_matroska(&[]));
    }

    #[test]
    fn der_urspruengliche_dateiname_ist_ein_kritischer_fund() {
        let i = inspect(&beispiel()).unwrap();
        assert!(i.understood);
        let f = i
            .findings
            .iter()
            .find(|f| f.kind == FindingKind::FileName)
            .expect("der Dateiname wurde nicht gefunden");
        assert_eq!(f.severity, Severity::Critical);
        assert!(
            f.value
                .as_deref()
                .unwrap()
                .contains("Rohschnitt_Anna_final")
        );
    }

    #[test]
    fn die_marken_werden_gelesen() {
        let i = inspect(&beispiel()).unwrap();
        let f = i
            .findings
            .iter()
            .find(|f| f.location == "Matroska:Tags/ARTIST")
            .expect("ARTIST fehlt");
        assert_eq!(f.kind, FindingKind::Author);
        assert_eq!(f.severity, Severity::Critical);
        assert_eq!(f.value.as_deref(), Some("Dr. Anna Beispiel"));
    }

    #[test]
    fn webm_und_mkv_werden_unterschieden() {
        let mkv = beispiel();
        assert_eq!(
            inspect(&mkv).unwrap().format.as_deref(),
            Some("Matroska (MKV)")
        );

        let webm = zusammen(&[
            el(&[0x1A, 0x45, 0xDF, 0xA3], &el(&[0x42, 0x82], b"webm")),
            el(&[0x18, 0x53, 0x80, 0x67], &[]),
        ]);
        assert_eq!(inspect(&webm).unwrap().format.as_deref(), Some("WebM"));
    }

    /// **Der Kern des Verfahrens.** `SeekHead` und `Cues` führen absolute
    /// Byte-Positionen; verschiebt sich etwas, zeigen sie ins Leere.
    #[test]
    fn die_dateilaenge_bleibt_unveraendert() {
        let vorher = beispiel();
        let (nachher, _) = strip(&vorher).unwrap();
        assert_eq!(nachher.len(), vorher.len());
    }

    #[test]
    fn nach_dem_bereinigen_ist_nichts_mehr_lesbar() {
        let (sauber, ergebnis) = strip(&beispiel()).unwrap();

        for spur in [
            &b"Rohschnitt_Anna_final"[..],
            b"Dr. Anna Beispiel",
            b"Angebot Nordstern",
            b"Bearbeitungsprogramm",
            b"libebml",
        ] {
            assert!(
                !sauber.windows(spur.len()).any(|f| f == spur),
                "noch lesbar: {}",
                String::from_utf8_lossy(spur)
            );
        }
        assert!(matches!(ergebnis, StripResult::Complete { .. }));
        assert!(inspect(&sauber).unwrap().findings.is_empty());
    }

    /// Die Pflichtelemente bleiben stehen, nur ihr Inhalt verschwindet.
    /// Ein `Void` an ihrer Stelle ergäbe eine formal fehlerhafte Datei.
    #[test]
    fn pflichtelemente_bleiben_stehen() {
        let (sauber, _) = strip(&beispiel()).unwrap();
        let baum = sammle(&sauber).unwrap();

        for pflicht in [MUXING_APP, WRITING_APP] {
            let e = baum
                .elemente
                .iter()
                .find(|e| e.kennung == pflicht)
                .expect("Pflichtelement wurde entfernt");
            assert!(text(&sauber, e).is_empty(), "der Inhalt blieb stehen");
        }
    }

    /// Das `Void` muss jede Größe annehmen können — genau dafür erlaubt EBML
    /// die überlange Längenangabe.
    #[test]
    fn ein_void_passt_auf_jede_groesse() {
        for groesse in 2usize..600 {
            let mut puffer = vec![0xAAu8; groesse];
            assert!(schreib_void(&mut puffer), "Groesse {groesse} scheiterte");
            assert_eq!(puffer.len(), groesse);

            let (kennung, k) = lies_kennung(&puffer, 0).unwrap();
            assert_eq!(kennung, u32::from(VOID));
            let (laenge, l) = lies_laenge(&puffer, k).unwrap();
            assert_eq!(
                k + l + usize::try_from(laenge.unwrap()).unwrap(),
                groesse,
                "das Void deckt den Bereich nicht genau ab"
            );
        }
        // Unter zwei Bytes geht es nicht, und das wird gemeldet.
        assert!(!schreib_void(&mut [0u8; 1]));
    }

    #[test]
    fn ein_zweiter_durchlauf_aendert_nichts() {
        let (einmal, _) = strip(&beispiel()).unwrap();
        let (zweimal, _) = strip(&einmal).unwrap();
        assert_eq!(einmal, zweimal);
    }

    /// Ein Anhang ist eine **vollständige zweite Datei** im Video. Er wiegt
    /// schwerer als jede Marke und muss samt Inhalt verschwinden.
    #[test]
    fn ein_anhang_ist_ein_kritischer_fund_und_faellt_ganz_weg() {
        let anhang = el(
            &[0x19, 0x41, 0xA4, 0x69],
            &el(
                &[0x61, 0xA7],
                &zusammen(&[
                    el(&[0x46, 0x6E], b"Vertragsentwurf.pdf"),
                    el(&[0x46, 0x5C], b"%PDF-1.4 hier stuende das ganze Dokument"),
                ]),
            ),
        );
        let datei = zusammen(&[
            el(&[0x1A, 0x45, 0xDF, 0xA3], &[]),
            el(&[0x18, 0x53, 0x80, 0x67], &anhang),
        ]);

        let f = inspect(&datei)
            .unwrap()
            .findings
            .into_iter()
            .find(|f| f.location == "Matroska:Attachments")
            .expect("der Anhang wurde nicht gefunden");
        assert_eq!(f.severity, Severity::Critical);
        assert!(f.value.as_deref().unwrap().contains("Vertragsentwurf.pdf"));

        let (sauber, _) = strip(&datei).unwrap();
        assert_eq!(sauber.len(), datei.len());
        for spur in [&b"Vertragsentwurf"[..], b"hier stuende das ganze Dokument"] {
            assert!(
                !sauber.windows(spur.len()).any(|f| f == spur),
                "der Anhang steht noch in der Datei"
            );
        }
    }

    /// **Feinheit zwei.** Der `SeekHead` ist ein Verzeichnis der Form „Tags
    /// stehen bei Byte 4711". Bliebe der Eintrag stehen, wäre weiterhin
    /// verzeichnet, dass es einmal Tags gab — auch wenn dort nur noch
    /// Leerraum liegt.
    #[test]
    fn der_seekhead_verrraet_die_entfernten_abschnitte_nicht() {
        let seek = el(
            &[0x4D, 0xBB],
            &zusammen(&[
                // SeekID zeigt auf Tags ...
                el(&[0x53, 0xAB], &[0x12, 0x54, 0xC3, 0x67]),
                // ... an dieser Stelle.
                el(&[0x53, 0xAC], &[0x00, 0x2A]),
            ]),
        );
        let seekhead = el(&[0x11, 0x4D, 0x9B, 0x74], &seek);
        let tags = el(
            &[0x12, 0x54, 0xC3, 0x67],
            &el(
                &[0x73, 0x73],
                &el(
                    &[0x67, 0xC8],
                    &zusammen(&[
                        el(&[0x45, 0xA3], b"ARTIST"),
                        el(&[0x44, 0x87], b"Dr. Anna Beispiel"),
                    ]),
                ),
            ),
        );
        let datei = zusammen(&[
            el(&[0x1A, 0x45, 0xDF, 0xA3], &[]),
            el(&[0x18, 0x53, 0x80, 0x67], &zusammen(&[seekhead, tags])),
        ]);

        let (sauber, _) = strip(&datei).unwrap();
        let baum = sammle(&sauber).unwrap();

        assert!(
            !baum.elemente.iter().any(|e| e.kennung == SEEK),
            "der Seek-Eintrag auf die Tags blieb stehen"
        );
        assert!(
            baum.elemente.iter().any(|e| e.kennung == SEEK_HEAD),
            "der SeekHead selbst darf bleiben"
        );
    }

    /// Kapitel sind Inhalt. Sie werden gemeldet und bleiben stehen — das
    /// Ergebnis ist deshalb `Partial`, nicht `Complete`.
    #[test]
    fn kapitel_bleiben_und_machen_das_ergebnis_teilweise() {
        let kapitel = el(
            &[0x10, 0x43, 0xA7, 0x70],
            &el(
                &[0x45, 0xB9],
                &el(
                    &[0xB6],
                    &el(&[0x80], &el(&[0x85], b"Gespraech mit der Quelle")),
                ),
            ),
        );
        let datei = zusammen(&[
            el(&[0x1A, 0x45, 0xDF, 0xA3], &el(&[0x42, 0x82], b"matroska")),
            el(&[0x18, 0x53, 0x80, 0x67], &kapitel),
        ]);

        let (sauber, ergebnis) = strip(&datei).unwrap();
        let StripResult::Partial { remaining, .. } = ergebnis else {
            panic!("Kapitel muessen als Rest gemeldet werden");
        };
        assert!(remaining.iter().any(|f| f.location == "Matroska:Chapters"));
        assert!(
            sauber.windows(9).any(|f| f == b"Gespraech"),
            "der Kapitelname wurde entfernt, obwohl er Inhalt ist"
        );
    }

    /// Der Cluster enthält die Bilddaten. Dort wird nicht abgestiegen — sonst
    /// würde jedes Bild einzeln durchlaufen.
    #[test]
    fn in_den_cluster_wird_nicht_abgestiegen() {
        let cluster = el(&[0x1F, 0x43, 0xB6, 0x75], &el(&[0x7B, 0xA9], b"kein Titel"));
        let datei = zusammen(&[
            el(&[0x1A, 0x45, 0xDF, 0xA3], &[]),
            el(&[0x18, 0x53, 0x80, 0x67], &cluster),
        ]);

        let i = inspect(&datei).unwrap();
        assert!(
            i.findings.is_empty(),
            "im Cluster wurde gesucht: {:?}",
            i.findings
        );
        let (sauber, _) = strip(&datei).unwrap();
        assert_eq!(sauber, datei, "der Cluster wurde angetastet");
    }

    #[test]
    fn kaputtes_ebml_ist_ein_fehler() {
        // Ein Element, das über seinen Bereich hinausreicht.
        let datei = zusammen(&[
            el(&[0x1A, 0x45, 0xDF, 0xA3], &[]),
            vec![0x18, 0x53, 0x80, 0x67, 0xFE],
        ]);
        assert!(inspect(&datei).is_err());
    }
}
