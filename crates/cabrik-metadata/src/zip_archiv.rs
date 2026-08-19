//! Einfache ZIP-Archive (`spec/metadata.md` §4).
//!
//! Bleibt `Partial`, und zwar aus einem Grund, der sich nicht beheben lässt:
//! **Die Namen der Einträge sind das Archiv.** Sie zu entfernen hieße, das
//! Archiv zu zerstören.
//!
//! # Was ein Dateiname verrät
//!
//! Mehr, als den meisten bewusst ist. Ein Archiv kann Einträge wie
//! `Kuendigung_Mueller_final_v3.docx` oder `C:/Users/m.mustermann/Desktop/…`
//! enthalten — der Benutzername steht dann im Klartext im Archiv, ohne dass
//! irgendetwas entpackt werden müsste.
//!
//! Solche Pfadangaben werden deshalb **gesondert gemeldet**: Ein Name mit
//! Laufwerksbuchstaben oder Heimatverzeichnis ist etwas anderes als
//! `bericht.pdf`.
//!
//! # Was hier bereinigt wird
//!
//! - **Zeitstempel** jedes Eintrags — sie verraten den Arbeitsverlauf auf zwei
//!   Sekunden genau und werden auf die ZIP-Epoche normalisiert.
//! - **Metadaten der enthaltenen Dateien** — ein Foto im Archiv bringt sein
//!   EXIF mit, ein Word-Dokument seinen Firmennamen.
//!
//! # Warum nicht in verschachtelte Archive hinein
//!
//! Ein ZIP in einem ZIP in einem ZIP hat keine natürliche Grenze. Wer eine
//! Datei baut, die sich tausendfach schachtelt, brächte ein Werkzeug ohne
//! Grenze zum Absturz — ohne jede kryptographische Kenntnis. Enthaltene
//! Archive werden deshalb **gemeldet, nicht geöffnet**, mit dem Hinweis, sie
//! einzeln zu bereinigen.

use crate::container::{self, Eintrag};
use crate::model::{Finding, FindingKind, Inspection, Severity, StripOptions, StripResult};

use cabrik_core::Result;

/// Untersucht ein ZIP-Archiv.
///
/// # Fehler
///
/// [`cabrik_core::Error::Malformed`] bei kaputtem Archiv.
pub fn inspect(daten: &[u8]) -> Result<Inspection> {
    let eintraege = container::lies(daten)?;
    let art = fremde_art(&eintraege);
    Ok(Inspection {
        format: Some(art.map_or_else(
            || "ZIP-Archiv".to_owned(),
            |a| format!("ZIP-Archiv (sieht aus wie {a})"),
        )),
        findings: sammle(&eintraege),
        // Die ZIP-Schicht wird verstanden. Steckt darin ein Format mit
        // eigenen Metadaten, sagt das eigens gemeldete Fund das deutlich.
        understood: true,
    })
}

/// Erkennt Formate, die ZIP als Behälter benutzen und **eigene** Metadaten
/// mitbringen, die dieses Programm nicht versteht.
///
/// # Warum das eigens gemeldet werden muss
///
/// Ohne diese Prüfung wurde ein EPUB als „ZIP-Archiv" gemeldet, mit einem
/// einzigen Fund über die Eintragsnamen — während in `OEBPS/content.opf` ein
/// Verfassername stand. Die Ausgabe sah aus wie ein sauberes Ergebnis und war
/// eine Falschaussage durch Auslassung: genau der v1-Fehler in neuem Gewand.
fn fremde_art(eintraege: &[Eintrag]) -> Option<&'static str> {
    if let Some(m) = container::finde(eintraege, "mimetype").and_then(Eintrag::text) {
        let typ = m.trim();
        if typ == "application/epub+zip" {
            return Some("EPUB");
        }
        if !typ.is_empty() {
            return Some("Container mit eigenem Typ");
        }
    }
    let hat = |n: &str| container::finde(eintraege, n).is_some();
    if hat("AndroidManifest.xml") {
        return Some("Android-Paket");
    }
    if hat("META-INF/MANIFEST.MF") {
        return Some("Java-Archiv");
    }
    None
}

