//! Kanonische JSON-Serialisierung, wie Pythons `json.dumps` sie erzeugt.
//!
//! v1 bildet die AEAD-AAD eines Envelopes mit
//!
//! ```python
//! json.dumps(header, separators=(",", ":"), sort_keys=True)
//! ```
//!
//! Den Envelope selbst serialisiert es dagegen **ohne** `sort_keys`. Die
//! Schlüsselreihenfolge in der Datei ist also eine andere als in der AAD —
//! der Leser kann die AAD daher **nicht** als Teilzeichenkette aus der Datei
//! übernehmen, sondern muss sie neu bilden.
//!
//! Genau diese Funktion tut das. Sie muss bitgenau mit CPython
//! übereinstimmen; jede Abweichung führt zu `AUTH_FAILED` beim Öffnen eines
//! sonst gültigen v1-Envelopes.
//!
//! # Die Regeln im Einzelnen
//!
//! | Aspekt | Verhalten |
//! |---|---|
//! | Trennzeichen | `,` und `:` ohne Leerzeichen |
//! | Schlüssel | aufsteigend nach Unicode-Codepoint, **rekursiv** |
//! | Nicht-ASCII | `\uXXXX` mit **kleinen** Hexziffern (`ensure_ascii=True`) |
//! | Steuerzeichen | `\b \f \n \r \t`, sonst `\u00XX` |
//! | `/` | wird **nicht** maskiert |
//! | Ganzzahlen | dezimal ohne Vorzeichenwechsel |

use serde_json::Value;

/// Serialisiert `value` so, wie CPythons `json.dumps` mit
/// `separators=(",", ":")`, `sort_keys=True` und `ensure_ascii=True`.
#[must_use]
pub fn dumps(value: &Value) -> String {
    let mut out = String::new();
    write_value(value, &mut out);
    out
}

fn write_value(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => out.push_str(&n.to_string()),
        Value::String(s) => write_string(s, out),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            // `sort_keys=True` wirkt rekursiv. serde_json nutzt ohne das
            // Feature `preserve_order` bereits eine BTreeMap und ist damit
            // sortiert — hier wird es trotzdem ausdrücklich gemacht, damit
            // die Zusicherung nicht an einer Fremdeinstellung hängt.
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(key, out);
                out.push(':');
                if let Some(v) = map.get(*key) {
                    write_value(v, out);
                }
            }
            out.push('}');
        }
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => push_escape(c as u32, out),
            c if (c as u32) < 0x7F => out.push(c),
            c => {
                let cp = c as u32;
                if cp > 0xFFFF {
                    // Ersatzzeichenpaar — Python maskiert Zeichen außerhalb
                    // der BMP als zwei \uXXXX-Folgen.
                    let v = cp.saturating_sub(0x1_0000);
                    push_escape(0xD800_u32.saturating_add(v >> 10), out);
                    push_escape(0xDC00_u32.saturating_add(v & 0x3FF), out);
                } else {
                    push_escape(cp, out);
                }
            }
        }
    }
    out.push('"');
}

fn push_escape(cp: u32, out: &mut String) {
    // Kleine Hexziffern, vierstellig — genau wie CPython.
    const HEX: &[u8; 16] = b"0123456789abcdef";
    out.push_str("\\u");
    for shift in [12_u32, 8, 4, 0] {
        let nibble = ((cp >> shift) & 0xF) as usize;
        if let Some(&c) = HEX.get(nibble) {
            out.push(char::from(c));
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Fehlschlag soll den Test abbrechen")]
mod tests {
    use super::*;

    fn j(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn keine_leerzeichen() {
        assert_eq!(dumps(&j(r#"{"a": 1, "b": 2}"#)), r#"{"a":1,"b":2}"#);
        assert_eq!(dumps(&j(r#"[1, 2, 3]"#)), "[1,2,3]");
    }

    #[test]
    fn schluessel_werden_sortiert() {
        assert_eq!(
            dumps(&j(r#"{"z":1,"a":2,"m":3}"#)),
            r#"{"a":2,"m":3,"z":1}"#
        );
    }

    #[test]
    fn sortierung_wirkt_rekursiv() {
        assert_eq!(
            dumps(&j(r#"{"b":{"z":1,"a":2},"a":3}"#)),
            r#"{"a":3,"b":{"a":2,"z":1}}"#
        );
    }

    #[test]
    fn nicht_ascii_wird_maskiert() {
        // ensure_ascii=True, kleine Hexziffern. Das Ergebnis ist reines
        // ASCII -- die Umlaute erscheinen NICHT roh.
        assert_eq!(dumps(&j(r#""Grün""#)), r#""Gr\u00fcn""#);
        assert!(dumps(&j(r#""Grün""#)).is_ascii());
        assert_eq!(dumps(&j(r#""Ü""#)), r#""\u00dc""#);
    }

    #[test]
    fn steuerzeichen_und_sonderfaelle() {
        assert_eq!(dumps(&Value::String("a\nb".into())), r#""a\nb""#);
        assert_eq!(dumps(&Value::String("a\tb".into())), r#""a\tb""#);
        assert_eq!(dumps(&Value::String("a\u{1}b".into())), r#""a\u0001b""#);
        assert_eq!(dumps(&Value::String("a\u{8}b".into())), r#""a\bb""#);
        assert_eq!(dumps(&Value::String("a\"b".into())), r#""a\"b""#);
        assert_eq!(dumps(&Value::String("a\\b".into())), r#""a\\b""#);
        // Python maskiert den Schrägstrich NICHT.
        assert_eq!(dumps(&Value::String("a/b".into())), r#""a/b""#);
    }

    #[test]
    fn ausserhalb_der_bmp_ergibt_ersatzzeichenpaar() {
        // U+1F510 -> 0xD83D 0xDD10
        assert_eq!(
            dumps(&Value::String("\u{1F510}".into())),
            r#""\ud83d\udd10""#
        );
    }

    #[test]
    fn null_und_wahrheitswerte() {
        assert_eq!(
            dumps(&j(r#"{"a":null,"b":true,"c":false}"#)),
            r#"{"a":null,"b":true,"c":false}"#
        );
    }

    #[test]
    fn ganzzahlen() {
        assert_eq!(dumps(&j(r#"{"ts":1754960343}"#)), r#"{"ts":1754960343}"#);
    }
}
