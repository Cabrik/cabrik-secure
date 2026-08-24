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

/// Die laufende Anmeldung.
///
/// **Am Leben halten.** Deshalb trägt sie `#[must_use]`.
///
/// # Was das Wegwerfen bewirkt — und wo nicht
///
/// | System | Beim Wegwerfen |
/// |---|---|
/// | Windows | Die Anmeldung wird zurückgenommen, sofort |
/// | Linux | Der Faden endet, sobald das nächste Signal kommt |
/// | macOS | **Nichts.** Die Anmeldung bleibt bis zum Prozessende |
///
/// Die Unterschiede stehen hier, statt in einem gemeinsamen Satz zu
/// verschwinden. „Beim Wegwerfen wird sie zurückgenommen" wäre auf zwei
/// von drei Systemen eine Halbwahrheit und auf dem dritten falsch — und
/// wer sich darauf verlässt, baut auf ein Aufräumen, das nicht stattfindet.
///
/// Für den Gebrauch macht es keinen Unterschied: Es gibt genau **eine**
/// Wacht, und sie lebt so lange wie das Fenster.
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
    #[cfg(target_os = "macos")]
    _anmeldung: macos::Anmeldung,
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
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
    /// * **macOS:** immer `true`, und hier am verlässlichsten: Das System
    ///   wartet auf `IOAllowPowerChange`. Der Aufschub muss nicht erbeten
    ///   werden, er ist eingebaut.
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
        // macOS wartet auf `IOAllowPowerChange`, und zwar zugesichert --
        // hier muss der Aufschub nicht erbeten werden wie unter Linux.
        #[cfg(target_os = "macos")]
        {
            true
        }
        #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
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

/// Meldet sich bei IOKit an.
///
/// # Fehler
///
/// [`NichtAngemeldet`], wenn IOKit die Anmeldung ablehnt oder kein Faden
/// zu bekommen ist.
#[cfg(target_os = "macos")]
pub fn anmelden<F>(rueckruf: F) -> Result<Wacht, NichtAngemeldet>
where
    F: Fn(Meldung) + Send + Sync + 'static,
{
    macos::anmelden(rueckruf).map(|a| Wacht { _anmeldung: a })
}

/// Auf diesem System gibt es (noch) keinen Weg dafür.
///
/// Kein stilles Nichtstun: Der Aufrufer bekommt einen Fehler und muss
/// entscheiden, was er dem Nutzer sagt.
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
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

/// Wie IOKit seine Nachrichtenart in eine [`Meldung`] übersetzt.
///
/// # Woher die Zahlen stammen
///
/// **Aus Apples Kopfdatei, nicht aus dem Gedächtnis.** Sie stehen in
/// `IOKit.framework/Headers/IOMessage.h`, und dieses Projekt hat keinen
/// Mac — der macOS-Läufer der Fortlaufprüfung hat sie am 23.08.2026
/// vorgelesen:
///
/// ```text
/// #define iokit_common_msg(message)  (UInt32)(sys_iokit|sub_iokit_common|message)
/// #define kIOMessageCanSystemSleep      iokit_common_msg(0x270)
/// #define kIOMessageSystemWillSleep     iokit_common_msg(0x280)
/// #define kIOMessageSystemWillNotSleep  iokit_common_msg(0x290)
/// #define kIOMessageSystemHasPoweredOn  iokit_common_msg(0x300)
/// #define kIOMessageSystemWillPowerOn   iokit_common_msg(0x320)
/// #define sys_iokit         err_system(0x38)
/// #define sub_iokit_common  err_sub(0)
/// #define err_system(x)     ((signed)((((unsigned)(x))&0x3f)<<26))
/// #define err_sub(x)        (((x)&0xfff)<<14)
/// ```
///
/// Daraus ergibt sich `sys_iokit = 0x38 << 26 = 0xE000_0000` und
/// `sub_iokit_common = 0`. Ein Schritt in `pruefung.yml` liest die Werte
/// bei jedem Lauf erneut und vergleicht sie mit denen hier: Ein geratener
/// Zahlenwert meldet entweder nie — dann trägt die Spezifikation eine
/// Zusage ohne Deckung — oder er meldet beim falschen Anlass, und das
/// Programm sperrt jemanden mitten im Arbeiten aus.
#[cfg(target_os = "macos")]
#[must_use]
const fn macos_meldung(art: u32) -> Meldung {
    match art {
        macos::KIO_MESSAGE_SYSTEM_WILL_SLEEP => Meldung::LegtSichSchlafen,
        macos::KIO_MESSAGE_SYSTEM_HAS_POWERED_ON | macos::KIO_MESSAGE_SYSTEM_WILL_POWER_ON => {
            Meldung::WiederWach
        }
        // `CanSystemSleep` ist eine **Frage**, keine Ankündigung: Das
        // System fragt, ob es in den Leerlaufschlaf darf. Wer hier sperrte,
        // sperrte, sobald der Rechner ein paar Minuten unbenutzt ist — also
        // bei jeder Kaffeepause, ohne dass er je einschläft.
        _ => Meldung::Belanglos,
    }
}

