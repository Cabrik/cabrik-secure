//! BMP (`spec/metadata.md` §4).
//!
//! Das schlichteste Format der Liste — und genau deshalb erwähnenswert.
//!
//! # Warum BMP trotzdem hierhergehört
//!
//! Ein BMP trägt „praktisch keine Metadaten". Der Reiz wäre, es deshalb
//! einfach durchzuwinken. Das wäre der v1-Fehler: Eine Datei ungeprüft
//! weiterzureichen und als sauber zu melden, ist etwas anderes, als sie zu
//! prüfen und nichts zu finden.
//!
//! Zwei Dinge gibt es nämlich doch:
//!
//! - Ein **eingebettetes Farbprofil**. Der `BITMAPV5HEADER` kann eines
//!   enthalten oder auf eine Datei verweisen — und ein Dateiverweis ist ein
//!   Pfad, meist mit dem Benutzernamen darin.
//! - **Anhängsel hinter den Bilddaten.** Das Feld `bfOffBits` sagt, wo die
//!   Pixel beginnen; die Datei kann danach beliebig weitergehen. Was dort
//!   steht, sieht kein Betrachter je — es wird aber mitverschickt.

use crate::model::{Finding, FindingKind, Inspection, Severity, StripResult};

use cabrik_core::{Error, Result};

/// Länge des Dateikopfs.
const DATEIKOPF: usize = 14;

/// Kennung eines Farbraums, der auf eine **Datei** verweist.
const PROFILE_LINKED: u32 = 0x4C49_4E4B; // "LINK"
/// Kennung eines **eingebetteten** Profils.
const PROFILE_EMBEDDED: u32 = 0x4D42_4544; // "MBED"

/// Bekannte Längen des Informationskopfs.
///
/// `BITMAPCOREHEADER` bis `BITMAPV5HEADER`. Diese Liste ist die eigentliche
/// Kennung: Zwei Buchstaben `BM` treffen leicht zufällig zu, eine dieser sechs
/// Zahlen an genau dieser Stelle nicht.
const KOPF_LAENGEN: [u32; 7] = [12, 40, 52, 56, 64, 108, 124];

/// Ob die Bytes wie ein BMP aussehen.
///
/// # Warum nicht über die Größenangabe
///
/// Ein früherer Entwurf verlangte, dass die im Kopf angegebene Dateigröße
/// **genau** der Länge entspricht. Das wies ausgerechnet den Fall ab, für den
/// dieses Modul da ist: Eine Datei mit Anhängsel ist länger als angegeben. Die
/// Erkennung hätte damit alles übersehen, was sie finden soll.
///
/// Geprüft wird deshalb die Länge des Informationskopfs. Sie ist ein weit
/// besseres Kennzeichen und vom Anhängsel unabhängig.
#[must_use]
pub fn looks_like_bmp(daten: &[u8]) -> bool {
    if !daten.starts_with(b"BM") || daten.len() < DATEIKOPF.saturating_add(4) {
        return false;
    }
    let Some(kopf_groesse) = lies_u32(daten, DATEIKOPF) else {
        return false;
    };
    if !KOPF_LAENGEN.contains(&kopf_groesse) {
        return false;
    }
    // Die angegebene Größe darf die Datei nicht überschreiten; kleiner sein
    // darf sie sehr wohl — dann hängt etwas dahinter. Null ist verbreitet.
    let angegeben = lies_u32(daten, 2).unwrap_or(0) as usize;
    angegeben == 0 || angegeben <= daten.len()
}

fn lies_u32(daten: &[u8], pos: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        daten.get(pos..pos.saturating_add(4))?.try_into().ok()?,
    ))
}

/// Untersucht ein BMP.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Struktur.
pub fn inspect(daten: &[u8]) -> Result<Inspection> {
    Ok(Inspection {
        format: Some("BMP".to_owned()),
        findings: sammle(daten)?,
        understood: true,
    })
}

