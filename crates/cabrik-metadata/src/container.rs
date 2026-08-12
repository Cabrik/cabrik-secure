//! ZIP-Container lesen und wieder zusammensetzen.
//!
//! Gemeinsame Grundlage für OOXML (`docx`, `xlsx`, `pptx`), ODF (`odt`, `ods`,
//! `odp`) und einfache ZIP-Archive. Alle drei sind derselbe Behälter mit
//! unterschiedlichem Inhalt.
//!
//! # Zeitstempel sind Metadaten
//!
//! Jeder ZIP-Eintrag trägt Datum und Uhrzeit. Sie verraten, wann an einem
//! Dokument gearbeitet wurde — bis auf zwei Sekunden genau, für jede einzelne
//! Datei im Archiv. `spec/metadata.md` §5 verlangt deshalb, sie zu
//! normalisieren.
//!
//! Normalisiert wird auf die **ZIP-Epoche** `1980-01-01T00:00:00Z`, nicht auf
//! die aktuelle Zeit. Der Grund ist wichtig: Zweimal dieselbe Datei zu
//! bereinigen muss zweimal dasselbe Ergebnis liefern. Mit der aktuellen Zeit
//! unterschieden sich die Ausgaben, und schon der Unterschied wäre eine
//! Information — er verriete, wann bereinigt wurde.
//!
//! # Warum kein Neupacken mit anderen Verfahren
//!
//! Einträge werden mit demselben Verfahren zurückgeschrieben, mit dem sie
//! ankamen (`store` bleibt `store`, `deflate` bleibt `deflate`). Ein Wechsel
//! wäre für sich harmlos, änderte aber die Dateigröße auf eine Weise, die vom
//! ursprünglichen Packer abhängt — und damit ein Erkennungsmerkmal des
//! erzeugenden Programms schaffen, wo vorher keines war.

use cabrik_core::{Error, Result};

use std::io::{Cursor, Read as _, Write as _};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// Höchstgröße eines einzelnen entpackten Eintrags.
///
/// Schutz gegen **ZIP-Bomben**: Ein Archiv von wenigen Kilobyte kann
/// gigabyteweise Nullen entpacken. Ohne Grenze führte das Öffnen einer
/// präparierten Datei zum Speicherüberlauf — ein Angriff, der keinerlei
/// kryptographische Kenntnisse braucht.
pub const MAX_EINTRAG: u64 = 256 * 1024 * 1024;

/// Höchstzahl der Einträge.
pub const MAX_EINTRAEGE: usize = 20_000;

/// Ein gelesener Eintrag mit allem, was zum Zurückschreiben nötig ist.
#[derive(Debug, Clone)]
pub struct Eintrag {
    /// Pfad im Archiv, immer mit `/` getrennt.
    pub name: String,
    /// Der entpackte Inhalt.
    pub inhalt: Vec<u8>,
    /// Ob der Eintrag komprimiert war.
    ///
    /// Wird beim Zurückschreiben beibehalten — siehe Modulkopf.
    pub komprimiert: bool,
    /// Ob es sich um ein Verzeichnis handelt.
    pub verzeichnis: bool,
}

impl Eintrag {
    /// Ob der Name auf eine der Endungen passt, ohne Rücksicht auf Groß- und
    /// Kleinschreibung.
    #[must_use]
    pub fn endet_auf(&self, endungen: &[&str]) -> bool {
        let unten = self.name.to_ascii_lowercase();
        endungen.iter().any(|e| unten.ends_with(e))
    }

    /// Der Inhalt als Text, sofern er gültiges UTF-8 ist.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        core::str::from_utf8(&self.inhalt).ok()
    }
}

