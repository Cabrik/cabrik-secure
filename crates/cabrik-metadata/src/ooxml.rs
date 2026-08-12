//! OOXML: `docx`, `xlsx`, `pptx` (`spec/metadata.md` §4.2, §7.2).
//!
//! # Was v1 übersah
//!
//! v1 setzte nur `docProps/core.xml` zurück. Unangetastet blieben:
//!
//! - `docProps/app.xml` — **Firmenname**, Vorlage, Bearbeitungsdauer
//! - `docProps/custom.xml` — beliebige benutzerdefinierte Felder
//! - `word/settings.xml` — `rsid`-Werte
//! - Kommentare und nachverfolgte Änderungen
//! - eingebettete Bilder **mit eigenem EXIF**
//! - das Vorschaubild unter `docProps/thumbnail.*`
//!
//! Der Firmenname in `app.xml` ist in der Praxis oft die verräterischste
//! Angabe überhaupt: Ein anonym gemeintes Schreiben trägt den Namen der
//! Kanzlei, die die Vorlage erstellt hat.
//!
//! # Was hier entfernt wird und was nicht
//!
//! Entfernt wird alles, was **Metadaten** sind: die Eigenschaftsteile, die
//! `rsid`-Werte, das Vorschaubild, die Metadaten eingebetteter Bilder.
//!
//! **Nicht** entfernt werden Kommentare und nachverfolgte Änderungen. Beide
//! sind Bestandteil des Dokuments, nicht Beiwerk: Eine nachverfolgte Löschung
//! zu entfernen heißt, sie anzunehmen oder zu verwerfen — eine inhaltliche
//! Entscheidung, die dem Nutzer gehört. Sie werden deshalb als `Critical`
//! gemeldet, und das Ergebnis ist dann [`StripResult::Partial`].
//!
//! Das ist dieselbe Trennlinie wie bei zugeschnittenen Bildern
//! (`spec/metadata.md` §7.2): melden, was ist — und den Eingriff dem Nutzer
//! überlassen, sobald er über das Löschen von Metadaten hinausgeht.
//!
//! # Warum ersetzt statt entfernt
//!
//! Die Eigenschaftsteile werden durch **leere Hüllen** ersetzt, nicht aus dem
//! Archiv genommen. `_rels/.rels` und `[Content_Types].xml` verweisen auf sie;
//! ein fehlender Teil führt in Word zur Reparaturabfrage. Eine leere Hülle
//! trägt dieselbe Information wie ein fehlender Teil — nämlich keine — und
//! lässt das Dokument heil.

use crate::container::{self, Eintrag};
use crate::model::{Finding, FindingKind, Inspection, Severity, StripOptions, StripResult};
use crate::xml;

use cabrik_core::Result;

/// Welche Art von OOXML-Dokument vorliegt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Art {
    /// Textdokument.
    Word,
    /// Tabelle.
    Excel,
    /// Präsentation.
    PowerPoint,
}

impl Art {
    /// Name für die Anzeige.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Word => "OOXML (Word)",
            Self::Excel => "OOXML (Excel)",
            Self::PowerPoint => "OOXML (PowerPoint)",
        }
    }
}

/// Erkennt ein OOXML-Dokument an seinen Bestandteilen.
///
/// Bewusst am Inhalt, nicht an der Endung: `.docx` ist nur ein Name.
#[must_use]
pub fn erkenne(eintraege: &[Eintrag]) -> Option<Art> {
    // Ohne diesen Teil ist es kein OOXML-Paket, sondern nur ein ZIP.
    container::finde(eintraege, "[Content_Types].xml")?;

    let hat = |p: &str| eintraege.iter().any(|e| e.name.starts_with(p));
    if hat("word/") {
        Some(Art::Word)
    } else if hat("xl/") {
        Some(Art::Excel)
    } else if hat("ppt/") {
        Some(Art::PowerPoint)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Die Teile, um die es geht
// ---------------------------------------------------------------------------

/// `docProps/core.xml` — die einzigen Felder, die v1 anfasste.
const CORE_FELDER: [(&str, FindingKind, Severity); 12] = [
    ("dc:creator", FindingKind::Author, Severity::Critical),
    ("cp:lastModifiedBy", FindingKind::Author, Severity::Critical),
    // Die interne Notiz. Steht in Word unter „Kommentare" und wird beim
    // Weitergeben regelmäßig vergessen — „Nicht an den Kunden geben".
    ("dc:description", FindingKind::Comment, Severity::Critical),
    (
        "cp:revision",
        FindingKind::EditingSession,
        Severity::Notable,
    ),
    ("dcterms:created", FindingKind::Timestamp, Severity::Notable),
    (
        "dcterms:modified",
        FindingKind::Timestamp,
        Severity::Notable,
    ),
    ("cp:lastPrinted", FindingKind::Timestamp, Severity::Notable),
    ("dc:title", FindingKind::Comment, Severity::Notable),
    ("dc:subject", FindingKind::Comment, Severity::Notable),
    ("cp:keywords", FindingKind::Comment, Severity::Notable),
    ("cp:category", FindingKind::Comment, Severity::Notable),
    ("cp:contentStatus", FindingKind::Comment, Severity::Notable),
];

/// `docProps/app.xml` — das, was v1 vollständig übersah.
const APP_FELDER: [(&str, FindingKind, Severity); 9] = [
    ("Company", FindingKind::Organization, Severity::Critical),
    ("Manager", FindingKind::Author, Severity::Critical),
    ("LastAuthor", FindingKind::Author, Severity::Critical),
    // Enthält die Überschriften des Dokuments im Klartext — also Inhalt,
    // nicht bloß eine Angabe über den Inhalt.
    ("TitlesOfParts", FindingKind::Comment, Severity::Critical),
    ("Template", FindingKind::Comment, Severity::Notable),
    ("Application", FindingKind::Software, Severity::Notable),
    ("TotalTime", FindingKind::EditingSession, Severity::Notable),
    ("HyperlinkBase", FindingKind::Comment, Severity::Notable),
    ("AppVersion", FindingKind::Software, Severity::Minor),
];

/// Leere Hüllen, die die entfernten Teile ersetzen.
const CORE_HUELLE: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    "\n",
    r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" "#,
    r#"xmlns:dc="http://purl.org/dc/elements/1.1/" "#,
    r#"xmlns:dcterms="http://purl.org/dc/terms/" "#,
    r#"xmlns:dcmitype="http://purl.org/dc/dcmitype/" "#,
    r#"xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"/>"#
);

const APP_HUELLE: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    "\n",
    r#"<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" "#,
    r#"xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"/>"#
);

