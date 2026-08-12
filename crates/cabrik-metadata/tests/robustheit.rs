//! Kein Formatleser darf bei einer verstümmelten Datei abstürzen.
//!
//! # Warum das hier noch wichtiger ist als beim Envelope
//!
//! Der Kern hat **einen** Parser. Dieses Crate hat siebzehn — PNG, JPEG,
//! WebP, GIF, BMP, TIFF, HEIC, SVG, PDF, OOXML, ODF, ZIP, MP4, Matroska,
//! AVI, MP3, FLAC, Ogg, WAV. Jeder davon liest Längen, Versätze und
//! Anzahlen aus einer Datei, die von außen kommt.
//!
//! Der Modulkopf von `lib.rs` sagt es selbst: *„Parser sind Angriffsfläche."*
//! Diese Prüfung ist die Gegenprobe zu diesem Satz.
//!
//! # Der entscheidende Unterschied zu allen anderen Tests
//!
//! Die Formattests füttern jeden Leser mit Dateien, die entweder dieses
//! Projekt oder ffmpeg erzeugt hat — also mit **wohlgeformten** Dateien.
//! Hier bekommt er das Gegenteil: dieselben Dateien, an einer zufälligen
//! Stelle beschädigt. Ein `Err` ist das erwartete Ergebnis. Geprüft wird
//! allein, dass die Funktion zurückkehrt.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

/// Fester Startwert, damit ein Fehlschlag nachstellbar bleibt.
struct Wuerfel(u64);

