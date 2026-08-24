//! Was ein Tastendruck im Passwortfeld bewirkt.
//!
//! # Warum das ein eigenes Modul ist
//!
//! Weil es der einzige Teil des nativen Passwortfensters ist, der sich
//! **ohne Fenster** prüfen lässt — und weil er der Teil ist, in dem ein
//! Fehler stillschweigend teuer wird.
//!
//! Ein Fenster lässt sich nicht in einem Test aufmachen, eine Taste nicht
//! drücken. Die Entscheidung dahinter dagegen sehr wohl: Was bedeutet
//! diese Zeicheneinheit? Was gehört in den Puffer, was nicht? Genau diese
//! Fragen beantwortet dieses Modul, und die Fensterhülle daneben besteht
//! aus Zeichnen und Nachrichtenschleife.
//!
//! # Die zwei Fallen von `WM_CHAR`
//!
//! **Erstens Steuerzeichen.** Windows schickt Rücktaste, Eingabe und
//! Escape als gewöhnliche Zeichennachricht — `0x08`, `0x0D`, `0x1B`. Wer
//! sie nicht abfängt, hat sie im Passwort stehen, und niemand sieht es:
//! Die Punkte sehen genauso aus.
//!
//! **Zweitens Ersatzzeichen.** `WM_CHAR` liefert UTF-16-Einheiten. Ein
//! Zeichen außerhalb der Grundebene — jedes Emoji — kommt als **zwei**
//! Nachrichten. Wer jede einzeln als Zeichen nimmt, schreibt zwei
//! ungültige Hälften in den Puffer; das Passwort ist dann ein anderes als
//! das getippte, und beim nächsten Entsperren geht es nicht mehr auf.

use crate::{Festgenagelt, Voll};

/// Was ein Tastendruck bewirken soll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wirkung {
    /// Der Puffer hat sich geändert — die Punkte sind neu zu zeichnen.
    Geaendert,
    /// Der Nutzer ist fertig.
    Bestaetigt,
    /// Der Nutzer will nicht mehr.
    Abgebrochen,
    /// Nichts zu tun.
    ///
    /// Ausdrücklich benannt statt als `Option`: Der häufigste Fall ist
    /// „diese Taste bedeutet hier nichts", und ein `None` dafür lüde dazu
    /// ein, ihn mit einem Fehler zu verwechseln.
    Nichts,
    /// Der Puffer ist voll — das Zeichen wurde **nicht** aufgenommen.
    ///
    /// Kein Fehler, aber auch kein stilles Verschlucken: Wer weitertippt,
    /// ohne dass sich etwas rührt, muss es merken.
    Voll,
}

/// Der Zustand des Eingabefeldes.
///
/// Enthält den festgenagelten Puffer und die eine Zusatzangabe, die
/// UTF-16 nötig macht.
pub struct Eingabe {
    puffer: Festgenagelt,
    /// Eine erste Hälfte, die auf ihre zweite wartet.
    ///
    /// Steht hier etwas, kam gerade ein hohes Ersatzzeichen. Es ist für
    /// sich genommen **kein** Zeichen und darf nicht in den Puffer.
    schwebend: Option<u16>,
    /// Wie viele Zeichen der Nutzer getippt hat.
    ///
    /// Nicht aus dem Puffer gezählt, sondern mitgeführt: Die Punkte
    /// dürfen nicht davon abhängen, dass jemand die Bytes liest.
    zeichen: usize,
}

impl Eingabe {
    /// Ein leeres Feld mit festgenageltem Puffer.
    ///
    /// `kapazitaet` ist die Obergrenze in **Bytes**, nicht in Zeichen.
    #[must_use]
    pub fn neu(kapazitaet: usize) -> Self {
        Self {
            puffer: Festgenagelt::neu(kapazitaet),
            schwebend: None,
            zeichen: 0,
        }
    }

    /// Ob die Seiten des Puffers festgenagelt sind.
    ///
    /// Wird durchgereicht, damit der Aufrufer es sagen kann — und nicht
    /// behaupten muss.
    #[must_use]
    pub const fn ist_festgenagelt(&self) -> bool {
        self.puffer.ist_festgenagelt()
    }

    /// Wie viele Punkte zu zeichnen sind.
    #[must_use]
    pub const fn punkte(&self) -> usize {
        self.zeichen
    }

    /// Ob nichts drinsteht.
    #[must_use]
    pub const fn ist_leer(&self) -> bool {
        self.zeichen == 0
    }

    /// Das Getippte, zum Weiterreichen an den Kern.
    #[must_use]
    pub fn als_bytes(&self) -> &[u8] {
        self.puffer.als_bytes()
    }

    /// Gibt den Puffer heraus und lässt das Feld zurück.
    #[must_use]
    pub fn nehmen(self) -> Festgenagelt {
        self.puffer
    }

