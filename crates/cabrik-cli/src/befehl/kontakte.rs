//! `contacts` und `safety-number`.

use super::jetzt;
use crate::ablage;
use crate::ausgabe::{Bericht, Meldung, zeile};
use crate::befehl::schluessel::lade_identitaet;
use crate::fehler::{Ergebnis, Fehler};
use crate::{ContactsBefehl, Global, SafetyNumberArgs, VerifikationsWeg};

use cabrik_core::trust::{self, Contact, TrustState, TrustStore, VerifiedVia};
use serde_json::{Value, json};

impl VerifikationsWeg {
    const fn zum_kern(self) -> VerifiedVia {
        match self {
            Self::QrCode => VerifiedVia::QrCode,
            Self::SafetyNumber => VerifiedVia::SafetyNumber,
            Self::Fingerprint => VerifiedVia::Fingerprint,
        }
    }
}

const fn zustand_text(z: TrustState) -> &'static str {
    match z {
        TrustState::Seen => "gesehen, nicht verifiziert",
        TrustState::Verified => "verifiziert",
        TrustState::Changed => "ACHTUNG: Schlüssel geändert",
        TrustState::Revoked => "ACHTUNG: als kompromittiert markiert",
    }
}

const fn zustand_kode(z: TrustState) -> &'static str {
    match z {
        TrustState::Seen => "seen",
        TrustState::Verified => "verified",
        TrustState::Changed => "changed",
        TrustState::Revoked => "revoked",
    }
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

struct ListeBericht {
    eintraege: Vec<(String, String, TrustState, bool)>,
}

impl Bericht for ListeBericht {
    fn text(&self) -> String {
        if self.eintraege.is_empty() {
            return "Keine Kontakte. Aufnehmen mit:\n  \
                    cabrik contacts add <nutzlast> --name \"<Name>\""
                .to_owned();
        }
        let mut s = String::new();
        for (name, kurz, zustand, pq) in &self.eintraege {
            s.push_str(&format!(
                "{name}  [{kurz}]  {}{}\n",
                zustand_text(*zustand),
                if *pq { "" } else { "  (kein Post-Quantum)" }
            ));
        }
        s.push_str(
            "\nDie Kurzform in Klammern dient nur der Unterscheidung in dieser\n\
             Liste. Zur Verifikation ist sie zu kurz — dafür `contacts show`.",
        );
        s
    }

    fn json(&self) -> Value {
        json!({
            "ok": true,
            "kontakte": self.eintraege.iter().map(|(name, kurz, z, pq)| json!({
                "name": name,
                "fingerprint_kurz": kurz,
                "zustand": zustand_kode(*z),
                "post_quantum": pq,
            })).collect::<Vec<_>>(),
        })
    }
}

// ---------------------------------------------------------------------------
// show
// ---------------------------------------------------------------------------

/// Wie der Verifikationsweg benannt wird.
const fn weg_wort(w: Option<VerifiedVia>) -> &'static str {
    match w {
        Some(VerifiedVia::QrCode) => "QR-Code",
        Some(VerifiedVia::SafetyNumber) => "Safety Number",
        Some(VerifiedVia::Fingerprint) => "Fingerprint",
        None => "nicht vermerkt",
    }
}

/// Derselbe Weg als stabiler Schluessel fuer `--json`.
const fn weg_kode(w: VerifiedVia) -> &'static str {
    match w {
        VerifiedVia::QrCode => "qr",
        VerifiedVia::SafetyNumber => "safety_number",
        VerifiedVia::Fingerprint => "fingerprint",
    }
}

struct ShowBericht {
    name: String,
    fingerprint: String,
    fingerprint_voll: String,
    zustand: TrustState,
    post_quantum: bool,
    erstkontakt: u64,
    verifiziert_am: Option<u64>,
    verifiziert_ueber: Option<VerifiedVia>,
    notiz: Option<String>,
    historie: Vec<String>,
}

