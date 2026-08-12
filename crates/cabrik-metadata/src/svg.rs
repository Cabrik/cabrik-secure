//! SVG (`spec/metadata.md` §7.4).
//!
//! Das ungewöhnlichste Format der Liste: **SVG ist beliebiges XML** und kann
//! Programmcode, Verweise auf fremde Rechner und ganze Rasterbilder tragen.
//! Das Ergebnis bleibt deshalb **immer** [`StripResult::Partial`] — nie
//! `Complete`. Eine Vollständigkeitszusage wäre bei einem Format, das sich
//! beliebig erweitern lässt, nicht haltbar.
//!
//! # Der schwerwiegendste Fund ist kein Metadatum
//!
//! Ein `xlink:href` auf eine fremde Adresse wird zum **Zählpixel**: Sobald der
//! Empfänger die Datei im Browser öffnet, meldet sein Rechner Zeitpunkt und
//! IP-Adresse an einen Dritten. Bei einem Werkzeug für vertrauliche
//! Kommunikation ist das gravierender als jeder Autorenname — der Absender
//! erfährt, *wann* und *von wo* gelesen wurde.
//!
//! Dasselbe gilt für `<script>`: Code, der beim Öffnen im Browser des
//! Empfängers ausgeführt wird.
//!
//! # Elemente nach Erlaubnisliste, Attribute nach Regel
//!
//! Bei **Elementen** wird nur behalten, was namentlich bekannt ist. Eine
//! Sperrliste übersähe zwangsläufig, was sie nicht kennt, und SVG entwickelt
//! sich weiter.
//!
//! Bei **Attributen** wäre eine Erlaubnisliste der falsche Weg, und das ist
//! eine bewusste Abweichung von `spec/metadata.md` §7.4: SVG kennt über
//! zweihundert Darstellungsattribute. Eine Liste davon wäre lang, unvollständig
//! und bräche jede Datei, die ein neueres Attribut benutzt — ohne
//! Sicherheitsgewinn.
//!
//! Der Unterschied liegt darin, dass die **gefährliche Menge bei Attributen
//! benennbar ist**, bei Elementen aber nicht:
//!
//! | Regel | Was sie erfasst |
//! |---|---|
//! | Name beginnt mit `on` | **alle** Ereignisbehandler — die Schreibweise ist im Standard festgelegt |
//! | Namensraumpräfix außer `xml:` | `inkscape:`, `sodipodi:`, `dc:`, `rdf:` — Bearbeitungsspuren |
//! | Verweis nach außen | `href`, `xlink:href`, `url(…)` auf fremde Adressen |
//!
//! Ein Element hingegen kann alles Mögliche sein und beliebig hinzukommen.
//! Deshalb dort die Erlaubnisliste.
//!
//! # Eingebettete Rasterbilder
//!
//! Ein `data:`-URI mit einem JPEG bringt dessen **eigenes EXIF** mit,
//! einschließlich GPS und Vorschaubild. Solche Bilder werden ausgepackt,
//! durch dieselbe Bereinigung geschickt und wieder eingesetzt.

use crate::model::{Finding, FindingKind, Inspection, Severity, StripResult};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use cabrik_core::{Error, Result};
use quick_xml::events::attributes::Attribute;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, Writer};

use std::io::Cursor;

/// Höchstgröße einer Datei, die wir anfassen.
const MAX_DATEI: usize = 64 * 1024 * 1024;

