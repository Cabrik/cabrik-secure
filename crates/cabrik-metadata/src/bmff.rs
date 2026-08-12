//! HEIC, HEIF und AVIF (`spec/metadata.md` §4).
//!
//! Alle drei sind ISO-BMFF: eine Folge von **Boxen** aus Größe(4) ‖ Typ(4) ‖
//! Inhalt. Ein Bild besteht aus *Items*, die in `meta` beschrieben und in
//! `mdat` abgelegt sind:
//!
//! ```text
//! ftyp
//! meta
//!   iinf   Item 1 = av01/hvc1 (das Bild), Item 2 = Exif, Item 3 = mime (XMP)
//!   iloc   Item 1 → Versatz 638, Item 2 → 412, Item 3 → 562
//!   iprp/ipco/colr   das Farbprofil, als Eigenschaft
//!   iref   verknüpft Items, etwa Vorschaubild → Bild
//! mdat   hier liegen die Nutzdaten aller Items
//! ```
//!
//! # Warum hier **nicht** neu gebaut wird
//!
//! Bei TIFF war der Neubau der einzige Weg. Hier wäre er der falsche Tausch.
//!
//! `iloc` speichert **absolute Dateiversätze**, hat aber anders als TIFF
//! veränderliche Feldbreiten; `ipma` verweist über Indizes in `ipco`; `iinf`
//! und `infe` gibt es in mehreren Fassungen. Ein Neubau bräuchte ein
//! Vielfaches an Code — und hätte dieselbe gefährliche Fehlerart wie TIFF:
//! eine Datei, die sich öffnen lässt und Müll zeigt.
//!
//! # Was stattdessen geschieht
//!
//! Die Exif- und XMP-Nutzdaten sind zusammenhängende Blöcke bekannter Länge.
//! Sie werden **an Ort und Stelle ersetzt** — durch ein gültiges, leeres Exif
//! beziehungsweise ein leeres XMP-Paket, jeweils auf die ursprüngliche Länge
//! aufgefüllt.
//!
//! Damit ändert sich **kein einziger Versatz**: Die Dateilänge bleibt gleich,
//! jede Boxgröße bleibt gültig, jeder `iloc`-Eintrag bleibt richtig. Die
//! gefährliche Fehlerart ist nicht unwahrscheinlich, sondern ausgeschlossen.
//!
//! Dass die Item-Deklaration stehen bleibt, ist **kein** Widerspruch zum
//! Vorgehen bei WebP. Dort kündigte ein Merkmalsbit einen Chunk an, den es
//! nicht mehr gab. Hier stimmen Deklaration und Inhalt überein: ein
//! Exif-Block mit null Einträgen. In sich schlüssig, nur leer.
//!
//! # Was bleibt
//!
//! Farbprofil (`colr`) und Vorschaubild-Items. Beide ließen sich nur durch
//! einen Neubau entfernen; sie werden benannt, und das Ergebnis ist ehrlich
//! [`StripResult::Partial`].

use crate::model::{Finding, FindingKind, Inspection, Severity, StripResult};

use cabrik_core::{Error, Result};

/// Höchstzahl der Boxen, die verfolgt werden.
///
/// Die Schachtelung braucht keine eigene Grenze: In dieses Modul wird nur an
/// festen, benannten Stellen abgestiegen (`meta`, `iprp`, `ipco`, `iinf`,
/// `iref`), nie beliebig tief.
const MAX_BOXEN: usize = 4096;
/// Höchstgröße einer Datei, die wir anfassen.
const MAX_DATEI: usize = 512 * 1024 * 1024;

/// Ob die Bytes wie ISO-BMFF aussehen.
///
/// Die Kennung steht ab Versatz 4, nicht am Anfang: Davor liegt die Größe der
/// `ftyp`-Box.
#[must_use]
pub fn looks_like_bmff(daten: &[u8]) -> bool {
    if daten.get(4..8) != Some(b"ftyp") {
        return false;
    }
    // Die Marke sagt, worum es sich handelt. Video (`mp4`, `isom`) wird
    // bewusst nicht beansprucht — dafür bräuchte es eine ganz andere
    // Behandlung, und ein halb verstandenes Format ist schlimmer als ein
    // ehrlich unbekanntes.
    let marke = daten.get(8..12).unwrap_or(&[]);
    matches!(
        marke,
        b"heic" | b"heix" | b"hevc" | b"heim" | b"heis" | b"mif1" | b"msf1" | b"avif" | b"avis"
    ) || kompatible_marke(daten)
}

