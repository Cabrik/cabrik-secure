//! `encrypt`, `decrypt`, `inspect`.

use super::{jetzt, lies_eingabe, schreib_ausgabe};
use crate::ablage;
use crate::ausgabe::{Bericht, zeile};
use crate::befehl::schluessel::lade_identitaet;
use crate::fehler::{Ergebnis, Fehler};
use crate::geheimnis;
use crate::{DecryptArgs, EncryptArgs, Global, InspectArgs, SuiteWahl};

use cabrik_core::envelope::{self, ContentType, Opener, SealOptions};
use cabrik_core::trust::{Authenticity, Contact, TrustState, TrustStore};
use cabrik_core::{Identity, OsRandom, Suite, trust};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Empfänger auflösen
// ---------------------------------------------------------------------------

/// Ein aufgelöster Empfänger mit allem, was für die Suite-Wahl nötig ist.
struct Empfaenger {
    name: String,
    enc_pub: [u8; 32],
    xwing_pub: Option<Box<[u8; 1216]>>,
    zustand: Option<TrustState>,
}

impl Empfaenger {
    const fn kann_post_quantum(&self) -> bool {
        self.xwing_pub.is_some()
    }
}

/// Löst `--to` gegen den Kontaktspeicher und `--to-key` gegen Nutzlasten auf.
fn loese_empfaenger(
    namen: &[String],
    nutzlasten: &[String],
    store: Option<&TrustStore>,
) -> Ergebnis<Vec<Empfaenger>> {
    let mut aus = Vec::new();

    for name in namen {
        let store = store.ok_or_else(|| {
            Fehler::bedienung(
                "--to braucht den Kontaktspeicher und damit ein Keyfile.\n\
                 Ohne Schlüssel geht nur --to-key mit einer Austausch-Nutzlast.",
            )
        })?;
        let k = finde_kontakt(store, name)?;
        aus.push(Empfaenger {
            name: k.name.clone(),
            enc_pub: k.enc_pub,
            xwing_pub: k.xwing_pub.clone(),
            zustand: Some(k.state),
        });
    }

    for (i, nutzlast) in nutzlasten.iter().enumerate() {
        // Dieselbe Beschaffung wie bei `contacts add`: Pfad oder Nutzlast,
        // mit einer Meldung, die zum tatsächlichen Fall passt.
        let roh = crate::befehl::kontakte::hole_nutzlast(nutzlast)?;
        let gelesen =
            trust::parse_qr(roh.trim()).map_err(crate::befehl::kontakte::nutzlast_fehler)?;
        aus.push(Empfaenger {
            name: format!("--to-key #{}", i.saturating_add(1)),
            enc_pub: gelesen.enc_pub,
            xwing_pub: gelesen.xwing_pub,
            zustand: None,
        });
    }

    Ok(aus)
}

/// Sucht einen Kontakt am Namen, ohne Rücksicht auf Groß- und Kleinschreibung.
fn finde_kontakt<'a>(store: &'a TrustStore, name: &str) -> Ergebnis<&'a Contact> {
    let treffer: Vec<&Contact> = store
        .contacts()
        .iter()
        .filter(|k| k.name.eq_ignore_ascii_case(name))
        .collect();

    match treffer.as_slice() {
        [k] => Ok(k),
        [] => Err(Fehler::bedienung(format!(
            "Kein Kontakt namens „{name}\". Vorhanden: {}",
            liste_namen(store)
        ))),
        _ => Err(Fehler::bedienung(format!(
            "„{name}\" ist mehrdeutig — es gibt mehrere Kontakte mit diesem Namen"
        ))),
    }
}