/// Elemente, die bleiben dürfen.
///
/// Bewusst großzügig: Alles, was zur Darstellung gehört, einschließlich
/// Animation und Filter. Was fehlt, fällt weg — und wird gemeldet.
const ERLAUBTE_ELEMENTE: &[&str] = &[
    // Aufbau
    "svg",
    "g",
    "defs",
    "symbol",
    "use",
    "switch",
    "view",
    "a",
    "style",
    // Formen und Text
    "path",
    "rect",
    "circle",
    "ellipse",
    "line",
    "polyline",
    "polygon",
    "text",
    "tspan",
    "textPath",
    "image",
    // Verläufe, Muster, Masken
    "linearGradient",
    "radialGradient",
    "stop",
    "pattern",
    "clipPath",
    "mask",
    "marker",
    // Filter
    "filter",
    "feBlend",
    "feColorMatrix",
    "feComponentTransfer",
    "feComposite",
    "feConvolveMatrix",
    "feDiffuseLighting",
    "feDisplacementMap",
    "feDistantLight",
    "feDropShadow",
    "feFlood",
    "feFuncA",
    "feFuncB",
    "feFuncG",
    "feFuncR",
    "feGaussianBlur",
    "feImage",
    "feMerge",
    "feMergeNode",
    "feMorphology",
    "feOffset",
    "fePointLight",
    "feSpecularLighting",
    "feSpotLight",
    "feTile",
    "feTurbulence",
    // Animation
    "animate",
    "animateMotion",
    "animateTransform",
    "mpath",
    "set",
    // Sonstiges zur Darstellung
    "solidColor",
    "hatch",
    "hatchpath",
];

/// Elemente, die ausdrücklich entfernt werden — mit ihrer Einordnung.
///
/// Sie stünden ohnehin nicht in der Erlaubnisliste; sie hier zu benennen dient
/// der **Meldung**: Der Nutzer soll erfahren, *was* verschwand, nicht nur
/// *dass* etwas verschwand.
fn einordnung(name: &str) -> Option<(FindingKind, Severity, &'static str)> {
    Some(match name {
        "script" => (
            FindingKind::UnknownExtension,
            Severity::Critical,
            "ausführbarer Code — läuft beim Öffnen im Browser des Empfängers",
        ),
        "foreignObject" => (
            FindingKind::UnknownExtension,
            Severity::Critical,
            "eingebettetes HTML — beliebig erweiterbar und nicht überschaubar",
        ),
        "metadata" => (
            FindingKind::Author,
            Severity::Critical,
            "Metadatenblock, trägt meist Autor, Lizenz und Bearbeitungsprogramm",
        ),
        "title" => (FindingKind::Comment, Severity::Notable, "Titel"),
        "desc" => (FindingKind::Comment, Severity::Notable, "Beschreibung"),
        _ => return None,
    })
}

/// Ob die Bytes wie ein SVG aussehen.
#[must_use]
pub fn looks_like_svg(daten: &[u8]) -> bool {
    let Ok(text) = core::str::from_utf8(daten) else {
        return false;
    };
    // Nur der Anfang wird durchsucht: Ein `<svg` tief in einem Textdokument
    // macht daraus kein SVG.
    let anfang: String = text.chars().take(1024).collect();
    let getrimmt = anfang.trim_start();
    (getrimmt.starts_with("<?xml") || getrimmt.starts_with("<svg") || getrimmt.starts_with("<!--"))
        && anfang.contains("<svg")
}

/// Der lokale Name ohne Namensraumpräfix.
fn lokal(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|b| *b == b':') {
        Some(i) => name.get(i.saturating_add(1)..).unwrap_or(name),
        None => name,
    }
}

/// Das Präfix vor dem Doppelpunkt, sofern vorhanden.
fn praefix(name: &[u8]) -> Option<&[u8]> {
    name.iter()
        .position(|b| *b == b':')
        .and_then(|i| name.get(..i))
}

/// Ob ein Verweis nach außen zeigt.
///
/// Innere Verweise (`#id`) und eingebettete Daten (`data:`) sind unbedenklich;
/// alles andere holt beim Öffnen etwas von einem fremden Rechner.
fn zeigt_nach_aussen(wert: &str) -> bool {
    let w = wert.trim();
    if w.is_empty() || w.starts_with('#') {
        return false;
    }
    if w.starts_with("data:") {
        return false;
    }
    // Auch relative Pfade holen etwas nach — beim Empfänger allerdings ins
    // Leere. Gemeldet wird trotzdem, denn der Pfad selbst kann verraten,
    // wie das Verzeichnis des Absenders heißt.
    true
}

