//! Die Meldung, dass das System gleich einschläft.
//!
//! # Warum das hierhergehört
//!
//! Weil es dieselbe Frage ist wie beim Festnageln: **Wie verhindern wir,
//! dass das Passwort auf die Platte kommt?**
//!
//! Festnageln beantwortet den einen Weg dorthin, die Auslagerung. Es
//! beantwortet den anderen ausdrücklich **nicht**: Das Ruhezustandsabbild
//! ist eine Kopie des physischen Arbeitsspeichers, und Festnageln
//! garantiert gerade, dass die Seite darin liegt. Wer entsperrt in den
//! Ruhezustand geht, schreibt sein Passwort neben das Keyfile — und damit
//! ist der Schutz aus `threat-model.md` A5 für dieses Gerät gegenstandslos.
//!
//! Dagegen hilft genau eines: **vorher sperren und überschreiben.** Dieses
//! Modul besorgt die Meldung, die das auslöst (`spec/entsperrung.md` §3.4).
//!
//! # Was es nicht zusagt
//!
//! Genug Zeit. Alle Systeme melden den bevorstehenden Wechsel, keines sagt
//! zu, wie lange es danach noch wartet. Überschreiben ist schnell — aber
//! ein leerer Akku, ein abrupter Stromausfall oder ein Deckel im
//! ungünstigsten Augenblick bleiben Fälle, in denen das Passwort im Abbild
//! landet.
//!
//! **Verbesserung des Regelfalls, keine Zusage.** Und wer die Frage
//! endgültig loswerden will, verschlüsselt den ganzen Datenträger.

/// Was das Betriebssystem gemeldet hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Meldung {
    /// Gleich geht es in Bereitschaft oder Ruhezustand.
    ///
    /// **Jetzt sperren.** Beides schreibt Speicherinhalte auf die Platte,
    /// und die beiden zu unterscheiden brächte nichts: Bereitschaft kann
    /// jederzeit in den Ruhezustand übergehen, ohne dass ein Programm
    /// davon noch etwas erfährt.
    LegtSichSchlafen,

    /// Das System ist wieder wach.
    ///
    /// Kein Anlass zu irgendetwas — hier steht kein „wieder entsperren".
    /// Wer aufwacht, gibt sein Passwort neu ein; das ist der ganze Sinn
    /// der Sache.
    WiederWach,

    /// Etwas anderes, das uns nichts angeht.
    ///
    /// Netzteil ein- oder ausgesteckt, Akku schwach, Bildschirm aus. Es
    /// wird ausdrücklich benannt statt verschwiegen: Wer die Meldungen
    /// später erweitert, sieht dann, wo sie ankommen.
    Belanglos,
}

/// Warum die Anmeldung nicht zustande kam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NichtAngemeldet {
    /// Was schiefging, in einem Satz und ohne Systemnummern.
    pub grund: String,
}

impl core::fmt::Display for NichtAngemeldet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.grund)
    }
}

impl core::error::Error for NichtAngemeldet {}

impl NichtAngemeldet {
    fn neu(grund: impl Into<String>) -> Self {
        Self {
            grund: grund.into(),
        }
    }
}

/// Wie das Betriebssystem eine Zahl in eine [`Meldung`] übersetzt.
///
/// **Der einzige Teil, der sich ohne Betriebssystem prüfen lässt** — und
/// deshalb der einzige, in dem eine Fehlentscheidung unbemerkt bliebe. Die
/// Zuordnung steht hier für sich, damit sie geprüft werden kann, ohne
/// einen Rechner schlafen zu legen.
#[cfg(windows)]
#[must_use]
const fn windows_meldung(art: u32) -> Meldung {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND,
    };
    match art {
        PBT_APMSUSPEND => Meldung::LegtSichSchlafen,
        PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => Meldung::WiederWach,
        _ => Meldung::Belanglos,
    }
}

