//! Ogg mit Vorbis, Opus und Speex (`spec/metadata.md` §4).
//!
//! **Der einzige Fall, in dem gerechnet werden muss.** Alle anderen Formate
//! ließen sich mit einem Platzhalter erledigen oder mit einem Schnitt. Ogg
//! nicht: Jede Seite trägt eine **CRC-Prüfsumme über sich selbst**. Wer auch
//! nur ein Byte ändert, macht sie falsch, und ein ordentlicher Leser wirft die
//! Seite dann weg.
//!
//! # Wie eine Ogg-Datei aufgebaut ist
//!
//! Der Tonstrom besteht aus **Paketen**, die Datei aus **Seiten**. Beides
//! deckt sich nicht: Ein Paket darf über mehrere Seiten laufen, eine Seite
//! mehrere Pakete tragen. Welche Bytes zu welchem Paket gehören, steht in der
//! **Segmenttabelle** im Seitenkopf.
//!
//! Bei Vorbis, Opus und Speex ist das **zweite Paket** der Kommentarblock.
//! Es zu ersetzen heißt: die Seiten, die es tragen, neu aufteilen, neu
//! nummerieren und neu prüfsummen.
//!
//! # Was dabei erhalten bleiben muss
//!
//! - **Die Seitennummern** laufen je Datenstrom fortlaufend. Werden aus zwei
//!   Kopfseiten eine, müssen alle folgenden Seiten dieses Stroms um eins
//!   heruntergezählt werden — sonst meldet der Leser eine Lücke.
//! - **Die Granulatposition** sagt, wie weit der Ton auf dieser Seite
//!   fortgeschritten ist. Kopfseiten führen dort null.
//! - **Das Rahmenbit** am Ende des Kommentarblocks verlangt **Vorbis**.
//!   Opus und Speex kennen es nicht. Es zu vergessen macht eine Vorbis-Datei
//!   unlesbar, es fälschlich zu setzen eine Opus-Datei.
//!
//! # Wo die Grenze verläuft
//!
//! Verstanden werden Vorbis, Opus und Speex. Ein Ogg mit Theora-Video oder
//! mehreren verschachtelten Strömen wird **gemeldet, aber nicht angetastet**
//! — das Ergebnis ist dann `Partial`. Eine Datei halb umzuschreiben wäre
//! schlimmer, als sie ehrlich stehen zu lassen.

use crate::model::{Finding, FindingKind, Inspection, Severity, StripResult};
use crate::vorbis;

use cabrik_core::{Error, Result};

/// Höchstzahl der Seiten, die verfolgt werden.
const MAX_SEITEN: usize = 1_000_000;
/// Größte Zahl von Segmenten je Seite — vom Format so festgelegt.
const MAX_SEGMENTE: usize = 255;

/// Ob die Bytes wie eine Ogg-Datei aussehen.
#[must_use]
pub fn looks_like_ogg(daten: &[u8]) -> bool {
    daten.starts_with(b"OggS") && daten.get(4) == Some(&0)
}

// ---------------------------------------------------------------------------
// Prüfsumme
// ---------------------------------------------------------------------------

/// Tabelle für die CRC-32 nach Ogg: Polynom `0x04C11DB7`, ohne Spiegelung,
/// Startwert null, kein abschließendes Verodern.
///
/// Das ist **nicht** dieselbe CRC-32 wie in ZIP oder PNG — die spiegeln die
/// Bits. Ein Verwechseln fällt erst auf, wenn ein Abspielprogramm die Seite
/// stillschweigend verwirft.
#[expect(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "Konstante zur Uebersetzungszeit: ein Ueberlauf waere ein Uebersetzungsfehler"
)]
const fn tabelle() -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut i = 0usize;
    while i < 256 {
        let mut r = (i as u32) << 24;
        let mut j = 0;
        while j < 8 {
            r = if r & 0x8000_0000 != 0 {
                (r << 1) ^ 0x04C1_1DB7
            } else {
                r << 1
            };
            j += 1;
        }
        t[i] = r;
        i += 1;
    }
    t
}

static CRC_TABELLE: [u32; 256] = tabelle();

fn crc(daten: &[u8]) -> u32 {
    let mut r: u32 = 0;
    for b in daten {
        let i = usize::from((r >> 24) as u8 ^ *b);
        r = (r << 8) ^ CRC_TABELLE.get(i).copied().unwrap_or(0);
    }
    r
}