/// Sucht in einem `style`-Wert nach Verweisen auf fremde Adressen.
fn stil_verweise(wert: &str) -> Vec<String> {
    let mut aus = Vec::new();
    let mut rest = wert;
    while let Some(i) = rest.find("url(") {
        let Some(nach) = rest.get(i.saturating_add(4)..) else {
            break;
        };
        let Some(ende) = nach.find(')') else {
            break;
        };
        let ziel = nach
            .get(..ende)
            .unwrap_or("")
            .trim()
            .trim_matches(['"', '\''])
            .to_owned();
        if zeigt_nach_aussen(&ziel) {
            aus.push(ziel);
        }
        rest = nach.get(ende..).unwrap_or("");
    }
    aus
}

// ---------------------------------------------------------------------------
// Durchgang
// ---------------------------------------------------------------------------

/// Ergebnis eines Durchgangs: die bereinigte Fassung und alles Gefundene.
struct Durchgang {
    ausgabe: String,
    funde: Vec<Finding>,
}

/// Geht das Dokument einmal durch — findet und bereinigt in einem Zug.
///
/// Beides zusammen, damit Meldung und Ergebnis nicht auseinanderlaufen
/// können: Was gemeldet wird, ist genau das, was auch entfernt wurde.
fn durchgang(quelle: &str) -> Durchgang {
    let mut leser = Reader::from_str(quelle);
    leser.config_mut().trim_text(false);
    let mut schreiber = Writer::new(Cursor::new(Vec::new()));

    let mut funde: Vec<Finding> = Vec::new();
    // Tiefe innerhalb eines verworfenen Teilbaums.
    let mut verworfen = 0usize;

    loop {
        match leser.read_event() {
            Ok(Event::Start(e)) => {
                if verworfen > 0 {
                    verworfen = verworfen.saturating_add(1);
                    continue;
                }
                if let Some(fund) = pruefe_element(&e) {
                    funde.push(fund);
                    verworfen = 1;
                    continue;
                }
                let _ = schreiber.write_event(Event::Start(saeubere(&e, &mut funde)));
            }
            Ok(Event::End(e)) => {
                if verworfen > 0 {
                    verworfen = verworfen.saturating_sub(1);
                    continue;
                }
                let _ = schreiber.write_event(Event::End(e));
            }
            Ok(Event::Empty(e)) => {
                if verworfen > 0 {
                    continue;
                }
                if let Some(fund) = pruefe_element(&e) {
                    funde.push(fund);
                    continue;
                }
                let _ = schreiber.write_event(Event::Empty(saeubere(&e, &mut funde)));
            }
            // Kommentare können Bearbeitungsspuren und Klarnamen enthalten.
            Ok(Event::Comment(c)) => {
                if verworfen == 0 {
                    let text = c.decode().unwrap_or_default().trim().to_owned();
                    if !text.is_empty() {
                        funde.push(Finding::new(
                            FindingKind::Comment,
                            "SVG:Kommentar".to_owned(),
                            Some(text),
                            Severity::Notable,
                        ));
                    }
                }
            }
            // Eine Dokumenttypdefinition kann Entitäten einführen, die auf
            // Dateien des Empfängers zeigen. Sie fällt ersatzlos weg.
            Ok(Event::DocType(_)) => {
                funde.push(Finding::new(
                    FindingKind::UnknownExtension,
                    "SVG:DOCTYPE".to_owned(),
                    Some(
                        "Dokumenttypdefinition — kann Entitäten einführen, die auf \
                         Dateien des Empfängers zeigen"
                            .to_owned(),
                    ),
                    Severity::Critical,
                ));
            }
            Ok(Event::Eof) => break,
            Ok(anderes) => {
                if verworfen == 0 {
                    let _ = schreiber.write_event(anderes);
                }
            }
            Err(_) => break,
        }
    }

    let ausgabe = String::from_utf8(schreiber.into_inner().into_inner())
        .unwrap_or_else(|_| quelle.to_owned());
    Durchgang { ausgabe, funde }
}