/// Die laufende Anmeldung. Beim Wegwerfen wird sie zurückgenommen.
///
/// **Am Leben halten.** Wird sie fallengelassen, meldet das System nichts
/// mehr — und dann sperrt auch nichts mehr vor dem Einschlafen. Deshalb
/// trägt sie `#[must_use]`.
#[must_use = "wird sie sofort fallengelassen, meldet das System nichts mehr"]
pub struct Wacht {
    /// Wird nie gelesen — sie wird **gehalten**.
    ///
    /// Der Unterstrich sagt das: Der ganze Wert dieses Feldes liegt in
    /// seinem `Drop`. Solange es lebt, ist die Anmeldung beim System
    /// gültig; fällt es, wird sie zurückgenommen.
    #[cfg(windows)]
    _anmeldung: windows::Anmeldung,
    #[cfg(not(windows))]
    _anmeldung: (),
}

impl core::fmt::Debug for Wacht {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Wacht")
    }
}

/// Meldet sich beim Betriebssystem an.
///
/// Der Rückruf kommt auf einem **fremden Faden** und in einem Augenblick,
/// in dem das System nicht lange wartet: kurz halten, nichts sperren, was
/// blockieren könnte.
///
/// # Fehler
///
/// [`NichtAngemeldet`], wenn das System die Anmeldung ablehnt oder es auf
/// diesem System keinen Weg dafür gibt. **Der Aufrufer muss das anzeigen
/// und darf nicht behaupten, es werde vor dem Einschlafen gesperrt.**
#[cfg(windows)]
pub fn anmelden<F>(rueckruf: F) -> Result<Wacht, NichtAngemeldet>
where
    F: Fn(Meldung) + Send + Sync + 'static,
{
    windows::anmelden(rueckruf).map(|a| Wacht { _anmeldung: a })
}

/// Auf diesem System gibt es (noch) keinen Weg dafür.
///
/// Kein stilles Nichtstun: Der Aufrufer bekommt einen Fehler und muss
/// entscheiden, was er dem Nutzer sagt.
#[cfg(not(windows))]
pub fn anmelden<F>(_rueckruf: F) -> Result<Wacht, NichtAngemeldet>
where
    F: Fn(Meldung) + Send + Sync + 'static,
{
    Err(NichtAngemeldet::neu(
        "Auf diesem Betriebssystem wird vor dem Ruhezustand noch nicht gesperrt.",
    ))
}

