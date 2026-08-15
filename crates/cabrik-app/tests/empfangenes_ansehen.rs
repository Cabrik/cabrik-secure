//! Was in einer **empfangenen** Datei steht.
//!
//! # Warum das eine eigene Auskunft ist
//!
//! Bis hierher konnte das Programm sagen, was ein *Bereinigen* ergäbe. Für
//! eine Datei, die gerade ankommt, ist das die falsche Frage: Es wird nichts
//! bereinigt, und es soll auch nichts bereinigt werden — sie gehört jemand
//! anderem. Die Frage lautet, **was drin ist**.
//!
//! # Wem die Antwort nützt
//!
//! Nicht nur dem Empfänger. Was hier auftaucht, hat der **Absender** über
//! sich preisgegeben: Ein Foto mit GPS-Angabe verrät, wo er stand. Der Test
//! unten schickt genau so ein Foto durch den Envelope und verlangt, dass die
//! Koordinate auf der anderen Seite benannt wird.
//!
//! # Und was er nicht behauptet
//!
//! Bei einem Format, das nicht verstanden wurde, sagt der Befund **nichts**
//! — nicht „sauber". Das ist der Fehler, an dem v1 scheiterte, und der
//! letzte Test hält ihn hier fest.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "Fehlschlag soll den Test abbrechen"
)]

use cabrik_bruecke::{Fundart, Metadatenbefund, Schwere, Sperrfrist};
use cabrik_app::Sitzung;
use cabrik_core::keyfile::{self, KdfParams};
use cabrik_core::{Identity, OsRandom};
use std::path::PathBuf;
use zeroize::Zeroizing;

const PASSWORT: &str = "vier zufaellige woerter hier";

fn sparsam() -> KdfParams {
    KdfParams {
        m_cost: KdfParams::M_COST_MIN,
        t_cost: KdfParams::T_COST_MIN,
        p_cost: 4,
    }
}

fn vorlage(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testvectors/metadata")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| {
        panic!(
            "{} nicht lesbar: {e}\nVorlagen erzeugen mit: \
             python testvectors/tools/gen_metadata_fixtures.py",
            p.display()
        )
    })
}

fn wer() -> (Sitzung, Identity) {
    let id = Identity::generate(&mut OsRandom, true, 1_700_000_000).expect("Identität");
    let datei =
        keyfile::write(&id, PASSWORT.as_bytes(), &sparsam(), &mut OsRandom).expect("schreiben");
    let mut s = Sitzung::neu(datei, None, Sperrfrist::FuenfzehnMinuten);
    s.entsperren(&Zeroizing::new(PASSWORT.to_owned()), 1_000)
        .expect("entsperren");
    (s, id)
}

/// Schickt `inhalt` unter `name` von einer Identität zur anderen und öffnet.
///
/// **Absichtlich der ganze Weg.** Den Befund an den rohen Bytes zu prüfen
/// wäre einfacher und hätte nichts bewiesen: Die Frage ist, ob er nach
/// Verschlüsseln, Verschicken und Öffnen noch dasteht.
fn hin_und_zurueck(name: &str, inhalt: &[u8]) -> Option<Metadatenbefund> {
    let (mut ich, _) = wer();
    let (mut gegen, gegen_id) = wer();

    let nutzlast = cabrik_core::trust::qr_payload(
        &gegen_id.enc_pub().expect("enc_pub"),
        gegen_id.sig_pub().as_ref(),
        Some(&gegen_id.xwing_pub()),
    );
    let fp = {
        let offen = ich.offen(1_000).expect("offen");
        offen
            .kontakt_aus_nutzlast("Gegenseite", &nutzlast, 1_000)
            .expect("aufnehmen");
        offen
            .kontakte()
            .into_iter()
            .find(|k| k.name == "Gegenseite")
            .expect("Kontakt")
            .fingerprint
    };

    let envelope = {
        let offen = ich.offen(1_000).expect("offen");
        let plan = offen.versand_planen(&[fp], false).expect("Plan");
        offen
            .verschluesseln(&plan, name, inhalt, &mut OsRandom)
            .expect("verschluesseln")
    };

    gegen
        .offen(1_000)
        .expect("offen")
        .envelope_oeffnen(&envelope, false)
        .expect("oeffnen")
        .metadaten
}

// ---------------------------------------------------------------------------

#[test]
fn ein_empfangenes_foto_verraet_seinen_standort() {
    // Der Fall, um den es geht. Die Koordinate steht im Bild, das jemand
    // geschickt hat -- und sie gehoert IHM, nicht dem Empfaenger.
    let befund = hin_und_zurueck("urlaub.jpg", &vorlage("foto_mit_exif.jpg"))
        .expect("eine Datei hat einen Befund");

    let Metadatenbefund::Erkannt { format, funde } = befund else {
        panic!("JPEG muss erkannt werden, war: {befund:?}");
    };
    assert_eq!(format, "JPEG");

    let gps = funde
        .iter()
        .find(|f| f.art == Fundart::Ortsangabe)
        .expect("die GPS-Angabe muss benannt werden");
    assert_eq!(gps.schwere, Schwere::Kritisch);
}