impl Wuerfel {
    const fn neu(saat: u64) -> Self {
        Self(saat ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn naechste(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn bis(&mut self, grenze: usize) -> usize {
        if grenze == 0 {
            return 0;
        }
        usize::try_from(self.naechste() % grenze as u64).unwrap_or(0)
    }
}

fn verstuemmle(vorlage: &[u8], w: &mut Wuerfel) -> Vec<u8> {
    let mut v = vorlage.to_vec();
    if v.is_empty() {
        return v;
    }
    match w.bis(6) {
        // Ein Bit kippt.
        0 => {
            let i = w.bis(v.len());
            v[i] ^= 1u8 << w.bis(8);
        }
        // Ein Byte wird ersetzt.
        1 => {
            let i = w.bis(v.len());
            v[i] = u8::try_from(w.naechste() & 0xFF).unwrap_or(0);
        }
        // Die Datei bricht ab — der häufigste Schaden in freier Wildbahn.
        2 => {
            let n = w.bis(v.len());
            v.truncate(n);
        }
        // Ein Längenfeld wird riesig. Zielt auf Zuweisungen, die der
        // Angabe vertrauen, und auf Überläufe beim Rechnen mit ihr.
        3 => {
            if v.len() >= 4 {
                let i = w.bis(v.len() - 3);
                v[i..i + 4].copy_from_slice(&u32::MAX.to_be_bytes());
            }
        }
        // Ein Längenfeld wird null. Zielt auf Schleifen, die Fortschritt
        // annehmen — der klassische Weg in eine Endlosschleife.
        4 => {
            if v.len() >= 4 {
                let i = w.bis(v.len() - 3);
                v[i..i + 4].copy_from_slice(&0u32.to_be_bytes());
            }
        }
        // Ein Stück wird verdoppelt.
        _ => {
            let von = w.bis(v.len());
            let bis = (von + 1 + w.bis(128)).min(v.len());
            let stueck = v[von..bis].to_vec();
            v.splice(bis..bis, stueck);
        }
    }
    v
}

fn vorlagen() -> Vec<(String, Vec<u8>)> {
    let ordner = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testvectors/metadata");
    let Ok(eintraege) = std::fs::read_dir(&ordner) else {
        return Vec::new();
    };
    let mut aus = Vec::new();
    for e in eintraege.flatten() {
        let p = e.path();
        // `.stripped` sind Ergebnisse anderer Tests, kein Ausgangsmaterial.
        if !p.is_file() || p.extension().is_some_and(|x| x == "stripped") {
            continue;
        }
        if p.file_name().is_some_and(|n| n == "manifest.json") {
            continue;
        }
        if let Ok(d) = std::fs::read(&p) {
            aus.push((
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                d,
            ));
        }
    }
    aus.sort_by(|a, b| a.0.cmp(&b.0));
    aus
}

/// **Die eigentliche Prüfung.** Jede Vorlage, hundertfach beschädigt, durch
/// beide öffentlichen Eingänge.
#[test]
fn kein_formatleser_stuerzt_an_einer_verstuemmelten_datei_ab() {
    let vorlagen = vorlagen();
    if vorlagen.is_empty() {
        eprintln!("uebersprungen: gen_metadata_fixtures.py wurde nicht ausgefuehrt");
        return;
    }

    let mut durchlaeufe = 0usize;
    for (name, roh) in &vorlagen {
        for saat in 0..100u64 {
            let mut w = Wuerfel::neu(saat);
            let kaputt = verstuemmle(roh, &mut w);

            let ergebnis = std::panic::catch_unwind(|| {
                let _ = cabrik_metadata::inspect(&kaputt);
                let _ = cabrik_metadata::strip(&kaputt);
            });
            assert!(
                ergebnis.is_ok(),
                "Absturz bei {name}, Saat {saat} ({} Bytes)",
                kaputt.len()
            );
            durchlaeufe += 1;
        }
    }
    eprintln!(
        "{durchlaeufe} verstuemmelte Dateien durch {} Vorlagen",
        vorlagen.len()
    );
}

/// Eingaben, die gar kein Format sind — und solche, die nur so tun.
///
/// Die Kennbytes echter Formate mit Unsinn dahinter sind der gemeinste Fall:
/// Die Erkennung greift, der Leser läuft an, und dann stimmt nichts mehr.
#[test]
fn falsche_kennbytes_mit_unsinn_dahinter_werden_nur_abgelehnt() {
    let kennungen: [&[u8]; 14] = [
        b"\x89PNG\r\n\x1a\n",
        b"\xFF\xD8\xFF\xE0",
        b"RIFF\x00\x00\x00\x00WEBPVP8 ",
        b"RIFF\x00\x00\x00\x00AVI LIST",
        b"RIFF\x00\x00\x00\x00WAVEfmt ",
        b"GIF89a",
        b"BM",
        b"II*\x00",
        b"MM\x00*",
        b"%PDF-1.7",
        b"PK\x03\x04",
        b"\x1A\x45\xDF\xA3",
        b"fLaC",
        b"OggS\x00",
    ];

    let mut w = Wuerfel::neu(0xDEAD);
    for (i, kennung) in kennungen.iter().enumerate() {
        for laenge in [0usize, 1, 16, 512, 5000] {
            let mut daten = kennung.to_vec();
            daten.extend((0..laenge).map(|_| u8::try_from(w.naechste() & 0xFF).unwrap_or(0)));

            let ergebnis = std::panic::catch_unwind(|| {
                let _ = cabrik_metadata::inspect(&daten);
                let _ = cabrik_metadata::strip(&daten);
            });
            assert!(
                ergebnis.is_ok(),
                "Absturz bei Kennung {i} mit {laenge} Bytes Unsinn"
            );
        }
    }
}

/// Eine bereinigte Datei muss sich erneut bereinigen lassen, **auch wenn**
/// sie zwischendurch beschädigt wurde. Der zweite Durchlauf ist der, den
/// niemand von Hand ausprobiert.
#[test]
fn auch_nach_dem_bereinigen_bleibt_der_leser_standhaft() {
    let vorlagen = vorlagen();
    if vorlagen.is_empty() {
        return;
    }

    for (name, roh) in vorlagen.iter().take(12) {
        let Ok((sauber, _)) = cabrik_metadata::strip(roh) else {
            continue;
        };
        for saat in 0..40u64 {
            let mut w = Wuerfel::neu(saat ^ 0xBEEF);
            let kaputt = verstuemmle(&sauber, &mut w);
            let ergebnis = std::panic::catch_unwind(|| {
                let _ = cabrik_metadata::strip(&kaputt);
            });
            assert!(
                ergebnis.is_ok(),
                "Absturz bei {name} (bereinigt), Saat {saat}"
            );
        }
    }
}

/// Was das Fuzzing findet, bleibt für immer geprüft.
#[test]
fn korpus_bleibt_beherrschbar() {
    let ordner = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testvectors/fuzz/metadata");
    let Ok(eintraege) = std::fs::read_dir(&ordner) else {
        eprintln!("kein Korpus unter {} -- uebersprungen", ordner.display());
        return;
    };

    let mut geprueft = 0usize;
    for e in eintraege.flatten() {
        let pfad = e.path();
        if !pfad.is_file() {
            continue;
        }
        let Ok(daten) = std::fs::read(&pfad) else {
            continue;
        };
        let ergebnis = std::panic::catch_unwind(|| {
            let _ = cabrik_metadata::inspect(&daten);
            let _ = cabrik_metadata::strip(&daten);
        });
        assert!(
            ergebnis.is_ok(),
            "Absturz bei {} -- ein alter Fehler ist zurueck",
            pfad.display()
        );
        geprueft += 1;
    }
    eprintln!("{geprueft} Korpusdatei(en) geprueft");
}