fn liste_namen(store: &TrustStore) -> String {
    if store.is_empty() {
        return "keine".to_owned();
    }
    store
        .contacts()
        .iter()
        .map(|k| k.name.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Wählt die Suite.
///
/// `Auto` nimmt Post-Quantum, sobald **alle** Empfänger es können. Ein
/// Envelope hat genau eine Suite; ein einziger Empfänger ohne X-Wing-Schlüssel
/// zwingt also alle auf den klassischen Pfad. Das wird gesagt, nicht
/// verschwiegen.
fn waehle_suite(wahl: SuiteWahl, empfaenger: &[Empfaenger]) -> Ergebnis<(Suite, Option<String>)> {
    let ohne: Vec<&str> = empfaenger
        .iter()
        .filter(|e| !e.kann_post_quantum())
        .map(|e| e.name.as_str())
        .collect();

    match wahl {
        SuiteWahl::Classical => Ok((Suite::Classical, None)),
        SuiteWahl::Hybrid => {
            if ohne.is_empty() {
                Ok((Suite::Hybrid, None))
            } else {
                Err(Fehler::bedienung(format!(
                    "--suite hybrid geht nicht: {} hat/haben keinen Post-Quantum-Schlüssel.\n\
                     Diese Kontakte stammen aus Version 1 oder aus einer verkürzten Nutzlast.\n\
                     Entweder neu austauschen oder --suite classical wählen.",
                    ohne.join(", ")
                )))
            }
        }
        SuiteWahl::Auto => {
            if empfaenger.is_empty() || ohne.is_empty() {
                Ok((Suite::Hybrid, None))
            } else {
                Ok((
                    Suite::Classical,
                    Some(format!(
                        "Klassisches Verfahren gewählt, weil {} keinen Post-Quantum-Schlüssel hat.",
                        ohne.join(", ")
                    )),
                ))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// encrypt
// ---------------------------------------------------------------------------

struct EncryptBericht {
    pfad: String,
    suite: &'static str,
    empfaenger: Vec<String>,
    passwort: bool,
    signiert: bool,
    gepolstert: bool,
    attrappen: bool,
    bytes: usize,
    metadaten: Option<String>,
}

impl Bericht for EncryptBericht {
    fn text(&self) -> String {
        let mut s = String::new();
        zeile(&mut s, "Geschrieben", &self.pfad);
        zeile(&mut s, "Verfahren", self.suite);
        zeile(
            &mut s,
            "Empfänger",
            &if self.empfaenger.is_empty() {
                "nur Passwort".to_owned()
            } else {
                self.empfaenger.join(", ")
            },
        );
        if self.passwort && !self.empfaenger.is_empty() {
            zeile(&mut s, "Zusätzlich", "mit Passwort öffenbar");
        }
        zeile(
            &mut s,
            "Signatur",
            if self.signiert {
                "ja"
            } else {
                "nein — der Empfänger kann den Absender nicht feststellen"
            },
        );
        zeile(
            &mut s,
            "Länge verschleiert",
            if self.gepolstert { "ja" } else { "nein" },
        );
        if self.attrappen {
            zeile(&mut s, "Empfängerzahl", "durch Attrappen verschleiert");
        }
        zeile(&mut s, "Größe", &format!("{} Bytes", self.bytes));
        if let Some(m) = &self.metadaten {
            zeile(&mut s, "Metadaten", m);
        }
        s
    }

    fn json(&self) -> Value {
        json!({
            "ok": true,
            "pfad": self.pfad,
            "suite": self.suite,
            "empfaenger": self.empfaenger,
            "passwort": self.passwort,
            "signiert": self.signiert,
            "gepolstert": self.gepolstert,
            "attrappen": self.attrappen,
            "bytes": self.bytes,
            "metadaten": self.metadaten,
        })
    }
}

/// Verschlüsselt eine Datei.
///
/// # Fehler
///
/// Bedien-, Datei- oder Kryptofehler.
pub fn encrypt(g: &Global, a: &EncryptArgs) -> Ergebnis<()> {
    let schreiber = g.schreiber();

    if a.an.is_empty() && a.an_schluessel.is_empty() && !a.password {
        return Err(Fehler::bedienung(
            "Kein Empfänger. Mindestens eines von --to, --to-key oder --password angeben.",
        ));
    }

    // Ein Keyfile wird nur geladen, wenn es wirklich gebraucht wird: für die
    // Kontaktauflösung oder zum Signieren. Sonst bleibt die Passwortabfrage
    // aus, die niemand versteht, der nur an einen fremden Schlüssel schreibt.
    let braucht_identitaet = !a.an.is_empty() || a.sign;

    // Hier werden zwei **verschiedene** Passwörter gebraucht: eines für das
    // Keyfile, eines für den Envelope. Aus einer Datei käme zweimal dasselbe,
    // aus der Standardeingabe beim zweiten Mal gar nichts. Beides wäre still
    // falsch — das Envelope-Passwort wäre dann das Keyfile-Passwort, und wer
    // die Datei weitergibt, gäbe sein Schlüsselpasswort mit.
    if braucht_identitaet && a.password && g.passwortquelle()?.ist_automatisch() {
        return Err(Fehler::bedienung(
            "Hier werden zwei verschiedene Passwörter gebraucht: eines für den\n\
             Schlüssel, eines für den Envelope. Aus --password-file oder\n\
             --password-stdin lässt sich nur eines lesen.\n\n\
             Entweder beide abfragen lassen (ohne die Schalter) oder den Envelope\n\
             ohne Signatur und ohne Kontaktauflösung erzeugen (--to-key statt --to).",
        ));
    }
    let identity = if braucht_identitaet {
        Some(lade_identitaet(g)?)
    } else {
        None
    };

    let store = match &identity {
        Some(id) => Some(ablage::lies_kontakte(
            &ablage::kontakte_pfad(g.contacts.as_deref())?,
            id,
        )?),
        None => None,
    };

    let empfaenger = loese_empfaenger(&a.an, &a.an_schluessel, store.as_ref())?;
    pruefe_vertrauenszustaende(&empfaenger, schreiber)?;

    let (suite, hinweis) = waehle_suite(a.suite, &empfaenger)?;
    if let Some(h) = hinweis {
        schreiber.hinweis(&h);
    }

    // Für Suite 0x0002 ist der Empfängerschlüssel der X-Wing-Schlüssel, für
    // 0x0001 der X25519-Schlüssel. Die Längen prüft der Kern.
    let schluessel: Vec<&[u8]> = empfaenger
        .iter()
        .map(|e| match suite {
            Suite::Hybrid => e.xwing_pub.as_deref().map_or(&e.enc_pub[..], |k| &k[..]),
            _ => &e.enc_pub[..],
        })
        .collect();

    let passwort = if a.password {
        let quelle = g.passwortquelle()?;
        schreiber.hinweis(
            "Passwort für den Envelope. Wer es kennt, kann die Datei öffnen —\n\
             es ist so stark wie das Passwort selbst, nicht wie der Schlüssel.",
        );
        Some(geheimnis::lies_neu(&quelle, "Envelope-Passwort")?)
    } else {
        None
    };

    // --- Nutzdaten -------------------------------------------------------
    let roh = lies_eingabe(&a.datei)?;
    let (nutzdaten, metadaten_meldung) = if a.strip_metadata {
        let (sauber, ergebnis) = cabrik_metadata::strip(&roh)?;
        (sauber, Some(ergebnis.to_string()))
    } else {
        (roh, None)
    };

    let ist_stdin = a.datei.as_os_str() == "-";
    let dateiname = if ist_stdin {
        None
    } else {
        a.datei.file_name().and_then(|n| n.to_str())
    };
    let content_type = if ist_stdin {
        ContentType::Text
    } else {
        ContentType::File
    };

    let padding = match (a.pad, a.no_pad) {
        (true, _) => Some(true),
        (_, true) => Some(false),
        _ => None,
    };

    let opts = SealOptions {
        content_type,
        filename: dateiname,
        timestamp: if a.timestamp { Some(jetzt()) } else { None },
        padding,
        dummy_stanzas: a.dummies,
    };

    let signierer = if a.anonymous { None } else { identity.as_ref() };
    let signiert = signierer.is_some_and(Identity::can_sign);

    let envelope = envelope::seal(
        suite,
        &schluessel,
        passwort.as_ref().map(|p| p.as_slice()),
        &nutzdaten,
        signierer,
        &opts,
        &mut OsRandom,
    )?;

    let ziel = a.out.clone().unwrap_or_else(|| ausgabename(&a.datei));
    schreib_ausgabe(&ziel, &envelope)?;

    schreiber.bericht(&EncryptBericht {
        pfad: ziel.display().to_string(),
        // Ohne Empfänger ist keine Kapsel im Envelope und die Suite damit
        // ohne Wirkung. „Post-Quantum" zu melden wäre schlicht falsch: Der
        // Schutz kommt hier allein aus Argon2id und dem Passwort.
        suite: if empfaenger.is_empty() {
            "Passwort (Argon2id + ChaCha20-Poly1305) — kein Schlüsselaustausch beteiligt"
        } else {
            suite_name(suite)
        },
        empfaenger: empfaenger.iter().map(|e| e.name.clone()).collect(),
        passwort: a.password,
        signiert,
        gepolstert: padding.unwrap_or(content_type == ContentType::Text),
        attrappen: a.dummies,
        bytes: envelope.len(),
        metadaten: metadaten_meldung,
    });
    Ok(())
}

/// Bricht ab bei widerrufenen Kontakten und warnt bei geänderten.
fn pruefe_vertrauenszustaende(
    empfaenger: &[Empfaenger],
    schreiber: crate::ausgabe::Schreiber,
) -> Ergebnis<()> {
    for e in empfaenger {
        match e.zustand {
            Some(TrustState::Revoked) => {
                return Err(Fehler::bedienung(format!(
                    "„{}\" ist als kompromittiert markiert.\n\
                     An diesen Schlüssel wird nicht verschlüsselt. Wer den privaten\n\
                     Schlüssel hat, läse mit. Erst neu austauschen und verifizieren.",
                    e.name
                )));
            }
            Some(TrustState::Changed) => schreiber.hinweis(&format!(
                "Achtung: „{}\" tritt mit einem anderen Schlüssel auf als zuvor.\n\
                 Das kann ein neues Gerät sein — oder ein Angriff. Vor dem Senden\n\
                 über einen zweiten Kanal klären.",
                e.name
            )),
            Some(TrustState::Seen) => schreiber.hinweis(&format!(
                "Hinweis: „{}\" ist nicht verifiziert. Sie wissen nicht sicher,\n\
                 wem dieser Schlüssel gehört.",
                e.name
            )),
            Some(TrustState::Verified) | None => {}
        }
    }
    Ok(())
}

fn suite_name(s: Suite) -> &'static str {
    match s {
        Suite::Classical => "X25519 + ChaCha20-Poly1305 (0x0001)",
        Suite::Hybrid => "X-Wing: X25519 + ML-KEM-768 (0x0002), post-quantum",
        _ => "unbekanntes Verfahren",
    }
}

fn ausgabename(eingabe: &Path) -> PathBuf {
    if eingabe.as_os_str() == "-" {
        return PathBuf::from("nachricht.cab");
    }
    let mut name = eingabe.as_os_str().to_os_string();
    name.push(".cab");
    PathBuf::from(name)
}

// ---------------------------------------------------------------------------
// decrypt
// ---------------------------------------------------------------------------

struct DecryptBericht {
    pfad: Option<String>,
    bytes: usize,
    dateiname: Option<String>,
    zeitstempel: Option<u64>,
    authentizitaet: String,
    warnung: bool,
    gruen: bool,
    unbekannter_absender: Option<String>,
    v1_warnungen: Vec<String>,
}

impl Bericht for DecryptBericht {
    fn text(&self) -> String {
        let mut s = String::new();
        match &self.pfad {
            Some(p) => zeile(&mut s, "Geschrieben", p),
            None => zeile(&mut s, "Ausgabe", "Standardausgabe"),
        }
        zeile(&mut s, "Größe", &format!("{} Bytes", self.bytes));
        if let Some(n) = &self.dateiname {
            zeile(&mut s, "Ursprünglicher Name", n);
        }
        if let Some(t) = self.zeitstempel {
            zeile(&mut s, "Sendezeitpunkt", &t.to_string());
        }
        zeile(&mut s, "Absender", &self.authentizitaet);

        if let Some(k) = &self.unbekannter_absender {
            s.push_str(&format!(
                "\nSignierschlüssel des Absenders:\n  {k}\n\n\
                 Daraus wurde **kein** Kontakt angelegt. Eine Nachricht verrät nur\n\
                 den Signierschlüssel, nicht den Verschlüsselungsschlüssel — das ist\n\
                 gewollt und schützt den Absender vor Mitlesern. Ein Kontakt ohne\n\
                 Verschlüsselungsschlüssel wäre aber keiner: Man könnte ihm weder\n\
                 antworten noch seinen Fingerprint abgleichen.\n\n\
                 Um diesen Absender künftig wiederzuerkennen, lassen Sie sich seine\n\
                 Identität geben und nehmen Sie ihn auf:\n  \
                 cabrik contacts add <nutzlast> --name \"<Name>\"\n"
            ));
        }
        if !self.v1_warnungen.is_empty() {
            s.push_str("\nDiese Datei stammt aus Version 1 und gab offen preis:\n");
            for w in &self.v1_warnungen {
                s.push_str("  - ");
                s.push_str(w);
                s.push('\n');
            }
        }
        s
    }

    fn json(&self) -> Value {
        json!({
            "ok": true,
            "pfad": self.pfad,
            "bytes": self.bytes,
            "dateiname": self.dateiname,
            "zeitstempel": self.zeitstempel,
            "absender": self.authentizitaet,
            "warnung": self.warnung,
            "darf_gruen_zeigen": self.gruen,
            "unbekannter_signierschluessel": self.unbekannter_absender,
            "v1_warnungen": self.v1_warnungen,
        })
    }
}

/// Entschlüsselt einen Envelope.
///
/// # Fehler
///
/// Bedien-, Datei- oder Kryptofehler.
pub fn decrypt(g: &Global, a: &DecryptArgs) -> Ergebnis<()> {
    let schreiber = g.schreiber();
    let daten = lies_eingabe(&a.datei)?;

    if cabrik_v1::envelope::looks_like_v1(&daten) {
        return decrypt_v1(g, a, &daten);
    }

    let quelle = g.passwortquelle()?;

    let (geoeffnet, identity) = if a.password {
        let pw = geheimnis::lies(&quelle, "Envelope-Passwort")?;
        (
            envelope::open(&Opener::Password(&pw), &daten, a.require_signature)?,
            None,
        )
    } else {
        let id = lade_identitaet(g)?;
        let auf = envelope::open(&Opener::Identity(&id), &daten, a.require_signature)?;
        (auf, Some(id))
    };

    // --- Absender einordnen ----------------------------------------------
    let kontakte_pfad = ablage::kontakte_pfad(g.contacts.as_deref())?;
    let store = match &identity {
        Some(id) => ablage::lies_kontakte(&kontakte_pfad, id)?,
        None => TrustStore::new(),
    };
    let authentizitaet = store.resolve(&geoeffnet.signer);

    let ziel = ziel_pfad(a.out.as_deref(), geoeffnet.filename.as_deref());
    let bytes = geoeffnet.plaintext.len();
    // Gehört die Standardausgabe den Nutzdaten, weicht der Bericht aus.
    let schreiber = if ziel.is_none() {
        schreiber.mit_belegtem_stdout()
    } else {
        schreiber
    };
    match &ziel {
        Some(p) => schreib_ausgabe(p, &geoeffnet.plaintext)?,
        None => schreib_auf_stdout(&geoeffnet.plaintext)?,
    }

    schreiber.bericht(&DecryptBericht {
        pfad: ziel.as_ref().map(|p| p.display().to_string()),
        bytes,
        dateiname: geoeffnet.filename.clone(),
        zeitstempel: geoeffnet.timestamp,
        authentizitaet: beschreibe_authentizitaet(&authentizitaet),
        warnung: authentizitaet.is_warning(),
        gruen: authentizitaet.may_show_green(),
        unbekannter_absender: unbekannter_absender(&authentizitaet),
        v1_warnungen: Vec::new(),
    });

    if authentizitaet.is_warning() {
        schreiber.hinweis(
            "\nDieser Absender ist ein Warnfall. Die Nachricht wurde entschlüsselt,\n\
             aber wer sie geschrieben hat, ist damit nicht geklärt.",
        );
    }
    Ok(())
}

/// Der Signierschlüssel eines unbekannten Absenders, zur Anzeige.
///
/// # Warum daraus kein Kontakt wird
///
/// Es wäre naheliegend, den Absender hier automatisch aufzunehmen — „Trust on
/// First Use". Es geht aber nicht, und der Grund ist eine **Stärke** des
/// Formats: Eine empfangene Nachricht verrät nur den *Signierschlüssel*. Der
/// Schlüsselaustausch ist ephemer, der dauerhafte Verschlüsselungsschlüssel
/// des Absenders steht nirgends im Envelope. Genau das war der schwerste
/// Fehler von v1, das ihn offen mitschickte und damit jeden Absender für
/// Mitleser erkennbar machte.
///
/// Ein Kontakt ohne Verschlüsselungsschlüssel wäre aber kein Kontakt: Man
/// könnte ihm nicht antworten, und sein Fingerprint — über einen leeren
/// Schlüssel gebildet — stimmte mit **nichts** überein, was die Gegenseite
/// anzeigt. Die Oberfläche lüde dann zu einer Verifikation ein, die nie
/// gelingen kann.
///
/// Deshalb wird hier nichts angelegt. Wer den Absender wiedererkennen will,
/// braucht dessen Austausch-Nutzlast — die er ohnehin braucht, um zu
/// antworten. Ab dann greift die Erkennung von Schlüsselwechseln.
fn unbekannter_absender(a: &Authenticity) -> Option<String> {
    match a {
        Authenticity::SignedUnknown { sig_pub } => Some(cabrik_core::base32::encode(sig_pub)),
        _ => None,
    }
}

/// Entschlüsselt einen v1-Envelope.
fn decrypt_v1(g: &Global, a: &DecryptArgs, daten: &[u8]) -> Ergebnis<()> {
    let schreiber = g.schreiber();

    if a.password {
        return Err(Fehler::bedienung(
            "Version 1 kannte keinen Passwortmodus — diese Datei braucht einen Schlüssel.",
        ));
    }

    let identity = lade_identitaet(g)?;
    let geoeffnet = cabrik_v1::envelope::open(daten, &identity.enc_sk, a.require_signature)?;

    let store = ablage::lies_kontakte(&ablage::kontakte_pfad(g.contacts.as_deref())?, &identity)?;
    let signer = geoeffnet
        .signer
        .map_or(envelope::Signer::None, envelope::Signer::Key);
    let authentizitaet = store.resolve(&signer);

    let ziel = ziel_pfad(
        a.out.as_deref(),
        geoeffnet.warnings.filename_exposed.as_deref(),
    );
    let bytes = geoeffnet.plaintext.len();
    let schreiber = if ziel.is_none() {
        schreiber.mit_belegtem_stdout()
    } else {
        schreiber
    };
    match &ziel {
        Some(p) => schreib_ausgabe(p, &geoeffnet.plaintext)?,
        None => schreib_auf_stdout(&geoeffnet.plaintext)?,
    }

    schreiber.bericht(&DecryptBericht {
        pfad: ziel.as_ref().map(|p| p.display().to_string()),
        bytes,
        dateiname: geoeffnet.warnings.filename_exposed.clone(),
        zeitstempel: geoeffnet.warnings.timestamp_exposed,
        authentizitaet: beschreibe_authentizitaet(&authentizitaet),
        warnung: authentizitaet.is_warning(),
        gruen: authentizitaet.may_show_green(),
        unbekannter_absender: unbekannter_absender(&authentizitaet),
        v1_warnungen: v1_warnungen(&geoeffnet.warnings),
    });
    Ok(())
}

fn v1_warnungen(w: &cabrik_v1::envelope::Warnings) -> Vec<String> {
    let mut aus = Vec::new();
    if let Some(n) = &w.filename_exposed {
        aus.push(format!("den Dateinamen „{n}\""));
    }
    if let Some(s) = &w.size_exposed {
        aus.push(format!("die Klartextgröße ({s})"));
    }
    if let Some(t) = w.timestamp_exposed {
        aus.push(format!("den Sendezeitpunkt ({t})"));
    }
    if w.sender_key_exposed {
        aus.push(
            "den dauerhaften Absenderschlüssel — damit war der Absender für \
             jeden erkennbar, der die Datei sah"
                .to_owned(),
        );
    }
    if let Some(p) = &w.product_named {
        aus.push(format!("das verwendete Programm („{p}\")"));
    }
    aus
}

fn ziel_pfad(angabe: Option<&Path>, aus_envelope: Option<&str>) -> Option<PathBuf> {
    match angabe {
        Some(p) if p.as_os_str() == "-" => None,
        Some(p) => Some(p.to_path_buf()),
        None => aus_envelope.map(PathBuf::from),
    }
}

fn schreib_auf_stdout(daten: &[u8]) -> Ergebnis<()> {
    use std::io::Write as _;
    std::io::stdout()
        .write_all(daten)
        .map_err(|e| Fehler::datei("<stdout>", e))
}

/// Setzt [`Authenticity`] in einen Satz um, der nicht mehr behauptet, als
/// die Kryptographie hergibt.
fn beschreibe_authentizitaet(a: &Authenticity) -> String {
    match a {
        Authenticity::Unsigned => "nicht signiert — der Absender ist unbekannt".to_owned(),
        Authenticity::SignedUnknown { .. } => {
            "gültig signiert, aber der Schlüssel ist unbekannt — das sagt nichts \
             über die Person"
                .to_owned()
        }
        Authenticity::SignedSeen { name, .. } => {
            format!("„{name}\" — bekannt, aber nie verifiziert")
        }
        Authenticity::SignedVerified { name, .. } => format!("„{name}\" — verifiziert"),
        Authenticity::SignedChanged {
            name,
            previous_was_verified,
            ..
        } => {
            if *previous_was_verified {
                format!(
                    "ACHTUNG: „{name}\" hat einen anderen Schlüssel als bei der \
                     Verifikation. Das kann ein neues Gerät sein — oder ein Angriff."
                )
            } else {
                format!("ACHTUNG: „{name}\" tritt mit einem anderen Schlüssel auf als zuvor")
            }
        }
        Authenticity::SignedRevoked { name, .. } => {
            format!("ACHTUNG: der Schlüssel von „{name}\" ist als kompromittiert markiert")
        }
    }
}

// ---------------------------------------------------------------------------
// inspect
// ---------------------------------------------------------------------------

struct InspectBericht {
    version: String,
    suite: Option<String>,
    stanzas: Option<usize>,
    bytes: usize,
    offengelegt: Vec<String>,
}

impl Bericht for InspectBericht {
    fn text(&self) -> String {
        let mut s = String::new();
        zeile(&mut s, "Format", &self.version);
        if let Some(x) = &self.suite {
            zeile(&mut s, "Verfahren", x);
        }
        if let Some(n) = self.stanzas {
            zeile(&mut s, "Kapseln", &format!("{n}"));
        }
        zeile(&mut s, "Größe", &format!("{} Bytes", self.bytes));

        s.push('\n');
        if self.offengelegt.is_empty() {
            s.push_str(
                "Ohne Schlüssel ist nichts weiter erkennbar — weder Empfänger noch\n\
                 Dateiname, Größe oder Absender.",
            );
        } else {
            s.push_str("Ohne Schlüssel ist erkennbar:\n");
            for w in &self.offengelegt {
                s.push_str("  - ");
                s.push_str(w);
                s.push('\n');
            }
        }
        s
    }

    fn json(&self) -> Value {
        json!({
            "ok": true,
            "version": self.version,
            "suite": self.suite,
            "kapseln": self.stanzas,
            "bytes": self.bytes,
            "offengelegt": self.offengelegt,
        })
    }
}

/// Zeigt, was ohne Schlüssel sichtbar ist.
///
/// # Fehler
///
/// Datei- oder Formatfehler.
pub fn inspect(g: &Global, a: &InspectArgs) -> Ergebnis<()> {
    let daten = lies_eingabe(&a.datei)?;
    let schreiber = g.schreiber();

    if cabrik_v1::envelope::looks_like_v1(&daten) {
        schreiber.bericht(&InspectBericht {
            version: "Cabrik Secure Version 1 (JSON über Base64)".to_owned(),
            suite: Some("X25519 + XChaCha20-Poly1305".to_owned()),
            stanzas: None,
            bytes: daten.len(),
            offengelegt: vec![
                "Der Kopf steht im Klartext. Dateiname, Größe, Sendezeitpunkt und \
                 der Absenderschlüssel sind ohne Schlüssel lesbar, sofern gesetzt."
                    .to_owned(),
            ],
        });
        return Ok(());
    }

    let (suite, kapseln) = lies_prolog(&daten)?;
    schreiber.bericht(&InspectBericht {
        version: "Cabrik Secure Version 2".to_owned(),
        suite: Some(suite_name(suite).to_owned()),
        stanzas: Some(kapseln),
        bytes: daten.len(),
        offengelegt: vec![format!(
            "die Zahl der Kapseln ({kapseln}). Sie ist eine Obergrenze für die \
             Empfängerzahl — beim Verschlüsseln mit --dummies liegt sie darüber \
             und sagt entsprechend weniger aus"
        )],
    });
    Ok(())
}

/// Liest Magic, Suite und Kapselzahl aus dem Prolog.
fn lies_prolog(daten: &[u8]) -> Ergebnis<(Suite, usize)> {
    use cabrik_core::Error;

    if daten.get(..2) != Some(&cabrik_core::ENVELOPE_MAGIC[..]) {
        return Err(Error::Malformed("envelope: bad magic").into());
    }
    let suite_id = u16::from_be_bytes([
        *daten
            .get(2)
            .ok_or(Error::Malformed("envelope: truncated"))?,
        *daten
            .get(3)
            .ok_or(Error::Malformed("envelope: truncated"))?,
    ]);
    let suite = match suite_id {
        0x0001 => Suite::Classical,
        0x0002 => Suite::Hybrid,
        _ => return Err(Error::UnsupportedSuite.into()),
    };
    let anzahl = *daten
        .get(4)
        .ok_or(Error::Malformed("envelope: truncated"))?;
    Ok((suite, usize::from(anzahl)))
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Fehlschlag soll den Test abbrechen")]
mod tests {
    use super::*;

    fn empfaenger(name: &str, pq: bool) -> Empfaenger {
        Empfaenger {
            name: name.to_owned(),
            enc_pub: [1u8; 32],
            xwing_pub: if pq {
                Some(Box::new([2u8; 1216]))
            } else {
                None
            },
            zustand: None,
        }
    }

    #[test]
    fn auto_nimmt_post_quantum_wenn_alle_es_koennen() {
        let e = vec![empfaenger("A", true), empfaenger("B", true)];
        let (s, hinweis) = waehle_suite(SuiteWahl::Auto, &e).unwrap();
        assert_eq!(s, Suite::Hybrid);
        assert!(hinweis.is_none());
    }

    /// Ein Envelope hat genau eine Suite. Ein einziger Empfaenger ohne
    /// X-Wing-Schluessel zwingt alle auf den klassischen Pfad — und das muss
    /// gesagt werden, sonst glaubt der Nutzer, er sei post-quantum geschuetzt.
    #[test]
    fn ein_einziger_alter_kontakt_zwingt_alle_auf_klassisch_und_sagt_es() {
        let e = vec![empfaenger("Neu", true), empfaenger("Aus v1", false)];
        let (s, hinweis) = waehle_suite(SuiteWahl::Auto, &e).unwrap();
        assert_eq!(s, Suite::Classical);
        assert!(hinweis.unwrap().contains("Aus v1"));
    }

    #[test]
    fn hybrid_erzwungen_scheitert_verstaendlich() {
        let e = vec![empfaenger("Aus v1", false)];
        let f = waehle_suite(SuiteWahl::Hybrid, &e).unwrap_err();
        assert_eq!(f.code(), "USAGE");
        assert!(f.to_string().contains("Aus v1"));
    }

    /// Reiner Passwortmodus hat keine Empfaenger — dort steht der
    /// Post-Quantum-Wahl nichts im Weg.
    #[test]
    fn ohne_empfaenger_bleibt_es_bei_post_quantum() {
        let (s, _) = waehle_suite(SuiteWahl::Auto, &[]).unwrap();
        assert_eq!(s, Suite::Hybrid);
    }

    #[test]
    fn ausgabename_haengt_an_statt_zu_ersetzen() {
        // Nicht `bericht.cab`: Sonst kollidieren `bericht.pdf` und
        // `bericht.docx` in derselben Datei.
        assert_eq!(
            ausgabename(Path::new("bericht.pdf")),
            PathBuf::from("bericht.pdf.cab")
        );
        assert_eq!(ausgabename(Path::new("-")), PathBuf::from("nachricht.cab"));
    }

    #[test]
    fn nur_verifiziert_darf_gruen_sein() {
        let unbekannt = Authenticity::SignedUnknown { sig_pub: [7; 32] };
        assert!(!unbekannt.may_show_green());
        assert!(beschreibe_authentizitaet(&unbekannt).contains("unbekannt"));

        let gewechselt = Authenticity::SignedChanged {
            fingerprint: cabrik_core::Fingerprint::from_bytes([0; 32]),
            name: "Bob".to_owned(),
            previous_fingerprint: None,
            previous_was_verified: true,
        };
        assert!(gewechselt.is_warning());
        assert!(beschreibe_authentizitaet(&gewechselt).contains("ACHTUNG"));
    }

    #[test]
    fn stdout_ziel_wird_erkannt() {
        assert!(ziel_pfad(Some(Path::new("-")), Some("x.txt")).is_none());
        assert_eq!(ziel_pfad(None, Some("x.txt")), Some(PathBuf::from("x.txt")));
    }
}