/// Sucht die Marken in der `ftyp`-Liste ab — manche Erzeuger tragen die
/// aussagekräftige Marke erst dort ein.
fn kompatible_marke(daten: &[u8]) -> bool {
    let Some(groesse) = u32_bei(daten, 0) else {
        return false;
    };
    let ende = usize::try_from(groesse).unwrap_or(0).min(daten.len());
    let mut p = 16usize; // Größe(4) Typ(4) Hauptmarke(4) Version(4)
    while p.saturating_add(4) <= ende {
        if matches!(
            daten.get(p..p.saturating_add(4)),
            Some(b"heic" | b"mif1" | b"avif" | b"msf1" | b"avis")
        ) {
            return true;
        }
        p = p.saturating_add(4);
    }
    false
}

fn u16_bei(d: &[u8], p: usize) -> Option<u16> {
    Some(u16::from_be_bytes(
        d.get(p..p.saturating_add(2))?.try_into().ok()?,
    ))
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

/// Liest eine ganze Zahl beliebiger Breite (0, 4 oder 8 Bytes).
fn zahl_bei(d: &[u8], p: usize, breite: usize) -> Option<u64> {
    match breite {
        0 => Some(0),
        4 => u32_bei(d, p).map(u64::from),
        8 => u64_bei(d, p),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Boxen
// ---------------------------------------------------------------------------

/// Eine gefundene Box: Typ, Anfang des Inhalts, Ende der Box.
#[derive(Debug, Clone, Copy)]
struct Box4 {
    typ: [u8; 4],
    inhalt: usize,
    ende: usize,
}

/// Durchläuft die Boxen eines Bereichs.
fn boxen(daten: &[u8], von: usize, bis: usize, zaehler: &mut usize) -> Result<Vec<Box4>> {
    let mut aus = Vec::new();
    let mut p = von;

    while p.saturating_add(8) <= bis {
        *zaehler = zaehler.saturating_add(1);
        if *zaehler > MAX_BOXEN {
            return Err(Error::Malformed("bmff: zu viele Boxen"));
        }

        let roh = u32_bei(daten, p).ok_or(Error::Malformed("bmff: Boxgroesse unlesbar"))?;
        let typ: [u8; 4] = daten
            .get(p.saturating_add(4)..p.saturating_add(8))
            .and_then(|s| s.try_into().ok())
            .ok_or(Error::Malformed("bmff: Boxtyp unlesbar"))?;

        let (groesse, kopf) = match roh {
            // 1 heißt: die wahre Größe steht als 64-Bit-Zahl dahinter.
            1 => {
                let g = u64_bei(daten, p.saturating_add(8))
                    .ok_or(Error::Malformed("bmff: grosse Boxgroesse unlesbar"))?;
                (
                    usize::try_from(g).map_err(|_| Error::Malformed("bmff: Box zu gross"))?,
                    16usize,
                )
            }
            // 0 heißt: bis zum Ende des Bereichs.
            0 => (bis.saturating_sub(p), 8usize),
            g => (
                usize::try_from(g).map_err(|_| Error::Malformed("bmff: Box zu gross"))?,
                8usize,
            ),
        };

        if groesse < kopf {
            return Err(Error::Malformed("bmff: Box kleiner als ihr Kopf"));
        }
        let ende = p
            .checked_add(groesse)
            .ok_or(Error::Malformed("bmff: Boxende ueberlaeuft"))?;
        if ende > bis {
            return Err(Error::Malformed("bmff: Box reicht ueber ihren Bereich"));
        }

        aus.push(Box4 {
            typ,
            inhalt: p.saturating_add(kopf),
            ende,
        });
        p = ende;
    }
    Ok(aus)
}

/// Sucht eine Box eines Typs in einem Bereich.
fn finde(daten: &[u8], von: usize, bis: usize, typ: &[u8; 4]) -> Result<Option<Box4>> {
    let mut z = 0usize;
    Ok(boxen(daten, von, bis, &mut z)?
        .into_iter()
        .find(|b| b.typ == *typ))
}

// ---------------------------------------------------------------------------
// Items
// ---------------------------------------------------------------------------

/// Ein Item mit Typ und Fundstelle seiner Nutzdaten.
#[derive(Debug, Clone)]
struct Item {
    id: u32,
    typ: [u8; 4],
    /// Versatz und Länge der Nutzdaten — sofern sie in der Datei liegen.
    stelle: Option<(usize, usize)>,
    /// Ob ein anderes Item dieses als Vorschaubild ausweist.
    ist_vorschau: bool,
}

impl Item {
    fn ist_exif(&self) -> bool {
        self.typ == *b"Exif"
    }

    fn ist_xmp(&self) -> bool {
        self.typ == *b"mime"
    }
}

/// Liest `iinf`, `iloc` und `iref` und setzt daraus die Item-Liste zusammen.
fn lies_items(daten: &[u8], meta: Box4) -> Result<Vec<Item>> {
    // `meta` ist eine FullBox: vier Bytes Version und Merkmale voraus.
    let meta_inhalt = meta.inhalt.saturating_add(4);

    let mut items = lies_iinf(daten, meta_inhalt, meta.ende)?;
    lies_iloc(daten, meta_inhalt, meta.ende, &mut items)?;
    lies_iref(daten, meta_inhalt, meta.ende, &mut items)?;
    Ok(items)
}

/// `iinf` — welche Items es gibt und welchen Typ sie haben.
fn lies_iinf(daten: &[u8], von: usize, bis: usize) -> Result<Vec<Item>> {
    let Some(iinf) = finde(daten, von, bis, b"iinf")? else {
        return Ok(Vec::new());
    };

    let version = *daten
        .get(iinf.inhalt)
        .ok_or(Error::Malformed("bmff: iinf-Version fehlt"))?;
    let mut p = iinf.inhalt.saturating_add(4);

    let anzahl = if version >= 1 {
        let n = u32_bei(daten, p).ok_or(Error::Malformed("bmff: iinf-Anzahl unlesbar"))?;
        p = p.saturating_add(4);
        n
    } else {
        let n = u16_bei(daten, p).ok_or(Error::Malformed("bmff: iinf-Anzahl unlesbar"))?;
        p = p.saturating_add(2);
        u32::from(n)
    };
    if anzahl > 4096 {
        return Err(Error::Malformed("bmff: zu viele Items"));
    }

    let mut z = 0usize;
    let infes = boxen(daten, p, iinf.ende, &mut z)?;
    let mut aus = Vec::with_capacity(infes.len());

    for b in infes {
        if b.typ != *b"infe" {
            continue;
        }
        let version = *daten
            .get(b.inhalt)
            .ok_or(Error::Malformed("bmff: infe-Version fehlt"))?;
        // Fassungen vor 2 kennen keinen Item-Typ; sie kommen nur in alten
        // Dateien vor und werden übergangen statt geraten.
        if version < 2 {
            continue;
        }

        let mut q = b.inhalt.saturating_add(4);
        let id = if version == 2 {
            let v = u16_bei(daten, q).ok_or(Error::Malformed("bmff: Item-Kennung unlesbar"))?;
            q = q.saturating_add(2);
            u32::from(v)
        } else {
            let v = u32_bei(daten, q).ok_or(Error::Malformed("bmff: Item-Kennung unlesbar"))?;
            q = q.saturating_add(4);
            v
        };
        q = q.saturating_add(2); // Schutzindex
        let typ: [u8; 4] = daten
            .get(q..q.saturating_add(4))
            .and_then(|s| s.try_into().ok())
            .ok_or(Error::Malformed("bmff: Item-Typ unlesbar"))?;

        aus.push(Item {
            id,
            typ,
            stelle: None,
            ist_vorschau: false,
        });
    }
    Ok(aus)
}

/// `iloc` — wo die Nutzdaten jedes Items liegen.
fn lies_iloc(daten: &[u8], von: usize, bis: usize, items: &mut [Item]) -> Result<()> {
    let Some(iloc) = finde(daten, von, bis, b"iloc")? else {
        return Ok(());
    };

    let version = *daten
        .get(iloc.inhalt)
        .ok_or(Error::Malformed("bmff: iloc-Version fehlt"))?;
    let mut p = iloc.inhalt.saturating_add(4);

    // Zwei Bytes mit vier Feldbreiten zu je vier Bit.
    let b0 = *daten
        .get(p)
        .ok_or(Error::Malformed("bmff: iloc-Feldbreiten fehlen"))?;
    let b1 = *daten
        .get(p.saturating_add(1))
        .ok_or(Error::Malformed("bmff: iloc-Feldbreiten fehlen"))?;
    let versatz_breite = usize::from(b0 >> 4);
    let laengen_breite = usize::from(b0 & 0x0F);
    let basis_breite = usize::from(b1 >> 4);
    let index_breite = if version >= 1 {
        usize::from(b1 & 0x0F)
    } else {
        0
    };
    p = p.saturating_add(2);

    let anzahl = if version >= 2 {
        let n = u32_bei(daten, p).ok_or(Error::Malformed("bmff: iloc-Anzahl unlesbar"))?;
        p = p.saturating_add(4);
        n
    } else {
        let n = u16_bei(daten, p).ok_or(Error::Malformed("bmff: iloc-Anzahl unlesbar"))?;
        p = p.saturating_add(2);
        u32::from(n)
    };
    if anzahl > 4096 {
        return Err(Error::Malformed("bmff: zu viele iloc-Eintraege"));
    }

    for _ in 0..anzahl {
        let id = if version >= 2 {
            let v = u32_bei(daten, p).ok_or(Error::Malformed("bmff: iloc-Kennung unlesbar"))?;
            p = p.saturating_add(4);
            v
        } else {
            let v = u16_bei(daten, p).ok_or(Error::Malformed("bmff: iloc-Kennung unlesbar"))?;
            p = p.saturating_add(2);
            u32::from(v)
        };

        // Ab Fassung 1 steht hier, wie die Daten zusammengesetzt sind.
        let bauart = if version >= 1 {
            let v = u16_bei(daten, p).ok_or(Error::Malformed("bmff: iloc-Bauart unlesbar"))?;
            p = p.saturating_add(2);
            v & 0x0F
        } else {
            0
        };
        p = p.saturating_add(2); // Verweis auf die Datenquelle

        let basis = zahl_bei(daten, p, basis_breite)
            .ok_or(Error::Malformed("bmff: iloc-Basis unlesbar"))?;
        p = p.saturating_add(basis_breite);

        let teile = u16_bei(daten, p).ok_or(Error::Malformed("bmff: iloc-Teilzahl unlesbar"))?;
        p = p.saturating_add(2);

        let mut stelle = None;
        for i in 0..teile {
            p = p.saturating_add(index_breite);
            let versatz = zahl_bei(daten, p, versatz_breite)
                .ok_or(Error::Malformed("bmff: iloc-Versatz unlesbar"))?;
            p = p.saturating_add(versatz_breite);
            let laenge = zahl_bei(daten, p, laengen_breite)
                .ok_or(Error::Malformed("bmff: iloc-Laenge unlesbar"))?;
            p = p.saturating_add(laengen_breite);

            // Nur der erste Teil wird verwendet, und nur wenn die Daten
            // wirklich in der Datei liegen (Bauart 0). Bauart 1 legt sie in
            // `idat`, Bauart 2 in einer anderen Datei — beides wird hier
            // nicht angefasst.
            if i == 0 && bauart == 0 {
                let start = usize::try_from(basis.saturating_add(versatz))
                    .map_err(|_| Error::Malformed("bmff: Versatz zu gross"))?;
                let len = usize::try_from(laenge)
                    .map_err(|_| Error::Malformed("bmff: Laenge zu gross"))?;
                if start.saturating_add(len) <= daten.len() {
                    stelle = Some((start, len));
                }
            }
        }

        if let Some(item) = items.iter_mut().find(|it| it.id == id) {
            item.stelle = stelle;
        }
    }
    Ok(())
}

/// `iref` — welche Items als Vorschaubild eines anderen ausgewiesen sind.
fn lies_iref(daten: &[u8], von: usize, bis: usize, items: &mut [Item]) -> Result<()> {
    let Some(iref) = finde(daten, von, bis, b"iref")? else {
        return Ok(());
    };
    let version = *daten
        .get(iref.inhalt)
        .ok_or(Error::Malformed("bmff: iref-Version fehlt"))?;
    let breit = version >= 1;

    let mut z = 0usize;
    for b in boxen(daten, iref.inhalt.saturating_add(4), iref.ende, &mut z)? {
        // `thmb` heißt: „von" ist das Vorschaubild von „nach".
        if b.typ != *b"thmb" {
            continue;
        }
        let von_id = if breit {
            u32_bei(daten, b.inhalt)
        } else {
            u16_bei(daten, b.inhalt).map(u32::from)
        }
        .ok_or(Error::Malformed("bmff: iref-Kennung unlesbar"))?;

        if let Some(item) = items.iter_mut().find(|it| it.id == von_id) {
            item.ist_vorschau = true;
        }
    }
    Ok(())
}

/// Sucht das Farbprofil in `iprp/ipco`.
fn farbprofil(daten: &[u8], meta: Box4) -> Result<Option<usize>> {
    let meta_inhalt = meta.inhalt.saturating_add(4);
    let Some(iprp) = finde(daten, meta_inhalt, meta.ende, b"iprp")? else {
        return Ok(None);
    };
    let Some(ipco) = finde(daten, iprp.inhalt, iprp.ende, b"ipco")? else {
        return Ok(None);
    };
    let Some(colr) = finde(daten, ipco.inhalt, ipco.ende, b"colr")? else {
        return Ok(None);
    };

    // Nur ein eingebettetes Profil zählt. `nclx` ist eine bloße Kennzahl für
    // den Farbraum und verrät nichts über Gerät oder Person.
    match daten.get(colr.inhalt..colr.inhalt.saturating_add(4)) {
        Some(b"prof" | b"rICC") => Ok(Some(colr.ende.saturating_sub(colr.inhalt))),
        _ => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// Untersuchen
// ---------------------------------------------------------------------------

/// Zerlegt die Datei so weit, wie es für Fund und Ersetzung nötig ist.
fn zerlege(daten: &[u8]) -> Result<(Vec<Item>, Option<usize>)> {
    if daten.len() > MAX_DATEI {
        return Err(Error::Malformed("bmff: Datei zu gross"));
    }
    if !looks_like_bmff(daten) {
        return Err(Error::Malformed("bmff: keine bekannte Marke"));
    }

    let mut z = 0usize;
    let oben = boxen(daten, 0, daten.len(), &mut z)?;
    let meta = oben
        .iter()
        .find(|b| b.typ == *b"meta")
        .copied()
        .ok_or(Error::Malformed("bmff: keine meta-Box"))?;

    Ok((lies_items(daten, meta)?, farbprofil(daten, meta)?))
}

/// Untersucht ein HEIC-, HEIF- oder AVIF-Bild.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Struktur.
pub fn inspect(daten: &[u8]) -> Result<Inspection> {
    let (items, profil) = zerlege(daten)?;
    Ok(Inspection {
        format: Some(marke_name(daten).to_owned()),
        findings: sammle(daten, &items, profil),
        understood: true,
    })
}

fn marke_name(daten: &[u8]) -> &'static str {
    match daten.get(8..12) {
        Some(b"avif" | b"avis") => "AVIF",
        Some(b"mif1" | b"msf1") => "HEIF",
        _ => "HEIC",
    }
}

fn sammle(daten: &[u8], items: &[Item], profil: Option<usize>) -> Vec<Finding> {
    let mut funde = Vec::new();

    for it in items {
        let Some((versatz, laenge)) = it.stelle else {
            continue;
        };

        if it.ist_exif() {
            // Gemeldet wird der **Inhalt**, nicht die Deklaration. Ein Item,
            // das nur noch eine leere Hülle enthält, ist kein Fund — sonst
            // meldete eine zweite Prüfung nach der Bereinigung weiterhin
            // „trägt häufig Verfasser", obwohl nichts mehr drinsteht.
            match tiff_teil(daten, versatz, laenge).map(crate::tiff::inspect) {
                Some(Ok(innen)) => {
                    for f in innen.findings {
                        funde.push(Finding::new(
                            f.kind,
                            format!("BMFF:Exif → {}", f.location),
                            f.value,
                            f.severity,
                        ));
                    }
                }
                // Nicht lesbar: Dann lässt sich über den Inhalt nichts sagen,
                // und genau das wird gemeldet.
                _ => funde.push(Finding::new(
                    FindingKind::Device,
                    "BMFF:Exif".to_owned(),
                    Some(format!(
                        "Exif-Block, {laenge} Bytes — nicht lesbar, Inhalt unbekannt"
                    )),
                    Severity::Notable,
                )),
            }
        } else if it.ist_xmp() {
            if xmp_hat_inhalt(daten, versatz, laenge) {
                funde.push(Finding::new(
                    FindingKind::Author,
                    "BMFF:XMP".to_owned(),
                    Some(format!(
                        "XMP-Block, {laenge} Bytes — trägt häufig Verfasser und \
                         Bearbeitungsverlauf"
                    )),
                    Severity::Critical,
                ));
            }
        } else if it.ist_vorschau {
            funde.push(Finding::new(
                FindingKind::EmbeddedPreview,
                format!("BMFF:Vorschaubild (Item {})", it.id),
                Some(format!(
                    "{laenge} Bytes — eine zweite Kopie des Inhalts, oft in einem \
                     Zustand, den der Nutzer gerade beseitigen wollte"
                )),
                Severity::Critical,
            ));
        }
    }

    if let Some(bytes) = profil {
        funde.push(Finding::new(
            FindingKind::ColorProfile,
            "BMFF:colr".to_owned(),
            Some(format!("eingebettetes Farbprofil, {bytes} Bytes")),
            Severity::Minor,
        ));
    }

    funde
}

/// Ob im XMP-Block überhaupt etwas steht.
///
/// Ein Paket aus bloßen Namensraumangaben und Leerraum trägt keine
/// Information. Nach der Bereinigung sieht genau so der zurückbleibende Block
/// aus — er darf dann nicht mehr als Fund erscheinen.
fn xmp_hat_inhalt(daten: &[u8], versatz: usize, laenge: usize) -> bool {
    let Some(block) = daten.get(versatz..versatz.saturating_add(laenge)) else {
        return false;
    };
    // Nicht lesbarer Text könnte alles Mögliche sein — im Zweifel melden.
    let Ok(text) = core::str::from_utf8(block) else {
        return true;
    };
    crate::xml::hat_textinhalt(text)
}

/// Schneidet aus den Exif-Nutzdaten den TIFF-Teil heraus.
///
/// Aufbau: Versatz(4) ‖ Füllung ‖ TIFF. Die ersten vier Bytes sagen, wie weit
/// es von hinter ihnen bis zum TIFF-Kopf ist — meist sechs Bytes für `Exif\0\0`.
fn tiff_teil(daten: &[u8], versatz: usize, laenge: usize) -> Option<&[u8]> {
    let block = daten.get(versatz..versatz.saturating_add(laenge))?;
    let bis_tiff = usize::try_from(u32_bei(block, 0)?).ok()?;
    let start = 4usize.checked_add(bis_tiff)?;
    block.get(start..)
}

// ---------------------------------------------------------------------------
// Ersetzen
// ---------------------------------------------------------------------------

/// Ein leeres, gültiges Exif: Versatz(4) ‖ `Exif\0\0` ‖ TIFF-Kopf ‖ IFD ohne
/// Einträge.
const LEERES_EXIF: [u8; 24] = [
    0x00, 0x00, 0x00, 0x06, // Versatz bis zum TIFF-Kopf
    b'E', b'x', b'i', b'f', 0x00, 0x00, //
    b'M', b'M', 0x00, 0x2A, // Byte-Reihenfolge und Kennzahl
    0x00, 0x00, 0x00, 0x08, // Versatz des ersten IFD
    0x00, 0x00, // null Einträge
    0x00, 0x00, 0x00, 0x00, // kein weiteres IFD
];

/// Ein leeres, gültiges XMP.
///
/// Nachfolgender Leerraum ist in XML hinter dem Wurzelelement erlaubt — und
/// XMP-Pakete werden ohnehin üblicherweise mit Leerraum aufgefüllt. Das ist
/// kein Kunstgriff, sondern das übliche Verfahren.
const LEERES_XMP: &[u8] = br#"<x:xmpmeta xmlns:x="adobe:ns:meta/"/>"#;

/// Ersetzt einen Block an Ort und Stelle, ohne seine Länge zu ändern.
fn ersetze(ziel: &mut [u8], versatz: usize, laenge: usize, muster: &[u8], fuellung: u8) {
    let Some(bereich) = ziel.get_mut(versatz..versatz.saturating_add(laenge)) else {
        return;
    };
    bereich.fill(fuellung);
    if muster.len() <= bereich.len()
        && let Some(anfang) = bereich.get_mut(..muster.len())
    {
        anfang.copy_from_slice(muster);
    }
}

/// Bereinigt ein HEIC-, HEIF- oder AVIF-Bild.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Struktur.
pub fn strip(daten: &[u8]) -> Result<(Vec<u8>, StripResult)> {
    let (items, profil) = zerlege(daten)?;
    let alle = sammle(daten, &items, profil);

    let mut aus = daten.to_vec();
    for it in &items {
        let Some((versatz, laenge)) = it.stelle else {
            continue;
        };
        if it.ist_exif() {
            ersetze(&mut aus, versatz, laenge, &LEERES_EXIF, 0x00);
        } else if it.ist_xmp() {
            // Mit Leerzeichen auffüllen, damit das Ergebnis gültiges XML
            // bleibt.
            ersetze(&mut aus, versatz, laenge, LEERES_XMP, b' ');
        }
    }
    debug_assert_eq!(aus.len(), daten.len(), "die Dateilaenge hat sich geaendert");

    // Getrennt nach dem, was tatsächlich geschehen ist.
    let (entfernt, geblieben): (Vec<Finding>, Vec<Finding>) = alle
        .into_iter()
        .partition(|f| f.location.starts_with("BMFF:Exif") || f.location == "BMFF:XMP");

    let ergebnis = if geblieben.is_empty() {
        StripResult::Complete { removed: entfernt }
    } else {
        StripResult::Partial {
            removed: entfernt,
            remaining: geblieben,
            reason: "Farbprofil und Vorschaubilder sind als Items beziehungsweise \
                     Eigenschaften in der Verzeichnisstruktur verankert. Sie zu \
                     entfernen hieße, die Datei vollständig neu aufzubauen und alle \
                     Versätze neu zu vergeben — ein Eingriff mit der Fehlerart, die \
                     eine Datei öffnen lässt und dennoch Müll zeigt. Exif und XMP \
                     wurden vollständig geleert; sie tragen das, was eine Person \
                     erkennbar macht."
                .to_owned(),
        }
    };

    Ok((aus, ergebnis))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;

    #[test]
    fn die_marken_werden_unterschieden() {
        let mut avif = vec![0, 0, 0, 16];
        avif.extend_from_slice(b"ftypavif");
        avif.extend_from_slice(&[0, 0, 0, 0]);
        assert!(looks_like_bmff(&avif));
        assert_eq!(marke_name(&avif), "AVIF");

        let mut heic = vec![0, 0, 0, 16];
        heic.extend_from_slice(b"ftypheic");
        heic.extend_from_slice(&[0, 0, 0, 0]);
        assert!(looks_like_bmff(&heic));
        assert_eq!(marke_name(&heic), "HEIC");
    }

    /// Video ist ISO-BMFF, wird hier aber **nicht** beansprucht: Es braucht
    /// eine ganz andere Behandlung, und halb verstanden ist schlimmer als
    /// ehrlich unbekannt.
    #[test]
    fn video_wird_nicht_beansprucht() {
        let mut mp4 = vec![0, 0, 0, 16];
        mp4.extend_from_slice(b"ftypisom");
        mp4.extend_from_slice(&[0, 0, 0, 0]);
        assert!(!looks_like_bmff(&mp4), "MP4 wurde faelschlich uebernommen");
    }

    #[test]
    fn kein_bmff_wird_abgelehnt() {
        assert!(!looks_like_bmff(b""));
        assert!(!looks_like_bmff(b"\x89PNG\r\n\x1a\n"));
        assert!(inspect(b"").is_err());
    }

    /// Das leere Exif muss ein gueltiger TIFF-Strom mit null Eintraegen sein --
    /// sonst haetten wir eine kaputte Datei statt einer leeren Angabe.
    #[test]
    fn das_leere_exif_ist_gueltig() {
        let tiff = &LEERES_EXIF[10..];
        assert!(crate::tiff::looks_like_tiff(tiff), "kein gueltiges TIFF");
        let i = crate::tiff::inspect(tiff).unwrap();
        assert!(i.findings.is_empty(), "das leere Exif traegt noch etwas");
    }

    /// Das leere XMP muss wohlgeformtes XML sein.
    #[test]
    fn das_leere_xmp_ist_wohlgeformt() {
        let text = core::str::from_utf8(LEERES_XMP).unwrap();
        assert!(text.starts_with("<x:xmpmeta"));
        assert!(text.ends_with("/>"));
        // Mit angehaengtem Leerraum bleibt es gueltig -- so wird aufgefuellt.
        let mit_fuellung = format!("{text}          ");
        assert_eq!(crate::xml::zaehle_elemente(&mit_fuellung, "xmpmeta"), 1);
    }

    /// **Gemeldet wird der Inhalt, nicht die Deklaration.** Nach der
    /// Bereinigung bleibt das Item stehen -- es darf dann aber nicht mehr
    /// als Fund erscheinen, sonst behauptet eine zweite Pruefung etwas
    /// Falsches.
    #[test]
    fn ein_geleerter_xmp_block_ist_kein_fund_mehr() {
        let mut voll = br#"<x:xmpmeta xmlns:x="adobe:ns:meta/">"#.to_vec();
        voll.extend_from_slice(b"<dc:creator>Dr. Anna Beispiel</dc:creator></x:xmpmeta>");
        assert!(
            xmp_hat_inhalt(&voll, 0, voll.len()),
            "der gefuellte Block wurde nicht erkannt"
        );

        let mut leer = LEERES_XMP.to_vec();
        leer.resize(voll.len(), b' ');
        assert!(
            !xmp_hat_inhalt(&leer, 0, leer.len()),
            "der geleerte Block gilt weiterhin als Fund"
        );
    }

    /// Ersetzen darf die Laenge **nie** aendern -- daran haengt alles.
    #[test]
    fn ersetzen_aendert_die_laenge_nicht() {
        let mut puffer = vec![0xAAu8; 100];
        ersetze(&mut puffer, 10, 40, &LEERES_EXIF, 0x00);

        assert_eq!(puffer.len(), 100, "die Laenge hat sich geaendert");
        assert_eq!(&puffer[..10], &[0xAA; 10], "davor wurde geschrieben");
        assert_eq!(&puffer[50..], &[0xAA; 50], "dahinter wurde geschrieben");
        assert_eq!(&puffer[10..34], &LEERES_EXIF, "das Muster fehlt");
        assert_eq!(&puffer[34..50], &[0x00; 16], "nicht aufgefuellt");
    }

    /// Ist der Block kuerzer als das Muster, wird nur gefuellt -- niemals
    /// ueber den Block hinaus geschrieben.
    #[test]
    fn ein_zu_kurzer_block_wird_nur_gefuellt() {
        let mut puffer = vec![0xAAu8; 20];
        ersetze(&mut puffer, 5, 6, &LEERES_EXIF, 0x00);

        assert_eq!(puffer.len(), 20);
        assert_eq!(&puffer[5..11], &[0x00; 6]);
        assert_eq!(&puffer[11..], &[0xAA; 9], "dahinter wurde geschrieben");
    }

    #[test]
    fn ein_versatz_ausserhalb_wird_verkraftet() {
        let mut puffer = vec![0xAAu8; 10];
        ersetze(&mut puffer, 8, 40, &LEERES_EXIF, 0x00);
        assert_eq!(puffer, vec![0xAAu8; 10], "es wurde doch geschrieben");
    }

    #[test]
    fn die_feldbreiten_werden_richtig_gelesen() {
        assert_eq!(zahl_bei(&[0, 0, 1, 0], 0, 4), Some(256));
        assert_eq!(zahl_bei(&[], 0, 0), Some(0));
        assert_eq!(zahl_bei(&[1, 2, 3], 0, 4), None, "zu kurz");
        assert_eq!(zahl_bei(&[0; 8], 0, 3), None, "unmoegliche Breite");
    }
}
