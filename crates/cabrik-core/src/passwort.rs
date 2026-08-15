//! Was von einem **neu gewählten** Passwort verlangt wird.
//!
//! # Warum das ein eigener Ort ist
//!
//! Weil die Zahl sonst mehrfach existiert. Die Mindestlänge stand zuerst
//! allein im Einrichtungsbildschirm — der Passwortwechsel kannte sie nicht,
//! die Kommandozeile auch nicht. Dieselbe Entscheidung war an einer Tür
//! bewacht und an dreien nicht.
//!
//! # Warum es zwei Zahlen sind
//!
//! Weil zwei verschiedene Dinge geschützt werden, und der naheliegende
//! Grund für den Unterschied der falsche wäre.
//!
//! **Nicht** weil eine Nachricht weniger wert wäre als ein Schlüssel — im
//! Gegenteil: Der Envelope reist durch Mailserver und Postfächer, die
//! Schlüsseldatei liegt meist auf einem Rechner.
//!
//! Sondern weil beim Envelope ein **kurzer, zufälliger Code** ein
//! legitimes Verfahren ist: über einen zweiten Weg vereinbart, einmal
//! benutzt, danach wertlos. Acht zufällige Zeichen sind bei dieser
//! Ableitung außer Reichweite. Eine hohe Schwelle stünde dieser guten
//! Praxis im Weg, ohne eine schlechte zu verhindern — denn ein zwölf
//! Zeichen langes `sommer2024ab` fällt einer Wortliste in Minuten.
//!
//! # Was eine Längenschwelle nicht ist
//!
//! Eine Aussage über die Güte. Die Länge ist das eine, was sich **wissen**
//! lässt; ob ein Passwort in einer Liste steht, weiß niemand, der die Liste
//! nicht hat. Deshalb gibt es hier eine Untergrenze und keine Anzeige.

use crate::error::{Error, Result};

/// Mindestlänge für das Passwort einer **Schlüsseldatei**, in Zeichen.
///
/// Zwölf ist keine magische Grenze. Es ist die Stelle, ab der ein reines
/// Durchprobieren **aller** Zeichenfolgen bei dieser Passwortableitung
/// aussichtslos wird.
pub const MIN_SCHLUESSEL: usize = 12;

/// Mindestlänge für das Passwort eines **Envelopes**, in Zeichen.
///
/// Niedriger als bei der Schlüsseldatei, und zwar mit Absicht: Hier ist ein
/// kurzer, zufälliger Code ein sinnvolles Verfahren. Siehe die Erläuterung
/// am Anfang dieses Moduls — der Unterschied hat nichts mit dem Wert der
/// Nachricht zu tun.
pub const MIN_NACHRICHT: usize = 8;

/// Zählt die **Zeichen** eines Passworts, nicht die Bytes.
///
/// Sonst wären drei Emoji zwölf Zeichen lang und ein „ä" zählte doppelt.
/// Was sich nicht als UTF-8 lesen lässt, wird byteweise gezählt — dann ist
/// das ohnehin die einzig mögliche Zählung.
#[must_use]
pub fn zeichen(passwort: &[u8]) -> usize {
    core::str::from_utf8(passwort).map_or_else(|_| passwort.len(), |s| s.chars().count())
}

/// Prüft ein **neu gewähltes** Passwort gegen eine Untergrenze.
///
/// **Nur beim Wählen, nie beim Öffnen.** Ein bestehender Schlüssel oder
/// Envelope mit kürzerem Passwort muss weiter aufgehen: Jemanden
/// auszusperren, weil eine Regel dazugekommen ist, wäre der schlimmste
/// Umgang mit einer Verschärfung.
///
/// # Fehler
///
/// [`Error::Malformed`], wenn es zu kurz ist. Die Zahl steht beim
/// Aufrufer — er kennt die Sprache seines Nutzers, diese Schicht nicht.
pub fn pruefe(passwort: &[u8], mindest: usize) -> Result<()> {
    if zeichen(passwort) < mindest {
        return Err(Error::Malformed("password shorter than the minimum"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{MIN_NACHRICHT, MIN_SCHLUESSEL, pruefe, zeichen};

    #[test]
    fn zeichen_werden_gezaehlt_und_nicht_bytes() {
        // Drei Emoji sind zwoelf Bytes. Eine byteweise Zaehlung liesse sie
        // als „lang genug" durch.
        assert_eq!(zeichen("🔑🔒🗝".as_bytes()), 3);
        assert_eq!("🔑🔒🗝".len(), 12, "zwoelf Bytes");
        // Umlaute zaehlen einfach, nicht doppelt.
        assert_eq!(zeichen("äöüäöüäöüäöü".as_bytes()), 12);
    }

    #[test]
    fn was_kein_utf8_ist_wird_byteweise_gezaehlt() {
        assert_eq!(zeichen(&[0xFF, 0xFE, 0xFD]), 3);
    }

    #[test]
    fn genau_die_grenze_geht_durch() {
        // Eine Schwelle, die auch das Erlaubte ablehnt, waere Schikane.
        assert!(pruefe(&b"a".repeat(MIN_SCHLUESSEL), MIN_SCHLUESSEL).is_ok());
        assert!(pruefe(&b"a".repeat(MIN_NACHRICHT), MIN_NACHRICHT).is_ok());
    }

    #[test]
    fn eines_zu_wenig_geht_nicht() {
        assert!(pruefe(&b"a".repeat(MIN_SCHLUESSEL - 1), MIN_SCHLUESSEL).is_err());
        assert!(pruefe(&b"a".repeat(MIN_NACHRICHT - 1), MIN_NACHRICHT).is_err());
    }

    /// Die Nachricht hat die niedrigere Schwelle — **zur Uebersetzungszeit**
    /// festgehalten.
    ///
    /// Nicht weil eine Nachricht weniger wert waere, sondern weil dort ein
    /// kurzer, ZUFAELLIGER Code ein sinnvolles Verfahren ist. Und trotzdem
    /// keine Alibigrenze: acht Zeichen bleiben acht Zeichen.
    ///
    /// Als `const`-Behauptung und nicht als Test: Sie haengt an nichts,
    /// was zur Laufzeit passieren koennte -- wer die Zahlen verdreht,
    /// bekommt keinen roten Test, sondern gar kein Programm.
    const _: () = {
        assert!(MIN_NACHRICHT < MIN_SCHLUESSEL);
        assert!(MIN_NACHRICHT >= 8);
    };
}