// ---------------------------------------------------------------------------
// macOS
//
// Der aufwendigste der drei Wege, und zwar nicht wegen der Aufrufe, sondern
// wegen der Schleife: IOKit stellt seine Meldungen über eine `CFRunLoop`
// zu. Es braucht also einen eigenen Faden, der eine solche Schleife dreht.
//
// Dafür gibt es einen Ausgleich, den die anderen beiden nicht haben: Das
// System **wartet** auf `IOAllowPowerChange`. Der Aufschub ist hier nicht
// zu erbitten wie unter Linux, sondern eingebaut.
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod macos {
    use super::{Meldung, NichtAngemeldet, macos_meldung};
    use core::ffi::c_void;

    /// `sys_iokit | sub_iokit_common`, siehe [`super::macos_meldung`].
    const IOKIT_COMMON: u32 = 0xE000_0000;

    pub(super) const KIO_MESSAGE_CAN_SYSTEM_SLEEP: u32 = IOKIT_COMMON | 0x270;
    pub(super) const KIO_MESSAGE_SYSTEM_WILL_SLEEP: u32 = IOKIT_COMMON | 0x280;
    pub(super) const KIO_MESSAGE_SYSTEM_HAS_POWERED_ON: u32 = IOKIT_COMMON | 0x300;
    pub(super) const KIO_MESSAGE_SYSTEM_WILL_POWER_ON: u32 = IOKIT_COMMON | 0x320;

    type IoConnectT = u32;
    type IoObjectT = u32;
    type IoServiceT = u32;

    type IoServiceInterestCallback =
        unsafe extern "C" fn(*mut c_void, IoServiceT, u32, *mut c_void);

    // SICHERHEIT: Nur Erklaerungen, kein Code. Die Formen stehen in
    // `IOKit.framework/Headers/IOPMLib.h`; falsch abgeschrieben waere hier
    // der gefaehrlichste Fehler des ganzen Moduls, denn er faellt beim
    // Uebersetzen nicht auf.
    #[allow(unsafe_code)]
    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IORegisterForSystemPower(
            refcon: *mut c_void,
            port: *mut *mut c_void,
            callback: IoServiceInterestCallback,
            notifier: *mut IoObjectT,
        ) -> IoConnectT;
        fn IONotificationPortGetRunLoopSource(port: *mut c_void) -> *mut c_void;
        fn IOAllowPowerChange(kernel_port: IoConnectT, notification_id: isize) -> i32;
    }

    // SICHERHEIT: wie oben, aus `CoreFoundation.framework`.
    #[allow(unsafe_code)]
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRunLoopGetCurrent() -> *mut c_void;
        fn CFRunLoopAddSource(rl: *mut c_void, source: *mut c_void, mode: *const c_void);
        fn CFRunLoopRun();
        static kCFRunLoopCommonModes: *const c_void;
    }

    /// Was der Rückruf braucht, um seine Arbeit zu tun.
    ///
    /// Es liegt in einer `Box`, deren Zeiger IOKit als `refcon`
    /// zurückreicht. Aufgeräumt wird es **nicht**: Der Faden dreht bis zum
    /// Prozessende (siehe unten), und einen Zeiger freizugeben, den das
    /// System noch benutzt, wäre der schlimmere Fehler.
    struct Empfaenger {
        rueckruf: Box<dyn Fn(Meldung) + Send + Sync + 'static>,
        wurzel: IoConnectT,
    }

    /// Die laufende Anmeldung.
    ///
    /// # Sie lässt sich **nicht** zurücknehmen
    ///
    /// Und das steht hier, statt es zu verschleiern. Der Faden hängt in
    /// `CFRunLoopRun`; das kehrt erst zurück, wenn jemand die Schleife
    /// anhält. Hier stand zuerst eine Abbruchmarke wie unter Linux — nur
    /// **las sie niemand**: Unter Linux prüft die Signalschleife sie
    /// zwischen zwei Meldungen, hier gibt es keine solche Stelle. Ein Feld,
    /// das ein Ende verspricht, das es nicht gibt, ist schlimmer als gar
    /// keines.
    ///
    /// Ein sauberer Abbruch ginge über `CFRunLoopStop`, gefolgt von
    /// `IODeregisterForSystemPower` und `IONotificationPortDestroy` — in
    /// dieser Reihenfolge, sonst darf IOKit noch einen Rückruf zustellen,
    /// dessen Empfänger schon weg ist. Es ist nicht gebaut, weil es
    /// **nichts nützt**: Es gibt genau eine Wacht, und sie lebt so lange
    /// wie das Fenster. Ungetesteter Aufräumcode an einer Stelle, die nie
    /// erreicht wird, wäre ein Risiko ohne Gegenwert.
    pub(super) struct Anmeldung {
        /// Nichts. Die Struktur trägt keinen Zustand — sie ist der
        /// Beleg, dass die Anmeldung geglückt ist.
        _leer: (),
    }

    /// Was IOKit aufruft. Läuft auf dem Faden der Ereignisschleife.
    ///
    /// # Sicherheit
    ///
    /// `refcon` ist der Zeiger, den wir bei der Anmeldung übergeben haben;
    /// IOKit reicht ihn unverändert zurück. Er bleibt gültig, solange der
    /// Prozess lebt.
    #[allow(unsafe_code)]
    unsafe extern "C" fn rueckruf(
        refcon: *mut c_void,
        _dienst: IoServiceT,
        art: u32,
        argument: *mut c_void,
    ) {
        if refcon.is_null() {
            return;
        }
        // SICHERHEIT: siehe oben -- der Zeiger stammt aus unserem eigenen
        // `Box::into_raw` und wird nie freigegeben.
        #[allow(unsafe_code)]
        let empfaenger = unsafe { &*refcon.cast::<Empfaenger>() };

        // Ueber eine `extern "C"`-Grenze zu entrollen ist nicht erlaubt.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            (empfaenger.rueckruf)(macos_meldung(art));
        }));

        // ZUSTIMMEN -- und zwar in BEIDEN Faellen.
        //
        // Bei `WillSleep` wartet das System auf diese Antwort; ohne sie
        // haengt es, bis eine Frist ablaeuft. Bei `CanSystemSleep` fragt
        // es, ob es in den Leerlaufschlaf darf: Wer dort nicht antwortet,
        // verhindert das Einschlafen ueberhaupt -- ein Verschluesselungs-
        // programm, das den Rechner wachhaelt, waere eine Zumutung.
        //
        // Die anderen Arten tragen keine Kennung, auf die zu antworten
        // waere.
        if art == KIO_MESSAGE_SYSTEM_WILL_SLEEP || art == KIO_MESSAGE_CAN_SYSTEM_SLEEP {
            // SICHERHEIT: `wurzel` stammt aus einer geglueckten Anmeldung,
            // und `argument` ist die Kennung, die IOKit gerade mitgegeben
            // hat. Sie wird genau einmal beantwortet.
            #[allow(unsafe_code)]
            unsafe {
                IOAllowPowerChange(empfaenger.wurzel, argument as isize);
            }
        }
    }

    pub(super) fn anmelden<F>(f: F) -> Result<Anmeldung, NichtAngemeldet>
    where
        F: Fn(Meldung) + Send + Sync + 'static,
    {
        let (sagen, hoeren) = std::sync::mpsc::channel::<Result<(), String>>();

        std::thread::Builder::new()
            .name("cabrik-ruhezustand".to_owned())
            .spawn(move || {
                let mut port: *mut c_void = core::ptr::null_mut();
                let mut notifier: IoObjectT = 0;

                // Der Empfaenger muss VOR der Anmeldung stehen: IOKit
                // bekommt seinen Zeiger und darf ihn ab dann jederzeit
                // zurueckreichen. `wurzel` traegt er noch nicht -- die gibt
                // es erst danach --, deshalb wird sie gleich nachgetragen.
                let empfaenger: *mut Empfaenger = Box::into_raw(Box::new(Empfaenger {
                    rueckruf: Box::new(f),
                    wurzel: 0,
                }));

                // SICHERHEIT: `port` und `notifier` sind gueltige
                // Ausgabezeiger, `empfaenger` ist ein lebender Zeiger aus
                // `Box::into_raw`, und `rueckruf` hat die Form, die IOKit
                // erwartet.
                #[allow(unsafe_code)]
                let wurzel = unsafe {
                    IORegisterForSystemPower(
                        empfaenger.cast(),
                        &raw mut port,
                        rueckruf,
                        &raw mut notifier,
                    )
                };

                if wurzel == 0 || port.is_null() {
                    // SICHERHEIT: Die Anmeldung ist gescheitert, IOKit hat
                    // den Zeiger also nicht behalten. Er stammt unmittelbar
                    // aus dem `Box::into_raw` oben.
                    #[allow(unsafe_code)]
                    drop(unsafe { Box::from_raw(empfaenger) });
                    let _ = sagen.send(Err(
                        "IOKit hat die Anmeldung fuer den Ruhezustand abgelehnt.".to_owned(),
                    ));
                    return;
                }

                // SICHERHEIT: Der Zeiger lebt, und niemand liest ihn in
                // diesem Augenblick -- die Ereignisschleife dreht sich noch
                // nicht, es kann also noch kein Rueckruf gekommen sein.
                #[allow(unsafe_code)]
                unsafe {
                    (*empfaenger).wurzel = wurzel;
                }

                // Die Quelle in die Schleife dieses Fadens haengen.
                //
                // Ein Vorgang, ein Block: Die Quelle allein nuetzt nichts,
                // und sie einzeln zu holen waere eine Zeile ohne Wirkung.
                //
                // SICHERHEIT: `port` stammt aus der geglueckten Anmeldung;
                // die Quelle gehoert ihm und wird nicht freigegeben.
                // `CFRunLoopGetCurrent` liefert die Schleife dieses
                // Fadens, `kCFRunLoopCommonModes` ist eine Konstante von
                // CoreFoundation.
                #[allow(unsafe_code)]
                unsafe {
                    let quelle = IONotificationPortGetRunLoopSource(port);
                    CFRunLoopAddSource(CFRunLoopGetCurrent(), quelle, kCFRunLoopCommonModes);
                }

                let _ = sagen.send(Ok(()));

                // SICHERHEIT: Dreht die Schleife dieses Fadens. Sie kehrt
                // erst zurueck, wenn jemand sie anhaelt -- in der Praxis
                // also nie.
                #[allow(unsafe_code)]
                unsafe {
                    CFRunLoopRun();
                }
            })
            .map_err(|e| NichtAngemeldet::neu(format!("Kein Faden fuer die Ruhewacht: {e}")))?;

        // Auf das Ergebnis der Anmeldung WARTEN.
        //
        // Sonst gaebe `anmelden` ein `Ok` zurueck und erfuehre erst
        // Sekunden spaeter, dass IOKit abgelehnt hat -- und dann gaebe es
        // niemanden mehr, dem man es sagen koennte. Derselbe Grund wie
        // unter Linux, nur dass die Anmeldung hier zwingend im Faden
        // stattfinden muss: Die Ereignisschleife gehoert dem Faden, der sie
        // dreht.
        match hoeren.recv() {
            Ok(Ok(())) => Ok(Anmeldung { _leer: () }),
            Ok(Err(grund)) => Err(NichtAngemeldet::neu(grund)),
            Err(_) => Err(NichtAngemeldet::neu(
                "Der Faden der Ruhewacht ist beendet, bevor er sich gemeldet hat.",
            )),
        }
    }
}

