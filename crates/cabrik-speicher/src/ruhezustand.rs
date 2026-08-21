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
    #[cfg(target_os = "linux")]
    _anmeldung: linux::Anmeldung,
    #[cfg(not(any(windows, target_os = "linux")))]
    _anmeldung: (),
}

impl Wacht {
    /// Ob das System uns Zeit zugesteht, bevor es einschläft.
    ///
    /// **Der Unterschied zwischen „wir werden gewarnt" und „wir kommen
    /// noch dazu".** Eine Warnung ohne Aufschub nützt wenig: Das
    /// Überschreiben liefe gegen ein System, das schon wegdämmert.
    ///
    /// * **Windows:** immer `true`. Das System wartet auf die Rückkehr des
    ///   Rückrufs — nicht unbegrenzt, aber es wartet.
    /// * **Linux:** ob logind die Verzögerungssperre gewährt hat. Sie kann
    ///   an einer Polkit-Regel scheitern; dann wird zwar weiterhin
    ///   gemeldet, aber ohne zugesicherte Zeit.
    ///
    /// Steht hier `false`, ist das **keine** Zusage, dass nichts
    /// überschrieben wird — nur, dass niemand dafür geradesteht. Die
    /// Spezifikation nennt den ganzen Punkt ohnehin eine Verbesserung des
    /// Regelfalls und keine Zusage (`spec/entsperrung.md` §3.4).
    #[must_use]
    pub const fn hat_aufschub(&self) -> bool {
        #[cfg(windows)]
        {
            true
        }
        #[cfg(target_os = "linux")]
        {
            self._anmeldung.aufschub
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            false
        }
    }
}