fn sammle(eintraege: &[Eintrag]) -> Vec<Finding> {
    let mut funde = Vec::new();

    if let Some(art) = fremde_art(eintraege) {
        funde.push(Finding::new(
            FindingKind::UnknownExtension,
            "ZIP:Behälterformat".to_owned(),
            Some(format!(
                "Dies ist ein {art} und bringt **eigene** Metadaten mit, die dieses \
                 Programm nicht versteht. Über sie sagt dieser Bericht nichts — \
                 eine kurze Fundliste bedeutet hier nicht, dass wenig drinsteckt"
            )),
            Severity::Critical,
        ));
    }

    let dateien = eintraege.iter().filter(|e| !e.verzeichnis).count();
    if dateien > 0 {
        funde.push(Finding::new(
            FindingKind::Comment,
            "ZIP:Eintragsnamen".to_owned(),
            Some(format!(
                "{dateien} Dateinamen — sie bleiben lesbar, ohne dass etwas \
                 entpackt werden muss"
            )),
            Severity::Notable,
        ));
    }

    for e in eintraege {
        if let Some(art) = pfadangabe(&e.name) {
            funde.push(Finding::new(
                FindingKind::Author,
                format!("ZIP:{}", e.name),
                Some(format!("{art} im Eintragsnamen")),
                Severity::Critical,
            ));
        }

        if e.verzeichnis || e.inhalt.is_empty() {
            continue;
        }

        if ist_blosses_archiv(&e.inhalt) {
            funde.push(Finding::new(
                FindingKind::UnknownExtension,
                format!("ZIP:{}", e.name),
                Some(
                    "enthaltenes Archiv — wird nicht geöffnet, bitte einzeln bereinigen".to_owned(),
                ),
                Severity::Notable,
            ));
            continue;
        }

        // Bilder und Office-Dokumente im Archiv bringen ihre eigenen
        // Metadaten mit.
        if let Ok(inner) = crate::inspect(&e.inhalt) {
            for f in inner.findings {
                funde.push(Finding::new(
                    f.kind,
                    format!("ZIP:{} → {}", e.name, f.location),
                    Some(f.value.unwrap_or_else(|| "enthaltene Datei".to_owned())),
                    f.severity,
                ));
            }
        }
    }
    funde
}

/// Erkennt einen absoluten oder benutzerbezogenen Pfad im Eintragsnamen.
///
/// # Warum ohne führenden Schrägstrich geprüft wird
///
/// Packprogramme entfernen beim Hinzufügen häufig den Laufwerksbuchstaben und
/// den führenden Schrägstrich: Aus `C:/Users/m.mustermann/Desktop/x.jpg` wird
/// `Users/m.mustermann/Desktop/x.jpg`. Der Benutzername steht dann immer noch drin —
/// eine Prüfung, die `/users/` mit Schrägstrich verlangt, übersieht genau den
/// Fall, der in der Praxis vorkommt.
fn pfadangabe(name: &str) -> Option<&'static str> {
    let unten = name.to_ascii_lowercase().replace('\\', "/");
    if unten.starts_with("users/") || unten.contains("/users/") {
        return Some("Windows-Benutzerpfad");
    }
    if unten.starts_with("home/") || unten.contains("/home/") {
        return Some("Unix-Heimatverzeichnis");
    }
    // `C:/…` oder `C:\…` am Anfang.
    let mut zeichen = name.chars();
    if let (Some(erst), Some(zweit), Some(dritt)) = (zeichen.next(), zeichen.next(), zeichen.next())
        && erst.is_ascii_alphabetic()
        && zweit == ':'
        && (dritt == '/' || dritt == '\\')
    {
        return Some("absoluter Pfad mit Laufwerksbuchstaben");
    }
    None
}