/// Prüft, ob ein Element ganz wegfällt.
fn pruefe_element(e: &BytesStart<'_>) -> Option<Finding> {
    let name = e.name();
    let roh = name.as_ref();
    let kurz = String::from_utf8_lossy(lokal(roh)).into_owned();

    // Ein Element mit fremdem Namensraum gehört nicht zur Darstellung.
    if let Some(p) = praefix(roh)
        && p != b"svg"
    {
        let p_name = String::from_utf8_lossy(p).into_owned();
        return Some(Finding::new(
            FindingKind::Software,
            format!("SVG:{p_name}:{kurz}"),
            Some(format!(
                "Element aus dem Namensraum „{p_name}\" — Bearbeitungsspur des \
                 erzeugenden Programms"
            )),
            Severity::Notable,
        ));
    }

    if let Some((art, schwere, was)) = einordnung(&kurz) {
        return Some(Finding::new(
            art,
            format!("SVG:{kurz}"),
            Some(was.to_owned()),
            schwere,
        ));
    }

    if ERLAUBTE_ELEMENTE.contains(&kurz.as_str()) {
        return None;
    }

    Some(Finding::new(
        FindingKind::UnknownExtension,
        format!("SVG:{kurz}"),
        Some(format!(
            "unbekanntes Element „{kurz}\" — entfernt, weil nicht einzuordnen"
        )),
        Severity::Notable,
    ))
}

/// Entfernt gefährliche Attribute und meldet sie.
fn saeubere<'a>(e: &BytesStart<'a>, funde: &mut Vec<Finding>) -> BytesStart<'a> {
    let element = String::from_utf8_lossy(lokal(e.name().as_ref())).into_owned();
    let mut neu = BytesStart::new(String::from_utf8_lossy(e.name().as_ref()).into_owned());

    for a in e.attributes().flatten() {
        match beurteile_attribut(&element, &a) {
            Beurteilung::Behalten => neu.push_attribute(a),
            Beurteilung::Ersetzen(wert) => {
                let name = String::from_utf8_lossy(a.key.as_ref()).into_owned();
                neu.push_attribute((name.as_str(), wert.as_str()));
            }
            Beurteilung::Entfernen(fund) => funde.push(fund),
        }
    }
    neu
}

enum Beurteilung {
    Behalten,
    /// Der Wert wird ersetzt — etwa ein bereinigtes eingebettetes Bild.
    Ersetzen(String),
    Entfernen(Finding),
}

fn beurteile_attribut(element: &str, a: &Attribute<'_>) -> Beurteilung {
    let roh = a.key.as_ref();
    let name = String::from_utf8_lossy(roh).into_owned();
    let kurz = String::from_utf8_lossy(lokal(roh)).into_owned();
    let wert = a
        .normalized_value(quick_xml::XmlVersion::Explicit1_0)
        .map(|c| c.into_owned())
        .unwrap_or_default();

    // 1. Ereignisbehandler. Die Schreibweise ist im Standard festgelegt,
    //    damit erfasst diese eine Regel sie alle.
    if kurz.starts_with("on") {
        return Beurteilung::Entfernen(Finding::new(
            FindingKind::UnknownExtension,
            format!("SVG:{element}/{name}"),
            Some("Ereignisbehandler — Code, der beim Öffnen ausgeführt wird".to_owned()),
            Severity::Critical,
        ));
    }

    // 2a. Die **Erklärung** eines fremden Namensraums.
    //
    // Sie bleibt sonst stehen, wenn alle Attribute daraus entfernt wurden —
    // und verrät weiterhin das erzeugende Programm. `xmlns:inkscape="…"` sagt
    // „diese Datei kommt aus Inkscape", auch ohne ein einziges
    // `inkscape:`-Attribut. Erhalten bleiben nur die Namensräume, die zum
    // Format gehören.
    if praefix(roh) == Some(b"xmlns") && !matches!(kurz.as_str(), "svg" | "xlink" | "xml") {
        return Beurteilung::Entfernen(Finding::new(
            FindingKind::Software,
            format!("SVG:{element}/{name}"),
            Some(format!(
                "Namensraum-Erklärung „{kurz}\" ({wert}) — nennt das erzeugende \
                 Programm, auch wenn keine Attribute daraus mehr übrig sind"
            )),
            Severity::Notable,
        ));
    }

    // 2b. Fremde Namensräume. `xml:` und `xlink:` gehören zum Format.
    if let Some(p) = praefix(roh)
        && !matches!(p, b"xml" | b"xlink" | b"xmlns")
    {
        let p_name = String::from_utf8_lossy(p).into_owned();
        return Beurteilung::Entfernen(Finding::new(
            FindingKind::Software,
            format!("SVG:{element}/{name}"),
            Some(format!("Attribut aus dem Namensraum „{p_name}\": {wert}")),
            Severity::Notable,
        ));
    }

    // 3. Verweise.
    if kurz == "href" || kurz == "src" {
        if let Some(bereinigt) = eingebettetes_bild(&wert) {
            return Beurteilung::Ersetzen(bereinigt);
        }
        if zeigt_nach_aussen(&wert) {
            return Beurteilung::Entfernen(Finding::new(
                FindingKind::UnknownExtension,
                format!("SVG:{element}/{name}"),
                Some(format!(
                    "Verweis nach außen auf „{wert}\" — wird beim Öffnen abgerufen \
                     und meldet Zeitpunkt und IP-Adresse des Empfängers an einen Dritten"
                )),
                Severity::Critical,
            ));
        }
    }

    // 4. Verweise im Stil.
    if kurz == "style" {
        let verweise = stil_verweise(&wert);
        if !verweise.is_empty() {
            return Beurteilung::Entfernen(Finding::new(
                FindingKind::UnknownExtension,
                format!("SVG:{element}/style"),
                Some(format!(
                    "Verweis nach außen im Stil: {} — wird beim Öffnen abgerufen",
                    verweise.join(", ")
                )),
                Severity::Critical,
            ));
        }
    }

    Beurteilung::Behalten
}