// ---------------------------------------------------------------------------
// Windows
//
// Die zweite Stelle mit `unsafe` in dieser Kiste. Anders als beim
// Festnageln geht es hier nicht um Zeiger auf eigenen Speicher, sondern um
// einen Rückruf, den das Betriebssystem aufruft — mit allem, was daran
// hängt: Lebensdauer, Faden, Rücknahme.
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod windows {
    use super::{Meldung, NichtAngemeldet, windows_meldung};
    use core::ffi::c_void;
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Power::{
        DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, HPOWERNOTIFY, PowerRegisterSuspendResumeNotification,
        PowerUnregisterSuspendResumeNotification,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::DEVICE_NOTIFY_CALLBACK;

    /// Was der Rückruf zurückgeben muss, wenn er fertig ist.
    const ERFOLG: u32 = 0;

    type Empfaenger = Box<dyn Fn(Meldung) + Send + Sync + 'static>;

    pub(super) struct Anmeldung {
        /// Das Handle, das die Anmeldung zurücknimmt.
        handle: *mut c_void,
        /// Die Angaben, die dem System übergeben wurden.
        ///
        /// Sie bleiben in einer `Box` liegen, statt auf dem Stapel zu
        /// stehen: Ob Windows die Struktur kopiert oder sich den Zeiger
        /// merkt, ist nicht zugesichert. Eine Adresse, die bis zur
        /// Rücknahme gültig bleibt, kostet nichts und beantwortet die
        /// Frage.
        _angaben: Box<DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS>,
        /// Der Rückruf selbst, als roher Zeiger.
        ///
        /// Er wird in [`Drop`] zurückgeholt und aufgeräumt — **erst nach**
        /// der Rücknahme, denn bis dahin kann das System ihn noch
        /// aufrufen.
        empfaenger: *mut Empfaenger,
    }

    // Beide Zeiger gehören dieser Struktur allein; das System ruft nur den
    // Rückruf auf, und der ist `Send + Sync`.
    #[allow(unsafe_code)]
    unsafe impl Send for Anmeldung {}
    #[allow(unsafe_code)]
    unsafe impl Sync for Anmeldung {}

    /// Was Windows aufruft. Läuft auf einem fremden Faden.
    ///
    /// # Sicherheit
    ///
    /// `kontext` ist der Zeiger, den wir bei der Anmeldung übergeben
    /// haben, und das System reicht ihn unverändert zurück. Er bleibt
    /// gültig, bis die Anmeldung zurückgenommen ist; das Aufräumen
    /// geschieht erst danach.
    #[allow(unsafe_code)]
    unsafe extern "system" fn rueckruf(
        kontext: *const c_void,
        art: u32,
        _einstellung: *const c_void,
    ) -> u32 {
        if kontext.is_null() {
            return ERFOLG;
        }
        // SICHERHEIT: siehe oben -- der Zeiger stammt aus unserem eigenen
        // `Box::into_raw` und lebt laenger als jede Zustellung.
        #[allow(unsafe_code)]
        let empfaenger = unsafe { &*kontext.cast::<Empfaenger>() };

        // Ein fremder Ruecksprung darf nicht in Panik enden: Ueber eine
        // `extern "system"`-Grenze zu entrollen ist nicht erlaubt.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            empfaenger(windows_meldung(art));
        }));
        ERFOLG
    }

    pub(super) fn anmelden<F>(f: F) -> Result<Anmeldung, NichtAngemeldet>
    where
        F: Fn(Meldung) + Send + Sync + 'static,
    {
        let empfaenger: *mut Empfaenger = Box::into_raw(Box::new(Box::new(f)));

        let mut angaben = Box::new(DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
            Callback: Some(rueckruf),
            Context: empfaenger.cast::<c_void>(),
        });

        let mut handle: *mut c_void = core::ptr::null_mut();
        // SICHERHEIT: `angaben` liegt auf der Halde und bleibt bis zur
        // Ruecknahme am Leben; `handle` ist ein gueltiger Ausgabezeiger.
        // Die Funktion liest die Angaben und schreibt das Handle.
        #[allow(unsafe_code)]
        let fehler = unsafe {
            PowerRegisterSuspendResumeNotification(
                DEVICE_NOTIFY_CALLBACK,
                core::ptr::from_mut(angaben.as_mut()).cast::<c_void>(),
                &raw mut handle,
            )
        };

        if fehler != ERROR_SUCCESS || handle.is_null() {
            // Der Rueckruf wurde nie uebergeben -- also hier aufraeumen,
            // sonst bliebe er fuer immer liegen.
            //
            // SICHERHEIT: Der Zeiger stammt unmittelbar aus dem
            // `Box::into_raw` oben und wurde seither nicht weitergegeben.
            #[allow(unsafe_code)]
            drop(unsafe { Box::from_raw(empfaenger) });
            return Err(NichtAngemeldet::neu(
                "Windows hat die Anmeldung für den Ruhezustand abgelehnt.",
            ));
        }

        Ok(Anmeldung {
            handle,
            _angaben: angaben,
            empfaenger,
        })
    }

    impl Drop for Anmeldung {
        /// Erst abmelden, dann aufräumen — nicht umgekehrt.
        ///
        /// Andersherum gäbe es einen Augenblick, in dem das System einen
        /// Rückruf zustellen darf, dessen Empfänger schon weg ist.
        fn drop(&mut self) {
            // SICHERHEIT: `handle` stammt aus einer geglueckten Anmeldung
            // und wird genau einmal zurueckgenommen.
            #[allow(unsafe_code)]
            unsafe {
                // `HPOWERNOTIFY` ist eine Zahl, kein Zeiger -- die
                // Anmeldung gibt sie ueber einen `*mut *mut c_void`
                // heraus, die Ruecknahme nimmt sie als `isize`.
                PowerUnregisterSuspendResumeNotification(self.handle as HPOWERNOTIFY);
            }
            // SICHERHEIT: Ab hier stellt das System nichts mehr zu. Der
            // Zeiger stammt aus `Box::into_raw` und wird genau einmal
            // zurueckgeholt.
            #[allow(unsafe_code)]
            drop(unsafe { Box::from_raw(self.empfaenger) });
        }
    }
}