    /// Eine Zeichennachricht von Windows — eine **UTF-16-Einheit**.
    ///
    /// Behandelt Steuerzeichen und Ersatzzeichen; siehe den Modulkopf.
    pub fn zeichen(&mut self, einheit: u16) -> Wirkung {
        // Erst die Steuerzeichen, und zwar VOR allem anderen. Sie kommen
        // als gewoehnliche Zeichennachricht, und wer sie durchlaesst, hat
        // sie im Passwort.
        match einheit {
            0x08 => return self.rueckschritt(),
            0x0D => return Wirkung::Bestaetigt,
            0x1B => return Wirkung::Abgebrochen,
            // Alles unter dem Leerzeichen ist eine Steuertaste: Tabulator,
            // Zeilenvorschub, Klingel. Nichts davon gehoert in ein
            // Passwort, und stillschweigend aufzunehmen waere schlimmer
            // als zu uebergehen -- der Nutzer saehe einen Punkt mehr und
            // wuesste nicht, wofuer.
            0..=0x1F | 0x7F => {
                self.schwebend = None;
                return Wirkung::Nichts;
            }
            _ => {}
        }

        // Ersatzzeichen: Ein hohes wartet auf sein niedriges.
        if (0xD800..=0xDBFF).contains(&einheit) {
            self.schwebend = Some(einheit);
            // NOCH KEINE Aenderung: Es ist ein halbes Zeichen, und der
            // Punkt gehoert erst gezeichnet, wenn es ganz ist.
            return Wirkung::Nichts;
        }

        let paar = self.schwebend.take();
        if (0xDC00..=0xDFFF).contains(&einheit) {
            let Some(hoch) = paar else {
                // Ein niedriges Ersatzzeichen ohne sein hohes. Das schickt
                // Windows nicht von sich aus -- es kaeme von einem fremden
                // Programm, das Tastendruecke einspeist. Uebergehen statt
                // raten.
                return Wirkung::Nichts;
            };
            return self.einfuegen(&[hoch, einheit]);
        }

        self.einfuegen(&[einheit])
    }

    /// Nimmt eine vollständige UTF-16-Folge auf.
    fn einfuegen(&mut self, einheiten: &[u16]) -> Wirkung {
        let mut text = String::new();
        for teil in char::decode_utf16(einheiten.iter().copied()) {
            match teil {
                Ok(c) => text.push(c),
                // Kann nach der Behandlung oben nicht mehr vorkommen. Wenn
                // doch, wird nichts geraten: Ein Ersatzzeichen als „?" in
                // ein Passwort zu schreiben ergaebe ein anderes Passwort
                // als das getippte, und das faellt erst beim naechsten
                // Entsperren auf.
                Err(_) => return Wirkung::Nichts,
            }
        }
        if text.is_empty() {
            return Wirkung::Nichts;
        }

        match self.puffer.anhaengen(&text) {
            Ok(()) => {
                self.zeichen = self.zeichen.saturating_add(text.chars().count());
                Wirkung::Geaendert
            }
            Err(Voll) => Wirkung::Voll,
        }
    }

    /// Nimmt das letzte Zeichen zurück.
    fn rueckschritt(&mut self) -> Wirkung {
        // Ein halbes Zeichen zaehlt nicht. Wer nach einem hohen
        // Ersatzzeichen die Ruecktaste drueckt, hat noch nichts getippt --
        // und darf nicht das ZEICHEN DAVOR verlieren.
        if self.schwebend.take().is_some() {
            return Wirkung::Nichts;
        }
        if self.ist_leer() {
            return Wirkung::Nichts;
        }
        self.puffer.letztes_zeichen_loeschen();
        self.zeichen = self.zeichen.saturating_sub(1);
        Wirkung::Geaendert
    }
}

impl core::fmt::Debug for Eingabe {
    /// Von Hand: Das abgeleitete druckte den Puffer mit aus.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Eingabe")
            .field("punkte", &self.zeichen)
            .field("schwebend", &self.schwebend.is_some())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Windows: die Hülle um die Tastenlogik
//
// Sie ist mit Absicht dünn. Alles, was eine Entscheidung trifft, steht
// oben in [`Eingabe`] und ist dort geprüft; hier stehen Fensterklasse,
// Fensterprozedur, Zeichnen und Nachrichtenschleife.
//
// WARUM NICHT DAS PASSWORTFELD VON WINDOWS. Ein `EDIT` mit `ES_PASSWORD`
// wäre in drei Zeilen da — und hielte den Text in **seinem eigenen**
// Puffer, den wir weder festnageln noch überschreiben können. Genau davon
// handelt dieses ganze Modul.
// ---------------------------------------------------------------------------

#[cfg(windows)]
pub use windows::{Antwort, KeinFenster, abfragen, moeglich};

#[cfg(not(windows))]
pub use anderswo::{Antwort, KeinFenster, abfragen, moeglich};

/// Auf diesem System gibt es (noch) kein eigenes Passwortfeld.
///
/// # Kein stilles Nichtstun
///
/// Der Aufrufer bekommt einen Fehler mit Grund und muss entscheiden, was
/// er dem Nutzer sagt. Ein leeres Ergebnis zurückzugeben hieße, ihn ohne
/// Passwort weiterzuschicken; ein erfundener Erfolg wäre noch schlimmer.
#[cfg(not(windows))]
mod anderswo {
    use crate::Festgenagelt;

    /// Was der Nutzer entschieden hat.
    ///
    /// Auf diesem System kommt es nie zustande — der Typ steht trotzdem
    /// da, damit der Aufrufer nicht für jedes System einen eigenen Zweig
    /// schreiben muss.
    #[derive(Debug)]
    pub enum Antwort {
        /// Er hat etwas eingegeben und bestätigt.
        Eingegeben(Festgenagelt),
        /// Er hat abgebrochen.
        Abgebrochen,
    }

    /// Warum kein Fenster aufging.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct KeinFenster {
        /// Was schiefging, in einem Satz.
        pub grund: String,
    }