// ---------------------------------------------------------------------------
// Seiten lesen
// ---------------------------------------------------------------------------

/// Merkbit: Diese Seite beginnt mit der Fortsetzung eines Pakets.
const FORTSETZUNG: u8 = 0x01;
/// Merkbit: erste Seite des Stroms.
const ANFANG: u8 = 0x02;
/// Merkbit: letzte Seite des Stroms.
const ENDE: u8 = 0x04;

#[derive(Debug, Clone)]
struct Seite {
    art: u8,
    granule: i64,
    serial: u32,
    folge: u32,
    anfang: usize,
    /// Anfang der Nutzdaten, also hinter der Segmenttabelle.
    nutz_von: usize,
    ende: usize,
    lacing: Vec<u8>,
}

fn u32_le(daten: &[u8], p: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        daten.get(p..p.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn sammle(daten: &[u8]) -> Result<Vec<Seite>> {
    let mut aus = Vec::new();
    let mut p = 0usize;

    while daten.get(p..p.saturating_add(4)) == Some(b"OggS") {
        if aus.len() >= MAX_SEITEN {
            return Err(Error::Malformed("ogg: zu viele Seiten"));
        }
        let art = *daten
            .get(p.saturating_add(5))
            .ok_or(Error::Malformed("ogg: Seitenkopf unvollstaendig"))?;
        let granule = daten
            .get(p.saturating_add(6)..p.saturating_add(14))
            .and_then(|b| Some(i64::from_le_bytes(b.try_into().ok()?)))
            .ok_or(Error::Malformed("ogg: Granulatposition unlesbar"))?;
        let serial =
            u32_le(daten, p.saturating_add(14)).ok_or(Error::Malformed("ogg: Kennung unlesbar"))?;
        let folge = u32_le(daten, p.saturating_add(18))
            .ok_or(Error::Malformed("ogg: Seitennummer unlesbar"))?;
        let anzahl = usize::from(
            *daten
                .get(p.saturating_add(26))
                .ok_or(Error::Malformed("ogg: Segmentzahl fehlt"))?,
        );

        let tab_von = p.saturating_add(27);
        let tab_bis = tab_von
            .checked_add(anzahl)
            .ok_or(Error::Malformed("ogg: Segmenttabelle ueberlaeuft"))?;
        let lacing = daten
            .get(tab_von..tab_bis)
            .ok_or(Error::Malformed("ogg: Segmenttabelle unvollstaendig"))?
            .to_vec();

        let nutz: usize = lacing.iter().map(|v| usize::from(*v)).sum();
        let ende = tab_bis
            .checked_add(nutz)
            .ok_or(Error::Malformed("ogg: Seitenende ueberlaeuft"))?;
        if ende > daten.len() {
            return Err(Error::Malformed("ogg: Seite reicht ueber das Dateiende"));
        }

        aus.push(Seite {
            art,
            granule,
            serial,
            folge,
            anfang: p,
            nutz_von: tab_bis,
            ende,
            lacing,
        });
        p = ende;
    }

    if aus.is_empty() {
        return Err(Error::Malformed("ogg: keine einzige Seite"));
    }
    Ok(aus)
}

/// Setzt die Pakete einer Seitenfolge wieder zusammen.
///
/// Gibt zusätzlich zurück, ob das letzte Paket **unvollständig** ist, also
/// über die letzte betrachtete Seite hinausläuft.
fn pakete(daten: &[u8], seiten: &[&Seite]) -> (Vec<Vec<u8>>, bool) {
    let mut aus: Vec<Vec<u8>> = Vec::new();
    let mut puffer: Vec<u8> = Vec::new();
    let mut offen = false;

    for s in seiten {
        let mut p = s.nutz_von;
        for v in &s.lacing {
            let n = usize::from(*v);
            let bis = p.saturating_add(n);
            puffer.extend_from_slice(daten.get(p..bis).unwrap_or(&[]));
            p = bis;
            if n < MAX_SEGMENTE {
                aus.push(core::mem::take(&mut puffer));
                offen = false;
            } else {
                offen = true;
            }
        }
    }
    if !puffer.is_empty() {
        aus.push(puffer);
        offen = true;
    }
    (aus, offen)
}

// ---------------------------------------------------------------------------
// Stromart
// ---------------------------------------------------------------------------

/// Welche Art von Datenstrom in der Datei steckt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Art {
    Vorbis,
    Opus,
    Speex,
    /// Etwas anderes — Theora etwa. Wird nicht angetastet.
    Fremd,
}

impl Art {
    /// Erkennt die Art am ersten Paket.
    fn erkenne(erstes: &[u8]) -> Self {
        if erstes.starts_with(b"\x01vorbis") {
            Self::Vorbis
        } else if erstes.starts_with(b"OpusHead") {
            Self::Opus
        } else if erstes.starts_with(b"Speex   ") {
            Self::Speex
        } else {
            Self::Fremd
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Vorbis => "Ogg Vorbis",
            Self::Opus => "Opus",
            Self::Speex => "Speex",
            Self::Fremd => "Ogg (unbekannter Inhalt)",
        }
    }

    /// Vorsilbe des Kommentarpakets.
    const fn vorsilbe(self) -> &'static [u8] {
        match self {
            Self::Vorbis => b"\x03vorbis",
            Self::Opus => b"OpusTags",
            // Speex trägt keine Vorsilbe: Das zweite Paket ist unmittelbar
            // der Kommentarblock.
            Self::Speex | Self::Fremd => b"",
        }
    }

    /// **Nur Vorbis** schließt den Kommentarblock mit einem Rahmenbit ab.
    const fn rahmenbit(self) -> bool {
        matches!(self, Self::Vorbis)
    }
}