impl Bericht for ShowBericht {
    fn text(&self) -> String {
        let mut s = String::new();
        zeile(&mut s, "Name", &self.name);
        zeile(&mut s, "Zustand", zustand_text(self.zustand));
        zeile(&mut s, "Fingerprint", &self.fingerprint);
        zeile(&mut s, "  vollständig", &self.fingerprint_voll);
        zeile(
            &mut s,
            "Post-Quantum",
            if self.post_quantum {
                "möglich"
            } else {
                "nicht möglich — dieser Kontakt wurde aus Version 1 übernommen.\n  \
                 An ihn wird klassisch verschlüsselt. Eine neue Austausch-Nutzlast\n  \
                 behebt das."
            },
        );
        zeile(&mut s, "Erstkontakt", &self.erstkontakt.to_string());
        if let Some(v) = self.verifiziert_am {
            zeile(&mut s, "Verifiziert am", &v.to_string());
            // Nicht nur WANN, sondern WODURCH: Die Wege sind nicht
            // gleichwertig (`spec/trust-store.md` §5). Ohne diese Zeile
            // sieht der schwaechste aus wie der staerkste.
            zeile(&mut s, "Verifiziert über", weg_wort(self.verifiziert_ueber));
        }
        if let Some(n) = &self.notiz {
            zeile(&mut s, "Notiz", n);
        }
        if !self.historie.is_empty() {
            s.push_str("\nFrühere Schlüssel:\n");
            for h in &self.historie {
                s.push_str("  - ");
                s.push_str(h);
                s.push('\n');
            }
        }
        if self.verifiziert_ueber == Some(VerifiedVia::Fingerprint) {
            s.push_str(
                "
Die Pruefung stuetzt sich auf einen abgeglichenen Fingerprint.
                 Das traegt nur, wenn er ueber einen anderen Weg kam als die
                 Nachrichten selbst - derselbe Kanal, derselbe Angreifer.",
            );
        }
        if self.zustand == TrustState::Seen {
            s.push_str(
                "\nNoch nicht verifiziert. Den Fingerprint über einen zweiten Kanal\n\
                 abgleichen — Telefon, persönlich — und dann:\n  \
                 cabrik contacts verify \"",
            );
            s.push_str(&self.name);
            s.push_str(
                "\"\n\nEin Fingerprint, der über denselben Kanal kommt wie die\n\
                 Nachrichten, beweist nichts.",
            );
        }
        s
    }

    fn json(&self) -> Value {
        json!({
            "ok": true,
            "name": self.name,
            "fingerprint": self.fingerprint,
            "fingerprint_voll": self.fingerprint_voll,
            "zustand": zustand_kode(self.zustand),
            "post_quantum": self.post_quantum,
            "erstkontakt": self.erstkontakt,
            "verifiziert_am": self.verifiziert_am,
            "verifiziert_ueber": self.verifiziert_ueber.map(weg_kode),
            "notiz": self.notiz,
            "historie": self.historie,
        })
    }
}

// ---------------------------------------------------------------------------
// Ausführung
// ---------------------------------------------------------------------------