const CUSTOM_HUELLE: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    "\n",
    r#"<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties" "#,
    r#"xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"/>"#
);

// ---------------------------------------------------------------------------
// Inspektion
// ---------------------------------------------------------------------------

/// Untersucht ein OOXML-Dokument.
///
/// # Fehler
///
/// [`cabrik_core::Error::Malformed`] bei kaputtem Container.
pub fn inspect(daten: &[u8]) -> Result<Inspection> {
    let eintraege = container::lies(daten)?;
    let art = erkenne(&eintraege);
    let funde = sammle(&eintraege);

    Ok(Inspection {
        format: Some(art.map_or_else(|| "OOXML".to_owned(), |a| a.name().to_owned())),
        findings: funde,
        understood: true,
    })
}

/// Sammelt alle Funde eines OOXML-Containers.
fn sammle(eintraege: &[Eintrag]) -> Vec<Finding> {
    let mut funde = Vec::new();

    for e in eintraege {
        match e.name.as_str() {
            "docProps/core.xml" => felder(&mut funde, e, &CORE_FELDER, "core.xml"),
            "docProps/app.xml" => felder(&mut funde, e, &APP_FELDER, "app.xml"),
            "docProps/custom.xml" => benutzerfelder(&mut funde, e),
            _ => {}
        }

        if e.name.starts_with("docProps/thumbnail.") {
            funde.push(Finding::new(
                FindingKind::EmbeddedPreview,
                format!("OOXML:{}", e.name),
                Some(format!(
                    "Vorschaubild, {} Bytes — zeigt den Dokumentinhalt",
                    e.inhalt.len()
                )),
                Severity::Critical,
            ));
        }

        // Die `_rels`-Datei eines customXml-Teils trägt nichts Eigenes; sie
        // gesondert zu melden wäre Rauschen in der Fundliste.
        if e.name.starts_with("customXml/") && !e.name.contains("/_rels/") {
            benutzer_xml(&mut funde, e);
        }

        if e.name.ends_with("settings.xml") {
            rsids(&mut funde, e);
        }

        if ist_kommentarteil(&e.name) {
            funde.push(Finding::new(
                FindingKind::Comment,
                format!("OOXML:{}", e.name),
                Some("Kommentare mit Namen und Zeitpunkten".to_owned()),
                Severity::Critical,
            ));
        }

        if ist_dokumentteil(&e.name) {
            nachverfolgte_aenderungen(&mut funde, e);
            zuschnitte(&mut funde, e);
        }

        if ist_medium(&e.name) {
            eingebettete_medien(&mut funde, e);
        }
    }

    funde
}

/// Zieht benannte Felder aus einem Eigenschaftsteil.
fn felder(
    funde: &mut Vec<Finding>,
    e: &Eintrag,
    felder: &[(&str, FindingKind, Severity)],
    ort: &str,
) {
    let Some(text) = e.text() else {
        return;
    };
    for (name, art, schwere) in felder {
        // Der lokale Name genügt: `dc:creator` und `creator` meinen dasselbe.
        let lokal = name.rsplit(':').next().unwrap_or(name);
        for wert in xml::element_texte(text, lokal) {
            if wert.trim().is_empty() {
                continue;
            }
            funde.push(Finding::new(
                *art,
                format!("OOXML:{ort}/{name}"),
                Some(wert),
                *schwere,
            ));
        }
    }
}

