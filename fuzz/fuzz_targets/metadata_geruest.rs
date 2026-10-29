//! Das Gerüst gegen die ganze Datei (`spec/grosse-dateien.md` §5.4).
//!
//! Nimmt das Gerüst an, müssen seine Flicken Byte für Byte das ergeben,
//! was `strip` über die ganze Datei liefert — mit demselben Ergebnis. Eine
//! Ablehnung ist immer erlaubt; ein Drittes nicht.
#![no_main]

use std::io::Cursor;

use cabrik_metadata::{Format, flicken_anwenden, strip, ueber_geruest};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|daten: &[u8]| {
    let Some(format) = Format::detect(daten)
        .filter(|f| matches!(f, Format::Video | Format::Matroska | Format::Avi))
    else {
        return;
    };
    let Ok(Some(b)) = ueber_geruest(&mut Cursor::new(daten), daten.len() as u64, format) else {
        return;
    };
    let (sauber, ergebnis) =
        strip(daten).expect("das Gerüst nahm an, die ganze Datei scheitert");
    let mut geflickt = daten.to_vec();
    flicken_anwenden(0, &mut geflickt, &b.flicken);
    assert!(geflickt == sauber, "andere Bytes als strip über die ganze Datei");
    assert_eq!(b.ergebnis, ergebnis, "anderes Ergebnis");
});