    impl core::fmt::Display for KeinFenster {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str(&self.grund)
        }
    }

    impl core::error::Error for KeinFenster {}

    /// Gibt es hier nicht — und sagt das.
    ///
    /// # Fehler
    ///
    /// Immer.
    pub fn abfragen(
        _frage: &str,
        _kapazitaet: usize,
        _besitzer: Option<isize>,
    ) -> Result<Antwort, KeinFenster> {
        moeglich().map(|()| Antwort::Abgebrochen)
    }

    /// Ob hier ein eigenes Feld möglich ist. Hier: nein.
    ///
    /// # Fehler
    ///
    /// Immer.
    pub fn moeglich() -> Result<(), KeinFenster> {
        Err(KeinFenster {
            grund: "Auf diesem Betriebssystem gibt es noch kein eigenes Passwortfeld.".to_owned(),
        })
    }
}

#[cfg(windows)]
mod windows {
    use super::{Eingabe, Wirkung};
    use crate::Festgenagelt;
    use core::ffi::c_void;
    use windows_sys::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        BeginPaint, CreateFontW, CreateSolidBrush, DT_LEFT, DT_SINGLELINE, DT_VCENTER,
        DeleteObject, DrawTextW, EndPaint, FillRect, HFONT, InvalidateRect, PAINTSTRUCT,
        SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    // `EnableWindow` steht bei der Tastatur- und Mausbehandlung, nicht
    // bei den Fensternachrichten -- eine Eigenart der Aufteilung.
    use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
        GWLP_USERDATA, GetMessageW, GetSystemMetrics, IDC_ARROW, LoadCursorW, MSG, PostQuitMessage,
        RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW, SetForegroundWindow, SetWindowLongPtrW,
        ShowWindow, TranslateMessage, WM_CHAR, WM_CLOSE, WM_DESTROY, WM_NCCREATE, WM_PAINT,
        WNDCLASSW, WS_CAPTION, WS_EX_TOPMOST, WS_POPUP, WS_SYSMENU, WS_VISIBLE,
    };

    /// Was der Nutzer entschieden hat.
    pub enum Antwort {
        /// Er hat etwas eingegeben und bestätigt.
        ///
        /// Auch ein **leerer** Puffer ist eine Eingabe: Wer nichts tippt
        /// und Eingabe drückt, hat ein leeres Passwort versucht. Das
        /// abzulehnen ist Sache des Kerns, nicht des Fensters.
        Eingegeben(Festgenagelt),
        /// Er hat abgebrochen — Escape oder das Kreuz.
        Abgebrochen,
    }

    /// Warum kein Fenster aufging.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct KeinFenster {
        /// Was schiefging, in einem Satz.
        pub grund: String,
    }

    impl core::fmt::Display for KeinFenster {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str(&self.grund)
        }
    }

    impl core::error::Error for KeinFenster {}

    /// Der Zustand, den die Fensterprozedur braucht.
    struct Fensterzustand {
        eingabe: Eingabe,
        frage: Vec<u16>,
        hinweis: Vec<u16>,
        /// `None`, solange nichts entschieden ist.
        fertig: Option<bool>,
        /// Ob das Feld gerade voll ist — für den Hinweis darunter.
        voll: bool,
    }

    /// Der Klassenname. Einmal registriert, danach wiederverwendet.
    fn klassenname() -> Vec<u16> {
        "CabrikPasswortfeld\0".encode_utf16().collect()
    }

    fn weit(text: &str) -> Vec<u16> {
        let mut v: Vec<u16> = text.encode_utf16().collect();
        v.push(0);
        v
    }

    /// Registriert die Fensterklasse — genau einmal je Prozess.
    fn klasse_sicherstellen() -> Result<(), KeinFenster> {
        use std::sync::OnceLock;
        static EINMAL: OnceLock<bool> = OnceLock::new();

        let geglueckt = *EINMAL.get_or_init(|| {
            let name = klassenname();
            // SICHERHEIT: `GetModuleHandleW(null)` liefert das Handle des
            // eigenen Prozesses und fasst keinen uebergebenen Speicher an.
            #[allow(unsafe_code)]
            let modul = unsafe { GetModuleHandleW(core::ptr::null()) };
            // SICHERHEIT: `LoadCursorW` mit `IDC_ARROW` nimmt keine
            // Zeiger, sondern eine eingebaute Kennung.
            #[allow(unsafe_code)]
            let zeiger = unsafe { LoadCursorW(core::ptr::null_mut(), IDC_ARROW) };

            let klasse = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(fensterprozedur),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: modul.cast(),
                hIcon: core::ptr::null_mut(),
                hCursor: zeiger,
                // Kein Hintergrundpinsel: Wir zeichnen selbst, und ein
                // Pinsel hier fuehrte zum Flimmern.
                hbrBackground: core::ptr::null_mut(),
                lpszMenuName: core::ptr::null(),
                lpszClassName: name.as_ptr(),
            };

            // SICHERHEIT: `klasse` lebt bis zum Ende dieses Aufrufs, und
            // `RegisterClassW` kopiert, was es braucht. `name` lebt
            // ebenso lange -- Windows behaelt den Zeiger allerdings, was
            // hier gutgeht, weil die Klasse den Prozess ueberdauert.
            #[allow(unsafe_code)]
            let atom = unsafe { RegisterClassW(&raw const klasse) };
            core::mem::forget(name);
            atom != 0
        });

        if geglueckt {
            Ok(())
        } else {
            Err(KeinFenster {
                grund: "Die Fensterklasse liess sich nicht anmelden.".to_owned(),
            })
        }
    }

    /// Ob hier überhaupt ein eigenes Feld möglich ist — **ohne** eines zu
    /// zeigen.
    ///
    /// # Was das prüft und was nicht
    ///
    /// Geprüft wird, ob die Fensterklasse sich anmelden lässt. Das ist die
    /// Hürde, an der es scheitert, wenn es scheitert — und sie lässt sich
    /// nehmen, ohne dass etwas auf dem Bildschirm erscheint.
    ///
    /// **Keine Zusage, dass das Fenster später aufgeht.** Eines
    /// tatsächlich zu erzeugen und gleich wieder zu schließen wäre die
    /// gründlichere Probe und die schlechtere: Es blitzte kurz auf, und
    /// niemand hätte es gewollt.
    ///
    /// # Fehler
    ///
    /// [`KeinFenster`] mit dem Grund.
    pub fn moeglich() -> Result<(), KeinFenster> {
        klasse_sicherstellen()
    }

    /// Fragt ein Passwort ab und gibt es festgenagelt zurück.
    ///
    /// **Blockiert**, bis der Nutzer bestätigt oder abbricht. Der Aufrufer
    /// gehört deshalb auf einen eigenen Faden — im Fenster ist das der
    /// `(async)`-Befehl.
    ///
    /// # Fehler
    ///
    /// [`KeinFenster`], wenn Windows kein Fenster hergibt. Dann bleibt der
    /// Weg über die Webansicht — und der Aufrufer muss sagen, dass er ihn
    /// genommen hat.
    pub fn abfragen(
        frage: &str,
        kapazitaet: usize,
        besitzer: Option<isize>,
    ) -> Result<Antwort, KeinFenster> {
        abfragen_mit_haken(frage, kapazitaet, besitzer, &mut |_| {})
    }

    /// Wie [`abfragen`], ruft aber `haken` mit dem frischen Fenster auf.
    ///
    /// # Wofür der Haken da ist
    ///
    /// Für die Tests. Eine Nachrichtenschleife lässt sich nicht von außen
    /// füttern: Wer Tastendrücke schicken will, braucht das Fensterhandle,
    /// und das gibt es erst, wenn die Schleife gleich losläuft.
    ///
    /// Ein Prüfhaken in ausgelieferten Code ist keine schöne Sache. Die
    /// Alternative wäre, die Fensterprozedur **ungeprüft** zu lassen — und
    /// das ist die deutlich schlechtere: Sie entscheidet, was ins Passwort
    /// kommt.
    ///
    /// Deshalb `pub(crate)` und nicht `pub`: Er ist ein Werkzeug dieser
    /// Kiste und keine Zusage an ihre Benutzer.
    pub(crate) fn abfragen_mit_haken(
        frage: &str,
        kapazitaet: usize,
        besitzer: Option<isize>,
        haken: &mut dyn FnMut(HWND),
    ) -> Result<Antwort, KeinFenster> {
        klasse_sicherstellen()?;

        let mut zustand = Box::new(Fensterzustand {
            eingabe: Eingabe::neu(kapazitaet),
            frage: weit(frage),
            hinweis: weit("Eingabe bestätigt · Escape bricht ab"),
            fertig: None,
            voll: false,
        });

        let name = klassenname();
        let titel = weit("Cabrik Secure");

        // Der Besitzer. Ohne ihn kann das Passwortfeld HINTER dem
        // Hauptfenster erscheinen -- Windows haelt ein besessenes Fenster
        // immer ueber seinem Besitzer, ein besitzerloses nicht.
        let eltern: HWND = match besitzer {
            Some(h) if h != 0 => h as HWND,
            _ => core::ptr::null_mut(),
        };

        // SICHERHEIT: Alle Zeiger zeigen auf lebende, nullterminierte
        // Puffer; `zustand` liegt auf der Halde und wird der Prozedur ueber
        // `lpCreateParams` gereicht, die ihn in `WM_NCCREATE` ablegt.
        #[allow(unsafe_code)]
        let hwnd = unsafe {
            let (breite, hoehe) = (420_i32, 190_i32);
            // Mittig, etwas ueber der Mitte. `saturating_*`, weil die
            // Bildschirmgroesse von aussen kommt: Ein Ueberlauf duerfte
            // nicht zum Absturz fuehren, nur zu einem schlecht sitzenden
            // Fenster.
            let x = GetSystemMetrics(SM_CXSCREEN)
                .saturating_sub(breite)
                .saturating_div(2);
            let y = GetSystemMetrics(SM_CYSCREEN)
                .saturating_sub(hoehe)
                .saturating_div(3);
            CreateWindowExW(
                WS_EX_TOPMOST,
                name.as_ptr(),
                titel.as_ptr(),
                WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
                x,
                y,
                breite,
                hoehe,
                eltern,
                core::ptr::null_mut(),
                GetModuleHandleW(core::ptr::null()).cast(),
                core::ptr::from_mut(zustand.as_mut()).cast::<c_void>(),
            )
        };

        if hwnd.is_null() {
            return Err(KeinFenster {
                grund: "Windows hat kein Fenster hergegeben.".to_owned(),
            });
        }

        // Das Hauptfenster stillstellen, solange gefragt wird.
        //
        // Ohne das kann der Nutzer nebenher weiterklicken und einen
        // zweiten Vorgang anstossen, waehrend sein Passwort halb getippt
        // dasteht. Ein Passwortdialog, an dem man vorbeiarbeiten kann, ist
        // keiner.
        if !eltern.is_null() {
            // SICHERHEIT: `eltern` ist ein gueltiges Handle -- es kam vom
            // Aufrufer, und `CreateWindowExW` hat es eben angenommen.
            #[allow(unsafe_code)]
            unsafe {
                EnableWindow(eltern, 0);
            }
        }

        // SICHERHEIT: `hwnd` stammt aus einer geglueckten Erzeugung.
        #[allow(unsafe_code)]
        unsafe {
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
        }

        haken(hwnd);

        // Die Nachrichtenschleife. Sie endet mit `WM_QUIT`, das die
        // Prozedur in `WM_DESTROY` schickt.
        let mut nachricht = MSG {
            hwnd: core::ptr::null_mut(),
            message: 0,
            wParam: 0,
            lParam: 0,
            time: 0,
            pt: windows_sys::Win32::Foundation::POINT { x: 0, y: 0 },
        };
        loop {
            // SICHERHEIT: `nachricht` ist eine gueltige, beschreibbare
            // Struktur; die uebrigen Werte begrenzen nichts.
            #[allow(unsafe_code)]
            let weiter = unsafe { GetMessageW(&raw mut nachricht, core::ptr::null_mut(), 0, 0) };
            if weiter <= 0 {
                break;
            }
            // SICHERHEIT: Beide lesen nur aus `nachricht`.
            #[allow(unsafe_code)]
            unsafe {
                TranslateMessage(&raw const nachricht);
                DispatchMessageW(&raw const nachricht);
            }
        }

        // Wieder freigeben -- und zwar VOR der Auswertung: Ein Fehler
        // beim Auswerten duerfte kein stillgestelltes Hauptfenster
        // hinterlassen.
        if !eltern.is_null() {
            // SICHERHEIT: wie oben.
            #[allow(unsafe_code)]
            unsafe {
                EnableWindow(eltern, 1);
                SetForegroundWindow(eltern);
            }
        }

        match zustand.fertig {
            Some(true) => Ok(Antwort::Eingegeben(zustand.eingabe.nehmen())),
            _ => Ok(Antwort::Abgebrochen),
        }
    }

    /// Die Fensterprozedur.
    ///
    /// # Sicherheit
    ///
    /// Windows ruft sie mit einem Handle auf, dessen `GWLP_USERDATA` wir
    /// in `WM_NCCREATE` gesetzt haben. Vorher ist es null, und dieser Fall
    /// wird behandelt statt vorausgesetzt.
    #[allow(unsafe_code)]
    unsafe extern "system" fn fensterprozedur(
        hwnd: HWND,
        nachricht: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if nachricht == WM_NCCREATE {
            // SICHERHEIT: Bei `WM_NCCREATE` traegt `lparam` einen Zeiger
            // auf ein `CREATESTRUCTW`, und dessen `lpCreateParams` ist
            // genau der Zeiger, den `CreateWindowExW` mitbekommen hat.
            #[allow(unsafe_code)]
            let erzeugung = unsafe {
                &*(lparam as *const windows_sys::Win32::UI::WindowsAndMessaging::CREATESTRUCTW)
            };
            // SICHERHEIT: Legt eine Zahl im Fenster ab; kein Speicherzugriff.
            #[allow(unsafe_code)]
            unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, erzeugung.lpCreateParams as isize);
            }
            // SICHERHEIT: Der Standardweg fuer alles Uebrige.
            #[allow(unsafe_code)]
            return unsafe { DefWindowProcW(hwnd, nachricht, wparam, lparam) };
        }

        // SICHERHEIT: Liest die Zahl zurueck, die oben abgelegt wurde.
        #[allow(unsafe_code)]
        let roh = unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA)
        };
        if roh == 0 {
            // SICHERHEIT: wie oben.
            #[allow(unsafe_code)]
            return unsafe { DefWindowProcW(hwnd, nachricht, wparam, lparam) };
        }
        // SICHERHEIT: Der Zeiger stammt aus dem `Box` in `abfragen`, das
        // bis zum Ende der Nachrichtenschleife lebt -- und die Prozedur
        // laeuft nur waehrend dieser Schleife.
        #[allow(unsafe_code)]
        let zustand = unsafe { &mut *(roh as *mut Fensterzustand) };

        match nachricht {
            WM_CHAR => {
                // Die eine Entscheidung -- und sie faellt nebenan, im
                // geprueften Teil.
                let wirkung = zustand.eingabe.zeichen(wparam as u16);
                match wirkung {
                    Wirkung::Bestaetigt => {
                        zustand.fertig = Some(true);
                        // SICHERHEIT: `hwnd` ist gueltig.
                        #[allow(unsafe_code)]
                        unsafe {
                            DestroyWindow(hwnd);
                        }
                    }
                    Wirkung::Abgebrochen => {
                        zustand.fertig = Some(false);
                        // SICHERHEIT: wie oben.
                        #[allow(unsafe_code)]
                        unsafe {
                            DestroyWindow(hwnd);
                        }
                    }
                    Wirkung::Geaendert | Wirkung::Voll => {
                        zustand.voll = wirkung == Wirkung::Voll;
                        // SICHERHEIT: Fordert nur ein Neuzeichnen an.
                        #[allow(unsafe_code)]
                        unsafe {
                            InvalidateRect(hwnd, core::ptr::null(), 1);
                        }
                    }
                    Wirkung::Nichts => {}
                }
                0
            }
            WM_PAINT => {
                zeichnen(hwnd, zustand);
                0
            }
            WM_CLOSE => {
                // Das Kreuz ist ein Abbruch, kein leeres Passwort.
                zustand.fertig = Some(false);
                // SICHERHEIT: `hwnd` ist gueltig.
                #[allow(unsafe_code)]
                unsafe {
                    DestroyWindow(hwnd);
                }
                0
            }
            WM_DESTROY => {
                // SICHERHEIT: Beendet die Schleife in `abfragen`.
                #[allow(unsafe_code)]
                unsafe {
                    PostQuitMessage(0);
                }
                0
            }
            // SICHERHEIT: Der Standardweg fuer alles Uebrige.
            #[allow(unsafe_code)]
            _ => unsafe { DefWindowProcW(hwnd, nachricht, wparam, lparam) },
        }
    }

    /// Zeichnet Frage, Punkte und Hinweis.
    ///
    /// Bewusst schlicht: Es gibt nichts zu gestalten, was das Passwort
    /// sicherer machte, und jede Zierde wäre weitere `unsafe`-Fläche.
    fn zeichnen(hwnd: HWND, zustand: &Fensterzustand) {
        const HINTERGRUND: COLORREF = 0x0020_1A16; // dunkel, BGR
        const SCHRIFT: COLORREF = 0x00E8_E4E0;
        const LEISE: COLORREF = 0x0090_8880;

        let mut ps = PAINTSTRUCT {
            hdc: core::ptr::null_mut(),
            fErase: 0,
            rcPaint: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            fRestore: 0,
            fIncUpdate: 0,
            rgbReserved: [0; 32],
        };

        // SICHERHEIT: `ps` ist eine gueltige, beschreibbare Struktur.
        #[allow(unsafe_code)]
        let hdc = unsafe { BeginPaint(hwnd, &raw mut ps) };
        if hdc.is_null() {
            return;
        }

        // SICHERHEIT: Alle folgenden Aufrufe arbeiten auf `hdc` aus
        // `BeginPaint` und auf Objekten, die unten wieder freigegeben
        // werden.
        #[allow(unsafe_code)]
        unsafe {
            let skala = i32::try_from(GetDpiForWindow(hwnd)).unwrap_or(96).max(96);
            // Auf die Bildschirmaufloesung umrechnen. `saturating_*` aus
            // demselben Grund wie oben: Die Skala kommt vom System.
            let mass = |px: i32| px.saturating_mul(skala).saturating_div(96);

            let mut flaeche = RECT {
                left: 0,
                top: 0,
                right: mass(420),
                bottom: mass(190),
            };
            let pinsel = CreateSolidBrush(HINTERGRUND);
            FillRect(hdc, &raw const flaeche, pinsel);
            DeleteObject(pinsel.cast());

            let schriftname = weit("Segoe UI");
            let schrift: HFONT = CreateFontW(
                mass(15).saturating_neg(),
                0,
                0,
                0,
                400,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                schriftname.as_ptr(),
            );
            let vorige = SelectObject(hdc, schrift.cast());
            SetBkMode(hdc, TRANSPARENT as i32);

            // Die Frage.
            SetTextColor(hdc, SCHRIFT);
            flaeche = RECT {
                left: mass(20),
                top: mass(18),
                right: mass(400),
                bottom: mass(48),
            };
            DrawTextW(
                hdc,
                zustand.frage.as_ptr(),
                -1,
                &raw mut flaeche,
                DT_LEFT | DT_SINGLELINE | DT_VCENTER,
            );

            // Die Punkte. Ein Punkt je Zeichen -- nie die Laenge in
            // Zahlen: Sie waere eine Auskunft ueber das Passwort, die
            // jeder mitlesen kann, der auf den Bildschirm sieht.
            let punkte: String = "\u{2022}".repeat(zustand.eingabe.punkte());
            let punkte = weit(&punkte);
            flaeche = RECT {
                left: mass(20),
                top: mass(64),
                right: mass(400),
                bottom: mass(104),
            };
            let feld = CreateSolidBrush(0x0038_302A);
            FillRect(hdc, &raw const flaeche, feld);
            DeleteObject(feld.cast());
            let mut innen = RECT {
                left: mass(30),
                top: mass(64),
                right: mass(390),
                bottom: mass(104),
            };
            DrawTextW(
                hdc,
                punkte.as_ptr(),
                -1,
                &raw mut innen,
                DT_LEFT | DT_SINGLELINE | DT_VCENTER,
            );

            // Der Hinweis.
            SetTextColor(hdc, LEISE);
            flaeche = RECT {
                left: mass(20),
                top: mass(120),
                right: mass(400),
                bottom: mass(170),
            };
            let voll = weit("Mehr passt nicht hinein.");
            DrawTextW(
                hdc,
                if zustand.voll {
                    voll.as_ptr()
                } else {
                    zustand.hinweis.as_ptr()
                },
                -1,
                &raw mut flaeche,
                DT_LEFT | DT_SINGLELINE | DT_VCENTER,
            );

            SelectObject(hdc, vorige);
            DeleteObject(schrift.cast());
            EndPaint(hwnd, &raw const ps);
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod pruefungen {
    use super::{Eingabe, Wirkung};

    /// Tippt eine Zeichenkette, wie Windows sie schicken würde.
    fn tippen(e: &mut Eingabe, text: &str) {
        for einheit in text.encode_utf16() {
            e.zeichen(einheit);
        }
    }

    // -- Das echte Fenster ------------------------------------------------
    //
    // Diese Tests machen ein Fenster auf. Sie sind der einzige Weg, die
    // Fensterprozedur überhaupt zu prüfen — und sie prüfen genau das, was
    // ohne sie ungeprüft bliebe: dass die Nachricht von Windows bis in den
    // festgenagelten Puffer durchkommt.
    //
    // Gezeigt wird das Fenster dabei tatsächlich. Auf einem Läufer ohne
    // Sitzung fällt das nicht auf; auf einem Arbeitsplatz blitzt es kurz
    // auf. Es unsichtbar zu machen wäre möglich, hieße aber, einen anderen
    // Weg zu prüfen als den ausgelieferten.

    /// Schickt eine Folge von Zeichennachrichten an das frische Fenster.
    ///
    /// `PostMessageW` und nicht `SendMessageW`: Die Nachrichten sollen in
    /// die Warteschlange, damit die Schleife sie holt — wie bei einem
    /// echten Tastendruck.
    #[cfg(windows)]
    fn tippen_ans_fenster(hwnd: windows_sys::Win32::Foundation::HWND, text: &str, schluss: u16) {
        use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CHAR};
        for einheit in text.encode_utf16() {
            // SICHERHEIT: `hwnd` stammt aus dem Haken und ist gueltig.
            #[allow(unsafe_code)]
            unsafe {
                PostMessageW(hwnd, WM_CHAR, einheit as usize, 0);
            }
        }
        // SICHERHEIT: wie oben.
        #[allow(unsafe_code)]
        unsafe {
            PostMessageW(hwnd, WM_CHAR, schluss as usize, 0);
        }
    }

    /// Die Eingabetaste, wie Windows sie als Zeichennachricht schickt.
    #[cfg(windows)]
    const EINGABE: u16 = 0x0D;
    /// Escape, ebenso.
    #[cfg(windows)]
    const ESCAPE: u16 = 0x1B;

    #[cfg(windows)]
    #[test]
    #[expect(
        clippy::expect_used,
        clippy::panic,
        reason = "Fehlschlag soll den Test abbrechen"
    )]
    fn was_getippt_wird_kommt_im_puffer_an() {
        use super::windows::{Antwort, abfragen_mit_haken};

        let antwort = abfragen_mit_haken("Passwort", 256, None, &mut |hwnd| {
            tippen_ans_fenster(hwnd, "geheim🔑", EINGABE);
        })
        .expect("Fenster");

        match antwort {
            Antwort::Eingegeben(puffer) => {
                assert_eq!(puffer.als_bytes(), "geheim🔑".as_bytes());
                assert!(
                    puffer.ist_festgenagelt(),
                    "der Puffer aus dem Fenster ist nicht festgenagelt"
                );
            }
            Antwort::Abgebrochen => panic!("es wurde bestaetigt, nicht abgebrochen"),
        }
    }

    #[cfg(windows)]
    #[test]
    #[expect(clippy::expect_used, reason = "Fehlschlag soll den Test abbrechen")]
    fn escape_bricht_ab_und_gibt_nichts_heraus() {
        use super::windows::{Antwort, abfragen_mit_haken};

        // DIE GEGENPROBE. Ohne sie bliebe offen, ob das Fenster überhaupt
        // zwischen den beiden Fällen unterscheidet — und ein Abbruch, der
        // als leeres Passwort durchginge, liefe in eine Fehlermeldung
        // statt in ein zurückgenommenes Vorhaben.
        let antwort = abfragen_mit_haken("Passwort", 256, None, &mut |hwnd| {
            tippen_ans_fenster(hwnd, "geheim", ESCAPE);
        })
        .expect("Fenster");

        assert!(
            matches!(antwort, Antwort::Abgebrochen),
            "Escape hat nicht abgebrochen"
        );
    }

    #[cfg(windows)]
    #[test]
    #[expect(
        clippy::expect_used,
        clippy::panic,
        reason = "Fehlschlag soll den Test abbrechen"
    )]
    fn die_ruecktaste_wirkt_auch_ueber_das_fenster() {
        use super::windows::{Antwort, abfragen_mit_haken};

        // Der Weg durch die echte Prozedur, nicht nur durch `Eingabe`:
        // Wäre `WM_CHAR` dort falsch verdrahtet, stünde die Rücktaste als
        // Zeichen im Passwort, und die Tests oben sähen es nicht.
        let antwort = abfragen_mit_haken("Passwort", 256, None, &mut |hwnd| {
            tippen_ans_fenster(hwnd, "geheimX\u{8}", EINGABE);
        })
        .expect("Fenster");

        match antwort {
            Antwort::Eingegeben(puffer) => assert_eq!(puffer.als_bytes(), b"geheim"),
            Antwort::Abgebrochen => panic!("es wurde bestaetigt"),
        }
    }

    #[test]
    fn getipptes_landet_im_puffer() {
        let mut e = Eingabe::neu(256);
        tippen(&mut e, "geheim");

        assert_eq!(e.als_bytes(), b"geheim");
        assert_eq!(e.punkte(), 6);
        assert!(!e.ist_leer());
    }

    #[test]
    fn steuerzeichen_landen_nicht_im_passwort() {
        // DIE ERSTE FALLE. Windows schickt Ruecktaste, Eingabe und Escape
        // als gewoehnliche Zeichennachricht. Wer sie durchlaesst, hat sie
        // im Passwort -- und niemand sieht es, denn die Punkte sehen
        // genauso aus.
        let mut e = Eingabe::neu(256);

        assert_eq!(e.zeichen(0x0D), Wirkung::Bestaetigt);
        assert_eq!(e.zeichen(0x1B), Wirkung::Abgebrochen);
        assert_eq!(e.zeichen(0x09), Wirkung::Nichts, "Tabulator");
        assert_eq!(e.zeichen(0x0A), Wirkung::Nichts, "Zeilenvorschub");
        assert_eq!(e.zeichen(0x07), Wirkung::Nichts, "Klingel");
        assert_eq!(e.zeichen(0x7F), Wirkung::Nichts, "Entfernen");

        assert!(e.ist_leer(), "eine Steuertaste ist im Puffer gelandet");
        assert_eq!(e.punkte(), 0);
    }

    #[test]
    fn ein_emoji_ergibt_einen_punkt_und_nicht_zwei() {
        // DIE ZWEITE FALLE. `WM_CHAR` liefert UTF-16-Einheiten; ein Emoji
        // kommt als ZWEI Nachrichten. Wer jede einzeln nimmt, schreibt
        // zwei ungueltige Haelften in den Puffer -- das Passwort ist dann
        // ein anderes als das getippte, und das faellt erst beim naechsten
        // Entsperren auf.
        let mut e = Eingabe::neu(256);
        tippen(&mut e, "🔑");

        assert_eq!(e.punkte(), 1, "ein Zeichen, ein Punkt");
        assert_eq!(e.als_bytes(), "🔑".as_bytes());
    }

    #[test]
    fn ein_halbes_zeichen_erscheint_noch_nicht() {
        // Zwischen den beiden Nachrichten darf kein Punkt dastehen: Der
        // Nutzer hat noch kein Zeichen getippt.
        let mut e = Eingabe::neu(256);
        let einheiten: Vec<u16> = "🔑".encode_utf16().collect();

        assert_eq!(e.zeichen(einheiten[0]), Wirkung::Nichts);
        assert_eq!(e.punkte(), 0, "ein halbes Zeichen ergibt keinen Punkt");
        assert!(e.als_bytes().is_empty(), "eine Haelfte steht im Puffer");

        assert_eq!(e.zeichen(einheiten[1]), Wirkung::Geaendert);
        assert_eq!(e.punkte(), 1);
    }

    #[test]
    fn die_ruecktaste_nach_einem_halben_zeichen_frisst_nicht_das_vorige() {
        // Der unangenehmste der Ersatzzeichen-Faelle: Wer nach der ersten
        // Haelfte abbricht, hat noch nichts getippt -- und darf nicht das
        // Zeichen DAVOR verlieren.
        let mut e = Eingabe::neu(256);
        tippen(&mut e, "ab");
        let hoch = "🔑".encode_utf16().next().unwrap();

        e.zeichen(hoch);
        assert_eq!(e.zeichen(0x08), Wirkung::Nichts);

        assert_eq!(e.als_bytes(), b"ab", "das Zeichen davor ist verschwunden");
        assert_eq!(e.punkte(), 2);
    }

    #[test]
    fn ein_niedriges_ersatzzeichen_allein_wird_uebergangen() {
        // Windows schickt das nicht von sich aus -- es kaeme von einem
        // fremden Programm, das Tastendruecke einspeist. Uebergehen statt
        // raten: Ein „?" im Passwort ergaebe ein anderes als das getippte.
        let mut e = Eingabe::neu(256);
        let niedrig = "🔑".encode_utf16().nth(1).unwrap();

        assert_eq!(e.zeichen(niedrig), Wirkung::Nichts);
        assert!(e.ist_leer());
    }

    #[test]
    fn die_ruecktaste_nimmt_ganze_zeichen() {
        let mut e = Eingabe::neu(256);
        tippen(&mut e, "aä🔑");
        assert_eq!(e.punkte(), 3);

        assert_eq!(e.zeichen(0x08), Wirkung::Geaendert);
        assert_eq!(e.als_bytes(), "aä".as_bytes(), "Emoji nur halb entfernt");
        assert_eq!(e.punkte(), 2);

        e.zeichen(0x08);
        assert_eq!(e.als_bytes(), b"a", "Umlaut nur halb entfernt");
    }

    #[test]
    fn auf_dem_leeren_feld_richtet_die_ruecktaste_nichts_an() {
        let mut e = Eingabe::neu(256);
        assert_eq!(e.zeichen(0x08), Wirkung::Nichts);
        assert!(e.ist_leer());
        assert_eq!(e.punkte(), 0);
    }

    #[test]
    fn ein_volles_feld_sagt_es_statt_stillzuhalten() {
        // Wer weitertippt, ohne dass sich etwas ruehrt, muss es merken.
        // Ein stilles Verschlucken waere die schlechtere Antwort: Er
        // glaubte, sein Passwort sei laenger als es ist.
        let mut e = Eingabe::neu(4);
        tippen(&mut e, "abcd");
        assert_eq!(e.punkte(), 4);

        assert_eq!(e.zeichen(u16::from(b'e')), Wirkung::Voll);
        assert_eq!(e.punkte(), 4, "es wurde doch etwas aufgenommen");
        assert_eq!(e.als_bytes(), b"abcd");
    }

    #[test]
    fn die_punkte_zaehlen_zeichen_und_nicht_bytes() {
        // Sonst waeren drei Emoji zwoelf Punkte, und ein Umlaut zwei.
        let mut e = Eingabe::neu(256);
        tippen(&mut e, "äöü🔑");

        assert_eq!(e.punkte(), 4);
        assert_eq!(e.als_bytes().len(), 10, "vier Zeichen, zehn Bytes");
    }

    #[test]
    fn debug_zeigt_das_passwort_nicht() {
        let mut e = Eingabe::neu(256);
        tippen(&mut e, "strenggeheim");

        let text = format!("{e:?}");
        assert!(
            !text.contains("geheim"),
            "Debug hat das Passwort ausgedruckt: {text}"
        );
    }
}