/// `docProps/custom.xml` trägt beliebige Felder — jedes einzeln melden.
fn benutzerfelder(funde: &mut Vec<Finding>, e: &Eintrag) {
    let Some(text) = e.text() else {
        return;
    };
    for name in xml::attribut_werte(text, "property", "name") {
        funde.push(Finding::new(
            FindingKind::Comment,
            "OOXML:custom.xml".to_owned(),
            Some(format!("benutzerdefiniertes Feld „{name}\"")),
            // Benutzerdefinierte Felder stammen meist aus
            // Dokumentenmanagementsystemen und tragen Aktenzeichen,
            // Mandantennamen oder Abteilungen.
            Severity::Critical,
        ));
    }
}

/// `customXml/` — beliebiges XML, das am Dokument hängt.
///
/// In Unternehmen füllen Dokumentenmanagementsysteme diese Teile: Aktenzeichen,
/// Mandant, Abteilung, Vertraulichkeitsstufe. `itemProps*.xml` trägt zusätzlich
/// eine **feste GUID** — dieselbe in jedem aus derselben Vorlage erzeugten
/// Dokument. Sie verknüpft Dokumente über Empfänger hinweg, auch wenn sonst
/// alles bereinigt wurde.
///
/// Behandlung nach `spec/metadata.md` §7.3: entfernen und namentlich melden.
fn benutzer_xml(funde: &mut Vec<Finding>, e: &Eintrag) {
    let guid = e
        .text()
        .map(|t| xml::attribut_werte(t, "datastoreItem", "itemID"))
        .unwrap_or_default();

    let (wert, schwere) = match guid.first() {
        Some(g) => (
            format!("feste Kennung {g} — verknüpft Dokumente derselben Vorlage"),
            Severity::Critical,
        ),
        None => (
            format!("angehängtes XML, {} Bytes", e.inhalt.len()),
            Severity::Notable,
        ),
    };

    funde.push(Finding::new(
        FindingKind::UnknownExtension,
        format!("OOXML:{}", e.name),
        Some(wert),
        schwere,
    ));
}

/// `rsid`-Werte verketten Bearbeitungssitzungen.
fn rsids(funde: &mut Vec<Finding>, e: &Eintrag) {
    let Some(text) = e.text() else {
        return;
    };
    let anzahl = xml::zaehle_elemente(text, "rsid");
    if anzahl > 0 {
        funde.push(Finding::new(
            FindingKind::EditingSession,
            format!("OOXML:{}", e.name),
            Some(format!(
                "{anzahl} Sitzungskennungen — zwei Dokumente mit gleichen Werten \
                 stammen aus derselben Bearbeitungssitzung"
            )),
            Severity::Notable,
        ));
    }
}

/// Nachverfolgte Änderungen tragen Name und Zeitpunkt **jeder** Bearbeitung.
fn nachverfolgte_aenderungen(funde: &mut Vec<Finding>, e: &Eintrag) {
    let Some(text) = e.text() else {
        return;
    };
    let eingefuegt = xml::zaehle_elemente(text, "ins");
    let geloescht = xml::zaehle_elemente(text, "del");
    if eingefuegt == 0 && geloescht == 0 {
        return;
    }

    let mut autoren = xml::attribut_werte(text, "ins", "author");
    autoren.extend(xml::attribut_werte(text, "del", "author"));
    autoren.sort_unstable();
    autoren.dedup();

    let wer = if autoren.is_empty() {
        String::new()
    } else {
        format!(" von {}", autoren.join(", "))
    };

    funde.push(Finding::new(
        FindingKind::TrackedChange,
        format!("OOXML:{}", e.name),
        Some(format!(
            "{eingefuegt} Einfügungen, {geloescht} Löschungen{wer} — \
             gelöschter Text steht weiterhin vollständig im Dokument"
        )),
        Severity::Critical,
    ));
}

/// Zugeschnittene Bilder (`spec/metadata.md` §7.2).
///
/// `a:srcRect` beschreibt, welcher Teil des Bildes **angezeigt** wird. Steht
/// dort ein von null verschiedener Wert, ist das Bild zugeschnitten — und das
/// Original liegt vollständig im Dokument.
fn zuschnitte(funde: &mut Vec<Finding>, e: &Eintrag) {
    let Some(text) = e.text() else {
        return;
    };
    let anzahl = xml::zaehle_zugeschnittene(text);
    if anzahl > 0 {
        funde.push(Finding::new(
            FindingKind::CroppedImage,
            format!("OOXML:{}", e.name),
            Some(format!(
                "{anzahl} zugeschnittene(s) Bild(er) — das vollständige Original \
                 liegt im Dokument und lässt sich mit zwei Klicks wiederherstellen"
            )),
            Severity::Critical,
        ));
    }
}

/// Eingebettete Bilder tragen ihre **eigenen** Metadaten mit ins Dokument.
fn eingebettete_medien(funde: &mut Vec<Finding>, e: &Eintrag) {
    let Ok(inner) = crate::inspect(&e.inhalt) else {
        return;
    };
    for f in inner.findings {
        funde.push(Finding::new(
            f.kind,
            format!("OOXML:{} → {}", e.name, f.location),
            Some(f.value.unwrap_or_else(|| "eingebettetes Medium".to_owned())),
            f.severity,
        ));
    }
}

