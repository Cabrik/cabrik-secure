//! TIFF (`spec/metadata.md` §4).
//!
//! # Warum TIFF anders ist als alles Bisherige
//!
//! Bei PNG, JPEG und WebP liegen Metadaten in abgegrenzten Blöcken. Man
//! entfernt den Block, hängt die übrigen aneinander — fertig. Die Bilddaten
//! werden dabei nie angefasst.
//!
//! TIFF funktioniert nicht so. Eine TIFF-Datei ist ein **Verzeichnis mit
//! Verweisen**: Der Kopf zeigt auf das erste IFD (Image File Directory), jeder
//! Eintrag darin ist zwölf Bytes lang, und passt sein Wert nicht in vier
//! Bytes, steht dort statt des Werts ein **Versatz** in die Datei. Auch die
//! Bilddaten selbst hängen an solchen Versätzen (`StripOffsets`,
//! `TileOffsets`).
//!
//! Daraus folgt: **Einen Eintrag zu entfernen verschiebt alles Nachfolgende.**
//! Es genügt nicht, ihn wegzulassen — die Datei muss vollständig neu
//! geschrieben und jeder Versatz neu vergeben werden. Wer das falsch macht,
//! erzeugt eine Datei, die keinen Fehler meldet und trotzdem Müll anzeigt,
//! weil die Bilddaten an der falschen Stelle gesucht werden.
//!
//! Deshalb wird hier nicht gestrichen, sondern **neu gebaut**: Struktur lesen,
//! behalten was bleibt, alles frisch schreiben. Die Bilddaten wandern dabei
//! byteweise unverändert mit; neu berechnet werden ausschließlich die
//! Versätze.
//!
//! # Was BigTIFF angeht
//!
//! BigTIFF (Kennzahl 43 statt 42) hat einen anderen Aufbau mit 64-Bit-
//! Versätzen. Es wird **erkannt und abgelehnt**, nicht halb verstanden. Eine
//! Datei falsch zu behandeln wäre schlimmer, als sie ehrlich unbehandelt zu
//! lassen.

use crate::model::{Finding, FindingKind, Inspection, Severity, StripResult};

use cabrik_core::{Error, Result};

/// Länge des Dateikopfs.
const KOPF_LEN: usize = 8;
/// Länge eines IFD-Eintrags.
const EINTRAG_LEN: usize = 12;
/// Höchstzahl der IFDs — Schutz gegen im Kreis zeigende Verweise.
const MAX_IFDS: usize = 64;
/// Höchstzahl der Einträge je IFD.
const MAX_EINTRAEGE: usize = 4096;
/// Höchstgröße einer Datei, die wir anfassen.
const MAX_DATEI: usize = 256 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Byte-Reihenfolge
// ---------------------------------------------------------------------------

/// In welcher Reihenfolge die Zahlen stehen.
///
/// TIFF gibt das in den ersten beiden Bytes an: `II` für Intel (kleinstes Byte
/// zuerst), `MM` für Motorola. Beides kommt in freier Wildbahn vor, und die
/// Ausgabe behält die Reihenfolge der Eingabe bei — sie zu wechseln wäre eine
/// Änderung ohne Nutzen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reihenfolge {
    Klein,
    Gross,
}

impl Reihenfolge {
    fn u16(self, b: &[u8]) -> Option<u16> {
        let a: [u8; 2] = b.get(..2)?.try_into().ok()?;
        Some(match self {
            Self::Klein => u16::from_le_bytes(a),
            Self::Gross => u16::from_be_bytes(a),
        })
    }

    fn u32(self, b: &[u8]) -> Option<u32> {
        let a: [u8; 4] = b.get(..4)?.try_into().ok()?;
        Some(match self {
            Self::Klein => u32::from_le_bytes(a),
            Self::Gross => u32::from_be_bytes(a),
        })
    }

    fn schreib_u16(self, v: u16) -> [u8; 2] {
        match self {
            Self::Klein => v.to_le_bytes(),
            Self::Gross => v.to_be_bytes(),
        }
    }

    fn schreib_u32(self, v: u32) -> [u8; 4] {
        match self {
            Self::Klein => v.to_le_bytes(),
            Self::Gross => v.to_be_bytes(),
        }
    }
}

