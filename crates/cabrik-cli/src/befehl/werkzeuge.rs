//! `metadata` und `shred`.

use super::{lies_eingabe, schreib_ausgabe};
use crate::ausgabe::{Bericht, zeile};
use crate::fehler::{Ergebnis, Fehler};
use crate::{Global, MetadataBefehl, ShredArgs};

use cabrik_metadata::model::{Finding, FindingKind, Inspection, Severity, StripResult};
use cabrik_shred::{DirOutcome, ShredOptions, ShredOutcome};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// metadata
// ---------------------------------------------------------------------------

const fn schwere_text(s: Severity) -> &'static str {
    match s {
        Severity::Minor => "gering",
        Severity::Notable => "beachtlich",
        Severity::Critical => "kritisch",
    }
}

const fn schwere_kode(s: Severity) -> &'static str {
    match s {
        Severity::Minor => "minor",
        Severity::Notable => "notable",
        Severity::Critical => "critical",
    }
}

fn art_text(k: FindingKind) -> &'static str {
    match k {
        FindingKind::Gps => "Ortsangabe",
        FindingKind::Author => "Personenname",
        FindingKind::Device => "Gerät oder Seriennummer",
        FindingKind::Software => "erzeugende Software",
        FindingKind::Timestamp => "Zeitangabe",
        FindingKind::Organization => "Firmen- oder Organisationsname",
        FindingKind::EmbeddedPreview => "eingebettetes Vorschaubild (zweite Kopie des Inhalts)",
        FindingKind::ColorProfile => "Farbprofil",
        FindingKind::Comment => "Kommentar",
        FindingKind::UnknownExtension => "unbekannte Erweiterung",
        _ => "sonstiges",
    }
}

fn fund_json(f: &Finding) -> Value {
    json!({
        "art": art_text(f.kind),
        "ort": f.location,
        "wert": f.value,
        "schwere": schwere_kode(f.severity),
    })
}

fn fund_zeile(f: &Finding) -> String {
    // Wiederholt der Wert nur die Art („Ortsangabe in EXIF:GPSInfo —
    // Ortsangabe"), bleibt er weg. Doppelte Wörter lassen den Leser suchen,
    // was der Unterschied sein soll.
    let wert = match &f.value {
        Some(v) if v != art_text(f.kind) => format!(" — {v}"),
        _ => String::new(),
    };
    format!(
        "  [{}] {} in {}{}\n",
        schwere_text(f.severity),
        art_text(f.kind),
        f.location,
        wert
    )
}

/// Nach Schwere absteigend — das Kritische zuerst, denn die Liste kann lang
/// werden und der Blick bleibt oben hängen.
fn nach_schwere(funde: &[Finding]) -> Vec<&Finding> {
    let mut sortiert: Vec<&Finding> = funde.iter().collect();
    sortiert.sort_by_key(|f| core::cmp::Reverse(f.severity));
    sortiert
}

struct InspectBericht {
    inspektion: Inspection,
}

impl Bericht for InspectBericht {
    fn text(&self) -> String {
        let mut s = String::new();
        zeile(
            &mut s,
            "Format",
            self.inspektion.format.as_deref().unwrap_or("unbekannt"),
        );

        if !self.inspektion.understood {
            s.push_str(
                "\nDieses Format wird nicht verstanden. Dass keine Funde aufgelistet\n\
                 sind, heißt **nicht**, dass die Datei sauber ist — es heißt, dass\n\
                 hier nichts geprüft werden konnte.",
            );
            return s;
        }

        if self.inspektion.findings.is_empty() {
            s.push_str(
                "\nKeine bekannten Metadatenträger gefunden.\n\
                 Das ist keine Garantie auf Metadatenfreiheit, sondern die Aussage:\n\
                 alles, was dieses Programm für dieses Format kennt, ist leer.",
            );
            return s;
        }

        s.push_str(&format!("\n{} Funde:\n", self.inspektion.findings.len()));
        for f in nach_schwere(&self.inspektion.findings) {
            s.push_str(&fund_zeile(f));
        }
        s
    }

    fn json(&self) -> Value {
        json!({
            "ok": true,
            "format": self.inspektion.format,
            "verstanden": self.inspektion.understood,
            "funde": self.inspektion.findings.iter().map(fund_json).collect::<Vec<_>>(),
        })
    }
}

struct StripBericht {
    pfad: String,
    ergebnis: StripResult,
}