/// Ob dieser Fund die Bereinigung überlebt.
///
/// Drei Dinge bleiben, weil ihr Entfernen eine **inhaltliche** Entscheidung
/// wäre (Modulkopf und `spec/metadata.md` §7.2): Kommentare, nachverfolgte
/// Änderungen und zugeschnittene Bilder.
fn bleibt_erhalten(f: &Finding, opts: StripOptions) -> bool {
    match f.kind {
        // Ein Zuschnitt bleibt **immer**: Ihn zu beheben hieße, das Bild neu
        // zu kodieren. Das kann dieses Modul nicht, und es wäre auch keine
        // Metadatenbereinigung mehr.
        FindingKind::CroppedImage => true,
        FindingKind::TrackedChange => !opts.accept_changes,
        // Nur der Kommentarteil selbst — nicht die Felder aus core.xml und
        // app.xml, die ebenfalls als `Comment` geführt werden.
        FindingKind::Comment => ist_kommentarteil(&f.location) && !opts.remove_comments,
        _ => false,
    }
}

fn ist_kommentarteil(name: &str) -> bool {
    let unten = name.to_ascii_lowercase();
    unten.contains("comments") && unten.ends_with(".xml")
}

fn ist_dokumentteil(name: &str) -> bool {
    matches!(name, "word/document.xml")
        || name.starts_with("ppt/slides/slide")
        || name.starts_with("xl/drawings/drawing")
        || name.starts_with("word/header")
        || name.starts_with("word/footer")
}

fn ist_medium(name: &str) -> bool {
    let unten = name.to_ascii_lowercase();
    (unten.starts_with("word/media/")
        || unten.starts_with("ppt/media/")
        || unten.starts_with("xl/media/"))
        && (unten.ends_with(".png") || unten.ends_with(".jpg") || unten.ends_with(".jpeg"))
}

// ---------------------------------------------------------------------------
// Bereinigung
// ---------------------------------------------------------------------------

/// Elemente einer nachverfolgten Änderung, die samt Inhalt verschwinden.
///
/// `w:del` enthält den gelöschten Text — er verschwindet mit. `*Change`
/// sind Vermerke über geänderte Formatierung und tragen Name und Zeitpunkt.
const AENDERUNG_VERWERFEN: [&str; 7] = [
    "del",
    "moveFrom",
    "pPrChange",
    "rPrChange",
    "sectPrChange",
    "tblPrChange",
    "tcPrChange",
];

/// Elemente, deren Umhüllung fällt, deren Inhalt aber bleibt.
///
/// `w:ins` umschließt eingefügten Text: Die Einfügung annehmen heißt, die
/// Marke zu entfernen und den Text zu behalten.
const AENDERUNG_ENTPACKEN: [&str; 2] = ["ins", "moveTo"];

/// Kommentarmarken im Dokumentkörper. Der sichtbare Text bleibt unberührt.
const KOMMENTAR_MARKEN: [&str; 4] = [
    "commentRangeStart",
    "commentRangeEnd",
    "commentReference",
    "annotationRef",
];

/// Bereinigt ein OOXML-Dokument — nur Metadaten.
///
/// # Fehler
///
/// [`cabrik_core::Error::Malformed`] bei kaputtem Container.
pub fn strip(daten: &[u8]) -> Result<(Vec<u8>, StripResult)> {
    strip_with(daten, StripOptions::nur_metadaten())
}