#[cfg(test)]
mod pruefungen {
    #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
    use super::Meldung;
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
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

    /// Der ganze Weg über IOKit, so weit er sich hier prüfen lässt.
    ///
    /// **Was er nicht beweist:** dass gesperrt wird. Dafür müsste ein Mac
    /// tatsächlich einschlafen. Was er beweist, ist trotzdem nicht nichts
    /// — und es ist genau das, was auf einem Läufer schiefgehen kann:
    ///
    /// * Die Anmeldung endet, statt zu hängen. Sie geschieht auf einem
    ///   eigenen Faden, und `anmelden` wartet auf dessen Antwort; hinge
    ///   sie, hinge der Programmstart.
    /// * Sie stürzt nicht ab. Ein Zahlendreher in einer der
    ///   `extern`-Formen fiele beim Übersetzen nicht auf, beim Aufruf
    ///   aber sehr wohl.
    /// * Und schlägt sie fehl, sagt sie warum.
    #[cfg(target_os = "macos")]
    #[test]
    fn anmelden_endet_und_sagt_was_dabei_herauskam() {
        match anmelden(|_| {}) {
            Ok(wacht) => {
                println!("RUHEWACHT: angemeldet -- IOKit hat angenommen");
                assert!(
                    wacht.hat_aufschub(),
                    "macOS wartet auf IOAllowPowerChange -- das ist keine Bitte"
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

    /// Die Zahlen aus Apples Kopfdatei, gegengeprüft.
    ///
    /// Ein Test für Konstanten wirkt überflüssig, ist er hier aber nicht:
    /// Sie stammen von einem fremden Rechner, wurden abgeschrieben, und
    /// ein Zahlendreher fiele beim Übersetzen **nicht** auf. Er fiele erst
    /// auf, wenn jemand seinen Mac zuklappt und das Passwort im Abbild
    /// landet.
    #[cfg(target_os = "macos")]
    #[test]
    fn die_zuordnung_der_iokit_meldung() {
        use super::macos_meldung;

        // sys_iokit | sub_iokit_common | 0x280
        assert_eq!(macos_meldung(0xE000_0280), Meldung::LegtSichSchlafen);
        assert_eq!(macos_meldung(0xE000_0300), Meldung::WiederWach);
        assert_eq!(macos_meldung(0xE000_0320), Meldung::WiederWach);

        // GEGENPROBE. `CanSystemSleep` ist eine FRAGE -- das System fragt,
        // ob es in den Leerlaufschlaf darf. Wer hier sperrte, sperrte bei
        // jeder Kaffeepause, ohne dass der Rechner je einschlaeft.
        assert_eq!(macos_meldung(0xE000_0270), Meldung::Belanglos);
        // Und "wird doch nicht schlafen" erst recht nicht.
        assert_eq!(macos_meldung(0xE000_0290), Meldung::Belanglos);

        for unbekannt in [0_u32, 42, 0xE000_0130, u32::MAX] {
            assert_eq!(
                macos_meldung(unbekannt),
                Meldung::Belanglos,
                "unbekannte Meldung {unbekannt:#x} wurde gedeutet"
            );
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

    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
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