fn sammle(daten: &[u8]) -> Result<Vec<Finding>> {
    if !looks_like_bmp(daten) {
        return Err(Error::Malformed("bmp: kein BMP-Kopf"));
    }

    let mut funde = Vec::new();

    let kopf_groesse = lies_u32(daten, DATEIKOPF).ok_or(Error::Malformed("bmp: Kopf zu kurz"))?;

    // Der Farbraum steht ab Byte 56 des Informationskopfs; er existiert erst
    // ab `BITMAPV4HEADER` (108 Bytes).
    if kopf_groesse >= 108 {
        let farbraum = lies_u32(daten, DATEIKOPF.saturating_add(56)).unwrap_or(0);
        match farbraum {
            PROFILE_EMBEDDED => funde.push(Finding::new(
                FindingKind::ColorProfile,
                "BMP:Farbprofil".to_owned(),
                Some("eingebettetes Farbprofil".to_owned()),
                Severity::Minor,
            )),
            PROFILE_LINKED => funde.push(Finding::new(
                // Ein Dateiverweis ist ein Pfad — meist mit Benutzernamen.
                FindingKind::Author,
                "BMP:Farbprofil".to_owned(),
                Some(
                    "Verweis auf eine Farbprofildatei — enthält einen Pfad, \
                     der den Benutzernamen preisgeben kann"
                        .to_owned(),
                ),
                Severity::Critical,
            )),
            _ => {}
        }
    }

    // Alles hinter den Bilddaten. `bfOffBits` sagt, wo sie beginnen; ihre
    // Länge steht im Informationskopf.
    if let Some(rest) = anhaengsel_laenge(daten)
        && rest > 0
    {
        funde.push(Finding::new(
            FindingKind::UnknownExtension,
            "BMP:Anhängsel".to_owned(),
            Some(format!(
                "{rest} Bytes hinter den Bilddaten — kein Betrachter zeigt sie an, \
                 mitverschickt werden sie trotzdem"
            )),
            Severity::Notable,
        ));
    }

    Ok(funde)
}

/// Zahl der Bytes hinter den Bilddaten.
///
/// `None`, wenn sich das nicht verlässlich bestimmen lässt — dann wird auch
/// nichts behauptet.
fn anhaengsel_laenge(daten: &[u8]) -> Option<usize> {
    let offset = usize::try_from(lies_u32(daten, 10)?).ok()?;
    let bild_groesse = usize::try_from(lies_u32(daten, DATEIKOPF.saturating_add(20))?).ok()?;
    if bild_groesse == 0 || offset == 0 {
        // Bei unkomprimierten Bildern darf `biSizeImage` null sein. Dann ist
        // die Länge nur über Breite, Höhe und Bittiefe zu errechnen — und
        // Zeilenausrichtung macht das fehleranfällig. Lieber nichts sagen.
        return None;
    }
    let ende = offset.checked_add(bild_groesse)?;
    daten.len().checked_sub(ende)
}

