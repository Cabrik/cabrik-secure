//! Bereinigen ohne die ganze Datei: das Gerüst (`spec/grosse-dateien.md` §5).
//!
//! # Warum es bei Video geht
//!
//! Die drei Video-Bereiniger verschieben kein Byte (`spec/metadata.md`
//! §4.2.9). Sie ersetzen Blöcke durch Platzhalter gleicher Größe und leeren
//! Felder an fester Stelle. Die bereinigte Datei ist also die ursprüngliche
//! mit einigen [`Flicken`] — Stelle und neue Bytes. Damit lässt sich
//! strömen: die Datei der Reihe nach lesen und an den Flickstellen die neuen
//! Bytes einsetzen ([`flicken_anwenden`]).
//!
//! # Woher die Flicken kommen
//!
//! Nicht aus einem zweiten Leser je Format. Der wäre eine zweite Antwort auf
//! die Frage „was ist hier Metadatum“, und eine Vorhersage, die nicht der
//! Vorgang selbst ist, läuft ihm irgendwann davon.
//!
//! Stattdessen bekommt **derselbe** Bereiniger eine kleine Datei: die
//! ursprüngliche ohne den Inhalt der Nutzdatenblöcke — `mdat` bei MP4/MOV,
//! `Cluster` bei Matroska, `LIST movi` bei AVI —, mit angepassten
//! Größenangaben. Wo sich Gerüst und bereinigtes Gerüst unterscheiden,
//! liegen die Flicken; ihre Stellen werden in die der Originaldatei
//! zurückgerechnet.
//!
//! # Wann der Weg nicht gilt
//!
//! Wenn der Aufbau sich nicht verfolgen lässt, wenn das Gerüst zu groß
//! würde, wenn der Bereiniger es zurückweist oder seine Länge ändert, oder
//! wenn ein Flicken in eine Größenangabe fiele, die beim Bauen angepasst
//! wurde. Dann gibt es kein Ergebnis, und der Aufrufer nimmt den Weg über
//! die ganze Datei. Das ist kein Fehler, sondern die Grenze dieses Wegs.
//!
//! Dass beide Wege dasselbe ergeben, prüfen `tests/geruest.rs` an echten
//! Dateien und das Fuzz-Ziel `metadata_geruest` an zufälligen
//! (`spec/grosse-dateien.md` §5.4).

use std::io::{Read, Seek, SeekFrom};

use crate::model::StripResult;
use crate::{Format, inspect, matroska, strip};

/// Größer wird kein Gerüst. Darüber lohnt der Weg nicht — oder die Datei
/// ist keine, deren Metadaten so viel Platz brauchen sollten.
///
/// Ein zweistündiges Handyvideo trägt in `moov` einige zehn MiB.
const GERUEST_HOECHSTENS: usize = 128 * 1024 * 1024;

/// Eine Stelle der Originaldatei und die Bytes, die dort hingehören.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flicken {
    /// Wo in der Originaldatei.
    pub versatz: u64,
    /// Was dort stattdessen steht.
    pub bytes: Vec<u8>,
}

/// Was das Gerüst über eine Datei sagt.
///
/// Dasselbe, was `inspect` und `strip` über die ganze Datei sagten — nur
/// dass die bereinigte Fassung als [`Flicken`] vorliegt statt als Kopie.
#[derive(Debug, Clone)]
pub struct Geruestbefund {
    /// Der Formatname, wie `inspect` ihn nennt.
    pub format: Option<String>,
    /// Das Ergebnis der Bereinigung.
    pub ergebnis: StripResult,
    /// Was an der Originaldatei zu ändern ist, aufsteigend nach Stelle.
    pub flicken: Vec<Flicken>,
}

/// Bereinigt über ein Gerüst.
///
/// `Ok(None)` heißt: Dieser Weg gilt für diese Datei nicht — der Aufrufer
/// nimmt den über die ganze Datei. Nur MP4/MOV, Matroska und AVI kennen
/// ihn.
///
/// # Fehler
///
/// Nur Lesefehler der Quelle. Eine Datei, deren Aufbau nicht passt, ist
/// kein Fehler, sondern `Ok(None)`.
pub fn ueber_geruest<R: Read + Seek>(
    quelle: &mut R,
    laenge: u64,
    format: Format,
) -> std::io::Result<Option<Geruestbefund>> {
    let geruest = match format {
        Format::Video => mp4(quelle, laenge)?,
        Format::Matroska => mkv(quelle, laenge)?,
        Format::Avi => avi(quelle, laenge)?,
        _ => None,
    };
    Ok(geruest.as_ref().and_then(auswerten))
}