/// Der Kommentarblock: die Nummer des Pakets und sein reiner Inhalt.
struct Kommentarpaket {
    nummer: usize,
    inhalt: Vec<u8>,
}

fn finde_kommentar(art: Art, pakete: &[Vec<u8>]) -> Option<Kommentarpaket> {
    // Bei allen drei Arten ist es das zweite Paket.
    let roh = pakete.get(1)?;
    let vorsilbe = art.vorsilbe();
    if !roh.starts_with(vorsilbe) {
        return None;
    }
    Some(Kommentarpaket {
        nummer: 1,
        inhalt: roh.get(vorsilbe.len()..)?.to_vec(),
    })
}

// ---------------------------------------------------------------------------
// Untersuchen
// ---------------------------------------------------------------------------

/// Was über die Datei bekannt ist.
struct Lage {
    art: Art,
    serial: u32,
    /// Die Kopfseiten des ersten Stroms.
    kopf_bis: usize,
    pakete: Vec<Vec<u8>>,
    /// Ob ein Paket über die Kopfseiten hinausläuft.
    offen: bool,
    /// Ob die Datei mehr als einen Datenstrom führt.
    mehrere: bool,
}

fn lage(daten: &[u8], seiten: &[Seite]) -> Result<Lage> {
    let erste = seiten
        .first()
        .ok_or(Error::Malformed("ogg: keine erste Seite"))?;
    let serial = erste.serial;
    let mehrere = seiten.iter().any(|s| s.serial != serial);

    // Kopfseiten sind die führenden Seiten dieses Stroms mit Granulat null.
    let mut kopf: Vec<&Seite> = Vec::new();
    let mut kopf_bis = 0usize;
    for (i, s) in seiten.iter().enumerate() {
        if s.serial != serial {
            continue;
        }
        if s.granule != 0 {
            break;
        }
        kopf.push(s);
        kopf_bis = i.saturating_add(1);
    }

    let (pakete, offen) = pakete(daten, &kopf);
    let art = pakete.first().map_or(Art::Fremd, |p| Art::erkenne(p));

    Ok(Lage {
        art,
        serial,
        kopf_bis,
        pakete,
        offen,
        mehrere,
    })
}

fn funde(l: &Lage) -> Vec<Finding> {
    let Some(k) = finde_kommentar(l.art, &l.pakete) else {
        return Vec::new();
    };
    match vorbis::lies(&k.inhalt) {
        Some(kom) => vorbis::funde(&kom, "Ogg:Kommentar"),
        None => vec![Finding {
            kind: FindingKind::UnknownExtension,
            location: "Ogg:Kommentar".to_owned(),
            value: Some("Kommentarblock nicht lesbar".to_owned()),
            severity: Severity::Notable,
        }],
    }
}