/// Ob der Inhalt ein **bloßes** Archiv ist — eines ohne erkennbares Format.
///
/// Die Unterscheidung ist der Kern der Schachtelungsgrenze. Ein `docx` oder
/// `odt` ist zwar technisch ein ZIP, aber eines mit bekanntem Aufbau: Seine
/// Bereinigung steigt nur noch in **Bilder** hinab und endet dort. Ein
/// gewöhnliches Archiv dagegen kann wieder Archive enthalten, ohne Ende.
///
/// Ein früherer Entwurf prüfte nur auf ZIP-Kennbytes und wies damit auch
/// Office-Dokumente ab. Ein Word-Dokument in einem Archiv wäre so nie
/// bereinigt worden — obwohl genau das der häufige Fall ist.
fn ist_blosses_archiv(inhalt: &[u8]) -> bool {
    container::sieht_aus_wie_zip(inhalt)
        && matches!(
            crate::Format::detect(inhalt),
            Some(crate::Format::Zip) | None
        )
}

/// Bereinigt ein ZIP-Archiv.
///
/// # Fehler
///
/// [`cabrik_core::Error::Malformed`] bei kaputtem Archiv.
pub fn strip(daten: &[u8]) -> Result<(Vec<u8>, StripResult)> {
    strip_with(daten, StripOptions::nur_metadaten())
}

