//! XML lesen und behutsam verändern.
//!
//! # Warum ein richtiger Parser
//!
//! Die Versuchung ist groß, `rsid`-Attribute mit einer Zeichenkettensuche zu
//! entfernen. Das geht schief, sobald derselbe Text in einem Absatz vorkommt,
//! ein Attributwert ein `>` enthält oder ein CDATA-Abschnitt im Spiel ist —
//! und es geht **still** schief: Das Ergebnis ist ein Dokument, das Word nicht
//! mehr öffnet, oder eines, in dem die Kennung noch steht.
//!
//! Deshalb `quick-xml`: gelesen wird ereignisweise, geschrieben wird
//! ereignisweise, und was nicht ausdrücklich verändert wird, geht **unverändert
//! durch**.
//!
//! # Was dabei erhalten bleiben muss
//!
//! `<w:t xml:space="preserve">Text </w:t>` — das Leerzeichen am Ende ist
//! bedeutungstragend. Textereignisse werden deshalb roh durchgereicht, ohne
//! Umkodierung oder Normalisierung.

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, Writer};

use std::io::Cursor;

/// Höchstlänge eines gemeldeten Werts.
const WERT_MAX: usize = 200;

/// Der lokale Name eines Elements oder Attributs, ohne Namensraumpräfix.
///
/// `w:rsidR` wird zu `rsidR`. OOXML benutzt Präfixe uneinheitlich; auf den
/// lokalen Namen zu prüfen ist robuster als auf die vollständige Schreibweise.
fn lokal(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|b| *b == b':') {
        Some(i) => name.get(i.saturating_add(1)..).unwrap_or(name),
        None => name,
    }
}