/// Bereinigt ein OOXML-Dokument mit ausdrücklichen Optionen.
///
/// Siehe [`StripOptions`] dazu, warum das Entfernen von Kommentaren und
/// nachverfolgten Änderungen eine gesonderte Entscheidung ist.
///
/// # Fehler
///
/// [`cabrik_core::Error::Malformed`] bei kaputtem Container.
pub fn strip_with(daten: &[u8], opts: StripOptions) -> Result<(Vec<u8>, StripResult)> {
    let eintraege = container::lies(daten)?;
    let alle_funde = sammle(&eintraege);

    let mut entfernt = Vec::new();
    let mut geblieben = Vec::new();
    let mut neu: Vec<Eintrag> = Vec::with_capacity(eintraege.len());

    // Eingeordnet wird danach, was **tatsächlich geschieht** — nicht nach der
    // Fundart. Ein früherer Entwurf sortierte nach `FindingKind` und meldete
    // `app.xml/Template` als „geblieben", obwohl `app.xml` vollständig ersetzt
    // wird. Eine Restliste, die Entferntes aufführt, ist schlimmer als keine:
    // Sie lässt den Nutzer etwas suchen, das nicht mehr da ist.
    for f in alle_funde {
        if bleibt_erhalten(&f, opts) {
            geblieben.push(f);
        } else {
            entfernt.push(f);
        }
    }

    for e in eintraege {
        // Vorschaubild und angehängtes XML verschwinden ganz. Auf einen
        // fehlenden Teil reagiert Word gelassen, sofern auch die Beziehung
        // entfällt — sonst kommt die Reparaturabfrage.
        if e.name.starts_with("docProps/thumbnail.") || e.name.starts_with("customXml/") {
            continue;
        }
        if opts.remove_comments && ist_kommentarteil(&e.name) {
            continue;
        }

        let inhalt = match e.name.as_str() {
            "docProps/core.xml" => CORE_HUELLE.as_bytes().to_vec(),
            "docProps/app.xml" => APP_HUELLE.as_bytes().to_vec(),
            "docProps/custom.xml" => CUSTOM_HUELLE.as_bytes().to_vec(),
            _ if e.name.ends_with(".rels") => e.text().map_or_else(
                || e.inhalt.clone(),
                |t| {
                    // Beziehungen auf entfernte Teile. Sie stehen in
                    // verschiedenen `.rels`-Dateien, deshalb alle prüfen.
                    let mut ohne = xml::entferne_beziehung(t, "thumbnail");
                    ohne = xml::entferne_beziehung(&ohne, "customXml");
                    if opts.remove_comments {
                        for typ in ["comments", "commentsExtended", "commentsIds"] {
                            ohne = xml::entferne_beziehung(&ohne, typ);
                        }
                    }
                    ohne.into_bytes()
                },
            ),
            _ => {
                if e.name.ends_with("settings.xml") {
                    e.text()
                        .map_or_else(|| e.inhalt.clone(), |t| xml::entferne_rsids(t).into_bytes())
                } else if ist_dokumentteil(&e.name) || e.name.ends_with(".xml") {
                    e.text().map_or_else(
                        || e.inhalt.clone(),
                        |t| behandle_xml_teil(t, &e.name, opts).into_bytes(),
                    )
                } else if ist_medium(&e.name) {
                    // Ein Bild im Dokument bringt seine eigenen Metadaten mit.
                    crate::strip(&e.inhalt).map_or_else(|_| e.inhalt.clone(), |(sauber, _)| sauber)
                } else {
                    e.inhalt.clone()
                }
            }
        };

        neu.push(Eintrag { inhalt, ..e });
    }

    let aus = container::schreib(&neu)?;

    let ergebnis = if geblieben.is_empty() {
        StripResult::Complete { removed: entfernt }
    } else {
        StripResult::Partial {
            removed: entfernt,
            remaining: geblieben,
            reason: grund(opts),
        }
    };
    Ok((aus, ergebnis))
}

/// Warum etwas geblieben ist — je nachdem, was der Nutzer erlaubt hat.
fn grund(opts: StripOptions) -> String {
    if opts.greift_in_den_inhalt_ein() {
        // Dann bleiben nur noch die Zuschnitte, und die aus einem anderen
        // Grund: Sie zu beheben hieße, das Bild neu zu kodieren.
        "Ein zugeschnittenes Bild lässt sich nur beheben, indem das Bild neu \
         kodiert und im Dokument ersetzt wird. Das verändert die Darstellung \
         und ist deshalb kein Schritt, den ein Bereinigungswerkzeug \
         ungefragt tut. Wer den weggeschnittenen Bereich wirklich loswerden \
         will, schneidet das Bild vor dem Einfügen zu."
            .to_owned()
    } else {
        "Kommentare, nachverfolgte Änderungen und zugeschnittene Bilder sind \
         Bestandteil des Dokuments, nicht Beiwerk. Sie zu entfernen hieße, \
         inhaltliche Entscheidungen zu treffen — das bleibt Ihnen überlassen. \
         Kommentare und Änderungen lassen sich auf ausdrückliche Anweisung \
         auflösen."
            .to_owned()
    }
}