/// Setzt die Flicken in einen Block der Originaldatei ein.
///
/// `versatz` ist die Stelle des Blocks in der Originaldatei. Flicken, die
/// nur teilweise in den Block fallen, werden anteilig eingesetzt — die
/// Blöcke beim Lesen richten sich nicht nach ihnen.
pub fn flicken_anwenden(versatz: u64, block: &mut [u8], flicken: &[Flicken]) {
    let ende = versatz.saturating_add(block.len() as u64);
    // Die Flicken sind aufsteigend sortiert. Der erste, der hineinreichen
    // kann, ist der letzte, der vor dem Blockende beginnt — davor wird
    // gesucht, damit nicht bei jedem Block alle durchlaufen werden.
    let erster =
        flicken.partition_point(|f| f.versatz.saturating_add(f.bytes.len() as u64) <= versatz);
    for f in flicken.get(erster..).unwrap_or(&[]) {
        if f.versatz >= ende {
            break;
        }
        let f_ende = f.versatz.saturating_add(f.bytes.len() as u64);
        let von = f.versatz.max(versatz);
        let bis = f_ende.min(ende);
        if von >= bis {
            continue;
        }
        let (Ok(im_block), Ok(im_flicken), Ok(n)) = (
            usize::try_from(von.saturating_sub(versatz)),
            usize::try_from(von.saturating_sub(f.versatz)),
            usize::try_from(bis.saturating_sub(von)),
        ) else {
            continue;
        };
        if let (Some(ziel), Some(quelle)) = (
            block.get_mut(im_block..im_block.saturating_add(n)),
            f.bytes.get(im_flicken..im_flicken.saturating_add(n)),
        ) {
            ziel.copy_from_slice(quelle);
        }
    }
}

// ---------------------------------------------------------------------------
// Das Gerüst
// ---------------------------------------------------------------------------

/// Ein Stück, das unverändert aus der Originaldatei stammt.
struct Abschnitt {
    im_geruest: usize,
    im_original: u64,
    laenge: usize,
}

/// Die kleine Datei: unveränderte Abschnitte und dazwischen neu
/// geschriebene Köpfe und Größen.
struct Geruest {
    daten: Vec<u8>,
    abschnitte: Vec<Abschnitt>,
}

impl Geruest {
    const fn neu() -> Self {
        Self {
            daten: Vec::new(),
            abschnitte: Vec::new(),
        }
    }

    /// Übernimmt `laenge` Bytes ab `von` unverändert. `false`, wenn das
    /// Gerüst damit zu groß würde.
    fn uebernehmen<R: Read + Seek>(
        &mut self,
        q: &mut R,
        von: u64,
        laenge: u64,
    ) -> std::io::Result<bool> {
        let Ok(n) = usize::try_from(laenge) else {
            return Ok(false);
        };
        let Some(neu) = self.daten.len().checked_add(n) else {
            return Ok(false);
        };
        if neu > GERUEST_HOECHSTENS {
            return Ok(false);
        }
        let im_geruest = self.daten.len();
        self.daten.resize(neu, 0);
        q.seek(SeekFrom::Start(von))?;
        if let Some(ziel) = self.daten.get_mut(im_geruest..neu) {
            q.read_exact(ziel)?;
        }
        self.abschnitte.push(Abschnitt {
            im_geruest,
            im_original: von,
            laenge: n,
        });
        Ok(true)
    }

    /// Schreibt Bytes, die es so in der Originaldatei nicht gibt.
    fn neu_geschrieben(&mut self, bytes: &[u8]) {
        self.daten.extend_from_slice(bytes);
    }
}