/// Größe eines Werttyps in Bytes. `None` bei unbekanntem Typ.
const fn typ_groesse(typ: u16) -> Option<usize> {
    Some(match typ {
        1 | 2 | 6 | 7 => 1,   // BYTE, ASCII, SBYTE, UNDEFINED
        3 | 8 => 2,           // SHORT, SSHORT
        4 | 9 | 11 | 13 => 4, // LONG, SLONG, FLOAT, IFD
        5 | 10 | 12 => 8,     // RATIONAL, SRATIONAL, DOUBLE
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Die Marken, um die es geht
// ---------------------------------------------------------------------------

/// Marken, die entfernt werden, mit ihrer Einordnung.
const METADATEN: [(u16, &str, FindingKind, Severity); 20] = [
    (
        0x010D,
        "DocumentName",
        FindingKind::Comment,
        Severity::Notable,
    ),
    (
        0x010E,
        "ImageDescription",
        FindingKind::Comment,
        Severity::Notable,
    ),
    (0x010F, "Make", FindingKind::Device, Severity::Notable),
    (0x0110, "Model", FindingKind::Device, Severity::Notable),
    (0x0131, "Software", FindingKind::Software, Severity::Notable),
    (
        0x0132,
        "DateTime",
        FindingKind::Timestamp,
        Severity::Notable,
    ),
    (0x013B, "Artist", FindingKind::Author, Severity::Critical),
    (
        0x013C,
        "HostComputer",
        FindingKind::Device,
        Severity::Critical,
    ),
    (0x8298, "Copyright", FindingKind::Author, Severity::Notable),
    // Ganze Unterverzeichnisse. Sie enthalten Dutzende weiterer Einträge —
    // beim Entfernen der Marke verschwinden sie mit, weil nichts mehr auf
    // sie zeigt und die Datei neu gebaut wird.
    (0x8769, "Exif-IFD", FindingKind::Device, Severity::Notable),
    (0x8825, "GPS-IFD", FindingKind::Gps, Severity::Critical),
    (
        0xA005,
        "Interoperability-IFD",
        FindingKind::Device,
        Severity::Minor,
    ),
    (0x02BC, "XMP", FindingKind::Author, Severity::Critical),
    (0x83BB, "IPTC", FindingKind::Author, Severity::Critical),
    (
        0x8649,
        "Photoshop-Ressourcen",
        FindingKind::Software,
        Severity::Notable,
    ),
    (
        0x8773,
        "ICC-Farbprofil",
        FindingKind::ColorProfile,
        Severity::Minor,
    ),
    (0xC4A5, "Print-IM", FindingKind::Software, Severity::Minor),
    (0x9C9B, "XP-Titel", FindingKind::Comment, Severity::Notable),
    (0x9C9D, "XP-Autor", FindingKind::Author, Severity::Critical),
    (
        0x9C9E,
        "XP-Stichwörter",
        FindingKind::Comment,
        Severity::Notable,
    ),
];

/// `SubIFDs` — verweist auf weitere Verzeichnisse, meist Vorschaubilder in
/// verringerter Auflösung. Also **zweite Kopien des Inhalts** (§7.1).
const TAG_SUB_IFDS: u16 = 0x014A;
/// Versätze und Längen der Bilddaten.
const TAG_STREIFEN_VERSATZ: u16 = 0x0111;
const TAG_STREIFEN_LAENGE: u16 = 0x0117;
const TAG_KACHEL_VERSATZ: u16 = 0x0144;
const TAG_KACHEL_LAENGE: u16 = 0x0145;
/// Eingebettetes JPEG. Je nach IFD Vorschaubild **oder** die Bilddaten selbst.
const TAG_JPEG_VERSATZ: u16 = 0x0201;
const TAG_JPEG_LAENGE: u16 = 0x0202;

fn einordnung(tag: u16) -> Option<(&'static str, FindingKind, Severity)> {
    METADATEN
        .iter()
        .find(|(t, ..)| *t == tag)
        .map(|(_, name, art, schwere)| (*name, *art, *schwere))
}

// ---------------------------------------------------------------------------
// Lesen
// ---------------------------------------------------------------------------

/// Ein Eintrag mit seinem bereits aufgelösten Wert.
#[derive(Debug, Clone)]
struct Eintrag {
    tag: u16,
    typ: u16,
    count: u32,
    wert: Wert,
}

/// Wo der Wert eines Eintrags steckt.
#[derive(Debug, Clone)]
enum Wert {
    /// Passt in die vier Bytes des Eintrags.
    Innen([u8; 4]),
    /// Steht anderswo in der Datei.
    Aussen(Vec<u8>),
    /// Versätze auf Bilddaten. Die Daten wandern mit, die Versätze werden
    /// beim Schreiben **neu vergeben**.
    Bilddaten(Vec<Vec<u8>>),
}

/// Ob die Bytes wie ein TIFF aussehen.
#[must_use]
pub fn looks_like_tiff(daten: &[u8]) -> bool {
    let Some(ordnung) = reihenfolge(daten) else {
        return false;
    };
    // Kennzahl 42 = klassisches TIFF, 43 = BigTIFF.
    matches!(ordnung.1.u16(daten.get(2..4).unwrap_or(&[])), Some(42 | 43))
}

fn reihenfolge(daten: &[u8]) -> Option<((), Reihenfolge)> {
    match daten.get(..2)? {
        b"II" => Some(((), Reihenfolge::Klein)),
        b"MM" => Some(((), Reihenfolge::Gross)),
        _ => None,
    }
}

/// Ob es sich um BigTIFF handelt.
fn ist_bigtiff(daten: &[u8]) -> bool {
    reihenfolge(daten)
        .and_then(|(_, o)| o.u16(daten.get(2..4).unwrap_or(&[])))
        .is_some_and(|k| k == 43)
}

/// Liest die IFD-Kette.
fn lies(daten: &[u8]) -> Result<(Reihenfolge, Vec<Vec<Eintrag>>)> {
    if daten.len() > MAX_DATEI {
        return Err(Error::Malformed("tiff: Datei zu gross"));
    }
    if ist_bigtiff(daten) {
        return Err(Error::Malformed("tiff: BigTIFF wird nicht behandelt"));
    }
    let (_, ordnung) =
        reihenfolge(daten).ok_or(Error::Malformed("tiff: keine Byte-Reihenfolge"))?;
    if ordnung.u16(daten.get(2..4).unwrap_or(&[])) != Some(42) {
        return Err(Error::Malformed("tiff: falsche Kennzahl"));
    }

    let mut versatz = usize::try_from(
        ordnung
            .u32(daten.get(4..KOPF_LEN).unwrap_or(&[]))
            .ok_or(Error::Malformed("tiff: Versatz des ersten IFD fehlt"))?,
    )
    .map_err(|_| Error::Malformed("tiff: Versatz zu gross"))?;

    let mut ifds = Vec::new();
    let mut gesehen: Vec<usize> = Vec::new();

    while versatz != 0 {
        if ifds.len() >= MAX_IFDS {
            return Err(Error::Malformed("tiff: zu viele Verzeichnisse"));
        }
        if gesehen.contains(&versatz) {
            // Ein Verweis im Kreis. Ohne diese Prüfung liefe das Lesen ewig.
            return Err(Error::Malformed("tiff: Verzeichnisse zeigen im Kreis"));
        }
        gesehen.push(versatz);

        let (eintraege, naechster) = lies_ifd(daten, ordnung, versatz)?;
        ifds.push(eintraege);
        versatz = naechster;
    }

    if ifds.is_empty() {
        return Err(Error::Malformed("tiff: kein Verzeichnis"));
    }
    Ok((ordnung, ifds))
}

fn lies_ifd(daten: &[u8], ordnung: Reihenfolge, versatz: usize) -> Result<(Vec<Eintrag>, usize)> {
    let anzahl = usize::from(
        ordnung
            .u16(daten.get(versatz..).unwrap_or(&[]))
            .ok_or(Error::Malformed("tiff: Eintragszahl unlesbar"))?,
    );
    if anzahl > MAX_EINTRAEGE {
        return Err(Error::Malformed("tiff: zu viele Eintraege"));
    }

    let mut roh: Vec<(u16, u16, u32, [u8; 4])> = Vec::with_capacity(anzahl);
    for i in 0..anzahl {
        let start = versatz
            .checked_add(2)
            .and_then(|v| v.checked_add(i.checked_mul(EINTRAG_LEN)?))
            .ok_or(Error::Malformed("tiff: Eintragsversatz ueberlaeuft"))?;
        let feld = daten
            .get(start..start.saturating_add(EINTRAG_LEN))
            .ok_or(Error::Malformed("tiff: Eintrag reicht ueber das Dateiende"))?;

        let tag = ordnung
            .u16(feld)
            .ok_or(Error::Malformed("tiff: Marke unlesbar"))?;
        let typ = ordnung
            .u16(feld.get(2..).unwrap_or(&[]))
            .ok_or(Error::Malformed("tiff: Typ unlesbar"))?;
        let count = ordnung
            .u32(feld.get(4..).unwrap_or(&[]))
            .ok_or(Error::Malformed("tiff: Anzahl unlesbar"))?;
        let wert: [u8; 4] = feld
            .get(8..12)
            .and_then(|s| s.try_into().ok())
            .ok_or(Error::Malformed("tiff: Wertfeld unlesbar"))?;
        roh.push((tag, typ, count, wert));
    }

    let naechster_start = versatz
        .checked_add(2)
        .and_then(|v| v.checked_add(anzahl.checked_mul(EINTRAG_LEN)?))
        .ok_or(Error::Malformed("tiff: Kettenversatz ueberlaeuft"))?;
    let naechster = usize::try_from(
        ordnung
            .u32(daten.get(naechster_start..).unwrap_or(&[]))
            .ok_or(Error::Malformed("tiff: Kettenversatz unlesbar"))?,
    )
    .map_err(|_| Error::Malformed("tiff: Kettenversatz zu gross"))?;

    // Erst jetzt die Werte auflösen: Für die Bilddaten wird der zugehörige
    // Längeneintrag desselben IFD gebraucht.
    let mut eintraege = Vec::with_capacity(roh.len());
    for &(tag, typ, count, feld) in &roh {
        let wert = loese_wert(daten, ordnung, tag, typ, count, feld, &roh)?;
        eintraege.push(Eintrag {
            tag,
            typ,
            count,
            wert,
        });
    }

    Ok((eintraege, naechster))
}

/// Löst den Wert eines Eintrags auf.
fn loese_wert(
    daten: &[u8],
    ordnung: Reihenfolge,
    tag: u16,
    typ: u16,
    count: u32,
    feld: [u8; 4],
    alle: &[(u16, u16, u32, [u8; 4])],
) -> Result<Wert> {
    // Bilddaten: Die Werte sind Versätze, die zugehörigen Längen stehen in
    // einem anderen Eintrag desselben Verzeichnisses.
    if let Some(laengen_tag) = laengen_marke(tag)
        && let Some(bloecke) = lies_bilddaten(daten, ordnung, typ, count, feld, alle, laengen_tag)?
    {
        return Ok(Wert::Bilddaten(bloecke));
    }

    let groesse = typ_groesse(typ).unwrap_or(1);
    let gesamt = usize::try_from(count)
        .ok()
        .and_then(|c| c.checked_mul(groesse))
        .ok_or(Error::Malformed("tiff: Wertlaenge ueberlaeuft"))?;

    if gesamt <= 4 {
        return Ok(Wert::Innen(feld));
    }

    let versatz = usize::try_from(
        ordnung
            .u32(&feld)
            .ok_or(Error::Malformed("tiff: Wertversatz unlesbar"))?,
    )
    .map_err(|_| Error::Malformed("tiff: Wertversatz zu gross"))?;

    let bytes = daten
        .get(versatz..versatz.saturating_add(gesamt))
        .ok_or(Error::Malformed("tiff: Wert reicht ueber das Dateiende"))?;
    Ok(Wert::Aussen(bytes.to_vec()))
}

/// Zu welcher Versatz-Marke die Längen-Marke gehört.
const fn laengen_marke(tag: u16) -> Option<u16> {
    match tag {
        TAG_STREIFEN_VERSATZ => Some(TAG_STREIFEN_LAENGE),
        TAG_KACHEL_VERSATZ => Some(TAG_KACHEL_LAENGE),
        TAG_JPEG_VERSATZ => Some(TAG_JPEG_LAENGE),
        _ => None,
    }
}

/// Liest die Bilddatenblöcke, auf die ein Versatz-Eintrag zeigt.
fn lies_bilddaten(
    daten: &[u8],
    ordnung: Reihenfolge,
    typ: u16,
    count: u32,
    feld: [u8; 4],
    alle: &[(u16, u16, u32, [u8; 4])],
    laengen_tag: u16,
) -> Result<Option<Vec<Vec<u8>>>> {
    let Some(&(_, l_typ, l_count, l_feld)) = alle.iter().find(|(t, ..)| *t == laengen_tag) else {
        // Ohne Längenangabe lässt sich nichts verlässlich lesen.
        return Ok(None);
    };

    let versaetze = zahlen(daten, ordnung, typ, count, feld)?;
    let laengen = zahlen(daten, ordnung, l_typ, l_count, l_feld)?;
    if versaetze.len() != laengen.len() {
        return Err(Error::Malformed("tiff: Versaetze und Laengen passen nicht"));
    }

    let mut bloecke = Vec::with_capacity(versaetze.len());
    for (v, l) in versaetze.iter().zip(laengen.iter()) {
        let start = usize::try_from(*v).map_err(|_| Error::Malformed("tiff: Versatz zu gross"))?;
        let laenge = usize::try_from(*l).map_err(|_| Error::Malformed("tiff: Laenge zu gross"))?;
        let block = daten
            .get(start..start.saturating_add(laenge))
            .ok_or(Error::Malformed("tiff: Bilddaten reichen ueber das Ende"))?;
        bloecke.push(block.to_vec());
    }
    Ok(Some(bloecke))
}

/// Liest eine Folge von SHORT- oder LONG-Werten.
fn zahlen(
    daten: &[u8],
    ordnung: Reihenfolge,
    typ: u16,
    count: u32,
    feld: [u8; 4],
) -> Result<Vec<u32>> {
    let groesse = match typ {
        3 => 2usize,
        4 => 4usize,
        _ => return Err(Error::Malformed("tiff: unerwarteter Typ fuer Versaetze")),
    };
    let anzahl = usize::try_from(count).map_err(|_| Error::Malformed("tiff: Anzahl zu gross"))?;
    let gesamt = anzahl
        .checked_mul(groesse)
        .ok_or(Error::Malformed("tiff: Laenge ueberlaeuft"))?;

    let quelle: &[u8] = if gesamt <= 4 {
        &feld
    } else {
        let versatz = usize::try_from(
            ordnung
                .u32(&feld)
                .ok_or(Error::Malformed("tiff: Versatz unlesbar"))?,
        )
        .map_err(|_| Error::Malformed("tiff: Versatz zu gross"))?;
        daten
            .get(versatz..versatz.saturating_add(gesamt))
            .ok_or(Error::Malformed("tiff: Versatzfeld reicht ueber das Ende"))?
    };

    let mut aus = Vec::with_capacity(anzahl);
    for i in 0..anzahl {
        let start = i.saturating_mul(groesse);
        let stueck = quelle
            .get(start..start.saturating_add(groesse))
            .ok_or(Error::Malformed("tiff: Zahl unlesbar"))?;
        aus.push(if groesse == 2 {
            u32::from(
                ordnung
                    .u16(stueck)
                    .ok_or(Error::Malformed("tiff: SHORT unlesbar"))?,
            )
        } else {
            ordnung
                .u32(stueck)
                .ok_or(Error::Malformed("tiff: LONG unlesbar"))?
        });
    }
    Ok(aus)
}

// ---------------------------------------------------------------------------
// Beurteilen
// ---------------------------------------------------------------------------

/// Ob dieser Eintrag entfernt wird.
fn ist_metadatum(e: &Eintrag, ifd: &[Eintrag], ifd_nummer: usize) -> bool {
    if einordnung(e.tag).is_some() || e.tag == TAG_SUB_IFDS {
        return true;
    }
    // Ein eingebettetes JPEG ist nur dann Vorschaubild, wenn das Verzeichnis
    // daneben eigene Bilddaten führt. Sonst **ist** es das Bild.
    if matches!(e.tag, TAG_JPEG_VERSATZ | TAG_JPEG_LAENGE) {
        return ifd
            .iter()
            .any(|x| matches!(x.tag, TAG_STREIFEN_VERSATZ | TAG_KACHEL_VERSATZ))
            || ifd_nummer > 0;
    }
    false
}

fn sammle(ifds: &[Vec<Eintrag>]) -> Vec<Finding> {
    let mut funde = Vec::new();

    for (nr, ifd) in ifds.iter().enumerate() {
        let ort = |name: &str| {
            if nr == 0 {
                format!("TIFF:{name}")
            } else {
                format!("TIFF:IFD{nr}/{name}")
            }
        };

        for e in ifd {
            if let Some((name, art, schwere)) = einordnung(e.tag) {
                funde.push(Finding::new(
                    art,
                    ort(name),
                    Some(beschreibe(e, name)),
                    schwere,
                ));
            } else if e.tag == TAG_SUB_IFDS {
                funde.push(Finding::new(
                    FindingKind::EmbeddedPreview,
                    ort("SubIFDs"),
                    Some(format!(
                        "{} weitere(s) Verzeichnis(se) — meist Vorschaubilder in \
                         verringerter Auflösung, also zweite Kopien des Inhalts",
                        e.count
                    )),
                    Severity::Critical,
                ));
            } else if e.tag == TAG_JPEG_VERSATZ && ist_metadatum(e, ifd, nr) {
                funde.push(Finding::new(
                    FindingKind::EmbeddedPreview,
                    ort("Vorschaubild"),
                    Some("eingebettetes JPEG — eine zweite Kopie des Inhalts".to_owned()),
                    Severity::Critical,
                ));
            }
        }

        // Ein weiteres Verzeichnis in der Kette ist bei Bilddateien fast
        // immer ein Vorschaubild; bei mehrseitigen Scans dagegen eine Seite.
        // Unterschieden wird an `NewSubfileType` (0x00FE, Bit 0 = verkleinert).
        if nr > 0 && ist_verkleinert(ifd) {
            funde.push(Finding::new(
                FindingKind::EmbeddedPreview,
                ort("Verzeichnis"),
                Some(
                    "ein Verzeichnis mit verkleinertem Bild — eine zweite Kopie \
                     des Inhalts"
                        .to_owned(),
                ),
                Severity::Critical,
            ));
        }
    }
    funde
}

// ---------------------------------------------------------------------------
// Rohdateien aus Kameras
// ---------------------------------------------------------------------------
//
// DNG, NEF, ARW, CR2 und die meisten anderen Rohformate **sind** TIFF: dieselbe
// Byte-Reihenfolge, dieselbe Kennzahl 42, dieselbe Verzeichnisstruktur. Die
// Erkennung oben beansprucht sie deshalb, und das wäre beinahe fatal geblieben.
//
// DENN SIE SIND ANDERS AUFGEBAUT. In einer gewöhnlichen TIFF-Datei steht das
// Bild im ersten Verzeichnis, und ein `SubIFD` trägt eine verkleinerte
// Fassung. In einer Rohdatei ist es **umgekehrt**: Das erste Verzeichnis
// enthält nur eine Vorschau, das eigentliche Foto liegt im `SubIFD`.
//
// Dieses Modul entfernt `SubIFDs` als Vorschaubilder — bei einer Rohdatei
// entfernt es damit das Foto und meldet „vollständig bereinigt". Eine
// 24-Megapixel-Aufnahme wurde im Versuch zu einem 8×8-Vorschaubild, ohne
// jede Warnung.
//
// Hinzu kommt, was auch ein richtiges Erkennen nicht löste: Der `MakerNote`
// enthält Versätze, die **relativ zum Dateianfang** gezählt sind. Dieses
// Modul baut die Datei neu auf und vergibt alle Versätze neu — jeder
// Zeiger im `MakerNote` zeigt danach ins Leere. Teile davon sind zudem
// verschlüsselt und herstellereigen.
//
// Deshalb: erkennen und **unangetastet lassen**. Was drinsteht, wird
// trotzdem gemeldet — das ist die eigentlich nützliche Hälfte.

/// `DNGVersion` — eindeutig eine Adobe-Rohdatei.
const TAG_DNG_FASSUNG: u16 = 0xC612;
/// `CFAPattern` und `CFARepeatPatternDim` — Farbfiltermatrix des Sensors.
const TAG_CFA_MUSTER: u16 = 0x828E;
const TAG_CFA_MASSE: u16 = 0x828D;
/// `PhotometricInterpretation`.
const TAG_FARBDEUTUNG: u16 = 0x0106;
/// Farbfiltermatrix beziehungsweise linearisierte Rohdaten — beides gibt es
/// nur in Rohdateien.
const DEUTUNG_CFA: u32 = 32803;
const DEUTUNG_LINEAR_ROH: u32 = 34892;

/// Liest die Zahlen eines **schon gelesenen** Eintrags.
///
/// Anders als [`zahlen`] greift diese Fassung nicht mehr auf die Datei zu —
/// der Wert liegt bereits in [`Wert`] vor.
fn eintrag_zahlen(e: &Eintrag, ordnung: Reihenfolge) -> Vec<u32> {
    let roh: &[u8] = match &e.wert {
        Wert::Innen(b) => b,
        Wert::Aussen(v) => v,
        Wert::Bilddaten(_) => return Vec::new(),
    };
    let n = usize::try_from(e.count).unwrap_or(0);
    match e.typ {
        3 => roh
            .chunks_exact(2)
            .take(n)
            .filter_map(|c| ordnung.u16(c))
            .map(u32::from)
            .collect(),
        4 => roh
            .chunks_exact(4)
            .take(n)
            .filter_map(|c| ordnung.u32(c))
            .collect(),
        _ => Vec::new(),
    }
}

/// Ob ein Verzeichnis Sensordaten beschreibt.
fn ifd_ist_roh(ifd: &[Eintrag], ordnung: Reihenfolge) -> bool {
    ifd.iter().any(|e| {
        matches!(e.tag, TAG_DNG_FASSUNG | TAG_CFA_MUSTER | TAG_CFA_MASSE)
            || (e.tag == TAG_FARBDEUTUNG
                && eintrag_zahlen(e, ordnung)
                    .iter()
                    .any(|w| matches!(*w, DEUTUNG_CFA | DEUTUNG_LINEAR_ROH)))
    })
}

/// Ob die Datei eine Rohdatei ist — und aus welchem Grund.
///
/// Beide Prüfungen sind **strukturell**. Eine Liste von Herstellern und
/// Dateiendungen wäre immer unvollständig; ein Verzeichnis, das sich selbst
/// als Vorschau ausweist, ist es nie.
fn roh_grund(daten: &[u8], ordnung: Reihenfolge, ifds: &[Vec<Eintrag>]) -> Option<&'static str> {
    if ifds.iter().any(|ifd| ifd_ist_roh(ifd, ordnung)) {
        return Some("die Datei enthält unentwickelte Sensordaten");
    }

    let erstes = ifds.first()?;
    // Ein erstes Verzeichnis, das sich selbst als verkleinerte Fassung
    // ausweist, kann nicht das Hauptbild enthalten. Dann liegt es woanders.
    if ist_verkleinert(erstes) && erstes.iter().any(|e| e.tag == TAG_SUB_IFDS) {
        return Some(
            "das erste Verzeichnis ist nur eine Vorschau, das Hauptbild liegt in einem SubIFD",
        );
    }

    // Und zur Sicherheit auch in die SubIFDs selbst sehen.
    let unter: Vec<u32> = erstes
        .iter()
        .filter(|e| e.tag == TAG_SUB_IFDS)
        .flat_map(|e| eintrag_zahlen(e, ordnung))
        .collect();
    for v in unter.iter().take(MAX_IFDS) {
        let Ok(pos) = usize::try_from(*v) else {
            continue;
        };
        if let Ok((unter_ifd, _)) = lies_ifd(daten, ordnung, pos)
            && ifd_ist_roh(&unter_ifd, ordnung)
        {
            return Some("ein SubIFD enthält unentwickelte Sensordaten");
        }
    }
    None
}

/// Ob das Verzeichnis sich selbst als verkleinerte Fassung ausweist.
///
/// Marke 254 (`NewSubfileType`), Bit 0. Sie unterscheidet die Seite eines
/// Scans von einem Vorschaubild — und, wie sich zeigte, ein gewöhnliches
/// TIFF von einer Rohdatei.
fn ist_verkleinert(ifd: &[Eintrag]) -> bool {
    ifd.iter().any(|e| {
        e.tag == 0x00FE
            && match &e.wert {
                Wert::Innen(b) => b.iter().any(|x| x & 0x01 != 0),
                _ => false,
            }
    })
}

/// Beschreibt einen Eintrag für die Meldung.
fn beschreibe(e: &Eintrag, name: &str) -> String {
    // ASCII-Werte lassen sich anzeigen und sind das Aussagekräftigste.
    if e.typ == 2 {
        let roh: &[u8] = match &e.wert {
            Wert::Innen(b) => b,
            Wert::Aussen(v) => v,
            Wert::Bilddaten(_) => &[],
        };
        let text = String::from_utf8_lossy(roh)
            .trim_end_matches('\0')
            .trim()
            .to_owned();
        if !text.is_empty() {
            return text;
        }
    }
    let bytes = match &e.wert {
        Wert::Innen(_) => 4,
        Wert::Aussen(v) => v.len(),
        Wert::Bilddaten(b) => b.iter().map(Vec::len).sum(),
    };
    format!("{name}, {bytes} Bytes")
}

// ---------------------------------------------------------------------------
// Schreiben
// ---------------------------------------------------------------------------

/// Baut die Datei neu auf.
///
/// Alle Versätze werden dabei frisch vergeben — siehe Modulkopf.
fn baue(ordnung: Reihenfolge, ifds: &[Vec<Eintrag>]) -> Result<Vec<u8>> {
    // Erst die Größe aller Verzeichnisse, damit feststeht, wo der Datenbereich
    // beginnt. Vorher lässt sich kein einziger Versatz vergeben.
    let mut ifd_bereich = 0usize;
    for ifd in ifds {
        ifd_bereich = ifd_bereich
            .checked_add(2)
            .and_then(|v| v.checked_add(ifd.len().checked_mul(EINTRAG_LEN)?))
            .and_then(|v| v.checked_add(4))
            .ok_or(Error::Malformed("tiff: Verzeichnisgroesse ueberlaeuft"))?;
    }

    let daten_start = KOPF_LEN
        .checked_add(ifd_bereich)
        .ok_or(Error::Malformed("tiff: Datenbereich ueberlaeuft"))?;

    let mut verzeichnisse: Vec<u8> = Vec::with_capacity(ifd_bereich);
    let mut anhang: Vec<u8> = Vec::new();

    // Versatz des jeweils nächsten Verzeichnisses.
    let mut naechster_ifd_versatz = KOPF_LEN;

    for (nr, ifd) in ifds.iter().enumerate() {
        naechster_ifd_versatz = naechster_ifd_versatz
            .checked_add(2)
            .and_then(|v| v.checked_add(ifd.len().checked_mul(EINTRAG_LEN)?))
            .and_then(|v| v.checked_add(4))
            .ok_or(Error::Malformed("tiff: Verzeichnisversatz ueberlaeuft"))?;

        let anzahl =
            u16::try_from(ifd.len()).map_err(|_| Error::Malformed("tiff: zu viele Eintraege"))?;
        verzeichnisse.extend_from_slice(&ordnung.schreib_u16(anzahl));

        for e in ifd {
            verzeichnisse.extend_from_slice(&ordnung.schreib_u16(e.tag));

            match &e.wert {
                Wert::Innen(b) => {
                    verzeichnisse.extend_from_slice(&ordnung.schreib_u16(e.typ));
                    verzeichnisse.extend_from_slice(&ordnung.schreib_u32(e.count));
                    verzeichnisse.extend_from_slice(b);
                }
                Wert::Aussen(v) => {
                    verzeichnisse.extend_from_slice(&ordnung.schreib_u16(e.typ));
                    verzeichnisse.extend_from_slice(&ordnung.schreib_u32(e.count));
                    let versatz = daten_start
                        .checked_add(anhang.len())
                        .ok_or(Error::Malformed("tiff: Wertversatz ueberlaeuft"))?;
                    verzeichnisse.extend_from_slice(
                        &ordnung.schreib_u32(
                            u32::try_from(versatz)
                                .map_err(|_| Error::Malformed("tiff: Datei zu gross"))?,
                        ),
                    );
                    anhang.extend_from_slice(v);
                    // TIFF verlangt gerade Versätze für Werte.
                    if anhang.len() % 2 == 1 {
                        anhang.push(0);
                    }
                }
                Wert::Bilddaten(bloecke) => {
                    // Die Blöcke zuerst ablegen, dann die neuen Versätze —
                    // immer als LONG, damit die Länge feststeht.
                    let mut neue: Vec<u32> = Vec::with_capacity(bloecke.len());
                    for b in bloecke {
                        if anhang.len() % 2 == 1 {
                            anhang.push(0);
                        }
                        let versatz = daten_start
                            .checked_add(anhang.len())
                            .ok_or(Error::Malformed("tiff: Bilddatenversatz ueberlaeuft"))?;
                        neue.push(
                            u32::try_from(versatz)
                                .map_err(|_| Error::Malformed("tiff: Datei zu gross"))?,
                        );
                        anhang.extend_from_slice(b);
                    }

                    verzeichnisse.extend_from_slice(&ordnung.schreib_u16(4)); // LONG
                    verzeichnisse.extend_from_slice(
                        &ordnung.schreib_u32(
                            u32::try_from(neue.len())
                                .map_err(|_| Error::Malformed("tiff: zu viele Bloecke"))?,
                        ),
                    );

                    if neue.len() == 1 {
                        let einziger = neue.first().copied().unwrap_or(0);
                        verzeichnisse.extend_from_slice(&ordnung.schreib_u32(einziger));
                    } else {
                        if anhang.len() % 2 == 1 {
                            anhang.push(0);
                        }
                        let tabelle_versatz = daten_start
                            .checked_add(anhang.len())
                            .ok_or(Error::Malformed("tiff: Tabellenversatz ueberlaeuft"))?;
                        verzeichnisse.extend_from_slice(
                            &ordnung.schreib_u32(
                                u32::try_from(tabelle_versatz)
                                    .map_err(|_| Error::Malformed("tiff: Datei zu gross"))?,
                            ),
                        );
                        for v in &neue {
                            anhang.extend_from_slice(&ordnung.schreib_u32(*v));
                        }
                    }
                }
            }
        }

        // Verweis auf das nächste Verzeichnis, null beim letzten.
        let weiter = if nr.saturating_add(1) < ifds.len() {
            u32::try_from(naechster_ifd_versatz)
                .map_err(|_| Error::Malformed("tiff: Datei zu gross"))?
        } else {
            0
        };
        verzeichnisse.extend_from_slice(&ordnung.schreib_u32(weiter));
    }

    let mut aus = Vec::with_capacity(daten_start.saturating_add(anhang.len()));
    aus.extend_from_slice(match ordnung {
        Reihenfolge::Klein => b"II",
        Reihenfolge::Gross => b"MM",
    });
    aus.extend_from_slice(&ordnung.schreib_u16(42));
    aus.extend_from_slice(
        &ordnung.schreib_u32(
            u32::try_from(KOPF_LEN).map_err(|_| Error::Malformed("tiff: unmoeglich"))?,
        ),
    );
    aus.extend_from_slice(&verzeichnisse);
    debug_assert_eq!(aus.len(), daten_start, "Datenbereich beginnt woanders");
    aus.extend_from_slice(&anhang);
    Ok(aus)
}

// ---------------------------------------------------------------------------
// Öffentlich
// ---------------------------------------------------------------------------

/// Untersucht ein TIFF.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Struktur oder BigTIFF.
pub fn inspect(daten: &[u8]) -> Result<Inspection> {
    let (ordnung, ifds) = lies(daten)?;
    let roh = roh_grund(daten, ordnung, &ifds);

    let mut funde = sammle(&ifds);
    if let Some(grund) = roh {
        funde.push(Finding::new(
            FindingKind::UnknownExtension,
            "TIFF:Rohdatei".to_owned(),
            Some(format!(
                "{grund} — die Datei wird deshalb gemeldet, aber nicht umgeschrieben"
            )),
            Severity::Notable,
        ));
    }

    Ok(Inspection {
        format: Some(match (roh.is_some(), ifds.len()) {
            (true, _) => "TIFF-Rohdatei (DNG, NEF, ARW, CR2)".to_owned(),
            (false, n) if n > 1 => format!("TIFF ({n} Verzeichnisse)"),
            _ => "TIFF".to_owned(),
        }),
        findings: funde,
        understood: true,
    })
}

/// Entfernt die Metadaten und baut die Datei neu auf.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Struktur oder BigTIFF.
pub fn strip(daten: &[u8]) -> Result<(Vec<u8>, StripResult)> {
    let (ordnung, ifds) = lies(daten)?;
    let entfernt = sammle(&ifds);

    // **Rohdateien werden nicht angefasst.** Siehe die Begründung oben bei
    // `roh_grund`: Dieses Modul hielte das Hauptbild für ein Vorschaubild.
    if let Some(grund) = roh_grund(daten, ordnung, &ifds) {
        let mut reste = entfernt;
        reste.push(Finding::new(
            FindingKind::UnknownExtension,
            "TIFF:Rohdatei".to_owned(),
            Some(grund.to_owned()),
            Severity::Notable,
        ));
        return Ok((
            daten.to_vec(),
            StripResult::Partial {
                removed: Vec::new(),
                remaining: reste,
                reason: format!(
                    "{grund}. Eine Rohdatei umzuschreiben hieße, ihr Hauptbild für ein \
                     Vorschaubild zu halten und die herstellereigenen Angaben mit ihren \
                     Verweisen zu zerreißen. Wer die Aufnahme weitergeben will, exportiert \
                     sie als JPEG oder TIFF — das Ergebnis wird dann vollständig bereinigt"
                ),
            },
        ));
    }

    let mut behalten: Vec<Vec<Eintrag>> = Vec::with_capacity(ifds.len());
    for (nr, ifd) in ifds.iter().enumerate() {
        // Ein Verzeichnis, das sich als verkleinerte Fassung ausweist, ist
        // ein Vorschaubild und fällt ganz weg. Seiten eines mehrseitigen
        // Scans weisen sich **nicht** so aus und bleiben.
        if nr > 0 && ist_verkleinert(ifd) {
            continue;
        }
        let gefiltert: Vec<Eintrag> = ifd
            .iter()
            .filter(|e| !ist_metadatum(e, ifd, nr))
            .cloned()
            .collect();
        behalten.push(gefiltert);
    }

    let aus = baue(ordnung, &behalten)?;
    Ok((aus, StripResult::Complete { removed: entfernt }))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;

    /// Baut ein TIFF von Hand — kleinstmöglich, aber echt.
    struct Bauer {
        ordnung: Reihenfolge,
        eintraege: Vec<(u16, u16, u32, Vec<u8>)>,
        bilddaten: Vec<u8>,
    }

    impl Bauer {
        fn neu(ordnung: Reihenfolge) -> Self {
            Self {
                ordnung,
                eintraege: Vec::new(),
                bilddaten: b"BILDDATEN-UNVERAENDERT".to_vec(),
            }
        }

        fn kurz(&mut self, tag: u16, wert: u16) -> &mut Self {
            let mut v = self.ordnung.schreib_u16(wert).to_vec();
            v.extend_from_slice(&[0, 0]);
            self.eintraege.push((tag, 3, 1, v));
            self
        }

        fn text(&mut self, tag: u16, wert: &str) -> &mut Self {
            let mut v = wert.as_bytes().to_vec();
            v.push(0);
            let n = u32::try_from(v.len()).unwrap();
            self.eintraege.push((tag, 2, n, v));
            self
        }

        fn lang(&mut self, tag: u16, wert: u32) -> &mut Self {
            self.eintraege
                .push((tag, 4, 1, self.ordnung.schreib_u32(wert).to_vec()));
            self
        }

        fn fertig(&self) -> Vec<u8> {
            // Marken müssen aufsteigend stehen.
            let mut eintraege = self.eintraege.clone();
            eintraege.push((
                TAG_STREIFEN_VERSATZ,
                4,
                1,
                vec![0, 0, 0, 0], // wird unten gefuellt
            ));
            eintraege.push((
                TAG_STREIFEN_LAENGE,
                4,
                1,
                self.ordnung
                    .schreib_u32(u32::try_from(self.bilddaten.len()).unwrap())
                    .to_vec(),
            ));
            eintraege.sort_by_key(|(t, ..)| *t);

            let ifd_len = 2 + eintraege.len() * EINTRAG_LEN + 4;
            let daten_start = KOPF_LEN + ifd_len;

            let mut anhang: Vec<u8> = Vec::new();
            let mut feld_von: Vec<[u8; 4]> = Vec::with_capacity(eintraege.len());

            for (tag, _typ, _count, wert) in &eintraege {
                if *tag == TAG_STREIFEN_VERSATZ {
                    // Platzhalter; der echte Versatz folgt nach dem Anhang.
                    feld_von.push([0xFF, 0xFF, 0xFF, 0xFF]);
                    continue;
                }
                if wert.len() <= 4 {
                    let mut f = [0u8; 4];
                    f[..wert.len()].copy_from_slice(wert);
                    feld_von.push(f);
                } else {
                    let versatz = daten_start + anhang.len();
                    feld_von.push(self.ordnung.schreib_u32(u32::try_from(versatz).unwrap()));
                    anhang.extend_from_slice(wert);
                    if anhang.len() % 2 == 1 {
                        anhang.push(0);
                    }
                }
            }

            // Bilddaten ans Ende.
            if anhang.len() % 2 == 1 {
                anhang.push(0);
            }
            let bild_versatz = daten_start + anhang.len();
            anhang.extend_from_slice(&self.bilddaten);

            for (i, (tag, ..)) in eintraege.iter().enumerate() {
                if *tag == TAG_STREIFEN_VERSATZ {
                    feld_von[i] = self
                        .ordnung
                        .schreib_u32(u32::try_from(bild_versatz).unwrap());
                }
            }

            let mut aus = match self.ordnung {
                Reihenfolge::Klein => b"II".to_vec(),
                Reihenfolge::Gross => b"MM".to_vec(),
            };
            aus.extend_from_slice(&self.ordnung.schreib_u16(42));
            aus.extend_from_slice(&self.ordnung.schreib_u32(u32::try_from(KOPF_LEN).unwrap()));

            aus.extend_from_slice(
                &self
                    .ordnung
                    .schreib_u16(u16::try_from(eintraege.len()).unwrap()),
            );
            for (i, (tag, typ, count, _)) in eintraege.iter().enumerate() {
                aus.extend_from_slice(&self.ordnung.schreib_u16(*tag));
                aus.extend_from_slice(&self.ordnung.schreib_u16(*typ));
                aus.extend_from_slice(&self.ordnung.schreib_u32(*count));
                aus.extend_from_slice(&feld_von[i]);
            }
            aus.extend_from_slice(&self.ordnung.schreib_u32(0));
            aus.extend_from_slice(&anhang);
            aus
        }
    }

    fn bild(ordnung: Reihenfolge) -> Vec<u8> {
        let mut b = Bauer::neu(ordnung);
        b.kurz(0x0100, 4) // ImageWidth
            .kurz(0x0101, 4) // ImageLength
            .kurz(0x0102, 8) // BitsPerSample
            .kurz(0x0103, 1) // Compression = keine
            .kurz(0x0106, 1) // Photometric
            .kurz(0x0115, 1) // SamplesPerPixel
            .kurz(0x0116, 4) // RowsPerStrip
            .text(0x010F, "Kamerahersteller") // Make
            .text(0x0110, "Modell XY-2000") // Model
            .text(0x0131, "Bearbeitungsprogramm 3.1") // Software
            .text(0x0132, "2026:03:01 09:12:00") // DateTime
            .text(0x013B, "Dr. Anna Beispiel") // Artist
            .text(0x013C, "ARBEITSPLATZ-MUSTER") // HostComputer
            .text(0x010E, "Interne Fassung") // ImageDescription
            .lang(0x8825, 12345) // GPS-IFD
            .lang(0x8769, 23456); // Exif-IFD
        b.fertig()
    }

    #[test]
    fn tiff_wird_an_beiden_byte_reihenfolgen_erkannt() {
        assert!(looks_like_tiff(&bild(Reihenfolge::Klein)));
        assert!(looks_like_tiff(&bild(Reihenfolge::Gross)));
        assert!(!looks_like_tiff(b"II\x00\x00"));
        assert!(!looks_like_tiff(b"XX\x2a\x00"));
    }

    /// BigTIFF wird erkannt und **abgelehnt**, nicht halb verstanden.
    #[test]
    fn bigtiff_wird_ehrlich_abgelehnt() {
        let mut roh = b"II".to_vec();
        roh.extend_from_slice(&43u16.to_le_bytes());
        roh.extend_from_slice(&[8, 0, 0, 0, 0, 0, 0, 0]);
        assert!(looks_like_tiff(&roh), "es ist ein TIFF");
        assert!(inspect(&roh).is_err(), "es wurde halb verstanden");
    }

    #[test]
    fn alle_metadaten_werden_gefunden() {
        for ordnung in [Reihenfolge::Klein, Reihenfolge::Gross] {
            let i = inspect(&bild(ordnung)).unwrap();
            for erwartet in [
                "TIFF:Make",
                "TIFF:Model",
                "TIFF:Software",
                "TIFF:DateTime",
                "TIFF:Artist",
                "TIFF:HostComputer",
                "TIFF:ImageDescription",
                "TIFF:GPS-IFD",
                "TIFF:Exif-IFD",
            ] {
                assert!(
                    i.findings.iter().any(|f| f.location == erwartet),
                    "{erwartet} fehlt bei {ordnung:?}: {:?}",
                    i.findings.iter().map(|f| &f.location).collect::<Vec<_>>()
                );
            }
        }
    }

    #[test]
    fn die_werte_werden_lesbar_gemeldet() {
        let i = inspect(&bild(Reihenfolge::Klein)).unwrap();
        let kuenstler = i
            .findings
            .iter()
            .find(|f| f.location == "TIFF:Artist")
            .unwrap();
        assert_eq!(kuenstler.value.as_deref(), Some("Dr. Anna Beispiel"));
        assert_eq!(kuenstler.severity, Severity::Critical);

        let gps = i
            .findings
            .iter()
            .find(|f| f.location == "TIFF:GPS-IFD")
            .unwrap();
        assert_eq!(gps.kind, FindingKind::Gps);
        assert_eq!(gps.severity, Severity::Critical);
    }

    /// **Der Kern des Moduls.** Nach dem Neubau muessen die Bilddaten
    /// byteweise dieselben sein und ueber den neuen Versatz erreichbar.
    #[test]
    fn die_bilddaten_ueberleben_den_neubau_bitgenau() {
        for ordnung in [Reihenfolge::Klein, Reihenfolge::Gross] {
            let (sauber, _) = strip(&bild(ordnung)).unwrap();
            let (o, ifds) = lies(&sauber).unwrap();
            assert_eq!(o, ordnung, "die Byte-Reihenfolge wurde gewechselt");

            let streifen = ifds[0]
                .iter()
                .find(|e| e.tag == TAG_STREIFEN_VERSATZ)
                .expect("Bilddaten verschwunden");
            match &streifen.wert {
                Wert::Bilddaten(b) => {
                    assert_eq!(b.len(), 1);
                    assert_eq!(b[0], b"BILDDATEN-UNVERAENDERT");
                }
                other => panic!("erwartete Bilddaten, bekam {other:?}"),
            }
        }
    }

    #[test]
    fn die_metadaten_sind_danach_wirklich_weg() {
        let (sauber, ergebnis) = strip(&bild(Reihenfolge::Klein)).unwrap();
        assert!(ergebnis.may_show_clean());

        for spur in [
            &b"Dr. Anna Beispiel"[..],
            b"Kamerahersteller",
            b"XY-2000",
            b"ARBEITSPLATZ-MUSTER",
            b"Interne Fassung",
        ] {
            assert!(
                !sauber.windows(spur.len()).any(|f| f == spur),
                "Spur blieb in den Bytes: {spur:?}"
            );
        }

        let i = inspect(&sauber).unwrap();
        assert!(i.findings.is_empty(), "es blieb etwas: {:?}", i.findings);
    }

    /// Die Strukturmarken duerfen **nicht** mit verschwinden.
    #[test]
    fn die_strukturmarken_bleiben() {
        let (sauber, _) = strip(&bild(Reihenfolge::Klein)).unwrap();
        let (_, ifds) = lies(&sauber).unwrap();
        let marken: Vec<u16> = ifds[0].iter().map(|e| e.tag).collect();

        for noetig in [0x0100, 0x0101, 0x0102, 0x0103, 0x0106, 0x0115, 0x0116] {
            assert!(marken.contains(&noetig), "Marke 0x{noetig:04X} fehlt");
        }
    }

    /// Marken muessen aufsteigend stehen -- strenge Leser verlangen das.
    #[test]
    fn die_marken_stehen_aufsteigend() {
        let (sauber, _) = strip(&bild(Reihenfolge::Klein)).unwrap();
        let (_, ifds) = lies(&sauber).unwrap();
        let marken: Vec<u16> = ifds[0].iter().map(|e| e.tag).collect();
        let mut sortiert = marken.clone();
        sortiert.sort_unstable();
        assert_eq!(marken, sortiert, "die Reihenfolge ging verloren");
    }

    #[test]
    fn die_bereinigung_ist_wiederholbar() {
        let einmal = strip(&bild(Reihenfolge::Klein)).unwrap().0;
        let zweimal = strip(&einmal).unwrap().0;
        assert_eq!(einmal, zweimal);
    }

    /// Ein Verweis im Kreis darf nicht zur Endlosschleife fuehren.
    #[test]
    fn verweise_im_kreis_werden_abgefangen() {
        let mut roh = bild(Reihenfolge::Klein);
        // Den Kettenversatz auf das erste Verzeichnis zeigen lassen.
        let anzahl = u16::from_le_bytes(roh[8..10].try_into().unwrap()) as usize;
        let ketten_pos = 8 + 2 + anzahl * EINTRAG_LEN;
        roh[ketten_pos..ketten_pos + 4].copy_from_slice(&8u32.to_le_bytes());
        assert!(inspect(&roh).is_err(), "der Kreis wurde nicht bemerkt");
    }

    #[test]
    fn kaputte_daten_ergeben_einen_fehler_keinen_absturz() {
        assert!(inspect(b"").is_err());
        assert!(inspect(b"II*\x00").is_err());
        assert!(inspect(b"II*\x00\xff\xff\xff\xff").is_err());
        let mut roh = bild(Reihenfolge::Klein);
        roh.truncate(20);
        assert!(inspect(&roh).is_err());
    }

    // -----------------------------------------------------------------------
    // Rohdateien aus Kameras
    // -----------------------------------------------------------------------

    /// **Der gefährlichste Fund des ganzen Moduls.**
    ///
    /// DNG, NEF und ARW *sind* TIFF und wurden deshalb hier behandelt. Sie sind
    /// aber umgekehrt aufgebaut: Das erste Verzeichnis trägt nur eine Vorschau,
    /// das eigentliche Foto liegt im `SubIFD`. Da dieses Modul `SubIFDs` für
    /// Vorschaubilder hält, entfernte es das Foto — und meldete „vollständig
    /// bereinigt".
    ///
    /// Im Versuch wurde aus einer 1368 Byte großen Datei eine von 198 Bytes:
    /// die Vorschau blieb, die Aufnahme war weg.
    #[test]
    fn eine_rohdatei_wird_erkannt_und_nicht_angetastet() {
        let mut b = Bauer::neu(Reihenfolge::Klein);
        // Erstes Verzeichnis: verkleinerte Fassung, und ein SubIFD daneben.
        b.lang(0x00FE, 1)
            .lang(TAG_SUB_IFDS, 4096)
            .text(0x010F, "NIKON");
        let datei = b.fertig();

        let i = inspect(&datei).unwrap();
        assert_eq!(
            i.format.as_deref(),
            Some("TIFF-Rohdatei (DNG, NEF, ARW, CR2)"),
            "die Rohdatei wurde für ein gewöhnliches TIFF gehalten"
        );
        assert!(i.findings.iter().any(|f| f.location == "TIFF:Rohdatei"));

        let (aus, ergebnis) = strip(&datei).unwrap();
        assert_eq!(aus, datei, "an einer Rohdatei darf sich nichts ändern");
        assert!(
            !ergebnis.may_show_clean(),
            "für eine Rohdatei darf keine Sauberkeit behauptet werden"
        );
        let StripResult::Partial { removed, .. } = &ergebnis else {
            panic!("erwartet wurde Partial, bekam {ergebnis:?}");
        };
        assert!(removed.is_empty(), "es wurde doch etwas entfernt");
    }

    /// Der zweite Weg zur Erkennung: Marken, die es **nur** in Rohdateien
    /// gibt. Sie greift auch dann, wenn das erste Verzeichnis das Bild führt.
    #[test]
    fn sensormarken_reichen_zur_erkennung() {
        for tag in [TAG_DNG_FASSUNG, TAG_CFA_MUSTER, TAG_CFA_MASSE] {
            let mut b = Bauer::neu(Reihenfolge::Klein);
            b.kurz(tag, 1);
            let datei = b.fertig();
            let (aus, ergebnis) = strip(&datei).unwrap();
            assert_eq!(aus, datei, "Marke {tag:#06X}: die Datei wurde verändert");
            assert!(!ergebnis.may_show_clean(), "Marke {tag:#06X}");
        }

        // Und die Farbdeutung, die Sensordaten ausweist.
        for deutung in [DEUTUNG_CFA, DEUTUNG_LINEAR_ROH] {
            let mut b = Bauer::neu(Reihenfolge::Klein);
            b.kurz(TAG_FARBDEUTUNG, u16::try_from(deutung).unwrap());
            let datei = b.fertig();
            assert!(!strip(&datei).unwrap().1.may_show_clean(), "{deutung}");
        }
    }

    /// **Die Gegenprobe.** Ein gewöhnliches TIFF mit `SubIFDs` muss weiterhin
    /// bereinigt werden — dort ist das SubIFD tatsächlich eine Vorschau.
    /// Ohne diesen Test hätte die neue Vorsicht das alte Verhalten erschlagen.
    #[test]
    fn ein_gewoehnliches_tiff_mit_subifds_wird_weiterhin_bereinigt() {
        let mut b = Bauer::neu(Reihenfolge::Klein);
        // Kein NewSubfileType: Das erste Verzeichnis führt das Bild selbst.
        b.lang(TAG_SUB_IFDS, 4096).text(0x013B, "Dr. Anna Beispiel");
        let datei = b.fertig();

        let i = inspect(&datei).unwrap();
        assert_eq!(i.format.as_deref(), Some("TIFF"));
        assert!(!i.findings.iter().any(|f| f.location == "TIFF:Rohdatei"));

        let (aus, ergebnis) = strip(&datei).unwrap();
        assert!(ergebnis.may_show_clean(), "{ergebnis:?}");
        assert!(
            !aus.windows(8).any(|f| f == b"Dr. Anna"),
            "der Verfasser blieb stehen"
        );
        assert!(
            aus.windows(9).any(|f| f == b"BILDDATEN"),
            "die Bilddaten gingen verloren"
        );
    }

    /// Eine verkleinerte Fassung **ohne** SubIFD ist keine Rohdatei, sondern
    /// nur ein Vorschaubild — etwa die zweite Seite eines Scans.
    #[test]
    fn eine_verkleinerte_fassung_allein_macht_noch_keine_rohdatei() {
        let mut b = Bauer::neu(Reihenfolge::Klein);
        b.lang(0x00FE, 1).text(0x010F, "Scanner XY");
        let datei = b.fertig();

        assert_eq!(inspect(&datei).unwrap().format.as_deref(), Some("TIFF"));
        assert!(strip(&datei).unwrap().1.may_show_clean());
    }
}