impl Bericht for StripBericht {
    fn text(&self) -> String {
        let mut s = String::new();
        zeile(&mut s, "Geschrieben", &self.pfad);
        zeile(&mut s, "Ergebnis", &self.ergebnis.to_string());

        match &self.ergebnis {
            StripResult::Complete { removed } => {
                if !removed.is_empty() {
                    s.push_str("\nEntfernt:\n");
                    for f in nach_schwere(removed) {
                        s.push_str(&fund_zeile(f));
                    }
                }
                s.push_str(
                    "\n„Vollständig\" heißt: alle Metadatenträger, die dieses Programm\n\
                     für dieses Format kennt. Es heißt nicht „garantiert metadatenfrei\".",
                );
            }
            StripResult::Partial {
                removed,
                remaining,
                reason,
            } => {
                if !removed.is_empty() {
                    s.push_str("\nEntfernt:\n");
                    for f in nach_schwere(removed) {
                        s.push_str(&fund_zeile(f));
                    }
                }
                s.push_str("\nGeblieben:\n");
                for f in nach_schwere(remaining) {
                    s.push_str(&fund_zeile(f));
                }
                zeile(&mut s, "Grund", reason);
            }
            StripResult::Unknown { .. } => {
                s.push_str(
                    "\nDas Format wurde nicht verstanden. Die Datei wurde unverändert\n\
                     übernommen — es wurde nichts entfernt, und über ihre Sauberkeit\n\
                     lässt sich nichts sagen.",
                );
            }
        }
        s
    }

    fn json(&self) -> Value {
        let (art, entfernt, geblieben, grund) = match &self.ergebnis {
            StripResult::Complete { removed } => ("complete", removed.clone(), Vec::new(), None),
            StripResult::Partial {
                removed,
                remaining,
                reason,
            } => (
                "partial",
                removed.clone(),
                remaining.clone(),
                Some(reason.clone()),
            ),
            StripResult::Unknown { .. } => ("unknown", Vec::new(), Vec::new(), None),
        };
        json!({
            "ok": true,
            "pfad": self.pfad,
            "ergebnis": art,
            "meldung": self.ergebnis.to_string(),
            "entfernt": entfernt.iter().map(fund_json).collect::<Vec<_>>(),
            "geblieben": geblieben.iter().map(fund_json).collect::<Vec<_>>(),
            "grund": grund,
        })
    }
}