/// Liest bis zu `n` Bytes ab `versatz` — weniger, wenn die Datei vorher
/// endet.
fn lies_bei<R: Read + Seek>(
    q: &mut R,
    versatz: u64,
    n: usize,
    laenge: u64,
) -> std::io::Result<Vec<u8>> {
    let rest = usize::try_from(laenge.saturating_sub(versatz)).unwrap_or(usize::MAX);
    let mut b = vec![0u8; n.min(rest)];
    q.seek(SeekFrom::Start(versatz))?;
    q.read_exact(&mut b)?;
    Ok(b)
}

/// Bereinigt das Gerüst und rechnet die Unterschiede zurück.
fn auswerten(g: &Geruest) -> Option<Geruestbefund> {
    let (sauber, ergebnis) = strip(&g.daten).ok()?;
    if sauber.len() != g.daten.len() {
        return None;
    }
    let flicken = unterschiede(g, &sauber)?;
    let format = inspect(&g.daten).ok().and_then(|i| i.format);
    Some(Geruestbefund {
        format,
        ergebnis,
        flicken,
    })
}

/// Die Flicken — oder `None`, wenn der Bereiniger etwas geändert hat, das
/// es in der Originaldatei so nicht gibt.
fn unterschiede(g: &Geruest, sauber: &[u8]) -> Option<Vec<Flicken>> {
    let mut flicken = Vec::new();
    let mut bisher = 0usize;

    for a in &g.abschnitte {
        // Was davor liegt, ist neu geschrieben und darf sich nicht ändern.
        if g.daten.get(bisher..a.im_geruest)? != sauber.get(bisher..a.im_geruest)? {
            return None;
        }
        let ende = a.im_geruest.checked_add(a.laenge)?;
        let alt = g.daten.get(a.im_geruest..ende)?;
        let neu = sauber.get(a.im_geruest..ende)?;

        let mut lauf: Option<(usize, Vec<u8>)> = None;
        for (i, (x, y)) in alt.iter().zip(neu).enumerate() {
            if x == y {
                if let Some((start, bytes)) = lauf.take() {
                    flicken.push(Flicken {
                        versatz: a.im_original.checked_add(start as u64)?,
                        bytes,
                    });
                }
            } else {
                lauf.get_or_insert_with(|| (i, Vec::new())).1.push(*y);
            }
        }
        if let Some((start, bytes)) = lauf {
            flicken.push(Flicken {
                versatz: a.im_original.checked_add(start as u64)?,
                bytes,
            });
        }
        bisher = ende;
    }
    if g.daten.get(bisher..)? != sauber.get(bisher..)? {
        return None;
    }
    flicken.sort_by_key(|f| f.versatz);
    Some(flicken)
}

fn u32_be(b: &[u8], p: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        b.get(p..p.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn u64_be(b: &[u8], p: usize) -> Option<u64> {
    Some(u64::from_be_bytes(
        b.get(p..p.checked_add(8)?)?.try_into().ok()?,
    ))
}

fn u32_le(b: &[u8], p: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        b.get(p..p.checked_add(4)?)?.try_into().ok()?,
    ))
}

// ---------------------------------------------------------------------------
// MP4 und MOV: `mdat` wird leer
// ---------------------------------------------------------------------------

/// `mdat` steht auf oberster Ebene, auch bei fragmentierten Dateien
/// (`moof` und `mdat` im Wechsel). Jede wird durch eine leere ersetzt; auf
/// sie zeigt im Gerüst nichts, was der Bereiniger liest.
fn mp4<R: Read + Seek>(q: &mut R, laenge: u64) -> std::io::Result<Option<Geruest>> {
    let mut g = Geruest::neu();
    let mut p = 0u64;
    while p < laenge {
        let rest = laenge.saturating_sub(p);
        // Weniger als ein Kopf: Der Bereiniger übergeht es, das Gerüst
        // übernimmt es unverändert.
        if rest < 8 {
            if !g.uebernehmen(q, p, rest)? {
                return Ok(None);
            }
            break;
        }
        let kopf = lies_bei(q, p, 16, laenge)?;
        let (Some(roh), Some(typ)) = (u32_be(&kopf, 0), kopf.get(4..8)) else {
            return Ok(None);
        };
        let (groesse, kopflaenge) = match roh {
            1 => match u64_be(&kopf, 8) {
                Some(g) => (g, 16),
                None => return Ok(None),
            },
            0 => (rest, 8),
            g => (u64::from(g), 8),
        };
        if groesse < kopflaenge || groesse > rest {
            return Ok(None);
        }
        if typ == b"mdat" {
            g.neu_geschrieben(&[0, 0, 0, 8, b'm', b'd', b'a', b't']);
        } else if !g.uebernehmen(q, p, groesse)? {
            return Ok(None);
        }
        p = p.saturating_add(groesse);
    }
    Ok(Some(g))
}