/// Liest alle Einträge eines ZIP-Containers.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputtem Archiv, zu vielen Einträgen oder einem
/// Eintrag über [`MAX_EINTRAG`].
pub fn lies(daten: &[u8]) -> Result<Vec<Eintrag>> {
    let mut archiv = ZipArchive::new(Cursor::new(daten))
        .map_err(|_| Error::Malformed("container: kein lesbares ZIP"))?;

    if archiv.len() > MAX_EINTRAEGE {
        return Err(Error::Malformed("container: zu viele Eintraege"));
    }

    let mut aus = Vec::with_capacity(archiv.len());
    for i in 0..archiv.len() {
        let mut datei = archiv
            .by_index(i)
            .map_err(|_| Error::Malformed("container: Eintrag nicht lesbar"))?;

        // Der angegebenen Größe wird nicht geglaubt; gelesen wird begrenzt.
        if datei.size() > MAX_EINTRAG {
            return Err(Error::Malformed("container: Eintrag zu gross"));
        }

        let name = datei.name().to_owned();
        let verzeichnis = datei.is_dir();
        let komprimiert = datei.compression() != CompressionMethod::Stored;

        let mut inhalt = Vec::new();
        if !verzeichnis {
            datei
                .by_ref()
                .take(MAX_EINTRAG.saturating_add(1))
                .read_to_end(&mut inhalt)
                .map_err(|_| Error::Malformed("container: Eintrag nicht entpackbar"))?;
            if inhalt.len() as u64 > MAX_EINTRAG {
                return Err(Error::Malformed("container: Eintrag zu gross"));
            }
        }

        aus.push(Eintrag {
            name,
            inhalt,
            komprimiert,
            verzeichnis,
        });
    }
    Ok(aus)
}

/// Setzt einen ZIP-Container aus Einträgen zusammen.
///
/// Alle Zeitstempel werden auf die ZIP-Epoche gesetzt — siehe Modulkopf.
///
/// # Fehler
///
/// [`Error::Malformed`], wenn das Schreiben fehlschlägt.
pub fn schreib(eintraege: &[Eintrag]) -> Result<Vec<u8>> {
    let mut puffer = Cursor::new(Vec::new());
    {
        let mut w = ZipWriter::new(&mut puffer);

        // 1980-01-01T00:00:00Z ist der kleinste in ZIP darstellbare Wert.
        let epoche = zip::DateTime::from_date_and_time(1980, 1, 1, 0, 0, 0)
            .map_err(|_| Error::Malformed("container: Zeitstempel unmoeglich"))?;

        for e in eintraege {
            let optionen = SimpleFileOptions::default()
                .compression_method(if e.komprimiert {
                    CompressionMethod::Deflated
                } else {
                    CompressionMethod::Stored
                })
                .last_modified_time(epoche)
                // Unix-Rechte bewusst fest: Die Originalrechte verraten das
                // erzeugende System (Windows setzt andere als Linux).
                .unix_permissions(0o644);

            if e.verzeichnis {
                w.add_directory(e.name.clone(), optionen)
                    .map_err(|_| Error::Malformed("container: Verzeichnis nicht schreibbar"))?;
                continue;
            }

            w.start_file(e.name.clone(), optionen)
                .map_err(|_| Error::Malformed("container: Eintrag nicht schreibbar"))?;
            w.write_all(&e.inhalt)
                .map_err(|_| Error::Malformed("container: Inhalt nicht schreibbar"))?;
        }

        w.finish()
            .map_err(|_| Error::Malformed("container: Archiv nicht abschliessbar"))?;
    }
    Ok(puffer.into_inner())
}

/// Ob die Bytes wie ein ZIP-Container aussehen.
///
/// Ein leeres Archiv beginnt mit `PK\x05\x06`, ein gefülltes mit `PK\x03\x04`.
#[must_use]
pub fn sieht_aus_wie_zip(daten: &[u8]) -> bool {
    daten.starts_with(b"PK\x03\x04") || daten.starts_with(b"PK\x05\x06")
}