/// Bereinigt ein als `data:`-URI eingebettetes Rasterbild.
///
/// Gibt `None` zurück, wenn es keins ist oder sich nichts ändern ließ.
fn eingebettetes_bild(wert: &str) -> Option<String> {
    let rest = wert.strip_prefix("data:")?;
    let (kopf, daten) = rest.split_once(',')?;
    if !kopf.contains("base64") {
        return None;
    }

    let roh = BASE64.decode(daten.trim()).ok()?;
    // Bewusst **nicht** in verschachtelte SVG hinein: Das hätte keine
    // natürliche Grenze. Nur Rasterbilder.
    if looks_like_svg(&roh) {
        return None;
    }
    let (sauber, _) = crate::strip(&roh).ok()?;
    if sauber == roh {
        return None;
    }
    Some(format!("data:{kopf},{}", BASE64.encode(&sauber)))
}

/// Meldet zusätzlich, was in eingebetteten Bildern steckt.
fn medien_funde(quelle: &str) -> Vec<Finding> {
    let mut leser = Reader::from_str(quelle);
    leser.config_mut().trim_text(false);
    let mut aus = Vec::new();

    loop {
        let treffer = match leser.read_event() {
            Ok(Event::Start(e) | Event::Empty(e)) => Some(e.into_owned()),
            Ok(Event::Eof) | Err(_) => break,
            _ => None,
        };
        let Some(e) = treffer else { continue };

        for a in e.attributes().flatten() {
            if !matches!(lokal(a.key.as_ref()), b"href" | b"src") {
                continue;
            }
            let wert = a
                .normalized_value(quick_xml::XmlVersion::Explicit1_0)
                .map(|c| c.into_owned())
                .unwrap_or_default();

            let Some(rest) = wert.strip_prefix("data:") else {
                continue;
            };
            let Some((kopf, daten)) = rest.split_once(',') else {
                continue;
            };
            if !kopf.contains("base64") {
                continue;
            }
            let Ok(roh) = BASE64.decode(daten.trim()) else {
                continue;
            };
            if looks_like_svg(&roh) {
                continue;
            }
            if let Ok(innen) = crate::inspect(&roh) {
                for f in innen.findings {
                    aus.push(Finding::new(
                        f.kind,
                        format!("SVG:eingebettetes Bild → {}", f.location),
                        f.value,
                        f.severity,
                    ));
                }
            }
        }
    }
    aus
}

// ---------------------------------------------------------------------------
// Öffentlich
// ---------------------------------------------------------------------------