// ---------------------------------------------------------------------------
// Matroska und WebM: die `Cluster` fallen weg
// ---------------------------------------------------------------------------

/// Schreibt eine Längenangabe in genau `breite` Bytes.
fn laenge_schreiben(wert: u64, breite: usize) -> Option<Vec<u8>> {
    // Der Wert aus lauter Einsen ist als „unbekannt“ belegt.
    if wert >= matroska::maske_fuer(breite)? {
        return None;
    }
    let roh = wert.to_be_bytes();
    let mut aus = roh.get(8usize.checked_sub(breite)?..)?.to_vec();
    let marke = 0x80u8.checked_shr(u32::try_from(breite.checked_sub(1)?).ok()?)?;
    *aus.first_mut()? |= marke;
    Some(aus)
}

/// Die `Cluster` eines Segments tragen die Bilddaten und fallen ganz weg.
/// Die Länge des Segments wird in derselben Breite zurückgeschrieben; eine
/// unbekannte Länge bleibt unbekannt.
///
/// Ein `Cluster` ohne Längenangabe — bei laufenden Aufnahmen zulässig —
/// lässt sich nicht überspringen. Dann gilt dieser Weg nicht.
fn mkv<R: Read + Seek>(q: &mut R, laenge: u64) -> std::io::Result<Option<Geruest>> {
    let mut g = Geruest::neu();
    let mut p = 0u64;
    while p < laenge {
        let kopf = lies_bei(q, p, 12, laenge)?;
        let element = matroska::lies_kennung(&kopf, 0).and_then(|(kennung, k)| {
            matroska::lies_laenge(&kopf, k).map(|(l, l_len)| (kennung, k, l, l_len))
        });
        let Some((kennung, k_len, angabe, l_len)) = element else {
            // Nichts Lesbares mehr. Der Bereiniger lässt es stehen, also
            // übernimmt es auch das Gerüst.
            if !g.uebernehmen(q, p, laenge.saturating_sub(p))? {
                return Ok(None);
            }
            break;
        };
        let kopf_len = k_len.saturating_add(l_len);
        let inhalt = p.saturating_add(kopf_len as u64);
        let ende = match angabe {
            Some(n) => inhalt.saturating_add(n),
            None => laenge,
        };
        if ende > laenge || ende <= p {
            return Ok(None);
        }

        if kennung == matroska::SEGMENT {
            let kopf_im_geruest = g.daten.len();
            g.neu_geschrieben(kopf.get(..kopf_len).unwrap_or(&[]));
            let kinder_anfang = g.daten.len();

            let mut c = inhalt;
            while c < ende {
                let kk = lies_bei(q, c, 12, ende)?;
                let kind = matroska::lies_kennung(&kk, 0).and_then(|(kennung, k)| {
                    matroska::lies_laenge(&kk, k).map(|(l, l_len)| (kennung, k, l, l_len))
                });
                let Some((k_kennung, kk_len, k_angabe, kl_len)) = kind else {
                    if !g.uebernehmen(q, c, ende.saturating_sub(c))? {
                        return Ok(None);
                    }
                    break;
                };
                let k_inhalt = c.saturating_add(kk_len.saturating_add(kl_len) as u64);
                let k_ende = match k_angabe {
                    Some(n) => k_inhalt.saturating_add(n),
                    None if k_kennung == matroska::CLUSTER => return Ok(None),
                    None => ende,
                };
                if k_ende > ende || k_ende <= c {
                    return Ok(None);
                }
                if k_kennung != matroska::CLUSTER
                    && !g.uebernehmen(q, c, k_ende.saturating_sub(c))?
                {
                    return Ok(None);
                }
                c = k_ende;
            }

            if angabe.is_some() {
                let neu = g.daten.len().saturating_sub(kinder_anfang) as u64;
                let Some(vint) = laenge_schreiben(neu, l_len) else {
                    return Ok(None);
                };
                let von = kopf_im_geruest.saturating_add(k_len);
                let Some(ziel) = g.daten.get_mut(von..von.saturating_add(l_len)) else {
                    return Ok(None);
                };
                ziel.copy_from_slice(&vint);
            }
        } else if !g.uebernehmen(q, p, ende.saturating_sub(p))? {
            return Ok(None);
        }
        p = ende;
    }
    Ok(Some(g))
}