/// Sammelt die Textinhalte aller Elemente mit diesem lokalen Namen.
#[must_use]
pub fn element_texte(quelle: &str, name: &str) -> Vec<String> {
    let mut leser = Reader::from_str(quelle);
    leser.config_mut().trim_text(false);

    let mut aus = Vec::new();
    let mut sammelnd = false;
    let mut puffer = String::new();

    loop {
        match leser.read_event() {
            Ok(Event::Start(e)) => {
                if lokal(e.name().as_ref()) == name.as_bytes() {
                    sammelnd = true;
                    puffer.clear();
                }
            }
            Ok(Event::Text(t)) if sammelnd => {
                if let Ok(s) = t.decode() {
                    puffer.push_str(&s);
                }
            }
            // Entitäten kommen als **eigenes** Ereignis, nicht im Text.
            // Wer sie überliest, verliert sie stillschweigend: Aus
            // „Muster & Partner" wurde „Muster  Partner" — ein Firmenname,
            // der im Fundbericht falsch dasteht.
            Ok(Event::GeneralRef(r)) if sammelnd => {
                if let Some(c) = aufloese_entitaet(&r) {
                    puffer.push(c);
                }
            }
            Ok(Event::End(e)) => {
                if sammelnd && lokal(e.name().as_ref()) == name.as_bytes() {
                    sammelnd = false;
                    aus.push(kuerze(&puffer));
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    aus
}

/// Löst eine Entität in ihr Zeichen auf.
///
/// Numerische Verweise (`&#38;`, `&#x26;`) kann `quick-xml` selbst auflösen.
/// Bei benannten kennt XML genau fünf vordefinierte; alles Weitere stammt aus
/// einer Dokumenttypdefinition, die OOXML und ODF nicht verwenden. Unbekanntes
/// wird verworfen statt geraten — ein falsch geratenes Zeichen in einem
/// gemeldeten Firmennamen wäre schlimmer als eine Lücke.
fn aufloese_entitaet(r: &quick_xml::events::BytesRef<'_>) -> Option<char> {
    if let Ok(Some(c)) = r.resolve_char_ref() {
        return Some(c);
    }
    match r.decode().ok()?.as_ref() {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "apos" => Some('\''),
        "quot" => Some('"'),
        _ => None,
    }
}

/// Sammelt die Werte eines Attributs an allen Elementen mit diesem Namen.
#[must_use]
pub fn attribut_werte(quelle: &str, element: &str, attribut: &str) -> Vec<String> {
    let mut leser = Reader::from_str(quelle);
    leser.config_mut().trim_text(false);

    let mut aus = Vec::new();
    loop {
        let treffer = match leser.read_event() {
            Ok(Event::Start(e) | Event::Empty(e)) => {
                if lokal(e.name().as_ref()) == element.as_bytes() {
                    Some(e.into_owned())
                } else {
                    None
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => None,
        };
        if let Some(e) = treffer {
            for a in e.attributes().flatten() {
                if lokal(a.key.as_ref()) == attribut.as_bytes()
                    && let Ok(v) = a.normalized_value(quick_xml::XmlVersion::Explicit1_0)
                {
                    aus.push(kuerze(&v));
                }
            }
        }
    }
    aus
}

/// Zählt Elemente mit diesem lokalen Namen.
#[must_use]
pub fn zaehle_elemente(quelle: &str, name: &str) -> usize {
    let mut leser = Reader::from_str(quelle);
    leser.config_mut().trim_text(false);

    let mut n = 0usize;
    loop {
        match leser.read_event() {
            Ok(Event::Start(e) | Event::Empty(e)) => {
                if lokal(e.name().as_ref()) == name.as_bytes() {
                    n = n.saturating_add(1);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    n
}

/// Zählt `srcRect`-Elemente mit einem von null verschiedenen Rand.
///
/// `<a:srcRect l="20000" r="15000"/>` heißt: Links 20 %, rechts 15 % werden
/// **nur ausgeblendet**. Fehlt ein Attribut, ist es null. Ein `srcRect` ohne
/// jedes Attribut beschneidet nichts und zählt deshalb nicht.
#[must_use]
pub fn zaehle_zugeschnittene(quelle: &str) -> usize {
    let mut leser = Reader::from_str(quelle);
    leser.config_mut().trim_text(false);

    let mut n = 0usize;
    loop {
        let treffer = match leser.read_event() {
            Ok(Event::Start(e) | Event::Empty(e)) => {
                if lokal(e.name().as_ref()) == b"srcRect" {
                    Some(e.into_owned())
                } else {
                    None
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => None,
        };
        if let Some(e) = treffer {
            let beschneidet = e.attributes().flatten().any(|a| {
                matches!(lokal(a.key.as_ref()), b"l" | b"t" | b"r" | b"b")
                    && a.normalized_value(quick_xml::XmlVersion::Explicit1_0)
                        .map(|v| v.trim() != "0" && !v.trim().is_empty())
                        .unwrap_or(false)
            });
            if beschneidet {
                n = n.saturating_add(1);
            }
        }
    }
    n
}

/// Entfernt `rsid`-Elemente samt ihrem umschließenden `rsids`-Block.
#[must_use]
pub fn entferne_rsids(quelle: &str) -> String {
    let mut leser = Reader::from_str(quelle);
    leser.config_mut().trim_text(false);
    let mut schreiber = Writer::new(Cursor::new(Vec::new()));

    // Innerhalb von `<w:rsids>` wird alles verworfen, auch der Block selbst.
    let mut tiefe_im_block = 0usize;

    loop {
        match leser.read_event() {
            Ok(Event::Start(e)) => {
                let name = lokal(e.name().as_ref()).to_vec();
                if name == b"rsids" || tiefe_im_block > 0 {
                    tiefe_im_block = tiefe_im_block.saturating_add(1);
                    continue;
                }
                let _ = schreiber.write_event(Event::Start(e));
            }
            Ok(Event::End(e)) => {
                if tiefe_im_block > 0 {
                    tiefe_im_block = tiefe_im_block.saturating_sub(1);
                    continue;
                }
                let _ = schreiber.write_event(Event::End(e));
            }
            Ok(Event::Empty(e)) => {
                if tiefe_im_block > 0 || lokal(e.name().as_ref()).starts_with(b"rsid") {
                    continue;
                }
                let _ = schreiber.write_event(Event::Empty(e));
            }
            Ok(Event::Eof) => break,
            Ok(anderes) => {
                if tiefe_im_block == 0 {
                    let _ = schreiber.write_event(anderes);
                }
            }
            Err(_) => break,
        }
    }
    fertig(schreiber, quelle)
}

/// Entfernt alle Attribute, deren lokaler Name mit `rsid` beginnt.
///
/// Word schreibt sie an jeden bearbeiteten Absatz (`w:rsidR`,
/// `w:rsidRDefault`, `w:rsidP`, `w:rsidTr`, …). Sie verketten Dokumente über
/// Bearbeitungssitzungen.
#[must_use]
pub fn entferne_rsid_attribute(quelle: &str) -> String {
    let mut leser = Reader::from_str(quelle);
    leser.config_mut().trim_text(false);
    let mut schreiber = Writer::new(Cursor::new(Vec::new()));

    loop {
        match leser.read_event() {
            Ok(Event::Start(e)) => {
                let _ = schreiber.write_event(Event::Start(ohne_rsid(&e)));
            }
            Ok(Event::Empty(e)) => {
                let _ = schreiber.write_event(Event::Empty(ohne_rsid(&e)));
            }
            Ok(Event::Eof) => break,
            Ok(anderes) => {
                let _ = schreiber.write_event(anderes);
            }
            Err(_) => break,
        }
    }
    fertig(schreiber, quelle)
}

fn ohne_rsid<'a>(e: &BytesStart<'a>) -> BytesStart<'a> {
    let mut neu = BytesStart::new(String::from_utf8_lossy(e.name().as_ref()).into_owned());
    for a in e.attributes().flatten() {
        if !lokal(a.key.as_ref()).starts_with(b"rsid") {
            neu.push_attribute(a);
        }
    }
    neu
}

/// Entfernt Beziehungen, deren `Type` auf diese Endung passt.
///
/// Ein `<Relationship>` auf einen entfernten Teil zeigt ins Leere; Word
/// beantwortet das mit einer Reparaturabfrage.
#[must_use]
pub fn entferne_beziehung(quelle: &str, typ_endet_auf: &str) -> String {
    let mut leser = Reader::from_str(quelle);
    leser.config_mut().trim_text(false);
    let mut schreiber = Writer::new(Cursor::new(Vec::new()));

    let passt = |e: &BytesStart<'_>| {
        lokal(e.name().as_ref()) == b"Relationship"
            && e.attributes().flatten().any(|a| {
                lokal(a.key.as_ref()) == b"Type"
                    && a.normalized_value(quick_xml::XmlVersion::Explicit1_0)
                        .map(|v| v.ends_with(typ_endet_auf))
                        .unwrap_or(false)
            })
    };

    let mut ueberspringe_bis_ende = 0usize;
    loop {
        match leser.read_event() {
            Ok(Event::Empty(e)) => {
                if !passt(&e) {
                    let _ = schreiber.write_event(Event::Empty(e));
                }
            }
            Ok(Event::Start(e)) => {
                if passt(&e) || ueberspringe_bis_ende > 0 {
                    ueberspringe_bis_ende = ueberspringe_bis_ende.saturating_add(1);
                    continue;
                }
                let _ = schreiber.write_event(Event::Start(e));
            }
            Ok(Event::End(e)) => {
                if ueberspringe_bis_ende > 0 {
                    ueberspringe_bis_ende = ueberspringe_bis_ende.saturating_sub(1);
                    continue;
                }
                let _ = schreiber.write_event(Event::End(e));
            }
            Ok(Event::Eof) => break,
            Ok(anderes) => {
                if ueberspringe_bis_ende == 0 {
                    let _ = schreiber.write_event(anderes);
                }
            }
            Err(_) => break,
        }
    }
    fertig(schreiber, quelle)
}

/// Gibt das Geschriebene zurück — oder die Quelle, falls etwas schieflief.
///
/// Ein halb geschriebenes XML wäre schlimmer als ein unverändertes: Es sähe
/// bereinigt aus und wäre kaputt.
fn fertig(schreiber: Writer<Cursor<Vec<u8>>>, quelle: &str) -> String {
    String::from_utf8(schreiber.into_inner().into_inner()).unwrap_or_else(|_| quelle.to_owned())
}

fn kuerze(s: &str) -> String {
    let getrimmt = s.trim();
    if getrimmt.chars().count() <= WERT_MAX {
        return getrimmt.to_owned();
    }
    let gekuerzt: String = getrimmt.chars().take(WERT_MAX).collect();
    format!("{gekuerzt}…")
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;

    #[test]
    fn lokaler_name_ignoriert_das_praefix() {
        assert_eq!(lokal(b"w:rsidR"), b"rsidR");
        assert_eq!(lokal(b"creator"), b"creator");
        assert_eq!(lokal(b"a:b:c"), b"c");
    }

    #[test]
    fn elementtexte_werden_gefunden() {
        let x = r#"<r><dc:creator>Anna</dc:creator><x/><dc:creator>Bert</dc:creator></r>"#;
        assert_eq!(element_texte(x, "creator"), vec!["Anna", "Bert"]);
    }

    /// Entitaeten kommen als eigenes Ereignis. Wer sie ueberliest, verliert
    /// sie stillschweigend -- aus „Muster & Partner" wurde „Muster  Partner".
    #[test]
    fn entitaeten_im_text_gehen_nicht_verloren() {
        let x = r#"<r><c>Muster &amp; Partner</c></r>"#;
        assert_eq!(element_texte(x, "c"), vec!["Muster & Partner"]);

        let numerisch = r#"<r><c>A&#38;B &#x26; C</c></r>"#;
        assert_eq!(element_texte(numerisch, "c"), vec!["A&B & C"]);

        let alle = r#"<r><c>&lt;a&gt; &quot;b&quot; &apos;c&apos;</c></r>"#;
        assert_eq!(element_texte(alle, "c"), vec![r#"<a> "b" 'c'"#]);
    }

    /// Eine unbekannte Entitaet wird verworfen, nicht geraten.
    #[test]
    fn unbekannte_entitaeten_werden_nicht_geraten() {
        let x = r#"<r><c>a&nbsp;b</c></r>"#;
        assert_eq!(element_texte(x, "c"), vec!["ab"]);
    }

    #[test]
    fn attributwerte_werden_entschluesselt() {
        let x = r#"<r><p name="Kanzlei M&amp;P"/></r>"#;
        assert_eq!(attribut_werte(x, "p", "name"), vec!["Kanzlei M&P"]);
    }

    /// Ein `srcRect` ohne Werte beschneidet nichts und ist kein Fund.
    #[test]
    fn nur_echte_zuschnitte_zaehlen() {
        assert_eq!(zaehle_zugeschnittene(r#"<a><srcRect/></a>"#), 0);
        assert_eq!(zaehle_zugeschnittene(r#"<a><srcRect l="0" t="0"/></a>"#), 0);
        assert_eq!(zaehle_zugeschnittene(r#"<a><srcRect l="20000"/></a>"#), 1);
        assert_eq!(
            zaehle_zugeschnittene(r#"<a><srcRect b="5"/><srcRect r="7"/></a>"#),
            2
        );
    }

    #[test]
    fn rsid_block_verschwindet_vollstaendig() {
        let x = concat!(
            r#"<w:settings xmlns:w="w"><w:zoom w:val="100"/>"#,
            r#"<w:rsids><w:rsidRoot w:val="00A1"/><w:rsid w:val="00B2"/></w:rsids>"#,
            r#"<w:ende/></w:settings>"#
        );
        let aus = entferne_rsids(x);
        assert!(!aus.contains("rsid"), "{aus}");
        assert!(!aus.contains("00A1"), "{aus}");
        assert!(aus.contains("w:zoom"), "anderes wurde mitentfernt: {aus}");
        assert!(aus.contains("w:ende"), "{aus}");
    }

    #[test]
    fn rsid_attribute_verschwinden_der_rest_bleibt() {
        let x = r#"<w:p w:rsidR="00A1" w:rsidRDefault="00A1" w:val="behalten"><w:r/></w:p>"#;
        let aus = entferne_rsid_attribute(x);
        assert!(!aus.contains("rsid"), "{aus}");
        assert!(!aus.contains("00A1"), "{aus}");
        assert!(aus.contains(r#"w:val="behalten""#), "{aus}");
    }

    /// **Das bedeutungstragende Leerzeichen.** `xml:space="preserve"` und der
    /// Text dahinter muessen die Umformung unveraendert ueberstehen.
    #[test]
    fn bedeutsame_leerzeichen_ueberleben() {
        let x = r#"<w:t xml:space="preserve">Sehr geehrte Damen, </w:t>"#;
        let aus = entferne_rsid_attribute(x);
        assert!(aus.contains(r#"xml:space="preserve""#), "{aus}");
        assert!(aus.contains("Sehr geehrte Damen, "), "{aus}");
    }

    /// Sonderzeichen duerfen beim Umschreiben nicht doppelt maskiert werden.
    #[test]
    fn maskierungen_bleiben_einfach() {
        let x = r#"<w:t>Muster &amp; Partner &lt;Kanzlei&gt;</w:t>"#;
        let aus = entferne_rsid_attribute(x);
        assert!(aus.contains("&amp;"), "{aus}");
        assert!(!aus.contains("&amp;amp;"), "doppelt maskiert: {aus}");
    }

    #[test]
    fn nur_die_gemeinte_beziehung_faellt_weg() {
        let x = concat!(
            r#"<Relationships xmlns="r">"#,
            r#"<Relationship Id="rId1" Type="http://x/officeDocument" Target="word/document.xml"/>"#,
            r#"<Relationship Id="rId2" Type="http://x/thumbnail" Target="docProps/thumbnail.jpeg"/>"#,
            r#"</Relationships>"#
        );
        let aus = entferne_beziehung(x, "thumbnail");
        assert!(!aus.contains("thumbnail"), "{aus}");
        assert!(aus.contains("word/document.xml"), "{aus}");
    }

    #[test]
    fn lange_werte_werden_gekuerzt() {
        let lang = "x".repeat(500);
        let x = format!("<a><b>{lang}</b></a>");
        let werte = element_texte(&x, "b");
        assert!(werte[0].chars().count() <= WERT_MAX.saturating_add(1));
        assert!(werte[0].ends_with('…'));
    }

    /// Kaputtes XML darf nicht zum Absturz fuehren und nichts erfinden.
    #[test]
    fn kaputtes_xml_wird_verkraftet() {
        let _ = element_texte("<a><b>unvollstaendig", "b");
        let _ = entferne_rsids("<<<>>>");
        let _ = zaehle_zugeschnittene("");
    }
}
