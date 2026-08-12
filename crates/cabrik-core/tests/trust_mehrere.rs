//! Der Trust Store mit **mehreren** Kontakten.
//!
//! `trust_flow.rs` prüft die Zustände eines einzelnen Kontakts gründlich —
//! Verifikation, Schlüsselwechsel, Widerruf, ausgemusterte Schlüssel. Was
//! dort fehlte, ist der Alltag: ein Verzeichnis mit mehreren Einträgen, aus
//! dem einer entfernt wird, während die übrigen weiter auffindbar bleiben
//! müssen.
//!
//! Diese Datei entstand beim Prüfdurchgang vor der CI. Sie hat **keinen
//! Fehler gefunden** — sie hält fest, was ohnehin schon stimmte. Genau das
//! ist ihr Zweck: Eine Eigenschaft ohne Test ist eine Eigenschaft auf Zeit.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use cabrik_core::envelope::Signer;
use cabrik_core::trust::{Contact, TrustStore, VerifiedVia};

fn kontakt(name: &str, kennung: u8) -> Contact {
    Contact::new_seen(
        name,
        [kennung; 32],
        Some([kennung; 32]),
        None,
        1_700_000_000,
    )
    .unwrap()
}

fn store() -> TrustStore {
    let mut s = TrustStore::default();
    for (i, n) in ["Anna", "Bert", "Cora"].iter().enumerate() {
        s.add(kontakt(n, u8::try_from(i + 1).unwrap())).unwrap();
    }
    s
}

/// **Die wichtigste Eigenschaft dieser Datei.**
///
/// Zwei Kontakte dürfen nicht denselben Signierschlüssel führen. Ohne diese
/// Sperre könnte jemand den Schlüssel eines Dritten unter eigenem Namen
/// eintragen — und ab da löste jede Nachricht dieses Dritten auf den falschen
/// Namen auf. Der Trust Store wäre damit als Zuordnung wertlos, und zwar
/// **still**: Es sähe alles richtig aus.
#[test]
fn derselbe_signierschluessel_darf_nicht_zweimal_vorkommen() {
    let mut s = store();
    let doppelt = s.add(kontakt("Anna Zweitgerät", 1));

    assert!(
        doppelt.is_err(),
        "ein zweiter Kontakt mit Annas Signierschlüssel wurde angenommen"
    );
    assert_eq!(
        s.len(),
        3,
        "der abgelehnte Kontakt hat es doch hineingeschafft"
    );

    // Und die Zuordnung zeigt weiterhin auf den Richtigen.
    assert_eq!(
        s.find_by_sig_pub(&[1u8; 32]).map(|c| c.name.as_str()),
        Some("Anna")
    );
}

#[test]
fn unter_mehreren_wird_der_richtige_gefunden() {
    let s = store();
    for (kennung, name) in [(1u8, "Anna"), (2, "Bert"), (3, "Cora")] {
        assert_eq!(
            s.find_by_sig_pub(&[kennung; 32]).map(|c| c.name.as_str()),
            Some(name),
            "Kennung {kennung} zeigt auf den falschen Kontakt"
        );
    }
    assert!(s.find_by_sig_pub(&[9u8; 32]).is_none());
}

/// Nach dem Entfernen rutschen die Indizes. Die Suche nach dem Schlüssel
/// darf davon nichts merken — sie ist der Weg, den das Programm wirklich
/// benutzt.
#[test]
fn nach_dem_entfernen_bleiben_die_uebrigen_auffindbar() {
    let mut s = store();
    let weg = s.remove(0).unwrap();

    assert_eq!(weg.name, "Anna");
    assert_eq!(s.len(), 2);
    assert!(
        s.find_by_sig_pub(&[1u8; 32]).is_none(),
        "der entfernte Kontakt ist noch auffindbar"
    );
    assert_eq!(
        s.find_by_sig_pub(&[3u8; 32]).map(|c| c.name.as_str()),
        Some("Cora"),
        "nach dem Verrutschen zeigt die Suche ins Leere"
    );

    // Und derselbe Schlüssel lässt sich danach wieder eintragen.
    assert!(s.add(kontakt("Anna, neu", 1)).is_ok());
}

#[test]
fn ein_index_jenseits_des_endes_ist_ein_fehler_kein_absturz() {
    let mut s = store();
    assert!(s.remove(99).is_err());
    assert!(
        s.remove(3).is_err(),
        "drei Kontakte haben die Indizes 0 bis 2"
    );
    assert_eq!(
        s.len(),
        3,
        "ein fehlgeschlagenes Entfernen hat etwas verändert"
    );
}

/// Der ganze Weg: unbekannt → bekannt → verifiziert, mit zwei anderen
/// Kontakten daneben, die sich dabei nicht verändern dürfen.
#[test]
fn der_weg_vom_unbekannten_zum_verifizierten_kontakt() {
    let mut s = store();

    // Ein Signierer, den niemand kennt.
    let fremd = s.resolve(&Signer::Key([9u8; 32]));
    assert!(
        matches!(
            fremd,
            cabrik_core::trust::Authenticity::SignedUnknown { .. }
        ),
        "{fremd:?}"
    );

    // Bert ist bekannt, aber nicht verifiziert.
    let bert_sig = s.contacts()[1].sig_pub.unwrap();
    assert!(
        !matches!(
            s.resolve(&Signer::Key(bert_sig)),
            cabrik_core::trust::Authenticity::SignedVerified { .. }
        ),
        "bekannt ist noch nicht verifiziert"
    );

    // Nach der Verifikation über die Safety Number.
    s.contacts_mut()[1]
        .verify(VerifiedVia::SafetyNumber, 1_700_000_100)
        .unwrap();
    match s.resolve(&Signer::Key(bert_sig)) {
        cabrik_core::trust::Authenticity::SignedVerified {
            name, verified_at, ..
        } => {
            assert_eq!(name, "Bert");
            assert_eq!(verified_at, Some(1_700_000_100));
        }
        anderes => panic!("erwartet wurde SignedVerified, bekam {anderes:?}"),
    }

    // Die anderen beiden sind davon unberührt geblieben.
    for i in [0usize, 2] {
        assert!(
            s.contacts()[i].verified_at.is_none(),
            "Kontakt {i} wurde mitverifiziert"
        );
    }
}