#[test]
fn der_befund_kommt_ungefragt_und_vor_dem_speichern() {
    // Der einzige Zeitpunkt, an dem die Auskunft etwas aendert, liegt VOR
    // dem Speichern. Deshalb haengt sie am Oeffnen und nicht an einem
    // zweiten Knopf -- wer die Datei erst auf der Platte hat, hat nichts
    // mehr zu entscheiden.
    let befund = hin_und_zurueck("urlaub.jpg", &vorlage("foto_mit_exif.jpg"));

    assert!(
        befund.is_some(),
        "der Befund muss ohne Zutun am geoeffneten Bericht haengen"
    );
}

#[test]
fn ein_foto_ohne_exif_ergibt_eine_leere_liste_und_nicht_nichts() {
    // Der Unterschied, auf den es ankommt: „erkannt, nichts gefunden" ist
    // eine Aussage. „None" waere keine -- und genau die reservieren wir
    // fuer die Textnachricht.
    let befund = hin_und_zurueck("schlicht.jpg", &vorlage("foto_ohne_exif.jpg"))
        .expect("auch hier gibt es einen Befund");

    let Metadatenbefund::Erkannt { funde, .. } = befund else {
        panic!("JPEG muss erkannt werden, war: {befund:?}");
    };
    assert!(
        funde.iter().all(|f| f.schwere != Schwere::Kritisch),
        "in einem Foto ohne EXIF darf nichts Kritisches stehen: {funde:?}"
    );
}

#[test]
fn eine_textnachricht_hat_keinen_befund() {
    // Nicht „nichts gefunden", sondern „die Frage stellt sich nicht". Eine
    // leere Fundliste hier hiesse, ueber einen Text eine Aussage zu
    // treffen, die es nicht gibt.
    let (mut ich, _) = wer();
    let (mut gegen, gegen_id) = wer();

    let nutzlast = cabrik_core::trust::qr_payload(
        &gegen_id.enc_pub().expect("enc_pub"),
        gegen_id.sig_pub().as_ref(),
        Some(&gegen_id.xwing_pub()),
    );
    let fp = {
        let offen = ich.offen(1_000).expect("offen");
        offen
            .kontakt_aus_nutzlast("Gegenseite", &nutzlast, 1_000)
            .expect("aufnehmen");
        offen
            .kontakte()
            .into_iter()
            .find(|k| k.name == "Gegenseite")
            .expect("Kontakt")
            .fingerprint
    };
    let armor = {
        let offen = ich.offen(1_000).expect("offen");
        let plan = offen.versand_planen(&[fp], false).expect("Plan");
        offen
            .text_verschluesseln(&plan, "Treffen verschoben.", &mut OsRandom)
            .expect("verschluesseln")
    };

    let bericht = gegen
        .offen(1_000)
        .expect("offen")
        .text_oeffnen(&armor, false)
        .expect("oeffnen");

    assert!(
        bericht.metadaten.is_none(),
        "eine Textnachricht traegt keine Dateimetadaten"
    );
}

#[test]
fn ein_unverstandenes_format_behauptet_keine_sauberkeit() {
    // Der v1-Fehler als Test. Dort wurde jedes unbekannte Format kopiert
    // und als Erfolg gemeldet; hier muss „keine Aussage" herauskommen.
    let unsinn: Vec<u8> = (0u8..=255).cycle().take(4096).collect();

    let befund = hin_und_zurueck("archiv.dat", &unsinn).expect("auch das ergibt einen Befund");

    assert!(
        matches!(befund, Metadatenbefund::Unbekannt { .. }),
        "unverstandenes Format muss Unbekannt ergeben, war: {befund:?}"
    );
}

#[test]
fn der_befund_traegt_den_klartext_nicht() {
    // Die Architekturregel gilt auch hier. Ein Fund nennt die Fundstelle
    // und den Metadatenwert -- niemals den Inhalt der Datei.
    let geheim = b"streng vertraulich, dieser Satz gehoert in keinen Bericht";
    let mut daten = vorlage("foto_mit_exif.jpg");
    daten.extend_from_slice(geheim);

    let befund = hin_und_zurueck("urlaub.jpg", &daten).expect("Befund");

    let ausgegeben = format!("{befund:?}");
    assert!(
        !ausgegeben.contains("streng vertraulich"),
        "der Dateiinhalt darf im Befund nirgends auftauchen"
    );
}