/// Entfernt, was sich entfernen lässt.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Struktur.
pub fn strip(daten: &[u8]) -> Result<(Vec<u8>, StripResult)> {
    let funde = sammle(daten)?;

    // Nur das Anhängsel lässt sich gefahrlos abschneiden. Ein Farbprofil sitzt
    // mitten in der Kopfstruktur; es zu entfernen hieße, Größenangaben und
    // Versätze neu zu berechnen — für einen Fund der Stufe „gering" ein
    // schlechter Tausch.
    let mut aus = daten.to_vec();
    let mut entfernt = Vec::new();
    let mut geblieben = Vec::new();

    for f in funde {
        if f.location == "BMP:Anhängsel" {
            entfernt.push(f);
        } else {
            geblieben.push(f);
        }
    }

    if !entfernt.is_empty()
        && let Some(rest) = anhaengsel_laenge(daten)
    {
        aus.truncate(daten.len().saturating_sub(rest));
        // Die Größenangabe im Kopf muss mitwandern.
        if let Ok(neu) = u32::try_from(aus.len())
            && lies_u32(daten, 2) != Some(0)
            && let Some(feld) = aus.get_mut(2..6)
        {
            feld.copy_from_slice(&neu.to_le_bytes());
        }
    }

    let ergebnis = if geblieben.is_empty() {
        StripResult::Complete { removed: entfernt }
    } else {
        StripResult::Partial {
            removed: entfernt,
            remaining: geblieben,
            reason: "Ein Farbprofil sitzt mitten in der Kopfstruktur. Es zu entfernen \
                     hieße, sämtliche Größenangaben und Versätze neu zu berechnen — \
                     ein Eingriff, der mehr kaputtmachen kann, als er nützt. Wer das \
                     Profil loswerden will, speichert das Bild ohne es neu ab."
                .to_owned(),
        }
    };

    Ok((aus, ergebnis))
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

    /// Ein BMP mit `BITMAPINFOHEADER` (40 Bytes) und vier Pixeln.
    fn bild(anhang: &[u8], kopf_groesse: u32, farbraum: u32) -> Vec<u8> {
        let info_len = usize::try_from(kopf_groesse).unwrap();
        let pixel = vec![0xFFu8; 16];
        let offset = DATEIKOPF + info_len;
        let gesamt = offset + pixel.len() + anhang.len();

        let mut v = b"BM".to_vec();
        v.extend_from_slice(&u32::try_from(gesamt).unwrap().to_le_bytes());
        v.extend_from_slice(&[0, 0, 0, 0]); // reserviert
        v.extend_from_slice(&u32::try_from(offset).unwrap().to_le_bytes());

        let mut info = vec![0u8; info_len];
        info[0..4].copy_from_slice(&kopf_groesse.to_le_bytes());
        info[4..8].copy_from_slice(&2i32.to_le_bytes()); // Breite
        info[8..12].copy_from_slice(&2i32.to_le_bytes()); // Hoehe
        info[20..24].copy_from_slice(&u32::try_from(pixel.len()).unwrap().to_le_bytes());
        if info_len >= 60 {
            info[56..60].copy_from_slice(&farbraum.to_le_bytes());
        }
        v.extend_from_slice(&info);
        v.extend_from_slice(&pixel);
        v.extend_from_slice(anhang);
        v
    }

    #[test]
    fn bmp_wird_erkannt_aber_nicht_zu_leichtfertig() {
        assert!(looks_like_bmp(&bild(b"", 40, 0)));
        assert!(looks_like_bmp(&bild(b"", 108, 0)));
        // Zwei Buchstaben allein genuegen nicht.
        assert!(!looks_like_bmp(b"BM"));
        assert!(!looks_like_bmp(b"BM ein gewoehnlicher Text ohne Struktur"));
    }

    /// **Die Falle in der Erkennung.** Eine Datei mit Anhaengsel ist laenger
    /// als im Kopf angegeben. Wer auf Gleichheit prueft, weist genau den Fall
    /// ab, den dieses Modul finden soll.
    #[test]
    fn eine_datei_mit_anhaengsel_wird_trotzdem_erkannt() {
        let roh = bild(b"HEIMLICHE-NUTZLAST-AM-ENDE", 40, 0);
        assert!(
            looks_like_bmp(&roh),
            "die Erkennung uebersieht ausgerechnet den Fund"
        );
        assert!(inspect(&roh).unwrap().understood);
    }

    /// Eine Datei, die mehr verspricht, als sie hat, ist kaputt.
    #[test]
    fn eine_zu_kurze_datei_wird_abgewiesen() {
        let mut roh = bild(b"", 40, 0);
        roh[2..6].copy_from_slice(&99_999u32.to_le_bytes());
        assert!(!looks_like_bmp(&roh));
    }

    /// Ein schlichtes BMP ist wirklich sauber -- und das darf gesagt werden,
    /// weil geprueft wurde.
    #[test]
    fn ein_schlichtes_bmp_hat_nichts_zu_verbergen() {
        let (sauber, ergebnis) = strip(&bild(b"", 40, 0)).unwrap();
        assert_eq!(sauber, bild(b"", 40, 0));
        assert!(ergebnis.may_show_clean());
        match ergebnis {
            StripResult::Complete { removed } => assert!(removed.is_empty()),
            other => panic!("erwartete Complete, bekam {other:?}"),
        }
    }

    /// Was hinter den Bilddaten steht, sieht niemand -- mitverschickt wird
    /// es trotzdem.
    #[test]
    fn ein_anhaengsel_wird_gefunden_und_abgeschnitten() {
        let roh = bild(b"HEIMLICHE-NUTZLAST", 40, 0);
        let i = inspect(&roh).unwrap();
        let f = i
            .findings
            .iter()
            .find(|f| f.location == "BMP:Anhängsel")
            .expect("Anhaengsel nicht gefunden");
        assert!(f.value.as_deref().unwrap_or_default().contains("18 Bytes"));

        let (sauber, _) = strip(&roh).unwrap();
        assert!(
            !sauber.windows(18).any(|f| f == b"HEIMLICHE-NUTZLAST"),
            "das Anhaengsel blieb"
        );
        assert_eq!(sauber, bild(b"", 40, 0), "es wurde zu viel abgeschnitten");
    }

    /// Die Groessenangabe im Kopf muss nach dem Kuerzen stimmen.
    #[test]
    fn die_groessenangabe_wandert_mit() {
        let (sauber, _) = strip(&bild(b"ANHANG", 40, 0)).unwrap();
        let angegeben = lies_u32(&sauber, 2).unwrap() as usize;
        assert_eq!(angegeben, sauber.len());
        assert!(looks_like_bmp(&sauber));
    }

    /// Ein Verweis auf eine Farbprofildatei ist ein Pfad -- und Pfade tragen
    /// Benutzernamen.
    #[test]
    fn ein_verwiesenes_farbprofil_ist_kritisch() {
        let roh = bild(b"", 108, PROFILE_LINKED);
        let i = inspect(&roh).unwrap();
        let f = i
            .findings
            .iter()
            .find(|f| f.location == "BMP:Farbprofil")
            .expect("Farbprofil nicht gefunden");
        assert_eq!(f.severity, Severity::Critical);
        assert!(
            f.value
                .as_deref()
                .unwrap_or_default()
                .contains("Benutzernamen")
        );
    }

    /// Ein eingebettetes Profil bleibt -- und das Ergebnis sagt das ehrlich.
    #[test]
    fn ein_eingebettetes_profil_macht_das_ergebnis_partial() {
        let (_, ergebnis) = strip(&bild(b"", 108, PROFILE_EMBEDDED)).unwrap();
        assert!(!ergebnis.may_show_clean());
        match ergebnis {
            StripResult::Partial { reason, .. } => {
                assert!(reason.contains("Kopfstruktur"), "{reason}");
            }
            other => panic!("erwartete Partial, bekam {other:?}"),
        }
    }

    /// Ohne verlaessliche Groessenangabe wird nichts behauptet.
    #[test]
    fn ohne_bildgroesse_wird_kein_anhaengsel_behauptet() {
        let mut roh = bild(b"", 40, 0);
        roh[DATEIKOPF + 20..DATEIKOPF + 24].copy_from_slice(&0u32.to_le_bytes());
        let i = inspect(&roh).unwrap();
        assert!(
            !i.findings.iter().any(|f| f.location == "BMP:Anhängsel"),
            "es wurde geraten statt geschwiegen"
        );
    }

    #[test]
    fn kaputte_daten_ergeben_einen_fehler_keinen_absturz() {
        assert!(inspect(b"").is_err());
        assert!(inspect(b"BM").is_err());
        assert!(inspect(b"nicht einmal BM").is_err());
    }
}