/// Führt einen `contacts`-Unterbefehl aus.
///
/// # Fehler
///
/// Bedien-, Datei- oder Kryptofehler.
pub fn fuehre_aus(g: &Global, b: &ContactsBefehl) -> Ergebnis<()> {
    let schreiber = g.schreiber();

    // Alles, was **ohne** den Schlüssel prüfbar ist, wird vorher geprüft.
    // Sonst tippt der Nutzer erst ein Passwort und erfährt danach, dass er
    // sich im Dateinamen vertippt hat. Die Reihenfolge der Prüfungen ist Teil
    // der Bedienung, nicht nur eine Frage der Umsetzung.
    let vorgelesene_nutzlast = match b {
        ContactsBefehl::Add { nutzlast, .. } => Some(hole_nutzlast(nutzlast)?),
        _ => None,
    };

    let identity = lade_identitaet(g)?;
    let pfad = ablage::kontakte_pfad(g.contacts.as_deref())?;
    let mut store = ablage::lies_kontakte(&pfad, &identity)?;

    match b {
        ContactsBefehl::List => {
            schreiber.bericht(&ListeBericht {
                eintraege: store
                    .contacts()
                    .iter()
                    .map(|k| {
                        (
                            k.name.clone(),
                            k.fingerprint().short(),
                            k.state,
                            k.supports_post_quantum(),
                        )
                    })
                    .collect(),
            });
        }

        ContactsBefehl::Add { name, .. } => {
            let roh = vorgelesene_nutzlast
                .ok_or_else(|| Fehler::bedienung("Keine Nutzlast eingelesen"))?;
            let gelesen = trust::parse_qr(roh.trim()).map_err(nutzlast_fehler)?;
            if store
                .contacts()
                .iter()
                .any(|k| k.name.eq_ignore_ascii_case(name))
            {
                return Err(Fehler::bedienung(format!(
                    "Es gibt bereits einen Kontakt namens „{name}\""
                )));
            }

            let kontakt = Contact::new_seen(
                name,
                gelesen.enc_pub,
                gelesen.sig_pub,
                gelesen.xwing_pub,
                jetzt(),
            )?;
            let fp = kontakt.fingerprint();
            let pq = kontakt.supports_post_quantum();
            store.add(kontakt)?;
            ablage::schreib_kontakte(&pfad, &store, &identity)?;

            let mut text = format!(
                "„{name}\" aufgenommen — Zustand: gesehen, **nicht** verifiziert.\n\
                 Fingerprint: {}\n\n\
                 Zum Verifizieren diesen Fingerprint über einen zweiten Kanal\n\
                 abgleichen und dann `cabrik contacts verify \"{name}\"` aufrufen.",
                fp.display()
            );
            if !pq {
                text.push_str(
                    "\n\nDieser Kontakt hat keinen Post-Quantum-Schlüssel. An ihn kann\n\
                     nur mit dem klassischen Verfahren verschlüsselt werden.",
                );
            }
            schreiber.bericht(&Meldung::mit(
                text,
                json!({
                    "name": name,
                    "fingerprint": fp.display(),
                    "zustand": "seen",
                    "post_quantum": pq,
                }),
            ));
        }

        ContactsBefehl::Show { name } => {
            let k = finde(&store, name)?;
            let fp = k.fingerprint();
            schreiber.bericht(&ShowBericht {
                name: k.name.clone(),
                fingerprint: fp.display(),
                fingerprint_voll: fp.display_full(),
                zustand: k.state,
                post_quantum: k.supports_post_quantum(),
                erstkontakt: k.first_seen,
                verifiziert_am: k.verified_at,
                verifiziert_ueber: k.verified_via,
                notiz: k.note.clone(),
                historie: k
                    .previous_keys
                    .iter()
                    .map(|p| {
                        format!(
                            "{} (abgelöst {}{})",
                            p.fingerprint.short(),
                            p.replaced_at,
                            if p.was_verified {
                                ", war verifiziert"
                            } else {
                                ""
                            }
                        )
                    })
                    .collect(),
            });
        }

        ContactsBefehl::Verify { name, via } => {
            let idx = finde_index(&store, name)?;
            let (angezeigt, neuer_zustand) = {
                let k = store
                    .contacts_mut()
                    .get_mut(idx)
                    .ok_or_else(|| Fehler::bedienung("Kontakt verschwunden"))?;
                k.verify(via.zum_kern(), jetzt())?;
                (k.fingerprint().display(), k.state)
            };
            ablage::schreib_kontakte(&pfad, &store, &identity)?;
            schreiber.bericht(&Meldung::mit(
                format!(
                    "„{name}\" als verifiziert markiert.\n\
                     Fingerprint: {angezeigt}\n\n\
                     Das gilt genau für diesen Schlüssel. Taucht der Kontakt später\n\
                     mit einem anderen auf, wird gewarnt."
                ),
                json!({ "name": name, "zustand": zustand_kode(neuer_zustand) }),
            ));
        }

        ContactsBefehl::Revoke { name, note } => {
            let idx = finde_index(&store, name)?;
            {
                let k = store
                    .contacts_mut()
                    .get_mut(idx)
                    .ok_or_else(|| Fehler::bedienung("Kontakt verschwunden"))?;
                k.revoke(jetzt(), note.as_deref())?;
            }
            ablage::schreib_kontakte(&pfad, &store, &identity)?;
            schreiber.bericht(&Meldung::mit(
                format!(
                    "„{name}\" ist lokal als kompromittiert markiert.\n\
                     An diesen Kontakt wird nicht mehr verschlüsselt.\n\n\
                     Der Widerruf gilt **nur auf diesem Gerät**. Es gibt keinen\n\
                     Verteilweg — wer sonst noch mit diesem Schlüssel schreibt,\n\
                     erfährt davon nichts. Bitte die Betroffenen selbst informieren."
                ),
                json!({ "name": name, "zustand": "revoked" }),
            ));
        }

        ContactsBefehl::Rename { name, neu } => {
            let idx = finde_index(&store, name)?;
            if store
                .contacts()
                .iter()
                .any(|k| k.name.eq_ignore_ascii_case(neu))
            {
                return Err(Fehler::bedienung(format!(
                    "Es gibt bereits einen Kontakt namens „{neu}\""
                )));
            }
            {
                let k = store
                    .contacts_mut()
                    .get_mut(idx)
                    .ok_or_else(|| Fehler::bedienung("Kontakt verschwunden"))?;
                k.name = neu.clone();
            }
            ablage::schreib_kontakte(&pfad, &store, &identity)?;
            schreiber.bericht(&Meldung::mit(
                format!("„{name}\" heißt jetzt „{neu}\". Der Schlüssel bleibt derselbe."),
                json!({ "name": neu }),
            ));
        }

        ContactsBefehl::Remove { name } => {
            let idx = finde_index(&store, name)?;
            let entfernt = store.remove(idx)?;
            ablage::schreib_kontakte(&pfad, &store, &identity)?;
            schreiber.bericht(&Meldung::mit(
                format!(
                    "„{}\" entfernt.\n\n\
                     Damit ist auch die Schlüsselhistorie weg: Meldet sich der\n\
                     Kontakt später mit einem anderen Schlüssel, fällt das nicht\n\
                     mehr auf. Bei Verdacht ist `revoke` das bessere Mittel.",
                    entfernt.name
                ),
                json!({ "name": entfernt.name }),
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// safety-number
// ---------------------------------------------------------------------------

struct SafetyBericht {
    name: String,
    nummer: String,
    verifiziert: bool,
}

impl Bericht for SafetyBericht {
    fn text(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("Safety Number mit „{}\":\n\n", self.name));
        for (i, gruppe) in self.nummer.split_whitespace().enumerate() {
            s.push_str(gruppe);
            if i % 6 == 5 {
                s.push('\n');
            } else {
                s.push_str("  ");
            }
        }
        s.push_str(
            "\n\nBeide Seiten sehen dieselbe Zahl. Am Telefon vorlesen und\n\
             vergleichen — die Stimme ist dabei der zweite Kanal.\n\
             Über denselben Weg wie die Nachrichten geschickt, beweist sie nichts.",
        );
        if !self.verifiziert {
            s.push_str("\n\nStimmt sie überein: cabrik contacts verify \"");
            s.push_str(&self.name);
            s.push_str("\" --via safety-number");
        }
        s
    }

    fn json(&self) -> Value {
        json!({
            "ok": true,
            "name": self.name,
            "safety_number": self.nummer,
            "bereits_verifiziert": self.verifiziert,
        })
    }
}

/// Zeigt die Safety Number mit einem Kontakt.
///
/// # Fehler
///
/// Bedien-, Datei- oder Kryptofehler.
pub fn safety_number(g: &Global, a: &SafetyNumberArgs) -> Ergebnis<()> {
    let schreiber = g.schreiber();
    let identity = lade_identitaet(g)?;
    let store = ablage::lies_kontakte(&ablage::kontakte_pfad(g.contacts.as_deref())?, &identity)?;

    let kontakt = finde(&store, &a.name)?;
    let eigener = trust::own_fingerprint(&identity);

    schreiber.bericht(&SafetyBericht {
        name: kontakt.name.clone(),
        nummer: cabrik_core::safety_number(&eigener, &kontakt.fingerprint()),
        verifiziert: kontakt.state == TrustState::Verified,
    });
    Ok(())
}

/// Präfix jeder Austausch-Nutzlast (`spec/trust-store.md` §5.1).
const NUTZLAST_PRAEFIX: &str = "cabrik:v2:";

/// Beschafft die Nutzlast: aus der Standardeingabe, aus einer Datei oder
/// direkt aus dem Argument.
///
/// # Warum die Unterscheidung wichtig ist
///
/// Vorher wurde alles, was keine existierende Datei war, als Nutzlast-Text
/// gedeutet. Wer sich beim Dateinamen vertippte, bekam die Meldung
/// „Die Datei ist beschädigt oder kein gültiger Envelope" — obwohl gar kein
/// Envelope im Spiel war und die Datei nur fehlte.
///
/// # Fehler
///
/// [`Fehler::Bedienung`] mit einer Erklärung, die zum tatsächlichen Fall passt.
pub fn hole_nutzlast(angabe: &str) -> Ergebnis<String> {
    if angabe == "-" {
        let mut s = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut s)
            .map_err(|e| Fehler::datei("<stdin>", e))?;
        return Ok(s);
    }

    // Sieht es aus wie eine Nutzlast, ist es eine — auch wenn zufällig eine
    // gleichnamige Datei herumliegt.
    if angabe.starts_with(NUTZLAST_PRAEFIX) {
        return Ok(angabe.to_owned());
    }

    let pfad = std::path::Path::new(angabe);
    if pfad.is_file() {
        // Bequemlichkeit: Wer die Nutzlast als Datei bekommen hat, soll nicht
        // erst `cat` bemühen müssen.
        return String::from_utf8(std::fs::read(pfad).map_err(|e| Fehler::datei(pfad, e))?)
            .map_err(|_| {
                Fehler::bedienung(format!(
                    "{} enthält keinen lesbaren Text und damit keine Nutzlast",
                    pfad.display()
                ))
            });
    }

    if pfad.is_dir() {
        return Err(Fehler::bedienung(format!(
            "{} ist ein Verzeichnis, keine Austausch-Nutzlast",
            pfad.display()
        )));
    }

    Err(Fehler::bedienung(format!(
        "„{angabe}\" ist weder eine vorhandene Datei noch eine Austausch-Nutzlast.\n\n\
         Eine Nutzlast beginnt mit „{NUTZLAST_PRAEFIX}\" und ist rund 2000 Zeichen lang.\n\
         Ihr Gegenüber erzeugt sie mit:\n  \
         cabrik identity export --out seine.contact\n\n\
         Diese Datei lassen Sie sich schicken und geben hier ihren Pfad an.\n\
         Das aktuelle Verzeichnis ist: {}",
        std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "unbekannt".to_owned())
    )))
}

/// Übersetzt einen Lesefehler der Nutzlast in eine Meldung, die von der
/// Nutzlast redet — nicht von einem Envelope.
pub fn nutzlast_fehler(e: cabrik_core::trust::QrFehler) -> Fehler {
    use cabrik_core::trust::QrFehler;

    // Zwei Faelle, zwei Ratschlaege. Vorher stand hier eine Meldung, die
    // beide moeglichen Ursachen aufzaehlte -- weil der Kern sie nicht
    // unterscheiden konnte. Jetzt kann er es.
    match e {
        QrFehler::Fremd => Fehler::bedienung(
            "Das ist keine Cabrik-Austausch-Nutzlast.\n\n\
             Sie beginnt mit `cabrik:v2:` und ist rund 2050 Zeichen lang.\n\
             Erzeugen laesst sie sich mit `cabrik identity export`.",
        ),
        QrFehler::Beschaedigt => Fehler::bedienung(
            "Die Austausch-Nutzlast ist beschaedigt angekommen.\n\n\
             Es ist erkennbar eine, aber sie laesst sich nicht lesen \u{2014} beim\n\
             Kopieren ist etwas verlorengegangen, oder ein Mailprogramm hat\n\
             einen Zeilenumbruch eingefuegt. Lassen Sie sie sich noch einmal\n\
             schicken, am sichersten als Datei statt ueber die Zwischenablage.\n\n\
             Das ist kein Angriff: Die Pruefsumme schuetzt gegen\n\
             Uebertragungsfehler, nicht gegen Faelschung.",
        ),
        andere => Fehler::from(cabrik_core::Error::from(andere)),
    }
}

fn finde<'a>(store: &'a TrustStore, name: &str) -> Ergebnis<&'a Contact> {
    store
        .contacts()
        .iter()
        .find(|k| k.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| Fehler::bedienung(format!("Kein Kontakt namens „{name}\"")))
}