/// Zeigt die Marken an, ohne die Datei zu verändern.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputtem Seitenaufbau.
pub fn inspect(daten: &[u8]) -> Result<Inspection> {
    let seiten = sammle(daten)?;
    let l = lage(daten, &seiten)?;

    Ok(Inspection {
        format: Some(l.art.name().to_owned()),
        findings: funde(&l),
        understood: true,
    })
}

// ---------------------------------------------------------------------------
// Seiten schreiben
// ---------------------------------------------------------------------------

/// Schreibt eine Folge von Paketen als Ogg-Seiten.
///
/// Die Aufteilung folgt der Norm: Ein Paket zerfällt in Segmente zu 255
/// Bytes, gefolgt von einem kürzeren — und ist die Länge genau ein Vielfaches
/// von 255, gehört ein Segment der Länge null dazu. Ohne dieses letzte
/// Nullsegment wüsste ein Leser nicht, dass das Paket zu Ende ist.
fn schreibe_seiten(
    pakete: &[Vec<u8>],
    serial: u32,
    erste_folge: u32,
    anfangsbit: bool,
    endbit: bool,
    letztes_granulat: i64,
) -> Vec<u8> {
    // Segmenttabelle und Nutzdaten, dazu die Anfänge der Pakete.
    let mut lacing: Vec<u8> = Vec::new();
    let mut nutz: Vec<u8> = Vec::new();
    let mut anfaenge: Vec<usize> = Vec::new();

    for p in pakete {
        anfaenge.push(lacing.len());
        let mut rest = p.len();
        loop {
            let n = rest.min(MAX_SEGMENTE);
            lacing.push(u8::try_from(n).unwrap_or(0));
            rest = rest.saturating_sub(n);
            if n < MAX_SEGMENTE {
                break;
            }
        }
        nutz.extend_from_slice(p);
    }

    let mut aus: Vec<u8> = Vec::new();
    let mut i = 0usize;
    let mut nutz_p = 0usize;
    let mut folge = erste_folge;
    let seitenzahl = lacing.len().div_ceil(MAX_SEGMENTE).max(1);
    let mut geschrieben = 0usize;

    while i < lacing.len() || geschrieben == 0 {
        let bis = i.saturating_add(MAX_SEGMENTE).min(lacing.len());
        let teil = lacing.get(i..bis).unwrap_or(&[]);
        let laenge: usize = teil.iter().map(|v| usize::from(*v)).sum();
        let letzte = bis >= lacing.len();

        let mut art = 0u8;
        if anfangsbit && geschrieben == 0 {
            art |= ANFANG;
        }
        if endbit && letzte {
            art |= ENDE;
        }
        // Beginnt die Seite mitten in einem Paket?
        if !anfaenge.contains(&i) && i != 0 {
            art |= FORTSETZUNG;
        }

        let kopf_anfang = aus.len();
        aus.extend_from_slice(b"OggS");
        aus.push(0);
        aus.push(art);
        // Nur die letzte Kopfseite trägt das Granulat, alle früheren null.
        let granulat = if letzte { letztes_granulat } else { 0 };
        aus.extend_from_slice(&granulat.to_le_bytes());
        aus.extend_from_slice(&serial.to_le_bytes());
        aus.extend_from_slice(&folge.to_le_bytes());
        // Platz für die Prüfsumme; sie wird über die Seite mit Nullen an
        // dieser Stelle gerechnet und danach eingetragen.
        aus.extend_from_slice(&0u32.to_le_bytes());
        aus.push(u8::try_from(teil.len()).unwrap_or(0));
        aus.extend_from_slice(teil);
        aus.extend_from_slice(
            nutz.get(nutz_p..nutz_p.saturating_add(laenge))
                .unwrap_or(&[]),
        );

        let summe = crc(aus.get(kopf_anfang..).unwrap_or(&[]));
        if let Some(feld) =
            aus.get_mut(kopf_anfang.saturating_add(22)..kopf_anfang.saturating_add(26))
        {
            feld.copy_from_slice(&summe.to_le_bytes());
        }

        nutz_p = nutz_p.saturating_add(laenge);
        i = bis;
        folge = folge.saturating_add(1);
        geschrieben = geschrieben.saturating_add(1);
        if geschrieben >= seitenzahl && i >= lacing.len() {
            break;
        }
    }
    aus
}