impl core::fmt::Debug for Wacht {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Wacht")
            .field("hat_aufschub", &self.hat_aufschub())
            .finish()
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

/// Meldet sich bei logind an.
///
/// # Fehler
///
/// [`NichtAngemeldet`], wenn der Systembus nicht erreichbar ist oder
/// logind nicht antwortet — etwa in einem Behälter ohne systemd.
#[cfg(target_os = "linux")]
pub fn anmelden<F>(rueckruf: F) -> Result<Wacht, NichtAngemeldet>
where
    F: Fn(Meldung) + Send + Sync + 'static,
{
    linux::anmelden(rueckruf).map(|a| Wacht { _anmeldung: a })
}

/// Auf diesem System gibt es (noch) keinen Weg dafür.
///
/// Kein stilles Nichtstun: Der Aufrufer bekommt einen Fehler und muss
/// entscheiden, was er dem Nutzer sagt.
#[cfg(not(any(windows, target_os = "linux")))]
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

/// Wie logind seinen Wahrheitswert in eine [`Meldung`] übersetzt.
///
/// `PrepareForSleep` trägt genau ein Argument: `true` heißt „gleich geht
/// es schlafen", `false` heißt „wieder da". Zwei Zeilen — die aber
/// vertauscht zu haben hieße, beim Aufwachen zu sperren und beim
/// Einschlafen nicht. Deshalb steht die Zuordnung für sich und wird
/// geprüft, genau wie die von Windows.
#[cfg(target_os = "linux")]
#[must_use]
const fn linux_meldung(schlafen_gleich: bool) -> Meldung {
    if schlafen_gleich {
        Meldung::LegtSichSchlafen
    } else {
        Meldung::WiederWach
    }
}

// ---------------------------------------------------------------------------
// Linux
//
// Der einzige der drei Wege ganz OHNE `unsafe`: logind meldet den
// bevorstehenden Wechsel über D-Bus, und `zbus` liegt ohnehin schon im
// Baum — `tauri-plugin-single-instance` zieht es unter Linux herein, in
// derselben Fassung und mit denselben Merkmalen.
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod linux {
    use super::{Meldung, NichtAngemeldet, linux_meldung};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    const DIENST: &str = "org.freedesktop.login1";
    const PFAD: &str = "/org/freedesktop/login1";
    const SCHNITTSTELLE: &str = "org.freedesktop.login1.Manager";

    pub(super) struct Anmeldung {
        /// Sagt dem Faden, dass Schluss ist.
        ///
        /// **Er endet trotzdem nicht sofort.** Der Faden hängt blockierend
        /// über den Signalen, und ein Wahrheitswert reißt ihn dort nicht
        /// heraus — gelesen wird er erst, wenn das nächste Signal kommt.
        /// In der Praxis heißt das: Der Faden lebt bis zum Prozessende.
        ///
        /// Vertretbar ist das, weil es genau **eine** Wacht gibt und sie
        /// so lange lebt wie das Fenster. Es steht hier trotzdem, weil
        /// eine Kiste, die Fäden hinterlässt, das sagen muss.
        ende: Arc<AtomicBool>,
        /// Ob logind uns Aufschub zugestanden hat.
        pub(super) aufschub: bool,
    }

    impl Drop for Anmeldung {
        fn drop(&mut self) {
            self.ende.store(true, Ordering::SeqCst);
        }
    }

    fn nicht(was: &str, fehler: &zbus::Error) -> NichtAngemeldet {
        NichtAngemeldet::neu(format!("{was}: {fehler}"))
    }

    /// Bittet logind um Aufschub vor dem Einschlafen.
    ///
    /// Ohne diese Sperre meldet logind den Wechsel zwar, wartet aber
    /// nicht — das Überschreiben liefe gegen ein System, das schon
    /// wegdämmert. Mit `delay` bleiben bis zu `InhibitDelayMaxSec`
    /// Sekunden, voreingestellt fünf.
    ///
    /// **Nach bestem Vermögen.** Sie kann an einer Polkit-Regel
    /// scheitern. Dann wird trotzdem gemeldet, nur ohne zugesicherte
    /// Zeit — und das ist immer noch besser als gar nicht. Zeit sagt
    /// `spec/entsperrung.md` §3.4 an dieser Stelle ohnehin keine zu.
    fn verzoegerungssperre(proxy: &zbus::blocking::Proxy<'_>) -> Option<zbus::zvariant::OwnedFd> {
        proxy
            .call(
                "Inhibit",
                &(
                    "sleep",
                    "Cabrik Secure",
                    "Das Passwort wird überschrieben, bevor der Speicher auf die Platte geht",
                    "delay",
                ),
            )
            .ok()
    }

    pub(super) fn anmelden<F>(rueckruf: F) -> Result<Anmeldung, NichtAngemeldet>
    where
        F: Fn(Meldung) + Send + Sync + 'static,
    {
        // Verbindung und Anmeldung SOFORT, nicht erst im Faden.
        //
        // Nur so kann `anmelden` ehrlich melden, ob es geklappt hat. Wer
        // das in den Faden schöbe, gäbe dem Aufrufer ein `Ok` zurück und
        // erführe erst Sekunden später, dass es kein D-Bus gibt -- und
        // dann gäbe es niemanden mehr, dem man es sagen könnte.
        let verbindung = zbus::blocking::Connection::system()
            .map_err(|e| nicht("Der Systembus ist nicht erreichbar", &e))?;

        let proxy = zbus::blocking::Proxy::new(&verbindung, DIENST, PFAD, SCHNITTSTELLE)
            .map_err(|e| nicht("logind antwortet nicht", &e))?;

        let signale = proxy
            .receive_signal("PrepareForSleep")
            .map_err(|e| nicht("logind meldet den Ruhezustand nicht", &e))?;

        // Die Verzoegerungssperre HIER greifen, nicht erst im Faden.
        //
        // Sie ist der Teil, der die Zeit zum Ueberschreiben verschafft --
        // und sie kann an einer Polkit-Regel scheitern. Griffe sie erst im
        // Faden, erfuehre der Aufrufer nie, ob er Aufschub hat oder nicht,
        // und koennte es folglich auch nicht sagen.
        let erste_sperre = verzoegerungssperre(&proxy);
        let aufschub = erste_sperre.is_some();

        let ende = Arc::new(AtomicBool::new(false));
        let im_faden = Arc::clone(&ende);

        std::thread::Builder::new()
            .name("cabrik-ruhezustand".to_owned())
            .spawn(move || {
                let mut sperre = erste_sperre;
                for nachricht in signale {
                    if im_faden.load(Ordering::SeqCst) {
                        break;
                    }
                    // Ein Signal, das sich nicht lesen lässt, wird
                    // übergangen und nicht geraten. Raten hieße hier:
                    // vielleicht beim Aufwachen sperren.
                    let Ok(schlafen_gleich) = nachricht.body().deserialize::<bool>() else {
                        continue;
                    };

                    rueckruf(linux_meldung(schlafen_gleich));

                    if schlafen_gleich {
                        // ERST JETZT loslassen. Solange die Sperre gehalten
                        // wird, wartet logind -- und genau diese Zeit
                        // brauchte das Überschreiben eine Zeile höher.
                        drop(sperre.take());
                    } else {
                        // Wieder wach: neu greifen, sonst gilt sie beim
                        // nächsten Einschlafen nicht mehr.
                        sperre = verzoegerungssperre(&proxy);
                    }
                }
            })
            .map_err(|e| NichtAngemeldet::neu(format!("Kein Faden für die Ruhewacht: {e}")))?;

        Ok(Anmeldung { ende, aufschub })
    }
}

#[cfg(test)]
mod pruefungen {
    #[cfg(any(windows, target_os = "linux"))]
    use super::Meldung;
    #[cfg(not(any(windows, target_os = "linux")))]
    use super::NichtAngemeldet;
    use super::anmelden;