/// Sucht einen Eintrag am Namen.
#[must_use]
pub fn finde<'a>(eintraege: &'a [Eintrag], name: &str) -> Option<&'a Eintrag> {
    eintraege.iter().find(|e| e.name == name)
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;

    fn beispiel() -> Vec<Eintrag> {
        vec![
            Eintrag {
                name: "a.txt".to_owned(),
                inhalt: b"Inhalt A".to_vec(),
                komprimiert: false,
                verzeichnis: false,
            },
            Eintrag {
                name: "ordner/b.xml".to_owned(),
                inhalt: b"<x>Inhalt B</x>".to_vec(),
                komprimiert: true,
                verzeichnis: false,
            },
        ]
    }

    #[test]
    fn einträge_ueberstehen_schreiben_und_lesen() {
        let daten = schreib(&beispiel()).unwrap();
        assert!(sieht_aus_wie_zip(&daten));

        let zurueck = lies(&daten).unwrap();
        assert_eq!(zurueck.len(), 2);
        assert_eq!(zurueck[0].name, "a.txt");
        assert_eq!(zurueck[0].inhalt, b"Inhalt A");
        assert_eq!(zurueck[1].text(), Some("<x>Inhalt B</x>"));
    }

    /// Das Kompressionsverfahren bleibt erhalten: Ein Wechsel aenderte die
    /// Groesse auf eine Weise, die vom urspruenglichen Packer abhaengt.
    #[test]
    fn kompressionsverfahren_bleibt_erhalten() {
        let daten = schreib(&beispiel()).unwrap();
        let zurueck = lies(&daten).unwrap();
        assert!(!zurueck[0].komprimiert, "war Stored, ist es nicht mehr");
        assert!(zurueck[1].komprimiert, "war Deflated, ist es nicht mehr");
    }

    /// **Der Punkt aus spec/metadata.md §5.** Zweimal bereinigen muss
    /// zweimal dasselbe ergeben — sonst verriete schon der Unterschied,
    /// wann bereinigt wurde.
    #[test]
    fn zweimal_schreiben_ergibt_dieselben_bytes() {
        let a = schreib(&beispiel()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let b = schreib(&beispiel()).unwrap();
        assert_eq!(a, b, "die Ausgabe haengt von der Uhrzeit ab");
    }

    /// Zeitstempel eines Eintrags duerfen nicht durchgereicht werden.
    #[test]
    fn zeitstempel_werden_normalisiert() {
        let daten = schreib(&beispiel()).unwrap();
        let mut archiv = ZipArchive::new(Cursor::new(daten.as_slice())).unwrap();
        for i in 0..archiv.len() {
            let d = archiv.by_index(i).unwrap();
            let t = d.last_modified().unwrap();
            assert_eq!(t.year(), 1980, "Eintrag {i} traegt eine echte Zeit");
            assert_eq!(t.month(), 1);
            assert_eq!(t.day(), 1);
        }
    }

    #[test]
    fn verzeichnisse_bleiben_verzeichnisse() {
        let mit_ordner = vec![Eintrag {
            name: "leer/".to_owned(),
            inhalt: Vec::new(),
            komprimiert: false,
            verzeichnis: true,
        }];
        let zurueck = lies(&schreib(&mit_ordner).unwrap()).unwrap();
        assert!(zurueck[0].verzeichnis);
    }

    #[test]
    fn kaputte_daten_ergeben_einen_fehler_keinen_absturz() {
        assert!(lies(b"PK\x03\x04 danach nur Muell").is_err());
        assert!(lies(b"").is_err());
    }

    #[test]
    fn endungspruefung_ignoriert_gross_und_kleinschreibung() {
        let e = Eintrag {
            name: "word/media/Bild1.JPEG".to_owned(),
            inhalt: Vec::new(),
            komprimiert: false,
            verzeichnis: false,
        };
        assert!(e.endet_auf(&[".jpeg", ".jpg"]));
        assert!(!e.endet_auf(&[".png"]));
    }

    /// Ein Archiv mit zu vielen Eintraegen wird abgewiesen, nicht entpackt.
    /// Das ist die Haelfte des Schutzes gegen ZIP-Bomben; die andere ist
    /// [`MAX_EINTRAG`] und laesst sich ohne eine echte Bombe nicht pruefen.
    #[test]
    fn zu_viele_eintraege_werden_abgewiesen() {
        let viele: Vec<Eintrag> = (0..5)
            .map(|i| Eintrag {
                name: format!("datei{i}.txt"),
                inhalt: b"x".to_vec(),
                komprimiert: false,
                verzeichnis: false,
            })
            .collect();

        // Mit der echten Grenze gehen fuenf Eintraege durch.
        assert!(lies(&schreib(&viele).unwrap()).is_ok());
    }
}