// ---------------------------------------------------------------------------
// AVI: `LIST movi` wird leer
// ---------------------------------------------------------------------------

/// Jede `LIST movi` — auch die der OpenDML-Erweiterungen `RIFF AVIX` —
/// wird durch eine leere ersetzt, und die Größe des umschließenden `RIFF`
/// angepasst. Der Index `idx1` bleibt: Er zählt ab `movi`, und der
/// Bereiniger liest ihn nicht.
fn avi<R: Read + Seek>(q: &mut R, laenge: u64) -> std::io::Result<Option<Geruest>> {
    let mut g = Geruest::neu();
    let mut p = 0u64;
    while p.saturating_add(8) <= laenge {
        let kopf = lies_bei(q, p, 12, laenge)?;
        let (Some(typ), Some(groesse)) = (kopf.get(0..4), u32_le(&kopf, 4)) else {
            return Ok(None);
        };
        let groesse = u64::from(groesse);
        let ende = p.saturating_add(8).saturating_add(groesse);
        if ende > laenge {
            return Ok(None);
        }
        // RIFF richtet jeden Block auf gerade Adressen aus.
        let weiter = ende.saturating_add(groesse & 1).min(laenge);

        if typ == b"RIFF" && kopf.len() >= 12 {
            let kopf_im_geruest = g.daten.len();
            g.neu_geschrieben(kopf.get(..12).unwrap_or(&[]));
            let kinder_anfang = g.daten.len();

            let mut c = p.saturating_add(12);
            while c.saturating_add(8) <= ende {
                let kk = lies_bei(q, c, 12, ende)?;
                let (Some(k_typ), Some(k_groesse)) = (kk.get(0..4), u32_le(&kk, 4)) else {
                    return Ok(None);
                };
                let k_groesse = u64::from(k_groesse);
                let k_ende = c.saturating_add(8).saturating_add(k_groesse);
                if k_ende > ende {
                    return Ok(None);
                }
                let k_weiter = k_ende.saturating_add(k_groesse & 1).min(ende);
                if k_typ == b"LIST" && kk.get(8..12) == Some(b"movi") {
                    g.neu_geschrieben(b"LIST\x04\x00\x00\x00movi");
                } else if !g.uebernehmen(q, c, k_weiter.saturating_sub(c))? {
                    return Ok(None);
                }
                c = k_weiter;
            }
            if c < ende && !g.uebernehmen(q, c, ende.saturating_sub(c))? {
                return Ok(None);
            }

            let Ok(neu) = u32::try_from(
                g.daten
                    .len()
                    .saturating_sub(kinder_anfang)
                    .saturating_add(4),
            ) else {
                return Ok(None);
            };
            let von = kopf_im_geruest.saturating_add(4);
            let Some(ziel) = g.daten.get_mut(von..von.saturating_add(4)) else {
                return Ok(None);
            };
            ziel.copy_from_slice(&neu.to_le_bytes());
            // Das Füllbyte richtet sich nach der NEUEN Größe. Das alte zu
            // übernehmen verschöbe den nächsten Block, sobald sich die
            // Parität geändert hat.
            if neu & 1 == 1 {
                g.neu_geschrieben(&[0]);
            }
        } else if !g.uebernehmen(q, p, weiter.saturating_sub(p))? {
            return Ok(None);
        }
        p = weiter;
    }
    if p < laenge && !g.uebernehmen(q, p, laenge.saturating_sub(p))? {
        return Ok(None);
    }
    Ok(Some(g))
}