/// Führt einen `metadata`-Unterbefehl aus.
///
/// # Fehler
///
/// Datei- oder Formatfehler.
pub fn metadata(g: &Global, b: &MetadataBefehl) -> Ergebnis<()> {
    let schreiber = g.schreiber();
    match b {
        MetadataBefehl::Inspect { datei } => {
            let daten = lies_eingabe(datei)?;
            schreiber.bericht(&InspectBericht {
                inspektion: cabrik_metadata::inspect(&daten)?,
            });
        }
        MetadataBefehl::Strip { datei, out } => {
            let daten = lies_eingabe(datei)?;
            let (sauber, ergebnis) = cabrik_metadata::strip(&daten)?;
            schreib_ausgabe(out, &sauber)?;
            schreiber.bericht(&StripBericht {
                pfad: out.display().to_string(),
                ergebnis,
            });
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// shred
// ---------------------------------------------------------------------------

struct ShredBericht {
    ergebnisse: Vec<ShredOutcome>,
}

impl Bericht for ShredBericht {
    fn text(&self) -> String {
        let mut s = String::new();
        for e in &self.ergebnisse {
            s.push_str(&format!("{}: {}\n", e.path.display(), e.message()));
            for w in &e.warnings {
                s.push_str("  ! ");
                s.push_str(&w.message());
                s.push('\n');
            }
        }
        s
    }

    fn json(&self) -> Value {
        json!({
            "ok": self.ergebnisse.iter().all(ShredOutcome::is_success),
            "dateien": self.ergebnisse.iter().map(|e| json!({
                "pfad": e.path.display().to_string(),
                "erfolg": e.is_success(),
                "ueberschrieben": e.overwritten,
                "umbenannt": e.renamed,
                "entfernt": e.removed,
                "meldung": e.message(),
                "warnungen": e.warnings.iter().map(|w| w.message()).collect::<Vec<_>>(),
                "fehler": e.error,
            })).collect::<Vec<_>>(),
        })
    }
}

struct DirBericht {
    ergebnis: DirOutcome,
}

impl Bericht for DirBericht {
    fn text(&self) -> String {
        let mut s = String::new();
        zeile(
            &mut s,
            "Gelöscht",
            &format!("{} Dateien", self.ergebnis.succeeded()),
        );
        if self.ergebnis.failed() > 0 {
            zeile(
                &mut s,
                "Fehlgeschlagen",
                &format!("{} Dateien", self.ergebnis.failed()),
            );
        }
        if self.ergebnis.links_skipped > 0 {
            zeile(
                &mut s,
                "Verknüpfungen übersprungen",
                &format!(
                    "{} — sie wurden nicht verfolgt, sonst wäre außerhalb gelöscht worden",
                    self.ergebnis.links_skipped
                ),
            );
        }
        zeile(
            &mut s,
            "Verzeichnis entfernt",
            if self.ergebnis.removed { "ja" } else { "nein" },
        );
        for w in self.ergebnis.warnings() {
            s.push_str("  ! ");
            s.push_str(&w.message());
            s.push('\n');
        }
        for f in &self.ergebnis.errors {
            s.push_str("  Fehler: ");
            s.push_str(f);
            s.push('\n');
        }
        s
    }

    fn json(&self) -> Value {
        json!({
            "ok": self.ergebnis.is_success(),
            "gelöscht": self.ergebnis.succeeded(),
            "fehlgeschlagen": self.ergebnis.failed(),
            "verknuepfungen_uebersprungen": self.ergebnis.links_skipped,
            "verzeichnis_entfernt": self.ergebnis.removed,
            "warnungen": self.ergebnis.warnings().iter().map(|w| w.message()).collect::<Vec<_>>(),
            "fehler": self.ergebnis.errors,
        })
    }
}

/// Löscht Dateien oder ein Verzeichnis.
///
/// # Fehler
///
/// Bedienfehler oder verweigerte Verzeichnisse.
pub fn shred(g: &Global, a: &ShredArgs) -> Ergebnis<()> {
    let schreiber = g.schreiber();
    let opts = ShredOptions {
        passes: a.passes,
        rename: !a.keep_name,
    };

    if let Some(dir) = &a.dir {
        if !a.pfade.is_empty() {
            return Err(Fehler::bedienung(
                "--dir und einzelne Pfade zugleich sind zu leicht zu verwechseln",
            ));
        }

        let vorschau = cabrik_shred::preview(dir).map_err(|r| Fehler::bedienung(r.message()))?;

        let Some(bestaetigung) = &a.confirm else {
            // Ohne Bestätigung wird nichts gelöscht, nur gezeigt.
            return zeige_vorschau(g, &vorschau);
        };

        let ergebnis = cabrik_shred::shred_dir(dir, bestaetigung, &opts)
            .map_err(|r| Fehler::bedienung(r.message()))?;
        schreiber.bericht(&DirBericht { ergebnis });
        return Ok(());
    }

    if a.pfade.is_empty() {
        return Err(Fehler::bedienung(
            "Keine Datei angegeben. Für ein ganzes Verzeichnis: --dir",
        ));
    }

    let ergebnisse: Vec<ShredOutcome> = a
        .pfade
        .iter()
        .map(|p| cabrik_shred::shred_file(p, &opts))
        .collect();

    let alle_gut = ergebnisse.iter().all(ShredOutcome::is_success);
    schreiber.bericht(&ShredBericht { ergebnisse });

    if !alle_gut {
        return Err(Fehler::bedienung(
            "Nicht alle Dateien wurden gelöscht — siehe oben",
        ));
    }
    Ok(())
}

struct VorschauBericht {
    vorschau: cabrik_shred::Preview,
}

impl Bericht for VorschauBericht {
    fn text(&self) -> String {
        let mut s = String::new();
        zeile(
            &mut s,
            "Verzeichnis",
            &self.vorschau.path.display().to_string(),
        );
        zeile(&mut s, "Dateien", &self.vorschau.file_count.to_string());
        zeile(
            &mut s,
            "Unterverzeichnisse",
            &self.vorschau.dir_count.to_string(),
        );
        zeile(
            &mut s,
            "Gesamtgröße",
            &format!("{} Bytes", self.vorschau.total_bytes),
        );
        if self.vorschau.links_skipped > 0 {
            zeile(
                &mut s,
                "Verknüpfungen",
                &format!("{} — werden nicht verfolgt", self.vorschau.links_skipped),
            );
        }
        s.push_str(&format!(
            "\nEs wurde nichts gelöscht. Der Vorgang ist unumkehrbar; zum\n\
             Ausführen muss der Verzeichnisname eingetippt werden:\n\n  \
             cabrik shred --dir \"{}\" --confirm \"{}\"",
            self.vorschau.path.display(),
            self.vorschau.confirmation_word
        ));
        s
    }

    fn json(&self) -> Value {
        json!({
            "ok": true,
            "ausgefuehrt": false,
            "verzeichnis": self.vorschau.path.display().to_string(),
            "dateien": self.vorschau.file_count,
            "unterverzeichnisse": self.vorschau.dir_count,
            "bytes": self.vorschau.total_bytes,
            "verknuepfungen": self.vorschau.links_skipped,
            "bestaetigungswort": self.vorschau.confirmation_word,
        })
    }
}

fn zeige_vorschau(g: &Global, vorschau: &cabrik_shred::Preview) -> Ergebnis<()> {
    g.schreiber().bericht(&VorschauBericht {
        vorschau: vorschau.clone(),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Die Kodes im JSON sind die Schnittstelle fuer Phase 3 und muessen
    /// stabil bleiben — uebersetzte Texte taugen dafuer nicht.
    #[test]
    fn schwerekodes_sind_stabil() {
        assert_eq!(schwere_kode(Severity::Minor), "minor");
        assert_eq!(schwere_kode(Severity::Notable), "notable");
        assert_eq!(schwere_kode(Severity::Critical), "critical");
    }

    /// Ein eingebettetes Vorschaubild ist keine Metadatenart im engeren
    /// Sinn, sondern eine zweite Kopie des Inhalts. Der Text muss das sagen.
    #[test]
    fn vorschaubild_wird_als_zweite_kopie_benannt() {
        let t = art_text(FindingKind::EmbeddedPreview);
        assert!(t.contains("Kopie"), "„{t}\" erklaert die Gefahr nicht");
    }
}