/// Schreibt eine unveränderte Seite mit neuer Nummer und neuer Prüfsumme.
fn seite_umnummerieren(daten: &[u8], s: &Seite, neue_folge: u32) -> Vec<u8> {
    let mut roh = daten.get(s.anfang..s.ende).unwrap_or(&[]).to_vec();
    if let Some(f) = roh.get_mut(18..22) {
        f.copy_from_slice(&neue_folge.to_le_bytes());
    }
    if let Some(f) = roh.get_mut(22..26) {
        f.copy_from_slice(&0u32.to_le_bytes());
    }
    let summe = crc(&roh);
    if let Some(f) = roh.get_mut(22..26) {
        f.copy_from_slice(&summe.to_le_bytes());
    }
    roh
}

/// Entfernt die Marken.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputtem Seitenaufbau.
pub fn strip(daten: &[u8]) -> Result<(Vec<u8>, StripResult)> {
    let seiten = sammle(daten)?;
    let l = lage(daten, &seiten)?;
    let entfernt = funde(&l);

    // Drei Gründe, die Finger davon zu lassen — jeder für sich genügt.
    let hindernis = if l.art == Art::Fremd {
        Some("der Inhalt dieses Ogg-Stroms ist diesem Programm unbekannt")
    } else if l.mehrere {
        Some(
            "die Datei führt mehrere Datenströme; ein halbes Umschreiben wäre schlimmer als gar keines",
        )
    } else if l.offen {
        Some("ein Paket läuft über die Kopfseiten hinaus")
    } else if finde_kommentar(l.art, &l.pakete).is_none() {
        Some("das erwartete Kommentarpaket ist nicht auffindbar")
    } else {
        None
    };

    if let Some(grund) = hindernis {
        return Ok((
            daten.to_vec(),
            StripResult::Partial {
                removed: Vec::new(),
                remaining: if entfernt.is_empty() {
                    vec![Finding {
                        kind: FindingKind::UnknownExtension,
                        location: "Ogg".to_owned(),
                        value: Some(grund.to_owned()),
                        severity: Severity::Notable,
                    }]
                } else {
                    entfernt
                },
                reason: grund.to_owned(),
            },
        ));
    }

    let Some(k) = finde_kommentar(l.art, &l.pakete) else {
        return Err(Error::Malformed("ogg: Kommentarpaket verschwunden"));
    };

    // Das Kommentarpaket durch ein leeres ersetzen, alle anderen behalten.
    let mut neue: Vec<Vec<u8>> = l.pakete.clone();
    let mut leer = l.art.vorsilbe().to_vec();
    leer.extend_from_slice(&vorbis::leer(l.art.rahmenbit()));
    if let Some(platz) = neue.get_mut(k.nummer) {
        *platz = leer;
    }

    let kopfseiten: Vec<&Seite> = seiten
        .iter()
        .take(l.kopf_bis)
        .filter(|s| s.serial == l.serial)
        .collect();
    let alte_anzahl = kopfseiten.len();
    let erste = kopfseiten
        .first()
        .ok_or(Error::Malformed("ogg: keine Kopfseite"))?;
    let letzte = kopfseiten
        .last()
        .ok_or(Error::Malformed("ogg: keine Kopfseite"))?;

    // **Das Identifikationspaket muss allein auf der ersten Seite stehen.**
    // Vorbis I §4.2 schreibt es vor, RFC 7845 §3 für Opus ebenso. Alle
    // Kopfpakete in eine Seite zu packen ergibt eine Datei, die ffmpeg noch
    // abspielt und andere Leser nicht mehr verstehen: Sie suchen die Seite,
    // deren **erstes** Paket der Kommentarblock ist, finden sie nicht und
    // lesen stattdessen die Tondaten als Kommentar.
    let (Some(ident), Some(rest)) = (neue.first(), neue.get(1..)) else {
        return Err(Error::Malformed("ogg: zu wenige Kopfpakete"));
    };

    let mut aus = schreibe_seiten(
        core::slice::from_ref(ident),
        l.serial,
        erste.folge,
        erste.art & ANFANG != 0,
        false,
        0,
    );
    let nach_ident = erste
        .folge
        .saturating_add(u32::try_from(sammle(&aus)?.len()).unwrap_or(1));

    if !rest.is_empty() {
        aus.extend_from_slice(&schreibe_seiten(
            rest,
            l.serial,
            nach_ident,
            false,
            letzte.art & ENDE != 0,
            letzte.granule,
        ));
    }

    // Wie viele Seiten sind daraus geworden? Danach richtet sich, ob die
    // folgenden Seiten umnummeriert werden müssen.
    let neue_anzahl = sammle(&aus)?.len();

    let folge_versatz: i64 = i64::try_from(neue_anzahl)
        .unwrap_or(0)
        .saturating_sub(i64::try_from(alte_anzahl).unwrap_or(0));

    for s in seiten.iter().skip(l.kopf_bis) {
        if s.serial == l.serial && folge_versatz != 0 {
            let neu = i64::from(s.folge).saturating_add(folge_versatz);
            aus.extend_from_slice(&seite_umnummerieren(
                daten,
                s,
                u32::try_from(neu).unwrap_or(s.folge),
            ));
        } else {
            aus.extend_from_slice(daten.get(s.anfang..s.ende).unwrap_or(&[]));
        }
    }

    Ok((aus, StripResult::Complete { removed: entfernt }))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "Tests duerfen laut werden"
)]
mod tests {
    use super::*;

