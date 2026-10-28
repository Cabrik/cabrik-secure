//! Große Dateien in der App-Schicht (`spec/grosse-dateien.md` §3, §6).
//!
//! Diese Schicht fasst kein Dateisystem an. Geprüft wird, was sie dazu
//! beiträgt: den Anfang der blockweisen Wege, den Bericht ohne Klartext
//! und das, was der zweite Durchgang braucht — und dass auch das mit der
//! Sperre geht.

#![expect(clippy::expect_used, reason = "Fehlschlag soll den Test abbrechen")]

use cabrik_app::Sitzung;
use cabrik_bruecke::{Absender, Metadatenbefund, Sperrfrist};
use cabrik_core::Speichergrenze;
use cabrik_core::envelope::{Ablage, Geprueft};
use cabrik_core::keyfile::{self, KdfParams};
use cabrik_core::{Identity, OsRandom};
use zeroize::Zeroizing;

const PASSWORT: &str = "vier zufaellige woerter hier";

fn wer() -> (Sitzung, Identity) {
    let id = Identity::generate(&mut OsRandom, true, 1_700_000_000).expect("Identität");
    let datei = keyfile::write(
        &id,
        PASSWORT.as_bytes(),
        &KdfParams {
            m_cost: KdfParams::M_COST_MIN,
            t_cost: KdfParams::T_COST_MIN,
            p_cost: 4,
        },
        Speichergrenze::UNBEKANNT,
        &mut OsRandom,
    )
    .expect("schreiben");
    let mut s = Sitzung::neu(datei, None, Sperrfrist::FuenfzehnMinuten);
    s.entsperren(
        &Zeroizing::new(PASSWORT.as_bytes().to_vec()),
        1_000,
        Speichergrenze::UNBEKANNT,
    )
    .expect("entsperren");
    (s, id)
}

fn nutzlast_von(id: &Identity) -> String {
    cabrik_core::trust::qr_payload(
        &id.enc_pub().expect("enc_pub"),
        id.sig_pub().as_ref(),
        Some(&id.xwing_pub()),
    )
}

fn fingerprint(s: &mut Sitzung, name: &str) -> String {
    s.offen(1_000)
        .expect("offen")
        .kontakte()
        .into_iter()
        .find(|k| k.name == name)
        .expect("Kontakt")
        .fingerprint
}

/// Ich und die Gegenseite, die einander kennen.
fn paar() -> (Sitzung, Sitzung, String) {
    let (mut ich, meine_id) = wer();
    let (mut gegen, gegen_id) = wer();
    gegen
        .offen(1_000)
        .expect("offen")
        .kontakt_aus_nutzlast("Ich", &nutzlast_von(&meine_id), 1_000)
        .expect("aufnehmen");
    ich.offen(1_000)
        .expect("offen")
        .kontakt_aus_nutzlast("Gegenseite", &nutzlast_von(&gegen_id), 1_000)
        .expect("aufnehmen");
    let fp = fingerprint(&mut ich, "Gegenseite");
    (ich, gegen, fp)
}

/// Die Ablage des ersten Durchgangs: nimmt und vergisst.
struct Verwerfer;

impl Ablage for Verwerfer {
    type Fehler = std::convert::Infallible;
    fn schreiben(&mut self, _: &[u8]) -> Result<(), Self::Fehler> {
        Ok(())
    }
    fn freigeben(&mut self, _: &Geprueft) -> Result<(), Self::Fehler> {
        Ok(())
    }
    fn verwerfen(&mut self) {}
}

#[test]
fn die_versiegelung_in_stuecken_ergibt_einen_lesbaren_envelope() {
    let (mut ich, mut gegen, fp) = paar();
    let klartext: Vec<u8> = (0..200_000_u32).map(|i| (i % 251) as u8).collect();

    let offen = ich.offen(1_000).expect("offen");
    let plan = offen.versand_planen(&[fp], true).expect("Plan");
    let (mut env, mut v) = offen
        .versiegelung_beginnen(&plan, "urlaub.mp4", klartext.len() as u64, &mut OsRandom)
        .expect("beginnen");
    for stueck in klartext.chunks(77_777) {
        v.weiter(stueck, &mut env).expect("weiter");
    }
    v.abschliessen(&mut env).expect("abschliessen");

    // Dieselbe Länge wie am Stück: Es ist derselbe Envelope, nur anders
    // entstanden.
    let am_stueck = offen
        .verschluesseln(&plan, "urlaub.mp4", &klartext, &mut OsRandom)
        .expect("am Stück");
    assert_eq!(env.len(), am_stueck.len());

    let auf = gegen.offen(1_000).expect("offen");
    auf.envelope_oeffnen(&env, true).expect("öffnen");
    let (inhalt, name) = auf.nutzlast().expect("Nutzlast");
    assert_eq!(inhalt, klartext.as_slice());
    assert_eq!(name, Some("urlaub.mp4"));
}