fn finde_index(store: &TrustStore, name: &str) -> Ergebnis<usize> {
    store
        .contacts()
        .iter()
        .position(|k| k.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| Fehler::bedienung(format!("Kein Kontakt namens „{name}\"")))
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Fehlschlag soll den Test abbrechen")]
mod tests {
    use super::*;

    fn store_mit(namen: &[&str]) -> TrustStore {
        let mut s = TrustStore::new();
        for (i, n) in namen.iter().enumerate() {
            let byte = u8::try_from(i).unwrap_or(0).saturating_add(1);
            s.add(Contact::new_seen(n, [byte; 32], Some([byte; 32]), None, 0).unwrap())
                .unwrap();
        }
        s
    }

    #[test]
    fn suche_ignoriert_gross_und_kleinschreibung() {
        let s = store_mit(&["Bob"]);
        assert_eq!(finde(&s, "bob").unwrap().name, "Bob");
        assert_eq!(finde(&s, "BOB").unwrap().name, "Bob");
    }

    #[test]
    fn unbekannter_name_nennt_den_namen() {
        let s = store_mit(&["Bob"]);
        let f = finde(&s, "Carla").unwrap_err();
        assert!(f.to_string().contains("Carla"));
    }

    /// Der Zustand muss im JSON stabil kodiert sein — die Oberflaeche in
    /// Phase 3 haengt daran, und uebersetzte Texte taugen dafuer nicht.
    #[test]
    fn zustandskodes_sind_stabil() {
        assert_eq!(zustand_kode(TrustState::Seen), "seen");
        assert_eq!(zustand_kode(TrustState::Verified), "verified");
        assert_eq!(zustand_kode(TrustState::Changed), "changed");
        assert_eq!(zustand_kode(TrustState::Revoked), "revoked");
    }

    /// Warnzustaende muessen im Text als solche erkennbar sein.
    #[test]
    fn warnzustaende_sind_im_text_deutlich() {
        assert!(zustand_text(TrustState::Changed).contains("ACHTUNG"));
        assert!(zustand_text(TrustState::Revoked).contains("ACHTUNG"));
        assert!(!zustand_text(TrustState::Verified).contains("ACHTUNG"));
    }

    /// Ein Tippfehler im Dateinamen ergab „Die Datei ist beschaedigt oder
    /// kein gueltiger Envelope" — obwohl kein Envelope im Spiel war und die
    /// Datei schlicht fehlte.
    #[test]
    fn eine_fehlende_datei_wird_als_solche_gemeldet() {
        let f = hole_nutzlast("bob.contact").unwrap_err();
        let text = f.to_string();

        assert!(
            !text.contains("Envelope"),
            "die Meldung redet vom falschen Ding: {text}"
        );
        assert!(text.contains("bob.contact"), "{text}");
        assert!(
            text.contains("identity export"),
            "der Weg zur Nutzlast fehlt: {text}"
        );
    }

    /// Eine echte Nutzlast wird auch dann erkannt, wenn sie als Argument
    /// kommt und keine Datei dieses Namens existiert.
    #[test]
    fn eine_nutzlast_als_argument_wird_erkannt() {
        let n = format!("{NUTZLAST_PRAEFIX}irgendwas:egal:xx");
        assert_eq!(hole_nutzlast(&n).unwrap(), n);
    }

    /// Auch eine kaputte Nutzlast darf nicht von Envelopes reden.
    #[test]
    fn eine_kaputte_nutzlast_redet_nicht_von_envelopes() {
        for fall in [
            cabrik_core::trust::QrFehler::Fremd,
            cabrik_core::trust::QrFehler::Beschaedigt,
        ] {
            let text = nutzlast_fehler(fall).to_string();
            assert!(!text.contains("Envelope"), "{text}");
            assert!(text.contains("Nutzlast"), "{text}");
        }
    }

    /// Die beiden Faelle geben verschiedene Ratschlaege.
    ///
    /// Vorher gab es nur eine Meldung, die beide moeglichen Ursachen
    /// aufzaehlte -- weil der Kern sie nicht unterscheiden konnte. Wenn
    /// hier wieder derselbe Text herauskaeme, waere die Unterscheidung
    /// umsonst gewesen.
    #[test]
    fn fremd_und_beschaedigt_raten_verschiedenes() {
        let fremd = nutzlast_fehler(cabrik_core::trust::QrFehler::Fremd).to_string();
        let kaputt = nutzlast_fehler(cabrik_core::trust::QrFehler::Beschaedigt).to_string();

        assert_ne!(fremd, kaputt);
        // Wer etwas Falsches eingefuegt hat, braucht die richtige Quelle.
        assert!(fremd.contains("identity export"), "{fremd}");
        // Wer die richtige eingefuegt hat, braucht sie noch einmal.
        assert!(kaputt.contains("noch einmal"), "{kaputt}");
        // Und darf nicht in Sorge versetzt werden.
        assert!(kaputt.contains("kein Angriff"), "{kaputt}");
    }
}
