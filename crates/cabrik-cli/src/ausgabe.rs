//! Ausgabe für Menschen und für Maschinen.
//!
//! Jeder Befehl liefert einen [`Bericht`]. Ob daraus Text oder JSON wird,
//! entscheidet allein `--json` — der Befehl selbst weiß davon nichts.
//!
//! Das ist mehr als Bequemlichkeit: Es zwingt dazu, Ergebnisse als **Daten**
//! zu modellieren statt als Sätze. Phase 4 braucht genau diese Datenform für
//! die Brücke nach Tauri; wer erst Text baut, muss sie dort neu erfinden.

use serde_json::{Value, json};

/// Ergebnis eines Befehls, noch ohne Darstellungsform.
pub trait Bericht {
    /// Fassung für Menschen.
    fn text(&self) -> String;
    /// Fassung für Maschinen.
    fn json(&self) -> Value;
}

/// Wohin und in welcher Form geschrieben wird.
#[derive(Debug, Clone, Copy)]
pub struct Schreiber {
    /// Ob JSON statt Text ausgegeben wird.
    pub json: bool,
    /// Ob Hinweise unterdrückt werden.
    pub still: bool,
    /// Ob die Standardausgabe bereits den Nutzdaten gehört.
    ///
    /// Bei `--out -` schreibt der Befehl den entschlüsselten Inhalt dorthin.
    /// Der Bericht muss dann nach `stderr` ausweichen, sonst steht er
    /// mitten in den Daten. `cabrik decrypt x.cab --out - > datei` ergäbe
    /// sonst eine Datei aus Klartext **und** Berichtstext — ein
    /// Datenschaden, der beim Lesen der Ausgabe am Bildschirm nicht auffällt.
    pub stdout_belegt: bool,
}

impl Schreiber {
    /// Verwendung, bei der die Standardausgabe den Nutzdaten gehört.
    #[must_use]
    pub const fn mit_belegtem_stdout(self) -> Self {
        Self {
            stdout_belegt: true,
            ..self
        }
    }

    /// Gibt einen Bericht aus.
    pub fn bericht(self, b: &dyn Bericht) {
        let text = if self.json {
            serde_json::to_string_pretty(&b.json()).unwrap_or_default()
        } else {
            b.text()
        };
        if text.is_empty() {
            return;
        }
        if self.stdout_belegt {
            eprintln!("{text}");
        } else {
            println!("{text}");
        }
    }

    /// Gibt einen Hinweis aus — im JSON-Modus gar nicht, weil er die Ausgabe
    /// unparsbar machen würde.
    pub fn hinweis(self, text: &str) {
        if !self.json && !self.still {
            eprintln!("{text}");
        }
    }

    /// Gibt einen Fehler aus.
    pub fn fehler(self, e: &crate::fehler::Fehler) {
        if self.json {
            let v = json!({
                "ok": false,
                "code": e.code(),
                "meldung": e.to_string(),
            });
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        } else {
            eprintln!("Fehler: {e}");
        }
    }
}

/// Bericht ohne Inhalt außer einer Meldung.
pub struct Meldung {
    /// Text für Menschen.
    pub text: String,
    /// Zusatzfelder für Maschinen.
    pub felder: Value,
}

impl Meldung {
    /// Text mit Zusatzfeldern.
    pub fn mit(text: impl Into<String>, felder: Value) -> Self {
        Self {
            text: text.into(),
            felder,
        }
    }
}

impl Bericht for Meldung {
    fn text(&self) -> String {
        self.text.clone()
    }

    fn json(&self) -> Value {
        let mut v = json!({ "ok": true, "meldung": self.text });
        if let (Some(ziel), Some(quelle)) = (v.as_object_mut(), self.felder.as_object()) {
            for (k, val) in quelle {
                ziel.insert(k.clone(), val.clone());
            }
        }
        v
    }
}

/// Hängt eine Zeile an, wenn der Wert vorhanden ist.
pub fn zeile(aus: &mut String, beschriftung: &str, wert: &str) {
    aus.push_str(beschriftung);
    aus.push_str(": ");
    aus.push_str(wert);
    aus.push('\n');
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
    fn meldung_traegt_zusatzfelder_in_json() {
        let m = Meldung::mit("fertig", json!({ "pfad": "a.cab" }));
        let v = m.json();
        assert_eq!(v["ok"], json!(true));
        assert_eq!(v["pfad"], json!("a.cab"));
        assert_eq!(m.text(), "fertig");
    }

    #[test]
    fn json_eines_fehlers_ist_gueltiges_json() {
        let e = crate::fehler::Fehler::bedienung("so nicht");
        let v = json!({ "ok": false, "code": e.code(), "meldung": e.to_string() });
        let s = serde_json::to_string(&v).unwrap();
        let zurueck: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(zurueck["code"], json!("USAGE"));
    }
}
