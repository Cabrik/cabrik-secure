//! Entsperren aus einem festgenagelten Puffer — ohne Umweg über `String`.
//!
//! # Worum es geht
//!
//! `spec/entsperrung.md` §5.2 stellt eine Entwurfsauflage: Die Entsperrung
//! wird so gebaut, dass die Webansicht **ein** Aufrufer ist und nicht
//! **der** Aufrufer. Ein natives Fenster später auszutauschen soll dann
//! eine Datei berühren und nicht das halbe Programm.
//!
//! Solange die Signaturen `&Zeroizing<String>` verlangten, war das eine
//! Absicht und keine Eigenschaft: Ein `String` liegt auf dem Haldenspeicher,
//! zieht beim Wachsen um und lässt sich nicht festnageln. Wer aus einem
//! gesicherten Puffer entsperren wollte, hätte vorher eine ungeschützte
//! Kopie anlegen müssen — genau die Kopie, die es zu vermeiden gilt.
//!
//! Seit die Signaturen `&[u8]` nehmen, geht es. Dieser Test hält das fest,
//! **bevor** es ein natives Fenster gibt. Ginge die Signatur je zurück auf
//! einen String, fiele es hier auf und nicht erst beim Bau des Fensters.
//!
//! # Was er nicht behauptet
//!
//! Dass damit alle Kopien verschwunden wären. Solange das Passwort durch
//! die Webansicht kommt, entstehen die JavaScript-Zeichenkette und der
//! Übergabepuffer weiterhin, und beide lassen sich nicht überschreiben
//! (§5.1). Dieser Test prüft die **Naht**, nicht den Weg davor.

#![expect(clippy::expect_used, reason = "Fehlschlag soll den Test abbrechen")]

use cabrik_app::Sitzung;
use cabrik_bruecke::{KdfStufe, Sperrfrist};
use cabrik_core::OsRandom;
use cabrik_speicher::Festgenagelt;

const PASSWORT: &str = "vier zufaellige woerter hier";

/// Wie es das native Fenster täte: Zeichen für Zeichen in den Puffer.
fn getippt(text: &str) -> Festgenagelt {
    let mut puffer = Festgenagelt::neu(512);
    for zeichen in text.chars() {
        let mut hilfe = [0_u8; 4];
        puffer
            .anhaengen(zeichen.encode_utf8(&mut hilfe))
            .expect("passt in 512 Bytes");
    }
    puffer
}

fn frisch(passwort: &Festgenagelt) -> Sitzung {
    Sitzung::anlegen(
        Some("Prüfling".to_owned()),
        passwort.als_bytes(),
        false,
        KdfStufe::Min,
        Sperrfrist::default(),
        1_000,
        &mut OsRandom,
    )
    .expect("anlegen")
}

#[test]
fn anlegen_und_entsperren_ohne_string_dazwischen() {
    let pw = getippt(PASSWORT);
    assert!(
        pw.ist_festgenagelt(),
        "der Puffer ist nicht festgenagelt -- dann prueft dieser Test nur die halbe Sache"
    );

    let mut s = frisch(&pw);
    s.sperren();
    assert!(s.ist_gesperrt());

    // Und wieder auf, aus einem zweiten, unabhaengig getippten Puffer --
    // nicht aus demselben. Sonst bewiese der Test nur, dass ein Wert mit
    // sich selbst uebereinstimmt.
    let nochmal = getippt(PASSWORT);
    s.entsperren(nochmal.als_bytes(), 2_000)
        .expect("mit demselben Passwort muss es aufgehen");
    assert!(!s.ist_gesperrt());
}

#[test]
fn ein_falsches_passwort_aus_dem_puffer_geht_nicht_auf() {
    // GEGENPROBE: Ohne sie bliebe offen, ob `entsperren` die Bytes
    // ueberhaupt ansieht.
    let pw = getippt(PASSWORT);
    let mut s = frisch(&pw);
    s.sperren();

    let falsch = getippt("vier ganz andere woerter hier");
    s.entsperren(falsch.als_bytes(), 2_000)
        .expect_err("ein falsches Passwort darf nicht aufgehen");
    assert!(
        s.ist_gesperrt(),
        "nach dem Fehlschlag muss gesperrt bleiben"
    );
}

#[test]
fn ein_rueckschritt_im_puffer_wirkt_sich_aus() {
    // Was das Fenster tut, wenn jemand sich vertippt und die Rücktaste
    // drückt. Der Puffer muss danach dasselbe liefern wie ein von Anfang an
    // richtig getippter -- sonst schluckt das Entsperren stillschweigend
    // Reste mit.
    let mut mit_tippfehler = getippt(PASSWORT);
    mit_tippfehler.anhaengen("x").expect("passt");
    mit_tippfehler.letztes_zeichen_loeschen();

    assert_eq!(
        mit_tippfehler.als_bytes(),
        getippt(PASSWORT).als_bytes(),
        "nach dem Rueckschritt steht etwas anderes im Puffer"
    );

    let mut s = frisch(&getippt(PASSWORT));
    s.sperren();
    s.entsperren(mit_tippfehler.als_bytes(), 2_000)
        .expect("der berichtigte Puffer muss aufgehen");
}

#[test]
fn das_passwort_aendern_geht_auch_mit_zwei_puffern() {
    // Der Fall, wegen dem die Puffer ganze Seiten fuer sich belegen: Hier
    // leben zwei gleichzeitig. Teilten sie sich eine Seite, naehme das
    // Wegwerfen des einen dem anderen den Schutz.
    let alt = getippt(PASSWORT);
    let neu = getippt("ganz andere vier woerter");
    let mut s = frisch(&alt);

    s.passwort_aendern(alt.als_bytes(), neu.als_bytes(), &mut OsRandom)
        .expect("wechseln");

    s.sperren();
    s.entsperren(alt.als_bytes(), 3_000)
        .expect_err("das alte Passwort darf nicht mehr aufgehen");
    s.entsperren(neu.als_bytes(), 3_000)
        .expect("das neue muss aufgehen");
}