    fn kommentarblock(hersteller: &str, eintraege: &[&str], rahmenbit: bool) -> Vec<u8> {
        let mut v = u32::try_from(hersteller.len())
            .unwrap()
            .to_le_bytes()
            .to_vec();
        v.extend_from_slice(hersteller.as_bytes());
        v.extend_from_slice(&u32::try_from(eintraege.len()).unwrap().to_le_bytes());
        for e in eintraege {
            v.extend_from_slice(&u32::try_from(e.len()).unwrap().to_le_bytes());
            v.extend_from_slice(e.as_bytes());
        }
        if rahmenbit {
            v.push(0x01);
        }
        v
    }

    fn vorbis_datei() -> Vec<u8> {
        let ident = [b"\x01vorbis".to_vec(), vec![0u8; 23]].concat();
        let kommentar = [
            b"\x03vorbis".to_vec(),
            kommentarblock(
                "Xiph.Org libVorbis",
                &[
                    "ARTIST=Dr. Anna Beispiel",
                    "TITLE=Angebot Nordstern",
                    "DESCRIPTION=Nicht an den Kunden geben",
                ],
                true,
            ),
        ]
        .concat();
        let setup = [b"\x05vorbis".to_vec(), vec![0xABu8; 300]].concat();
        let ton = vec![0x42u8; 120];

        let mut aus = schreibe_seiten(&[ident], 0xDEAD_BEEF, 0, true, false, 0);
        aus.extend_from_slice(&schreibe_seiten(
            &[kommentar, setup],
            0xDEAD_BEEF,
            1,
            false,
            false,
            0,
        ));
        aus.extend_from_slice(&schreibe_seiten(&[ton], 0xDEAD_BEEF, 2, false, true, 4096));
        aus
    }

    #[test]
    fn ogg_wird_erkannt() {
        assert!(looks_like_ogg(&vorbis_datei()));
        assert!(!looks_like_ogg(b"OggT irgendwas"));
        assert!(!looks_like_ogg(b""));
    }

    /// **Die Prüfsumme.** Sie ist nicht dieselbe wie in ZIP oder PNG. Ohne
    /// festen Prüfwert bliebe ein Verwechseln der Spielart unbemerkt, bis ein
    /// Abspielprogramm die Seite stillschweigend verwirft.
    #[test]
    fn die_pruefsumme_folgt_der_ogg_spielart() {
        // Bekannte Werte für dieses Polynom, ohne Spiegelung, Startwert null.
        assert_eq!(crc(b""), 0x0000_0000);
        assert_eq!(crc(b"123456789"), 0x89A1_897F);
    }