fn als_text(daten: &[u8]) -> Result<&str> {
    if daten.len() > MAX_DATEI {
        return Err(Error::Malformed("svg: Datei zu gross"));
    }
    core::str::from_utf8(daten).map_err(|_| Error::Malformed("svg: kein gueltiges UTF-8"))
}

/// Untersucht ein SVG.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Kodierung oder zu großer Datei.
pub fn inspect(daten: &[u8]) -> Result<Inspection> {
    let text = als_text(daten)?;
    let mut funde = durchgang(text).funde;
    funde.extend(medien_funde(text));

    Ok(Inspection {
        format: Some("SVG".to_owned()),
        findings: funde,
        understood: true,
    })
}

/// Bereinigt ein SVG.
///
/// Das Ergebnis ist **immer** [`StripResult::Partial`] — siehe Modulkopf.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Kodierung oder zu großer Datei.
pub fn strip(daten: &[u8]) -> Result<(Vec<u8>, StripResult)> {
    let text = als_text(daten)?;
    let d = durchgang(text);

    let mut entfernt = d.funde;
    entfernt.extend(medien_funde(text));

    Ok((
        d.ausgabe.into_bytes(),
        StripResult::Partial {
            removed: entfernt,
            remaining: Vec::new(),
            reason: "SVG ist beliebiges XML und lässt sich unbegrenzt erweitern. \
                     Entfernt wurde alles, was dieses Programm als Metadatum, \
                     ausführbaren Code oder Verweis nach außen erkennt — eine \
                     Zusage auf Vollständigkeit wäre bei diesem Format aber nicht \
                     haltbar und wird deshalb nicht gemacht."
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

    fn bereinige(quelle: &str) -> (String, Vec<Finding>) {
        let (bytes, ergebnis) = strip(quelle.as_bytes()).unwrap();
        let funde = match ergebnis {
            StripResult::Partial { removed, .. } => removed,
            other => panic!("SVG muss immer Partial sein, war {other:?}"),
        };
        (String::from_utf8(bytes).unwrap(), funde)
    }

    fn hat(funde: &[Finding], teil: &str) -> bool {
        funde.iter().any(|f| f.location.contains(teil))
    }

    #[test]
    fn svg_wird_erkannt_aber_nicht_jedes_xml() {
        assert!(looks_like_svg(
            br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#
        ));
        assert!(looks_like_svg(
            br#"<?xml version="1.0"?><svg xmlns="x"></svg>"#
        ));
        assert!(!looks_like_svg(b"<html><body></body></html>"));
        assert!(!looks_like_svg(b"kein XML"));
    }

    /// **Der schwerwiegendste Fund.** Ein Verweis nach aussen wird zum
    /// Zaehlpixel: Der Empfaenger meldet beim Oeffnen Zeitpunkt und
    /// IP-Adresse an einen Dritten.
    #[test]
    fn ein_verweis_nach_aussen_ist_kritisch() {
        let (aus, funde) =
            bereinige(r#"<svg xmlns="x"><image href="https://tracker.example/pixel.png"/></svg>"#);
        let f = funde
            .iter()
            .find(|f| f.location.contains("href"))
            .expect("der Verweis wurde nicht gefunden");
        assert_eq!(f.severity, Severity::Critical);
        assert!(
            f.value
                .as_deref()
                .unwrap_or_default()
                .contains("IP-Adresse")
        );
        assert!(!aus.contains("tracker.example"), "{aus}");
    }

    /// Innere Verweise gehoeren zur Darstellung und bleiben.
    #[test]
    fn innere_verweise_bleiben() {
        let (aus, funde) = bereinige(
            r##"<svg xmlns="x"><defs><linearGradient id="g"/></defs><rect fill="url(#g)" href="#g"/></svg>"##,
        );
        assert!(
            aus.contains("#g"),
            "der innere Verweis ging verloren: {aus}"
        );
        assert!(!hat(&funde, "href"), "ein innerer Verweis wurde gemeldet");
    }

    #[test]
    fn skript_und_ereignisbehandler_verschwinden() {
        let (aus, funde) = bereinige(
            r#"<svg xmlns="x"><script>alert(1)</script><rect onclick="boese()" onload="auch()"/></svg>"#,
        );
        assert!(!aus.contains("alert"), "{aus}");
        assert!(!aus.contains("onclick"), "{aus}");
        assert!(!aus.contains("onload"), "{aus}");

        assert!(hat(&funde, "SVG:script"));
        assert!(
            funde.iter().filter(|f| f.location.contains("/on")).count() >= 2,
            "nicht alle Ereignisbehandler gemeldet: {funde:?}"
        );
    }

    #[test]
    fn metadaten_und_editorspuren_verschwinden() {
        let (aus, funde) = bereinige(concat!(
            r#"<svg xmlns="x" xmlns:inkscape="i" inkscape:version="1.1" "#,
            r#"sodipodi:docname="C:\Users\daniw\Entwurf.svg">"#,
            r#"<metadata><rdf:RDF><dc:creator>Dr. Anna Beispiel</dc:creator></rdf:RDF></metadata>"#,
            r#"<title>Interner Entwurf</title><desc>nicht weitergeben</desc>"#,
            r#"<rect width="10" height="10"/></svg>"#
        ));

        for spur in [
            "Anna Beispiel",
            "Interner Entwurf",
            "nicht weitergeben",
            "daniw",
        ] {
            assert!(!aus.contains(spur), "„{spur}\" blieb: {aus}");
        }
        assert!(aus.contains("<rect"), "der Inhalt ging verloren: {aus}");

        assert!(hat(&funde, "SVG:metadata"));
        assert!(hat(&funde, "SVG:title"));
        assert!(hat(&funde, "SVG:desc"));
        assert!(hat(&funde, "inkscape"), "{funde:?}");
        assert!(hat(&funde, "sodipodi"), "{funde:?}");
    }

    /// Eine Namensraum-Erklaerung bleibt sonst stehen, wenn alle Attribute
    /// daraus entfernt wurden -- und verraet weiterhin das Programm.
    #[test]
    fn die_namensraum_erklaerung_verraet_nicht_mehr_das_programm() {
        let (aus, funde) = bereinige(concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" "#,
            r#"xmlns:xlink="http://www.w3.org/1999/xlink" "#,
            r#"xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" "#,
            r#"xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd">"#,
            r#"<rect/></svg>"#
        ));

        assert!(!aus.contains("inkscape"), "die Erklaerung blieb: {aus}");
        assert!(!aus.contains("sodipodi"), "{aus}");
        assert!(hat(&funde, "xmlns:inkscape"), "{funde:?}");

        // Was zum Format gehoert, bleibt.
        assert!(
            aus.contains(r#"xmlns="http://www.w3.org/2000/svg""#),
            "{aus}"
        );
        assert!(aus.contains("xmlns:xlink"), "xlink wird gebraucht: {aus}");
    }

    /// Ein Kommentar kann einen Klarnamen enthalten.
    #[test]
    fn kommentare_verschwinden_und_werden_gemeldet() {
        let (aus, funde) =
            bereinige(r#"<svg xmlns="x"><!-- Entwurf von Dr. Anna Beispiel --><rect/></svg>"#);
        assert!(!aus.contains("Anna Beispiel"), "{aus}");
        assert!(hat(&funde, "SVG:Kommentar"));
    }

    /// `foreignObject` kann beliebiges HTML tragen.
    #[test]
    fn fremdobjekte_verschwinden_samt_inhalt() {
        let (aus, funde) = bereinige(concat!(
            r#"<svg xmlns="x"><foreignObject><div>HEIMLICH</div></foreignObject>"#,
            r#"<rect/></svg>"#
        ));
        assert!(!aus.contains("HEIMLICH"), "der Inhalt blieb: {aus}");
        assert!(hat(&funde, "foreignObject"));
        assert!(aus.contains("<rect"));
    }

    /// Was nicht in der Erlaubnisliste steht, faellt weg -- die sichere
    /// Richtung bei einem Format, das sich beliebig erweitern laesst.
    #[test]
    fn unbekannte_elemente_fallen_weg_und_werden_benannt() {
        let (aus, funde) =
            bereinige(r#"<svg xmlns="x"><neuesElement>X</neuesElement><rect/></svg>"#);
        assert!(!aus.contains("neuesElement"), "{aus}");
        let f = funde
            .iter()
            .find(|f| f.location.contains("neuesElement"))
            .expect("nicht gemeldet");
        assert!(f.value.as_deref().unwrap_or_default().contains("unbekannt"));
    }

    /// Eine Dokumenttypdefinition kann Entitaeten einfuehren, die Dateien des
    /// Empfaengers auslesen.
    #[test]
    fn eine_doctype_erklaerung_verschwindet() {
        let (aus, funde) = bereinige(concat!(
            r#"<?xml version="1.0"?>"#,
            r#"<!DOCTYPE svg [<!ENTITY x SYSTEM "file:///etc/passwd">]>"#,
            r#"<svg xmlns="x"><rect/></svg>"#
        ));
        assert!(!aus.contains("DOCTYPE"), "{aus}");
        assert!(!aus.contains("passwd"), "{aus}");
        assert!(hat(&funde, "DOCTYPE"));
    }

    /// Die Darstellung darf nicht kaputtgehen: Formen, Pfaddaten und die
    /// gewoehnlichen Attribute bleiben unangetastet.
    #[test]
    fn die_darstellung_bleibt_erhalten() {
        let quelle = concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100">"#,
            r##"<g transform="translate(5,5)"><path d="M0 0 L10 10 Z" fill="#ff0000" "##,
            r#"stroke-width="2" opacity="0.5"/>"#,
            r#"<text x="1" y="2" font-family="serif">Sichtbarer Text</text></g></svg>"#
        );
        let (aus, _) = bereinige(quelle);

        for noetig in [
            "viewBox",
            "transform",
            "M0 0 L10 10 Z",
            "#ff0000",
            "stroke-width",
            "opacity",
            "font-family",
            "Sichtbarer Text",
        ] {
            assert!(aus.contains(noetig), "„{noetig}\" ging verloren: {aus}");
        }
    }

    #[test]
    fn verweise_im_stil_werden_gefunden() {
        assert!(stil_verweise("fill:url(#innen)").is_empty());
        assert_eq!(
            stil_verweise("fill:url(https://fremd.example/x.png); stroke:red"),
            vec!["https://fremd.example/x.png"]
        );

        let (aus, funde) = bereinige(
            r#"<svg xmlns="x"><rect style="fill:url(https://fremd.example/p.png)"/></svg>"#,
        );
        assert!(!aus.contains("fremd.example"), "{aus}");
        assert!(hat(&funde, "/style"));
    }

    /// Das Ergebnis ist **immer** Partial -- auch wenn nichts gefunden wurde.
    #[test]
    fn das_ergebnis_ist_nie_vollstaendig() {
        let (_, ergebnis) = strip(br#"<svg xmlns="x"><rect/></svg>"#).unwrap();
        assert!(
            !ergebnis.may_show_clean(),
            "fuer SVG darf keine Vollstaendigkeit behauptet werden"
        );
        match ergebnis {
            StripResult::Partial {
                removed, reason, ..
            } => {
                assert!(removed.is_empty());
                assert!(
                    reason.contains("nicht\n                     haltbar")
                        || reason.contains("nicht haltbar"),
                    "{reason}"
                );
            }
            other => panic!("erwartete Partial, bekam {other:?}"),
        }
    }

    #[test]
    fn die_bereinigung_ist_wiederholbar() {
        let quelle = r#"<svg xmlns="x"><title>weg</title><rect onclick="x()"/></svg>"#;
        let einmal = strip(quelle.as_bytes()).unwrap().0;
        let zweimal = strip(&einmal).unwrap().0;
        assert_eq!(einmal, zweimal);
    }

    #[test]
    fn kaputte_eingaben_ergeben_einen_fehler_keinen_absturz() {
        assert!(inspect(&[0xFF, 0xFE, 0x00]).is_err(), "kein UTF-8");
        let _ = inspect(b"<svg><unvollstaendig");
    }
}