/// Behandelt einen XML-Teil des Dokuments.
fn behandle_xml_teil(text: &str, name: &str, opts: StripOptions) -> String {
    let ohne_rsid = xml::entferne_rsid_attribute(text);

    if !ist_dokumentteil(name) {
        return ohne_rsid;
    }

    let mut verwerfen: Vec<&str> = Vec::new();
    let mut entpacken: Vec<&str> = Vec::new();

    if opts.accept_changes {
        verwerfen.extend(AENDERUNG_VERWERFEN);
        entpacken.extend(AENDERUNG_ENTPACKEN);
    }
    if opts.remove_comments {
        // Nur die Marken. Der sichtbare Text bleibt unberührt.
        verwerfen.extend(KOMMENTAR_MARKEN);
    }

    if verwerfen.is_empty() && entpacken.is_empty() {
        return ohne_rsid;
    }
    xml::forme_um(&ohne_rsid, &verwerfen, &entpacken)
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

    fn eintrag(name: &str, inhalt: &str) -> Eintrag {
        Eintrag {
            name: name.to_owned(),
            inhalt: inhalt.as_bytes().to_vec(),
            komprimiert: true,
            verzeichnis: false,
        }
    }

    /// Ein Dokument, wie Word es schreibt — mit allem, was v1 uebersah.
    fn dokument() -> Vec<u8> {
        let eintraege = vec![
            eintrag(
                "[Content_Types].xml",
                r#"<?xml version="1.0"?><Types xmlns="x"/>"#,
            ),
            eintrag(
                "_rels/.rels",
                concat!(
                    r#"<?xml version="1.0"?><Relationships xmlns="r">"#,
                    r#"<Relationship Id="rId1" Type="http://.../officeDocument" Target="word/document.xml"/>"#,
                    r#"<Relationship Id="rId2" Type="http://.../thumbnail" Target="docProps/thumbnail.jpeg"/>"#,
                    r#"</Relationships>"#
                ),
            ),
            eintrag(
                "docProps/core.xml",
                concat!(
                    r#"<?xml version="1.0"?><cp:coreProperties xmlns:cp="c" xmlns:dc="d" xmlns:dcterms="t">"#,
                    r#"<dc:creator>Dr. Anna Beispiel</dc:creator>"#,
                    r#"<cp:lastModifiedBy>Dr. Anna Beispiel</cp:lastModifiedBy>"#,
                    r#"<cp:revision>17</cp:revision>"#,
                    r#"<dcterms:created>2026-03-01T09:12:00Z</dcterms:created>"#,
                    r#"</cp:coreProperties>"#
                ),
            ),
            eintrag(
                "docProps/app.xml",
                concat!(
                    r#"<?xml version="1.0"?><Properties xmlns="e">"#,
                    r#"<Application>Microsoft Office Word</Application>"#,
                    r#"<Company>Kanzlei Muster &amp; Partner</Company>"#,
                    r#"<Manager>Prof. Carl Chef</Manager>"#,
                    r#"<Template>Mandantenschreiben.dotx</Template>"#,
                    r#"<TotalTime>428</TotalTime>"#,
                    r#"</Properties>"#
                ),
            ),
            eintrag(
                "docProps/custom.xml",
                concat!(
                    r#"<?xml version="1.0"?><Properties xmlns="c">"#,
                    r#"<property fmtid="{x}" pid="2" name="Aktenzeichen"><vt:lpwstr>2026-0815</vt:lpwstr></property>"#,
                    r#"</Properties>"#
                ),
            ),
            eintrag("docProps/thumbnail.jpeg", "\u{FFFD}JPEG-Vorschau"),
            eintrag(
                "word/settings.xml",
                concat!(
                    r#"<?xml version="1.0"?><w:settings xmlns:w="w"><w:rsids>"#,
                    r#"<w:rsidRoot w:val="00A1B2C3"/><w:rsid w:val="00A1B2C3"/><w:rsid w:val="00D4E5F6"/>"#,
                    r#"</w:rsids></w:settings>"#
                ),
            ),
            eintrag(
                "word/document.xml",
                concat!(
                    r#"<?xml version="1.0"?><w:document xmlns:w="w" xmlns:a="a">"#,
                    r#"<w:p w:rsidR="00A1B2C3" w:rsidRDefault="00A1B2C3">"#,
                    r#"<w:r><w:t xml:space="preserve">Sehr geehrte Damen und Herren, </w:t></w:r>"#,
                    r#"<w:ins w:id="1" w:author="Dr. Anna Beispiel" w:date="2026-03-01T10:00:00Z">"#,
                    r#"<w:r><w:t>eingefuegter Text</w:t></w:r></w:ins>"#,
                    r#"<w:del w:id="2" w:author="Prof. Carl Chef" w:date="2026-03-02T11:00:00Z">"#,
                    r#"<w:r><w:delText>vertraulicher geloeschter Text</w:delText></w:r></w:del>"#,
                    r#"</w:p>"#,
                    r#"<a:blipFill><a:srcRect l="20000" t="0" r="15000" b="0"/></a:blipFill>"#,
                    r#"</w:document>"#
                ),
            ),
            eintrag(
                "word/comments.xml",
                r#"<?xml version="1.0"?><w:comments xmlns:w="w"><w:comment w:author="Prof. Carl Chef"/></w:comments>"#,
            ),
        ];
        container::schreib(&eintraege).unwrap()
    }

    fn finde_ort<'a>(funde: &'a [Finding], teil: &str) -> Option<&'a Finding> {
        funde.iter().find(|f| f.location.contains(teil))
    }

    #[test]
    fn art_wird_am_inhalt_erkannt_nicht_an_der_endung() {
        let e = container::lies(&dokument()).unwrap();
        assert_eq!(erkenne(&e), Some(Art::Word));
    }

    /// **Der Firmenname ist der wichtigste Fund.** Ein anonym gemeintes
    /// Schreiben traegt sonst den Namen der Kanzlei.
    #[test]
    fn der_firmenname_wird_als_kritisch_gemeldet() {
        let i = inspect(&dokument()).unwrap();
        let f = finde_ort(&i.findings, "app.xml/Company").expect("Company nicht gefunden");
        assert_eq!(f.severity, Severity::Critical);
        assert_eq!(f.kind, FindingKind::Organization);
        assert_eq!(f.value.as_deref(), Some("Kanzlei Muster & Partner"));
    }

    /// Genau die Teile, die v1 unangetastet liess.
    #[test]
    fn alles_was_v1_uebersah_wird_gefunden() {
        let i = inspect(&dokument()).unwrap();
        for erwartet in [
            "app.xml/Company",
            "app.xml/Manager",
            "app.xml/Template",
            "custom.xml",
            "settings.xml",
            "thumbnail",
            "comments.xml",
        ] {
            assert!(
                finde_ort(&i.findings, erwartet).is_some(),
                "{erwartet} wurde nicht gefunden. Funde: {:?}",
                i.findings.iter().map(|f| &f.location).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn nachverfolgte_aenderungen_nennen_die_autoren() {
        let i = inspect(&dokument()).unwrap();
        let f = finde_ort(&i.findings, "document.xml")
            .filter(|f| f.kind == FindingKind::TrackedChange)
            .expect("keine nachverfolgten Aenderungen gefunden");
        let wert = f.value.as_deref().unwrap_or_default();
        assert!(wert.contains("Dr. Anna Beispiel"), "{wert}");
        assert!(wert.contains("Prof. Carl Chef"), "{wert}");
        assert_eq!(f.severity, Severity::Critical);
    }

    /// spec/metadata.md §7.2 — der haeufigste und unbekannteste Fall.
    #[test]
    fn zugeschnittene_bilder_werden_als_kritisch_gemeldet() {
        let i = inspect(&dokument()).unwrap();
        let f = i
            .findings
            .iter()
            .find(|f| f.kind == FindingKind::CroppedImage)
            .expect("Zuschnitt nicht erkannt");
        assert_eq!(f.severity, Severity::Critical);
        assert!(f.value.as_deref().unwrap_or_default().contains("Original"));
    }

    #[test]
    fn bereinigung_entfernt_die_eigenschaften() {
        let (sauber, _) = strip(&dokument()).unwrap();
        let e = container::lies(&sauber).unwrap();

        let core = container::finde(&e, "docProps/core.xml").unwrap();
        assert!(!core.text().unwrap().contains("Anna Beispiel"));

        let app = container::finde(&e, "docProps/app.xml").unwrap();
        let app_text = app.text().unwrap();
        assert!(!app_text.contains("Kanzlei"), "Firmenname blieb stehen");
        assert!(!app_text.contains("Carl Chef"));
        assert!(!app_text.contains("Mandantenschreiben"));

        let custom = container::finde(&e, "docProps/custom.xml").unwrap();
        assert!(!custom.text().unwrap().contains("Aktenzeichen"));
    }

    #[test]
    fn das_vorschaubild_verschwindet_samt_beziehung() {
        let (sauber, _) = strip(&dokument()).unwrap();
        let e = container::lies(&sauber).unwrap();

        assert!(
            container::finde(&e, "docProps/thumbnail.jpeg").is_none(),
            "Vorschaubild blieb im Archiv"
        );
        let rels = container::finde(&e, "_rels/.rels").unwrap().text().unwrap();
        assert!(
            !rels.contains("thumbnail"),
            "die Beziehung zeigt ins Leere: {rels}"
        );
        assert!(
            rels.contains("word/document.xml"),
            "die uebrigen Beziehungen wurden mitentfernt"
        );
    }

    #[test]
    fn sitzungskennungen_verschwinden() {
        let (sauber, _) = strip(&dokument()).unwrap();
        let e = container::lies(&sauber).unwrap();

        let settings = container::finde(&e, "word/settings.xml").unwrap();
        assert!(
            !settings.text().unwrap().contains("00A1B2C3"),
            "rsid blieb in settings.xml"
        );

        let doc = container::finde(&e, "word/document.xml").unwrap();
        assert!(
            !doc.text().unwrap().contains("00A1B2C3"),
            "rsid-Attribute blieben im Dokument"
        );
    }

    /// Der Text des Dokuments darf dabei nicht verlorengehen -- auch nicht
    /// das bedeutsame Leerzeichen in `xml:space="preserve"`.
    #[test]
    fn der_dokumenttext_ueberlebt_die_bereinigung() {
        let (sauber, _) = strip(&dokument()).unwrap();
        let e = container::lies(&sauber).unwrap();
        let doc = container::finde(&e, "word/document.xml").unwrap();
        let text = doc.text().unwrap();

        assert!(text.contains("Sehr geehrte Damen und Herren, "), "{text}");
        assert!(text.contains(r#"xml:space="preserve""#), "{text}");
    }

    /// Kommentare und nachverfolgte Aenderungen sind Inhalt, nicht Beiwerk.
    /// Sie bleiben -- und deshalb darf nicht `Complete` gemeldet werden.
    #[test]
    fn mit_kommentaren_ist_das_ergebnis_ehrlich_partial() {
        let (_, ergebnis) = strip(&dokument()).unwrap();
        match ergebnis {
            StripResult::Partial {
                remaining, reason, ..
            } => {
                assert!(
                    remaining
                        .iter()
                        .any(|f| f.kind == FindingKind::TrackedChange),
                    "nachverfolgte Aenderungen fehlen in der Restliste"
                );
                assert!(reason.contains("Bestandteil des Dokuments"), "{reason}");
            }
            other => panic!("erwartete Partial, bekam {other:?}"),
        }
        assert!(
            !strip(&dokument()).unwrap().1.may_show_clean(),
            "es darf keine Sauberkeit behauptet werden"
        );
    }

    /// Ohne Kommentare und Aenderungen ist `Complete` gerechtfertigt.
    #[test]
    fn ohne_inhaltliche_reste_ist_das_ergebnis_complete() {
        let schlicht = container::schreib(&[
            eintrag(
                "[Content_Types].xml",
                r#"<?xml version="1.0"?><Types xmlns="x"/>"#,
            ),
            eintrag(
                "docProps/app.xml",
                r#"<?xml version="1.0"?><Properties xmlns="e"><Company>Muster GmbH</Company></Properties>"#,
            ),
            eintrag(
                "word/document.xml",
                r#"<?xml version="1.0"?><w:document xmlns:w="w"><w:p><w:r><w:t>Text</w:t></w:r></w:p></w:document>"#,
            ),
        ])
        .unwrap();

        let (sauber, ergebnis) = strip(&schlicht).unwrap();
        assert!(ergebnis.may_show_clean(), "{ergebnis:?}");

        let e = container::lies(&sauber).unwrap();
        assert!(
            !container::finde(&e, "docProps/app.xml")
                .unwrap()
                .text()
                .unwrap()
                .contains("Muster GmbH")
        );
    }

    /// **Die Zusatzentscheidung.** Auf ausdrückliche Anweisung verschwinden
    /// auch Kommentare und nachverfolgte Änderungen.
    #[test]
    fn auf_anweisung_verschwinden_auch_die_inhaltlichen_reste() {
        let (sauber, ergebnis) =
            strip_with(&dokument(), StripOptions::auch_inhaltliche_reste()).unwrap();
        let e = container::lies(&sauber).unwrap();

        // Der Kommentarteil ist weg.
        assert!(
            container::finde(&e, "word/comments.xml").is_none(),
            "der Kommentarteil blieb im Archiv"
        );

        let doc = container::finde(&e, "word/document.xml")
            .unwrap()
            .text()
            .unwrap();

        // Der geloeschte Text ist wirklich weg -- das ist der Kern.
        assert!(
            !doc.contains("vertraulicher geloeschter Text"),
            "der geloeschte Text steht weiterhin im Dokument: {doc}"
        );
        // Die eingefuegte Passage bleibt, nur ihre Marke faellt weg.
        assert!(doc.contains("eingefuegter Text"), "{doc}");
        assert!(!doc.contains("w:ins"), "{doc}");
        assert!(!doc.contains("w:del"), "{doc}");
        // Der gewoehnliche Text ist unberuehrt.
        assert!(doc.contains("Sehr geehrte Damen und Herren, "), "{doc}");

        // Nur der Zuschnitt bleibt -- und der Grund sagt, warum.
        match ergebnis {
            StripResult::Partial {
                remaining, reason, ..
            } => {
                assert_eq!(remaining.len(), 1, "{remaining:?}");
                assert_eq!(
                    remaining.first().map(|f| f.kind),
                    Some(FindingKind::CroppedImage)
                );
                assert!(
                    reason.contains("neu\nkodiert") || reason.contains("neu kodiert"),
                    "{reason}"
                );
            }
            other => panic!("erwartete Partial wegen des Zuschnitts, bekam {other:?}"),
        }
    }

    /// Kommentare entfernen darf den **Text** nicht antasten.
    #[test]
    fn kommentare_entfernen_laesst_den_text_unberuehrt() {
        let opts = StripOptions {
            remove_comments: true,
            accept_changes: false,
        };
        let (sauber, _) = strip_with(&dokument(), opts).unwrap();
        let e = container::lies(&sauber).unwrap();
        let doc = container::finde(&e, "word/document.xml")
            .unwrap()
            .text()
            .unwrap();

        assert!(doc.contains("Sehr geehrte Damen und Herren, "), "{doc}");
        // Ohne accept_changes bleiben die Aenderungen stehen.
        assert!(
            doc.contains("vertraulicher geloeschter Text"),
            "ohne --accept-changes darf nichts am Inhalt geschehen"
        );
    }

    /// Die Voreinstellung greift nicht in den Inhalt ein.
    #[test]
    fn ohne_anweisung_bleibt_der_inhalt_unangetastet() {
        let (sauber, _) = strip(&dokument()).unwrap();
        let e = container::lies(&sauber).unwrap();

        assert!(
            container::finde(&e, "word/comments.xml").is_some(),
            "Kommentare wurden ungefragt entfernt"
        );
        let doc = container::finde(&e, "word/document.xml")
            .unwrap()
            .text()
            .unwrap();
        assert!(doc.contains("vertraulicher geloeschter Text"));
    }

    /// Zweimal bereinigen muss zweimal dasselbe ergeben.
    #[test]
    fn die_bereinigung_ist_wiederholbar() {
        let a = strip(&dokument()).unwrap().0;
        let b = strip(&dokument()).unwrap().0;
        assert_eq!(a, b);
    }

    /// Nach der Bereinigung darf nichts mehr zu finden sein -- ausser dem,
    /// was bewusst bleibt.
    #[test]
    fn eine_zweite_pruefung_findet_nur_noch_die_reste() {
        let (sauber, _) = strip(&dokument()).unwrap();
        let i = inspect(&sauber).unwrap();

        for f in &i.findings {
            assert!(
                matches!(
                    f.kind,
                    FindingKind::Comment | FindingKind::TrackedChange | FindingKind::CroppedImage
                ),
                "nach der Bereinigung blieb {f:?}"
            );
        }
    }
}
