//! `keygen`, `identity`, `migrate`.

use super::{jetzt, schreib_ausgabe};
use crate::ablage;
use crate::ausgabe::{Bericht, zeile};
use crate::fehler::{Ergebnis, Fehler};
use crate::geheimnis;
use crate::{Global, IdentityBefehl, KdfStufe, KeygenArgs, MigrateArgs};

use cabrik_core::keyfile::KdfParams;
use cabrik_core::{Identity, OsRandom, trust};
use serde_json::{Value, json};

impl KdfStufe {
    fn params(self) -> KdfParams {
        match self {
            Self::Min => KdfParams {
                m_cost: KdfParams::M_COST_MIN,
                t_cost: KdfParams::T_COST_MIN,
                p_cost: 1,
            },
            Self::Recommended => KdfParams::recommended(),
            Self::Strong => KdfParams {
                m_cost: 1_048_576,
                t_cost: 4,
                p_cost: 4,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// keygen
// ---------------------------------------------------------------------------

struct KeygenBericht {
    pfad: String,
    fingerprint: String,
    kann_signieren: bool,
}

impl Bericht for KeygenBericht {
    fn text(&self) -> String {
        let mut s = String::new();
        zeile(&mut s, "Schlüssel angelegt", &self.pfad);
        zeile(&mut s, "Fingerprint", &self.fingerprint);
        if !self.kann_signieren {
            s.push_str(
                "\nOhne Signierschlüssel: Empfänger können nie feststellen, dass\n\
                 eine Nachricht von Ihnen stammt.\n",
            );
        }
        s.push_str(
            "\nDas Passwort lässt sich nicht zurücksetzen. Geht es verloren,\n\
             ist der Schlüssel verloren — und mit ihm alles, was für ihn\n\
             verschlüsselt wurde.",
        );
        s
    }

    fn json(&self) -> Value {
        json!({
            "ok": true,
            "pfad": self.pfad,
            "fingerprint": self.fingerprint,
            "kann_signieren": self.kann_signieren,
        })
    }
}

/// Erzeugt eine neue Identität.
///
/// # Fehler
///
/// Passwort-, Datei- oder Zufallsfehler.
pub fn keygen(g: &Global, a: &KeygenArgs) -> Ergebnis<()> {
    let schreiber = g.schreiber();
    let pfad = ablage::keyfile_pfad(a.out.as_deref().or(g.keyfile.as_deref()))?;

    if pfad.exists() {
        return Err(Fehler::bedienung(format!(
            "{} existiert bereits. Ein überschriebener Schlüssel ist unwiederbringlich.",
            pfad.display()
        )));
    }

    let quelle = g.passwortquelle()?;
    schreiber.hinweis("Das Passwort schützt den Schlüssel auf der Platte.");
    let passwort = geheimnis::lies_neu(&quelle, "Neues Passwort")?;

    let mut identity = Identity::generate(&mut OsRandom, !a.no_signing, jetzt())?;
    identity.label = a.label.clone();

    schreiber.hinweis("Leite den Schlüssel ab — das dauert bewusst einen Moment.");
    ablage::schreib_keyfile(&pfad, &identity, &passwort, &a.kdf.params())?;

    schreiber.bericht(&KeygenBericht {
        pfad: pfad.display().to_string(),
        fingerprint: trust::own_fingerprint(&identity).display(),
        kann_signieren: identity.can_sign(),
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// identity
// ---------------------------------------------------------------------------

struct IdentityBericht {
    label: Option<String>,
    fingerprint: String,
    fingerprint_voll: String,
    enc_pub: String,
    sig_pub: Option<String>,
    xwing_pub_kurz: String,
    erstellt: u64,
}

impl Bericht for IdentityBericht {
    fn text(&self) -> String {
        let mut s = String::new();
        if let Some(l) = &self.label {
            zeile(&mut s, "Bezeichnung", l);
        }
        zeile(&mut s, "Fingerprint", &self.fingerprint);
        zeile(&mut s, "  vollständig", &self.fingerprint_voll);
        zeile(&mut s, "Verschlüsselung", &self.enc_pub);
        match &self.sig_pub {
            Some(k) => zeile(&mut s, "Signatur", k),
            None => zeile(&mut s, "Signatur", "keine — anonyme Identität"),
        }
        zeile(&mut s, "Post-Quantum", &self.xwing_pub_kurz);
        zeile(&mut s, "Erstellt", &self.erstellt.to_string());
        s.push_str(
            "\nDer Fingerprint beweist nichts, solange er nicht über einen\n\
             zweiten Kanal abgeglichen wurde. Ein Fingerprint, der über\n\
             denselben Weg kommt wie die Nachricht, ist wertlos.",
        );
        s
    }

    fn json(&self) -> Value {
        json!({
            "ok": true,
            "label": self.label,
            "fingerprint": self.fingerprint,
            "fingerprint_voll": self.fingerprint_voll,
            "enc_pub": self.enc_pub,
            "sig_pub": self.sig_pub,
            "post_quantum": true,
            "erstellt": self.erstellt,
        })
    }
}

/// Zeigt oder exportiert die eigene Identität.
///
/// # Fehler
///
/// Passwort- oder Dateifehler.
pub fn identity(g: &Global, b: &IdentityBefehl) -> Ergebnis<()> {
    let schreiber = g.schreiber();
    let identity = lade_identitaet(g)?;

    match b {
        IdentityBefehl::Show => {
            let fp = trust::own_fingerprint(&identity);
            let xwing = identity.xwing_pub();

            schreiber.bericht(&IdentityBericht {
                label: identity.label.clone(),
                fingerprint: fp.display(),
                fingerprint_voll: fp.display_full(),
                enc_pub: cabrik_core::base32::encode(&identity.enc_pub()?),
                sig_pub: identity.sig_pub().map(|k| cabrik_core::base32::encode(&k)),
                xwing_pub_kurz: format!("vorhanden ({} Bytes, X-Wing)", xwing.len()),
                erstellt: identity.created,
            });
        }
        IdentityBefehl::Export { out } => {
            let nutzlast = trust::own_qr_payload(&identity);
            match out {
                Some(p) => {
                    schreib_ausgabe(p, nutzlast.as_bytes())?;
                    schreiber.bericht(&crate::ausgabe::Meldung::mit(
                        format!("Austauschdatei geschrieben: {}", p.display()),
                        json!({ "pfad": p.display().to_string() }),
                    ));
                }
                None => {
                    schreiber.bericht(&crate::ausgabe::Meldung::mit(
                        nutzlast.clone(),
                        json!({ "nutzlast": nutzlast }),
                    ));
                }
            }
            schreiber.hinweis(
                "\nDiese Zeichenfolge enthält nur öffentliche Schlüssel und darf\n\
                 weitergegeben werden. Sie ersetzt aber keine Verifikation:\n\
                 Wer sie über denselben Kanal schickt wie seine Nachrichten,\n\
                 beweist damit nichts.",
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// migrate
// ---------------------------------------------------------------------------

struct MigrateBericht {
    pfad: String,
    alter_fingerprint_hinweis: bool,
    fingerprint: String,
    kann_signieren: bool,
}

impl Bericht for MigrateBericht {
    fn text(&self) -> String {
        let mut s = String::new();
        zeile(&mut s, "Übernommen nach", &self.pfad);
        zeile(&mut s, "Fingerprint", &self.fingerprint);
        if !self.kann_signieren {
            zeile(&mut s, "Signatur", "keine — der alte Schlüssel hatte keine");
        }
        if self.alter_fingerprint_hinweis {
            s.push_str(
                "\nDer Fingerprint hat sich geändert, und das ist richtig so:\n\
                 Die Identität hat einen neuen Post-Quantum-Schlüssel bekommen,\n\
                 der in den Fingerprint eingeht.\n\n\
                 Folgen:\n\
                 - Ihre bisherigen Gegenüber sehen den Zustand „Geändert\".\n\
                 - Bitte einmalig neu verifizieren.\n\
                 - Empfangen bleibt uneingeschränkt möglich: Alte Nachrichten\n  \
                   an den alten Schlüssel lassen sich weiterhin öffnen.",
            );
        }
        s
    }

    fn json(&self) -> Value {
        json!({
            "ok": true,
            "pfad": self.pfad,
            "fingerprint": self.fingerprint,
            "kann_signieren": self.kann_signieren,
            "fingerprint_geaendert": self.alter_fingerprint_hinweis,
        })
    }
}

/// Übernimmt ein v1-Keyfile.
///
/// # Fehler
///
/// Passwort-, Datei- oder Formatfehler.
pub fn migrate(g: &Global, a: &MigrateArgs) -> Ergebnis<()> {
    let schreiber = g.schreiber();

    let alt = std::fs::read(&a.datei).map_err(|e| Fehler::datei(&a.datei, e))?;
    if !cabrik_v1::keyfile::looks_like_v1(&alt) {
        return Err(Fehler::bedienung(format!(
            "{} sieht nicht wie ein Schlüssel aus Version 1 aus",
            a.datei.display()
        )));
    }
    if a.out.exists() {
        return Err(Fehler::bedienung(format!(
            "{} existiert bereits",
            a.out.display()
        )));
    }

    let quelle = g.passwortquelle()?;
    let alt_pw = geheimnis::lies(&quelle, "Passwort des alten Schlüssels")?;

    schreiber.hinweis("Öffne den alten Schlüssel — Version 1 leitet langsamer ab.");
    let identity = cabrik_v1::keyfile::migrate(&alt, &alt_pw, &mut OsRandom)?;

    // Bewusst getrennt abfragen: Das alte Passwort weiterzuverwenden wäre
    // bequem, hieße aber, eine womöglich alte und schwache Wahl mitzuschleppen.
    let neu_pw = if quelle.ist_automatisch() {
        alt_pw
    } else {
        schreiber.hinweis("\nPasswort für den neuen Schlüssel:");
        geheimnis::lies_neu(&quelle, "Neues Passwort")?
    };

    ablage::schreib_keyfile(&a.out, &identity, &neu_pw, &a.kdf.params())?;

    schreiber.bericht(&MigrateBericht {
        pfad: a.out.display().to_string(),
        fingerprint: trust::own_fingerprint(&identity).display(),
        kann_signieren: identity.can_sign(),
        alter_fingerprint_hinweis: true,
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// Gemeinsam genutzt
// ---------------------------------------------------------------------------

/// Öffnet das Keyfile, fragt dabei nach dem Passwort.
///
/// # Fehler
///
/// Passwort- oder Dateifehler.
pub fn lade_identitaet(g: &Global) -> Ergebnis<Identity> {
    let pfad = ablage::keyfile_pfad(g.keyfile.as_deref())?;
    if !pfad.exists() {
        return Err(Fehler::bedienung(format!(
            "Kein Schlüssel unter {}.\nMit `cabrik keygen` einen anlegen \
             oder mit --keyfile einen anderen Pfad angeben.",
            pfad.display()
        )));
    }
    let quelle = g.passwortquelle()?;
    let passwort = geheimnis::lies(&quelle, "Passwort")?;
    ablage::lies_keyfile(&pfad, &passwort)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Die drei Stufen müssen die Grenzen der Spezifikation einhalten,
    /// sonst scheitert das Schreiben erst nach der Passworteingabe.
    #[test]
    fn alle_kdf_stufen_sind_gueltig() {
        for stufe in [KdfStufe::Min, KdfStufe::Recommended, KdfStufe::Strong] {
            assert!(
                stufe.params().validate().is_ok(),
                "{stufe:?} liegt ausserhalb der Spezifikation"
            );
        }
    }

    #[test]
    fn min_ist_wirklich_die_untergrenze() {
        assert_eq!(KdfStufe::Min.params().m_cost, KdfParams::M_COST_MIN);
        assert!(KdfStufe::Strong.params().m_cost > KdfParams::recommended().m_cost);
    }
}