/// Bereinigt ein ZIP-Archiv mit ausdrücklichen Optionen.
///
/// Die Optionen werden an enthaltene Office-Dokumente durchgereicht.
///
/// # Fehler
///
/// [`cabrik_core::Error::Malformed`] bei kaputtem Archiv.
pub fn strip_with(daten: &[u8], opts: StripOptions) -> Result<(Vec<u8>, StripResult)> {
    let eintraege = container::lies(daten)?;
    let alle = sammle(&eintraege);

    let mut entfernt = Vec::new();
    let mut geblieben = Vec::new();
    for f in alle {
        // Namen, enthaltene Archive und fremde Behälterformate bleiben
        // zwangsläufig.
        if f.location == "ZIP:Eintragsnamen"
            || f.location == "ZIP:Behälterformat"
            || f.kind == FindingKind::Author && f.location.starts_with("ZIP:")
            || f.value
                .as_deref()
                .is_some_and(|v| v.contains("enthaltenes Archiv"))
        {
            geblieben.push(f);
        } else {
            entfernt.push(f);
        }
    }

    let neu: Vec<Eintrag> = eintraege
        .into_iter()
        .map(|e| {
            if e.verzeichnis || e.inhalt.is_empty() || ist_blosses_archiv(&e.inhalt) {
                return e;
            }
            let inhalt = crate::strip_with(&e.inhalt, opts)
                .map_or_else(|_| e.inhalt.clone(), |(sauber, _)| sauber);
            Eintrag { inhalt, ..e }
        })
        .collect();

    let aus = container::schreib(&neu)?;

    Ok((
        aus,
        StripResult::Partial {
            removed: entfernt,
            remaining: geblieben,
            reason: "Die Namen der Einträge sind das Archiv — sie zu entfernen hieße, \
                     es zu zerstören. Zeitstempel wurden normalisiert und die Metadaten \
                     der enthaltenen Dateien entfernt. Enthaltene Archive werden nicht \
                     geöffnet; sie sind einzeln zu bereinigen."
                .to_owned(),
        },
    ))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;

    fn eintrag(name: &str, inhalt: &[u8]) -> Eintrag {
        Eintrag {
            name: name.to_owned(),
            inhalt: inhalt.to_vec(),
            komprimiert: true,
            verzeichnis: false,
        }
    }

    #[test]
    fn pfade_im_namen_werden_erkannt() {
        assert_eq!(pfadangabe("bericht.pdf"), None);
        assert_eq!(pfadangabe("ordner/bericht.pdf"), None);
        assert!(pfadangabe("C:/Users/m.mustermann/Desktop/x.pdf").is_some());
        assert!(pfadangabe("C:\\Users\\m.mustermann\\x.pdf").is_some());
        assert!(pfadangabe("/home/m.mustermann/x.pdf").is_some());
        assert!(pfadangabe("D:\\Projekte\\x.pdf").is_some());
    }

    /// Packprogramme entfernen den Laufwerksbuchstaben. Der Benutzername
    /// bleibt trotzdem im Namen -- genau der Fall kommt in der Praxis vor.
    #[test]
    fn ein_pfad_ohne_laufwerksbuchstaben_faellt_trotzdem_auf() {
        assert!(pfadangabe("Users/m.mustermann/Desktop/x.jpg").is_some());
        assert!(pfadangabe("home/m.mustermann/x.jpg").is_some());
        assert!(pfadangabe("unterlagen/Angebot.docx").is_none());
    }

    /// Ein Office-Dokument im Archiv ist zwar technisch ein ZIP, aber eines
    /// mit bekanntem Aufbau -- es wird bereinigt, nicht abgewiesen.
    #[test]
    fn office_dokumente_im_archiv_werden_behandelt() {
        // Ein minimales, aber erkennbares OOXML-Paket.
        let docx = container::schreib(&[
            eintrag("[Content_Types].xml", br#"<?xml version="1.0"?><Types/>"#),
            eintrag(
                "docProps/app.xml",
                br#"<?xml version="1.0"?><Properties><Company>Muster GmbH</Company></Properties>"#,
            ),
            eintrag(
                "word/document.xml",
                br#"<?xml version="1.0"?><w:document/>"#,
            ),
        ])
        .unwrap();

        assert!(
            !ist_blosses_archiv(&docx),
            "ein docx wurde als blosses Archiv abgewiesen"
        );

        let archiv = container::schreib(&[eintrag("unterlagen/Angebot.docx", &docx)]).unwrap();
        let i = inspect(&archiv).unwrap();
        assert!(
            i.findings.iter().any(|f| f
                .value
                .as_deref()
                .unwrap_or_default()
                .contains("Muster GmbH")),
            "der Firmenname im enthaltenen Dokument wurde nicht gefunden: {:?}",
            i.findings
        );

        let (sauber, _) = strip(&archiv).unwrap();
        let e = container::lies(&sauber).unwrap();
        let innen = &container::finde(&e, "unterlagen/Angebot.docx")
            .unwrap()
            .inhalt;
        assert!(
            !innen.windows(11).any(|f| f == b"Muster GmbH"),
            "der Firmenname blieb im enthaltenen Dokument"
        );
    }

    /// Ein Benutzerpfad im Eintragsnamen ist ohne jedes Entpacken lesbar.
    #[test]
    fn ein_benutzerpfad_ist_kritisch() {
        let archiv = container::schreib(&[eintrag(
            "C:/Users/m.mustermann/Desktop/Kuendigung.txt",
            b"Text",
        )])
        .unwrap();

        let i = inspect(&archiv).unwrap();
        let f = i
            .findings
            .iter()
            .find(|f| f.severity == Severity::Critical)
            .expect("Benutzerpfad nicht erkannt");
        assert!(f.value.as_deref().unwrap_or_default().contains("Benutzer"));
    }

    /// Ein Archiv ist nie `Complete` -- die Namen bleiben.
    #[test]
    fn ein_archiv_ist_nie_vollstaendig_bereinigt() {
        let archiv = container::schreib(&[eintrag("bericht.txt", b"Text")]).unwrap();
        let (_, ergebnis) = strip(&archiv).unwrap();

        assert!(
            !ergebnis.may_show_clean(),
            "fuer ein Archiv darf keine Sauberkeit behauptet werden"
        );
        match ergebnis {
            StripResult::Partial { reason, .. } => {
                assert!(reason.contains("Namen der Einträge"), "{reason}");
            }
            other => panic!("erwartete Partial, bekam {other:?}"),
        }
    }

    /// Enthaltene Archive werden gemeldet, aber nicht geoeffnet -- sonst gaebe
    /// es keine Grenze fuer die Schachtelungstiefe.
    #[test]
    fn verschachtelte_archive_werden_nicht_geoeffnet() {
        let innen = container::schreib(&[eintrag("tief.txt", b"x")]).unwrap();
        let aussen = container::schreib(&[eintrag("innen.zip", &innen)]).unwrap();

        let i = inspect(&aussen).unwrap();
        let f = i
            .findings
            .iter()
            .find(|f| f.location.contains("innen.zip"))
            .expect("enthaltenes Archiv nicht gemeldet");
        assert!(
            f.value
                .as_deref()
                .unwrap_or_default()
                .contains("nicht geöffnet")
        );

        // Der Inhalt bleibt unveraendert.
        let (sauber, _) = strip(&aussen).unwrap();
        let e = container::lies(&sauber).unwrap();
        assert_eq!(container::finde(&e, "innen.zip").unwrap().inhalt, innen);
    }

    /// **Falschaussage durch Auslassung.** Ein EPUB traegt seine Metadaten in
    /// einer OPF-Datei. Wer es als gewoehnliches Archiv meldet, gibt eine
    /// kurze Fundliste aus und laesst den Nutzer glauben, es stecke wenig
    /// drin -- genau der v1-Fehler in neuem Gewand.
    #[test]
    fn ein_fremdes_behaelterformat_wird_benannt() {
        let epub = container::schreib(&[
            Eintrag {
                name: "mimetype".to_owned(),
                inhalt: b"application/epub+zip".to_vec(),
                komprimiert: false,
                verzeichnis: false,
            },
            eintrag(
                "OEBPS/content.opf",
                br#"<package><metadata><dc:creator>Dr. Anna Beispiel</dc:creator></metadata></package>"#,
            ),
        ])
        .unwrap();

        let i = inspect(&epub).unwrap();
        assert_eq!(i.format.as_deref(), Some("ZIP-Archiv (sieht aus wie EPUB)"));

        let f = i
            .findings
            .iter()
            .find(|f| f.location == "ZIP:Behälterformat")
            .expect("das Behaelterformat wurde nicht benannt");
        assert_eq!(f.severity, Severity::Critical);
        let wert = f.value.as_deref().unwrap_or_default();
        assert!(wert.contains("EPUB"), "{wert}");
        assert!(
            wert.contains("sagt dieser Bericht nichts"),
            "die Aussagegrenze fehlt: {wert}"
        );
    }

    #[test]
    fn ein_java_archiv_wird_ebenfalls_erkannt() {
        let jar = container::schreib(&[eintrag("META-INF/MANIFEST.MF", b"Manifest-Version: 1.0")])
            .unwrap();
        let i = inspect(&jar).unwrap();
        assert_eq!(
            i.format.as_deref(),
            Some("ZIP-Archiv (sieht aus wie Java-Archiv)")
        );
    }

    /// Ein gewoehnliches Archiv soll **nicht** als fremdes Format gelten.
    #[test]
    fn ein_schlichtes_archiv_wird_nicht_falsch_gemeldet() {
        let archiv = container::schreib(&[eintrag("bericht.txt", b"Text")]).unwrap();
        let i = inspect(&archiv).unwrap();
        assert_eq!(i.format.as_deref(), Some("ZIP-Archiv"));
        assert!(
            !i.findings
                .iter()
                .any(|f| f.location == "ZIP:Behälterformat")
        );
    }

    /// Zeitstempel werden normalisiert -- das ist der eigentliche Gewinn.
    #[test]
    fn zweimal_bereinigen_ergibt_dieselben_bytes() {
        let archiv = container::schreib(&[eintrag("a.txt", b"x")]).unwrap();
        assert_eq!(strip(&archiv).unwrap().0, strip(&archiv).unwrap().0);
    }
}