#[cfg(test)]
#[expect(clippy::expect_used, reason = "Fehlschlag soll den Test abbrechen")]
mod pruefungen {
    #[cfg(windows)]
    use super::Meldung;
    #[cfg(not(windows))]
    use super::NichtAngemeldet;
    use super::anmelden;

    #[cfg(windows)]
    #[test]
    fn die_zuordnung_der_windows_meldungen() {
        use super::windows_meldung;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            PBT_APMBATTERYLOW, PBT_APMPOWERSTATUSCHANGE, PBT_APMRESUMEAUTOMATIC,
            PBT_APMRESUMESUSPEND, PBT_APMSUSPEND,
        };

        // Der eine Fall, auf den es ankommt.
        assert_eq!(windows_meldung(PBT_APMSUSPEND), Meldung::LegtSichSchlafen);

        // Zwei Arten aufzuwachen -- Windows schickt je nach Anlass die eine
        // oder die andere.
        assert_eq!(windows_meldung(PBT_APMRESUMEAUTOMATIC), Meldung::WiederWach);
        assert_eq!(windows_meldung(PBT_APMRESUMESUSPEND), Meldung::WiederWach);

        // GEGENPROBE: Was NICHT sperren darf. Ein leerer Akku ist kein
        // Grund, jemanden auszusperren, und ein gewechseltes Netzteil erst
        // recht nicht -- beides kaeme sonst mitten im Arbeiten.
        assert_eq!(windows_meldung(PBT_APMBATTERYLOW), Meldung::Belanglos);
        assert_eq!(
            windows_meldung(PBT_APMPOWERSTATUSCHANGE),
            Meldung::Belanglos
        );

        // Und alles Unbekannte ist belanglos, nicht etwa ein Anlass zu
        // sperren. Eine kuenftige Windows-Fassung darf neue Zahlen
        // schicken, ohne dass jemand mitten im Satz gesperrt wird.
        for unbekannt in [42_u32, 99, 1000, u32::MAX] {
            assert_eq!(
                windows_meldung(unbekannt),
                Meldung::Belanglos,
                "unbekannte Meldung {unbekannt} wurde gedeutet"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn anmelden_und_wieder_abmelden() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Was hier NICHT geprueft wird: dass der Rueckruf kommt. Dafuer
        // muesste der Rechner tatsaechlich einschlafen, und das ist keine
        // Zumutung, die ein Test sich herausnehmen darf. Geprueft wird,
        // dass Windows die Anmeldung annimmt und die Ruecknahme nicht
        // abstuerzt -- alles Weitere haengt am Betriebssystem.
        let zaehler = Arc::new(AtomicUsize::new(0));
        let mit = Arc::clone(&zaehler);

        let wacht = anmelden(move |_| {
            mit.fetch_add(1, Ordering::SeqCst);
        })
        .expect("Windows muss die Anmeldung annehmen");

        drop(wacht);

        // Zweimal hintereinander muss ebenso gehen: Beim Sperren und
        // Wiederentsperren entsteht die Wacht neu.
        let wacht = anmelden(|_| {}).expect("auch beim zweiten Mal");
        drop(wacht);
    }

    #[cfg(not(windows))]
    #[test]
    fn ohne_umsetzung_wird_das_auch_gesagt() {
        // Kein stilles `Ok(())`. Wer nicht sperren kann, muss das melden --
        // sonst zeigt die Oberflaeche eine Zusage an, die niemand einloest.
        let ergebnis = anmelden(|_| {});
        let fehler: NichtAngemeldet = ergebnis.expect_err("darf nicht gelingen");
        assert!(!fehler.grund.is_empty(), "ein Fehler ohne Begruendung");
    }
}