#[test]
fn eine_grosse_datei_liegt_nicht_bereit_und_wurde_nicht_angesehen() {
    let (mut ich, mut gegen, fp) = paar();
    let offen = ich.offen(1_000).expect("offen");
    let plan = offen.versand_planen(&[fp], true).expect("Plan");
    let env = offen
        .verschluesseln(&plan, "urlaub.mp4", b"Bilddaten", &mut OsRandom)
        .expect("verschluesseln");

    let auf = gegen.offen(1_000).expect("offen");
    let (mut o, verbraucht) = auf.oeffnung_beginnen(&env, false).expect("beginnen");
    o.weiter(env.get(verbraucht..).expect("Rest"), &mut Verwerfer)
        .expect("weiter");
    let bericht = o.abschliessen(&mut Verwerfer).expect("abschliessen");
    let fingerabdruck = bericht.fingerabdruck;

    let g = auf.gross_geoeffnet(bericht, "C:/x/urlaub.mp4.cabrik".to_owned(), false);

    assert!(!g.liegt_bereit);
    assert!(g.text.is_none());
    assert_eq!(g.dateiname.as_deref(), Some("urlaub.mp4"));
    assert_eq!(g.groesse_bytes, 9);
    assert!(
        matches!(g.metadaten, Some(Metadatenbefund::NichtAngesehen { .. })),
        "{:?}",
        g.metadaten
    );
    assert!(
        matches!(
            g.absender,
            Absender::Bekannt { .. } | Absender::Verifiziert { .. }
        ),
        "der Absender wurde nicht aufgelöst: {:?}",
        g.absender
    );

    // Kein Klartext — aber alles, was der zweite Durchgang braucht.
    assert!(auf.nutzlast().is_none());
    let n = auf.grosse_nutzlast().expect("große Nutzlast");
    assert_eq!(n.quelle, "C:/x/urlaub.mp4.cabrik");
    assert_eq!(n.fingerabdruck, fingerabdruck);
    assert_eq!(n.dateiname.as_deref(), Some("urlaub.mp4"));
    assert!(!n.signatur_verlangt);

    // Und es geht mit der Sperre, wie der Klartext auch.
    gegen.sperren();
    gegen
        .entsperren(
            &Zeroizing::new(PASSWORT.as_bytes().to_vec()),
            2_000,
            Speichergrenze::UNBEKANNT,
        )
        .expect("entsperren");
    assert!(
        gegen
            .offen(2_000)
            .expect("offen")
            .grosse_nutzlast()
            .is_none()
    );
}

#[test]
fn ein_fremder_envelope_scheitert_beim_beginnen_mit_denselben_worten() {
    // `AUTH_FAILED` und `NO_MATCHING_RECIPIENT` sagen dasselbe — am Stück
    // wie blockweise (`spec/test-vectors.md` §7).
    let (mut ich, _, fp) = paar();
    let (mut dritter, _) = wer();

    let offen = ich.offen(1_000).expect("offen");
    let plan = offen.versand_planen(&[fp], false).expect("Plan");
    let env = offen
        .verschluesseln(&plan, "x.bin", b"nicht fuer dich", &mut OsRandom)
        .expect("verschluesseln");

    let d = dritter.offen(1_000).expect("offen");
    let am_stueck = d.envelope_oeffnen(&env, false).expect_err("am Stück");
    let blockweise = d.oeffnung_beginnen(&env, false).expect_err("blockweise");
    assert_eq!(am_stueck.meldung, blockweise.meldung);
    assert!(
        blockweise.meldung.contains("nicht an Ihre"),
        "{}",
        blockweise.meldung
    );
}
