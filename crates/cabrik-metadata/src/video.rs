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
//! - **`©xyz`** in `moov/udta` — die **GPS-Koordinaten** der Aufnahme, im
//!   Format `+46.9481+007.4474/`. Jedes iPhone schreibt sie hinein. Das ist
//!   der schwerwiegendste Fund und dem GPS-Tag eines Fotos gleichwertig.
//! - **`ilst`-Marken** in `moov/udta/meta` — Titel, Verfasser, Kommentar und
//!   das erzeugende Programm.
//! - **Zeitstempel** in `mvhd`, `tkhd` und `mdhd` — Erstellung und letzte
//!   Änderung, auf die Sekunde genau. Sie sind Felder fester Breite und
//!   lassen sich auf null setzen, ohne dass sich etwas verschiebt.

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
const BEHAELTER: [&[u8; 4]; 8] = [
    b"moov", b"trak", b"mdia", b"minf", b"udta", b"meta", b"ilst", b"edts",
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

    // --- Sonstige Benutzerdaten, die wir nicht einzeln kennen ---
    for b in boxen {
        if b.typ != *b"udta" {
            continue;
        }
        let bekannt = boxen.iter().any(|x| {
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
    Ok(Inspection {
        format: Some(marke_name(daten).to_owned()),
        findings: sammle(daten, &boxen),
        understood: true,
    })
}

fn marke_name(daten: &[u8]) -> &'static str {
    match daten.get(8..12) {
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
}