    /// Was logind schickt, in beide Richtungen.
    ///
    /// Zwei Zeilen Code und trotzdem ein eigener Test: Vertauscht hieße
    /// das, beim **Aufwachen** zu sperren und beim Einschlafen nicht —
    /// also den Nutzer zu ärgern und ihn gleichzeitig ungeschützt zu
    /// lassen. Im Alltag wäre das kaum von „Frist abgelaufen" zu
    /// unterscheiden, und niemand käme auf die Idee, hier zu suchen.
    /// Der ganze Weg über D-Bus, so weit er sich hier prüfen lässt.
    ///
    /// **Was er nicht beweist:** dass gesperrt wird. Dafür müsste ein
    /// Rechner tatsächlich einschlafen. Was er beweist, ist trotzdem
    /// nicht nichts — und es ist genau das, was auf einem Läufer
    /// schiefgehen kann:
    ///
    /// * Der Weg endet, statt zu hängen. Ein blockierender D-Bus-Aufruf
    ///   ohne Bus wäre ein Aufhänger beim Programmstart.
    /// * Er endet ohne Absturz. In einem Behälter ohne systemd gibt es
    ///   weder Systembus noch logind, und das ist der Normalfall, nicht
    ///   der Ausnahmefall.
    /// * Und schlägt er fehl, sagt er warum.
    ///
    /// Beide Ausgänge sind erlaubt, weil beide richtig sind: Auf einem
    /// Läufer mit systemd gelingt die Anmeldung, in einem Behälter nicht.
    /// Einen davon zu verlangen hieße, die Umgebung zu prüfen statt den
    /// Code.
    #[cfg(target_os = "linux")]
    #[test]
    fn anmelden_endet_auch_ohne_logind_und_sagt_warum() {
        // Der Ausgang wird GEMELDET, nicht nur geduldet.
        //
        // Ohne diese Zeilen bestuende der Test in beiden Faellen, und
        // niemand wuesste, welcher eingetreten ist -- die Spezifikation
        // koennte dann nicht sagen, ob die Anmeldung je gegen ein echtes
        // logind gelaufen ist. Sichtbar wird es mit `--nocapture`; die
        // Fortlaufpruefung ruft den Test eigens so auf.
        match anmelden(|_| {}) {
            Ok(wacht) => {
                println!(
                    "RUHEWACHT: angemeldet -- es gibt hier ein logind; Aufschub: {}",
                    if wacht.hat_aufschub() {
                        "ja, die Verzoegerungssperre wurde gewaehrt"
                    } else {
                        "NEIN -- logind meldet, wartet aber nicht"
                    }
                );
                drop(wacht);
            }
            Err(fehler) => {
                println!("RUHEWACHT: nicht angemeldet -- {}", fehler.grund);
                assert!(
                    !fehler.grund.is_empty(),
                    "ein Fehler ohne Begruendung -- der Aufrufer kann dem Nutzer nichts sagen"
                );
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn die_zuordnung_der_logind_meldung() {
        use super::linux_meldung;

        assert_eq!(linux_meldung(true), Meldung::LegtSichSchlafen);
        assert_eq!(linux_meldung(false), Meldung::WiederWach);

        // `Belanglos` kommt hier nie vor: `PrepareForSleep` trägt genau
        // einen Wahrheitswert, es gibt keinen dritten Fall. Stünde hier je
        // etwas anderes, wäre die Zuordnung geraten statt gelesen.
        assert_ne!(linux_meldung(true), Meldung::Belanglos);
        assert_ne!(linux_meldung(false), Meldung::Belanglos);
    }

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
    #[expect(clippy::expect_used, reason = "Fehlschlag soll den Test abbrechen")]
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

    #[cfg(windows)]
    #[test]
    #[expect(clippy::expect_used, reason = "Fehlschlag soll den Test abbrechen")]
    fn unter_windows_gibt_es_immer_aufschub() {
        // Windows wartet auf die Rueckkehr des Rueckrufs -- nicht
        // unbegrenzt, aber es wartet. Das ist der Unterschied zu Linux,
        // wo die Verzoegerungssperre an einer Polkit-Regel scheitern kann
        // und `hat_aufschub()` dann `false` meldet.
        //
        // Der Test haelt fest, dass die beiden Systeme hier verschieden
        // sind und nicht versehentlich gleich behandelt werden.
        let wacht = anmelden(|_| {}).expect("Anmeldung");
        assert!(wacht.hat_aufschub());
        assert!(
            format!("{wacht:?}").contains("hat_aufschub"),
            "die Auskunft soll auch im Debug stehen"
        );
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    #[test]
    #[expect(clippy::expect_used, reason = "Fehlschlag soll den Test abbrechen")]
    fn ohne_umsetzung_wird_das_auch_gesagt() {
        // Kein stilles `Ok(())`. Wer nicht sperren kann, muss das melden --
        // sonst zeigt die Oberflaeche eine Zusage an, die niemand einloest.
        let ergebnis = anmelden(|_| {});
        let fehler: NichtAngemeldet = ergebnis.expect_err("darf nicht gelingen");
        assert!(!fehler.grund.is_empty(), "ein Fehler ohne Begruendung");
    }
}