    /// Jede geschriebene Seite muss ihre eigene Prüfsumme tragen — sonst
    /// verwirft ein ordentlicher Leser sie.
    #[test]
    fn jede_geschriebene_seite_traegt_eine_gueltige_pruefsumme() {
        let datei = vorbis_datei();
        let seiten = sammle(&datei).unwrap();
        assert_eq!(seiten.len(), 3);

        for s in &seiten {
            let mut roh = datei[s.anfang..s.ende].to_vec();
            let gespeichert = u32::from_le_bytes(roh[22..26].try_into().unwrap());
            roh[22..26].copy_from_slice(&0u32.to_le_bytes());
            assert_eq!(
                crc(&roh),
                gespeichert,
                "Seite {} traegt eine falsche Pruefsumme",
                s.folge
            );
        }
    }

    #[test]
    fn die_kommentare_werden_gelesen() {
        let i = inspect(&vorbis_datei()).unwrap();
        assert!(i.understood);
        assert_eq!(i.format.as_deref(), Some("Ogg Vorbis"));

        let f = i
            .findings
            .iter()
            .find(|f| f.location == "Ogg:Kommentar/ARTIST")
            .expect("ARTIST fehlt");
        assert_eq!(f.kind, FindingKind::Author);
        assert_eq!(f.value.as_deref(), Some("Dr. Anna Beispiel"));
    }

    #[test]
    fn nach_dem_bereinigen_ist_nichts_mehr_lesbar() {
        let (sauber, ergebnis) = strip(&vorbis_datei()).unwrap();
        assert!(matches!(ergebnis, StripResult::Complete { .. }));

        for spur in [
            &b"Dr. Anna Beispiel"[..],
            b"Angebot Nordstern",
            b"Nicht an den Kunden",
            b"Xiph.Org",
        ] {
            assert!(
                !sauber.windows(spur.len()).any(|f| f == spur),
                "noch lesbar: {}",
                String::from_utf8_lossy(spur)
            );
        }
        assert!(inspect(&sauber).unwrap().findings.is_empty());
    }

    /// Alles außer dem Kommentarpaket muss Byte für Byte überleben — vor
    /// allem das Setup-Paket, ohne das sich kein Vorbis dekodieren lässt.
    #[test]
    fn die_uebrigen_pakete_bleiben_unveraendert() {
        let vorher = vorbis_datei();
        let (nachher, _) = strip(&vorher).unwrap();

        let alt = pakete(
            &vorher,
            &sammle(&vorher).unwrap().iter().collect::<Vec<_>>(),
        )
        .0;
        let neu = pakete(
            &nachher,
            &sammle(&nachher).unwrap().iter().collect::<Vec<_>>(),
        )
        .0;

        assert_eq!(
            alt.len(),
            neu.len(),
            "die Zahl der Pakete hat sich geändert"
        );
        assert_eq!(alt[0], neu[0], "das Identifikationspaket wurde verändert");
        assert_eq!(alt[2], neu[2], "das Setup-Paket wurde verändert");
        assert_eq!(alt[3], neu[3], "die Tondaten wurden verändert");
        assert!(neu[1].starts_with(b"\x03vorbis"), "die Vorsilbe fehlt");
    }

    /// **Der Fund dieser Runde.** Vorbis I §4.2 und RFC 7845 §3 verlangen,
    /// dass das Identifikationspaket **allein** auf der ersten Seite steht.
    ///
    /// Ein erster Entwurf packte alle drei Kopfpakete in eine Seite, weil sie
    /// hineinpassten. ffmpeg spielte die Datei weiterhin ab — mutagen nicht:
    /// Es sucht die Seite, deren **erstes** Paket der Kommentarblock ist,
    /// fand keine und las die Tondaten als Kommentar. Der Fehler wäre an den
    /// selbstgebauten Testdateien nie aufgefallen.
    #[test]
    fn das_identifikationspaket_steht_allein_auf_der_ersten_seite() {
        let (sauber, _) = strip(&vorbis_datei()).unwrap();
        let seiten = sammle(&sauber).unwrap();

        let (erste, _) = pakete(&sauber, &[&seiten[0]]);
        assert_eq!(
            erste.len(),
            1,
            "auf der ersten Seite stehen {} Pakete statt einem",
            erste.len()
        );
        assert!(erste[0].starts_with(b"\x01vorbis"));

        // Und der Kommentarblock ist das erste Paket der zweiten Seite —
        // genau danach suchen die Leser.
        let (zweite, _) = pakete(&sauber, &[&seiten[1]]);
        assert!(
            zweite[0].starts_with(b"\x03vorbis"),
            "der Kommentarblock beginnt nicht die zweite Seite"
        );
    }

    /// **Das Rahmenbit.** Vorbis verlangt es, Opus kennt es nicht.
    #[test]
    fn das_rahmenbit_wird_nur_bei_vorbis_gesetzt() {
        let (sauber, _) = strip(&vorbis_datei()).unwrap();
        let p = pakete(
            &sauber,
            &sammle(&sauber).unwrap().iter().collect::<Vec<_>>(),
        )
        .0;
        assert_eq!(
            p[1].last(),
            Some(&0x01),
            "das Rahmenbit fehlt — die Datei waere unlesbar"
        );

        assert!(Art::Vorbis.rahmenbit());
        assert!(!Art::Opus.rahmenbit());
        assert!(!Art::Speex.rahmenbit());
    }

    /// Die Seitennummern müssen lückenlos bleiben, auch wenn aus zwei
    /// Kopfseiten eine wird.
    #[test]
    fn die_seitennummern_bleiben_lueckenlos() {
        let (sauber, _) = strip(&vorbis_datei()).unwrap();
        let seiten = sammle(&sauber).unwrap();

        for (i, s) in seiten.iter().enumerate() {
            assert_eq!(
                s.folge,
                u32::try_from(i).unwrap(),
                "Seite {i} traegt die Nummer {}",
                s.folge
            );
        }
        // Und die Prüfsummen stimmen weiterhin.
        for s in &seiten {
            let mut roh = sauber[s.anfang..s.ende].to_vec();
            let gespeichert = u32::from_le_bytes(roh[22..26].try_into().unwrap());
            roh[22..26].copy_from_slice(&0u32.to_le_bytes());
            assert_eq!(
                crc(&roh),
                gespeichert,
                "Seite {} nach dem Umnummerieren",
                s.folge
            );
        }
    }

    /// Ein Paket von genau 255 Bytes braucht ein Segment der Länge null
    /// hinterher. Ohne das wüsste ein Leser nicht, dass es zu Ende ist.
    #[test]
    fn ein_paket_von_genau_255_bytes_bekommt_ein_nullsegment() {
        let roh = schreibe_seiten(&[vec![0x77u8; 255]], 1, 0, true, true, 0);
        let seiten = sammle(&roh).unwrap();
        assert_eq!(seiten[0].lacing, vec![255, 0]);

        let (p, offen) = pakete(&roh, &seiten.iter().collect::<Vec<_>>());
        assert!(!offen, "das Paket gilt als unvollstaendig");
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].len(), 255);
    }

    /// Ein fremder Inhalt wird gemeldet und **nicht angetastet**.
    #[test]
    fn ein_fremder_strom_bleibt_unveraendert() {
        let theora = [b"\x80theora".to_vec(), vec![0u8; 30]].concat();
        let datei = schreibe_seiten(&[theora], 7, 0, true, true, 0);

        let i = inspect(&datei).unwrap();
        assert_eq!(i.format.as_deref(), Some("Ogg (unbekannter Inhalt)"));

        let (aus, ergebnis) = strip(&datei).unwrap();
        assert_eq!(aus, datei, "ein unbekannter Strom wurde angefasst");
        assert!(!ergebnis.may_show_clean());
    }

    #[test]
    fn ein_zweiter_durchlauf_aendert_nichts() {
        let (einmal, _) = strip(&vorbis_datei()).unwrap();
        let (zweimal, _) = strip(&einmal).unwrap();
        assert_eq!(einmal, zweimal);
    }

    #[test]
    fn eine_seite_ueber_das_dateiende_hinaus_ist_ein_fehler() {
        // Eine einzelne Seite mit 30 Bytes Nutzdaten ...
        let mut datei = schreibe_seiten(&[vec![0u8; 30]], 1, 0, true, true, 0);
        assert!(
            inspect(&datei).is_ok(),
            "die Vorlage selbst ist schon kaputt"
        );

        // ... deren Segmenttabelle plötzlich 255 Bytes ankündigt.
        datei[27] = 255;
        assert!(inspect(&datei).is_err());

        // Und ein abgeschnittener Kopf.
        assert!(inspect(b"OggS\x00\x00\x00").is_err());
    }
}
