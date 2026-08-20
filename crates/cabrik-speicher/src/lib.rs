//! Speicher, den das Betriebssystem nicht auf die Platte auslagern darf.
//!
//! # Warum es diese Kiste gibt
//!
//! Weil das Passwort sonst in der Auslagerungsdatei landen kann, bevor wir
//! es überschreiben. Dagegen hilft genau ein Mittel: die Seiten mit dem
//! Puffer im Arbeitsspeicher festzunageln. Das sind Systemaufrufe, und
//! Systemaufrufe gehen in Rust nicht ohne `unsafe`.
//!
//! Der Arbeitsbereich setzt `unsafe_code = "forbid"`. Diese Kiste ist die
//! **einzige** Ausnahme, sie steht auf `deny`, und die sechs Aufhebungen
//! stehen unten alle beieinander. Die Begründung dafür steht in ihrer
//! `Cargo.toml`; die Spezifikation hat diesen Zuschnitt vorgezeichnet
//! (`spec/entsperrung.md` §5.2).
//!
//! # Was das erreicht — und was nicht
//!
//! **Erreicht:** Die gewöhnliche Auslagerung im laufenden Betrieb fasst
//! diese Seiten nicht an.
//!
//! **Nicht erreicht:** den Ruhezustand. Das Ruhezustandsabbild ist eine
//! Kopie des *physischen* Arbeitsspeichers, und Festnageln garantiert
//! gerade, dass die Seite dort liegt. Dagegen hilft nur, vorher zu sperren
//! und zu überschreiben (`spec/entsperrung.md` §3.4). Dasselbe gilt für
//! Absturzabbilder.
//!
//! **Und es kann schlicht fehlschlagen.** Unter Linux begrenzt
//! `RLIMIT_MEMLOCK`, wie viel ein Prozess festnageln darf; unter Windows
//! die Größe des Arbeitssatzes. Deshalb gibt es
//! [`Festgenagelt::ist_festgenagelt`] und keine stillschweigende Zusage:
//! Wer nicht weiß, ob es geklappt hat, darf nicht behaupten, es habe.
//!
//! # Warum ganze Seiten
//!
//! Weil `mlock` und `VirtualLock` auf Seiten arbeiten, nicht auf Bytes, und
//! weil **keiner der beiden mitzählt**: Ein einziges `munlock` löst eine
//! Seite wieder, gleichgültig wie oft sie vorher genagelt wurde. Lägen zwei
//! Puffer auf derselben Seite, würde das Wegwerfen des einen dem anderen
//! stillschweigend den Schutz nehmen — und beim Passwortwechsel gibt es
//! genau zwei gleichzeitig, das alte und das neue.
//!
//! Deshalb belegt jeder Puffer **ganze Seiten für sich allein**. Erreicht
//! wird das ohne weiteres `unsafe`: Es wird schlicht zwei Seiten mehr
//! angefordert als gebraucht, und darin die auf eine Seitengrenze
//! ausgerichtete Innenfläche benutzt.

use zeroize::Zeroize;

/// Der Puffer ist voll.
///
/// Er wächst nicht nach. Das ist Absicht: Ein wachsender Puffer zieht um,
/// und der alte Inhalt bliebe unüberschrieben im freigegebenen Speicher
/// liegen — genau der Fehler, den ein `String` hier machen würde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Voll;

impl core::fmt::Display for Voll {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("der festgenagelte Puffer ist voll")
    }
}

impl core::error::Error for Voll {}

/// Ein Puffer fester Größe, dessen Seiten festgenagelt sind.
///
/// Er überschreibt sich selbst, wenn er weggeworfen wird, und löst die
/// Seiten erst danach wieder.
pub struct Festgenagelt {
    /// Die Anforderung samt Zugabe. Sie zieht nie um: Die Größe steht bei
    /// der Erzeugung fest und ändert sich nie wieder.
    speicher: Vec<u8>,
    /// Wo darin die ausgerichtete Innenfläche beginnt.
    beginn: usize,
    /// Wie viele Bytes ab `beginn` festgenagelt wurden — immer ein
    /// Vielfaches der Seitengröße.
    genagelte_bytes: usize,
    /// Wie viel davon benutzt werden darf.
    kapazitaet: usize,
    /// Wie viel davon belegt ist.
    laenge: usize,
    /// Ob das Festnageln geklappt hat.
    genagelt: bool,
}

impl Festgenagelt {
    /// Legt einen Puffer für `kapazitaet` Bytes an und nagelt ihn fest.
    ///
    /// Schlägt das Festnageln fehl, entsteht der Puffer trotzdem — nur
    /// meldet [`Self::ist_festgenagelt`] dann `false`. Ein Passwortfenster,
    /// das gar nicht erst aufginge, weil `RLIMIT_MEMLOCK` zu klein ist,
    /// wäre die schlechtere Antwort.
    #[must_use]
    pub fn neu(kapazitaet: usize) -> Self {
        let seite = system::seitengroesse();

        // Zwei Seiten Zugabe. Die erste schiebt den Anfang auf eine
        // Seitengrenze, die zweite sorgt dafuer, dass auch die letzte
        // angefasste Seite noch ganz im eigenen Puffer liegt.
        let brutto = kapazitaet.saturating_add(seite.saturating_mul(2));
        let speicher = vec![0_u8; brutto];

        // `align_offset` ist sicher und darf `usize::MAX` liefern, wenn die
        // Ausrichtung nicht erreichbar ist. Auf keinem Ziel dieses
        // Programms ist das der Fall -- der Zweig wird trotzdem behandelt,
        // statt ihn wegzunehmen.
        let versatz = speicher.as_ptr().align_offset(seite);
        let (beginn, genagelte_bytes) = match kapazitaet.checked_next_multiple_of(seite) {
            Some(ganze) if versatz != usize::MAX => (versatz, ganze),
            _ => (0, 0),
        };

        let genagelt = speicher
            .get(beginn..beginn.saturating_add(genagelte_bytes))
            .is_some_and(|bereich| !bereich.is_empty() && system::nageln(bereich));

        Self {
            speicher,
            beginn,
            genagelte_bytes,
            kapazitaet,
            laenge: 0,
            genagelt,
        }
    }

    /// Ob die Seiten tatsächlich festgenagelt sind.
    ///
    /// **Nicht schmücken.** Wenn das hier `false` ist, darf die Oberfläche
    /// nicht behaupten, das Passwort sei gegen Auslagerung geschützt.
    #[must_use]
    pub const fn ist_festgenagelt(&self) -> bool {
        self.genagelt
    }

    /// Wie viele Bytes noch hineinpassen.
    #[must_use]
    pub const fn freier_platz(&self) -> usize {
        self.kapazitaet.saturating_sub(self.laenge)
    }

    /// Hängt Text an.
    ///
    /// # Fehler
    ///
    /// [`Voll`], wenn der Platz nicht reicht. Der Puffer bleibt dann
    /// unverändert — es wird **nichts** teilweise geschrieben.
    pub fn anhaengen(&mut self, text: &str) -> Result<(), Voll> {
        let ende = self.laenge.checked_add(text.len()).ok_or(Voll)?;
        if ende > self.kapazitaet {
            return Err(Voll);
        }
        let von = self.beginn.saturating_add(self.laenge);
        let bis = self.beginn.saturating_add(ende);
        let ziel = self.speicher.get_mut(von..bis).ok_or(Voll)?;
        ziel.copy_from_slice(text.as_bytes());
        self.laenge = ende;
        Ok(())
    }

    /// Nimmt das letzte **Zeichen** zurück, nicht das letzte Byte.
    ///
    /// Ein Rückschritt hinter einem Umlaut darf nicht das halbe Zeichen
    /// stehenlassen. Die freigewordenen Bytes werden sofort überschrieben
    /// und nicht erst beim Wegwerfen.
    pub fn letztes_zeichen_loeschen(&mut self) {
        let mut neue = self.laenge;
        {
            let bytes = self.als_bytes();
            while let Some(vorher) = neue.checked_sub(1) {
                neue = vorher;
                // Fortsetzungsbytes von UTF-8 beginnen mit `10`.
                match bytes.get(neue) {
                    Some(b) if (b & 0b1100_0000) != 0b1000_0000 => break,
                    Some(_) => {}
                    None => break,
                }
            }
        }
        self.kuerzen_auf(neue);
    }

    /// Überschreibt den Inhalt und setzt die Länge auf null.
    pub fn leeren(&mut self) {
        self.kuerzen_auf(0);
    }

    /// Der Inhalt als Bytes.
    ///
    /// Bytes und nicht `&str`, weil der Kern Passwörter ohnehin als `&[u8]`
    /// entgegennimmt (`cabrik_core::passwort`) und jede Umwandlung in einen
    /// `String` eine zweite, ungenagelte Kopie erzeugte.
    #[must_use]
    pub fn als_bytes(&self) -> &[u8] {
        self.speicher
            .get(self.beginn..self.beginn.saturating_add(self.laenge))
            .unwrap_or(&[])
    }

    /// Zählt die **Zeichen**, nicht die Bytes.
    ///
    /// Dieselbe Zählung wie `cabrik_core::passwort::zeichen`, damit die
    /// Anzeige der Punkte und die Mindestlänge dasselbe meinen.
    #[must_use]
    pub fn zeichen(&self) -> usize {
        let bytes = self.als_bytes();
        core::str::from_utf8(bytes).map_or_else(|_| bytes.len(), |s| s.chars().count())
    }

    /// Ob nichts drinsteht.
    #[must_use]
    pub const fn ist_leer(&self) -> bool {
        self.laenge == 0
    }

    /// Kürzt auf `neue_laenge` und überschreibt dabei den Rest.
    fn kuerzen_auf(&mut self, neue_laenge: usize) {
        let von = self.beginn.saturating_add(neue_laenge);
        let bis = self.beginn.saturating_add(self.laenge);
        if let Some(rest) = self.speicher.get_mut(von..bis) {
            rest.zeroize();
        }
        self.laenge = neue_laenge;
    }
}

impl Drop for Festgenagelt {
    /// Erst überschreiben, dann lösen — nicht umgekehrt.
    ///
    /// Zwischen dem Lösen und dem Überschreiben läge sonst ein Zeitraum, in
    /// dem die Seite mit dem Passwort darin wieder auslagerbar wäre. Er
    /// wäre kurz, aber er wäre genau die Lücke, die zu schließen der Zweck
    /// dieser Kiste ist.
    fn drop(&mut self) {
        self.speicher.zeroize();
        if self.genagelt {
            let von = self.beginn;
            let bis = self.beginn.saturating_add(self.genagelte_bytes);
            if let Some(bereich) = self.speicher.get(von..bis) {
                system::loesen(bereich);
            }
        }
    }
}

/// `Debug` von Hand: Das abgeleitete druckte den Puffer mit aus.
impl core::fmt::Debug for Festgenagelt {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Festgenagelt")
            .field("kapazitaet", &self.kapazitaet)
            .field("laenge", &self.laenge)
            .field("genagelt", &self.genagelt)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Ab hier, und nur hier, `unsafe`.
//
// Sechs Aufhebungen, jede mit ihrer Begründung darüber. Wer diese Kiste
// prüft, muss genau diesen Block lesen und sonst nichts.
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod system {
    use windows_sys::Win32::System::Memory::{VirtualLock, VirtualUnlock};
    use windows_sys::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};

    pub(super) fn seitengroesse() -> usize {
        let mut info = core::mem::MaybeUninit::<SYSTEM_INFO>::uninit();
        // SICHERHEIT: `GetSystemInfo` schreibt in den uebergebenen Zeiger
        // und liest ihn nicht. Der Zeiger stammt aus einem gueltigen,
        // ausreichend grossen und richtig ausgerichteten `MaybeUninit`.
        #[allow(unsafe_code)]
        unsafe {
            GetSystemInfo(info.as_mut_ptr());
        }
        // SICHERHEIT: `GetSystemInfo` fuellt die Struktur vollstaendig aus;
        // sie besteht ausschliesslich aus einfachen Zahlenfeldern ohne
        // ungueltige Bitmuster.
        #[allow(unsafe_code)]
        let info = unsafe { info.assume_init() };
        // Sollte das System je 0 melden, waere jede Rechnung damit falsch.
        // Dann lieber der uebliche Wert.
        match usize::try_from(info.dwPageSize) {
            Ok(n) if n > 0 => n,
            _ => 4096,
        }
    }

    pub(super) fn nageln(bereich: &[u8]) -> bool {
        // SICHERHEIT: `bereich` ist ein lebender, zusammenhaengender
        // Speicherbereich der angegebenen Laenge. `VirtualLock` veraendert
        // seinen Inhalt nicht -- es beeinflusst nur, ob die Seiten
        // ausgelagert werden duerfen. Deshalb genuegt eine geteilte
        // Referenz.
        #[allow(unsafe_code)]
        let ergebnis = unsafe { VirtualLock(bereich.as_ptr().cast(), bereich.len()) };
        ergebnis != 0
    }

    pub(super) fn loesen(bereich: &[u8]) {
        // SICHERHEIT: wie oben. Der Rueckgabewert wird bewusst verworfen:
        // Beim Wegwerfen ist nichts mehr zu retten, und der Inhalt ist zu
        // diesem Zeitpunkt bereits ueberschrieben.
        #[allow(unsafe_code)]
        unsafe {
            VirtualUnlock(bereich.as_ptr().cast(), bereich.len());
        }
    }
}

#[cfg(unix)]
mod system {
    pub(super) fn seitengroesse() -> usize {
        // SICHERHEIT: `sysconf` liest nur eine Systemgroesse und fasst
        // keinen uebergebenen Speicher an.
        #[allow(unsafe_code)]
        let roh = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        match usize::try_from(roh) {
            Ok(n) if n > 0 => n,
            _ => 4096,
        }
    }

    pub(super) fn nageln(bereich: &[u8]) -> bool {
        // SICHERHEIT: `bereich` ist ein lebender, zusammenhaengender
        // Speicherbereich der angegebenen Laenge. `mlock` veraendert seinen
        // Inhalt nicht.
        #[allow(unsafe_code)]
        let ergebnis = unsafe { libc::mlock(bereich.as_ptr().cast(), bereich.len()) };
        ergebnis == 0
    }

    pub(super) fn loesen(bereich: &[u8]) {
        // SICHERHEIT: wie oben. Rueckgabewert bewusst verworfen.
        #[allow(unsafe_code)]
        unsafe {
            libc::munlock(bereich.as_ptr().cast(), bereich.len());
        }
    }
}

#[cfg(not(any(windows, unix)))]
mod system {
    pub(super) const fn seitengroesse() -> usize {
        4096
    }

    /// Ohne bekannten Systemaufruf wird **nicht** festgenagelt — und das
    /// wird auch so gemeldet, statt es zu behaupten.
    pub(super) const fn nageln(_bereich: &[u8]) -> bool {
        false
    }

    pub(super) const fn loesen(_bereich: &[u8]) {}
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod pruefungen {
    use super::{Festgenagelt, Voll, system};

    /// Wo die festgenagelten Seiten im Adressraum liegen.
    fn genagelter_bereich(p: &Festgenagelt) -> (usize, usize) {
        let start = p.speicher.as_ptr().addr() + p.beginn;
        (start, start + p.genagelte_bytes)
    }

    #[test]
    fn anhaengen_und_lesen() {
        let mut p = Festgenagelt::neu(64);
        p.anhaengen("hallo").unwrap();
        p.anhaengen(" welt").unwrap();
        assert_eq!(p.als_bytes(), b"hallo welt");
        assert_eq!(p.zeichen(), 10);
        assert!(!p.ist_leer());
    }

    #[test]
    fn ueber_die_kapazitaet_hinaus_wird_nichts_geschrieben() {
        let mut p = Festgenagelt::neu(8);
        p.anhaengen("1234").unwrap();
        // Passt nicht mehr: 4 + 6 > 8.
        assert_eq!(p.anhaengen("abcdef"), Err(Voll));
        // Und zwar GAR nicht -- kein angefangenes Stueck.
        assert_eq!(p.als_bytes(), b"1234", "teilweise geschrieben");
        assert_eq!(p.freier_platz(), 4);
        // Was genau passt, geht noch.
        p.anhaengen("abcd").unwrap();
        assert_eq!(p.freier_platz(), 0);
    }

    #[test]
    fn rueckschritt_nimmt_ganze_zeichen() {
        let mut p = Festgenagelt::neu(64);
        // Ein Emoji sind vier Bytes, ein Umlaut zwei.
        p.anhaengen("aä🔑").unwrap();
        assert_eq!(p.als_bytes().len(), 7);

        p.letztes_zeichen_loeschen();
        assert_eq!(p.als_bytes(), "aä".as_bytes(), "Emoji nur halb entfernt");

        p.letztes_zeichen_loeschen();
        assert_eq!(p.als_bytes(), b"a", "Umlaut nur halb entfernt");

        p.letztes_zeichen_loeschen();
        assert!(p.ist_leer());

        // Auf dem leeren Puffer darf er nichts anrichten.
        p.letztes_zeichen_loeschen();
        assert!(p.ist_leer());
    }

    #[test]
    fn rueckschritt_ueberschreibt_sofort() {
        // GEGENPROBE ZUM VORIGEN: Dass die Laenge stimmt, heisst nicht, dass
        // die Bytes weg sind. Ohne das Ueberschreiben stuende das Passwort
        // noch im Puffer, nur hinter der Laengenmarke -- und beim naechsten
        // Anhaengen waere es zufaellig wieder sichtbar.
        let mut p = Festgenagelt::neu(64);
        p.anhaengen("geheim").unwrap();
        p.letztes_zeichen_loeschen();
        p.letztes_zeichen_loeschen();

        let ab = p.beginn + p.laenge;
        let dahinter = &p.speicher[ab..ab + 4];
        assert_eq!(dahinter, &[0, 0, 0, 0], "Reste hinter der Laengenmarke");
    }

    #[test]
    fn leeren_ueberschreibt_alles() {
        let mut p = Festgenagelt::neu(64);
        p.anhaengen("geheim").unwrap();
        p.leeren();
        assert!(p.ist_leer());
        assert_eq!(p.als_bytes(), b"");
        let bereich = &p.speicher[p.beginn..p.beginn + 6];
        assert_eq!(bereich, &[0; 6], "Inhalt stand noch da");
    }

    #[test]
    fn ungueltiges_utf8_wird_byteweise_gezaehlt() {
        // Kann ueber die Zwischenablage hereinkommen. Es darf nicht in
        // Panik enden, und `zeichen` faellt dann auf die Bytezahl zurueck --
        // dieselbe Regel wie in `cabrik_core::passwort::zeichen`.
        let mut p = Festgenagelt::neu(16);
        p.anhaengen("ä").unwrap();
        p.kuerzen_auf(1); // haelftiges Zeichen stehenlassen
        assert_eq!(p.zeichen(), 1);
    }

    #[test]
    fn die_flaeche_liegt_auf_ganzen_seiten() {
        // DIE ZUSAGE DIESER KISTE. Wenn das hier nicht gilt, teilt sich der
        // Puffer eine Seite mit fremdem Speicher -- und weil `munlock`
        // nicht mitzaehlt, nimmt jedes Wegwerfen daneben ihm den Schutz.
        let seite = system::seitengroesse();
        assert!(seite.is_power_of_two(), "Seitengroesse {seite} ist krumm");

        for kapazitaet in [1_usize, 7, 63, 64, 4095, 4096, 4097, 20_000] {
            let p = Festgenagelt::neu(kapazitaet);
            let (start, ende) = genagelter_bereich(&p);

            assert_eq!(start % seite, 0, "Anfang nicht auf einer Seitengrenze");
            assert_eq!(p.genagelte_bytes % seite, 0, "keine ganzen Seiten");
            assert!(
                p.genagelte_bytes >= kapazitaet,
                "die Nutzflaeche ragt aus dem genagelten Bereich heraus"
            );
            // Und alles davon liegt im eigenen Puffer -- sonst waere der
            // Aufruf auf fremdem Speicher gelandet.
            let puffer_ende = p.speicher.as_ptr().addr() + p.speicher.len();
            assert!(ende <= puffer_ende, "genagelter Bereich ragt heraus");
        }
    }

    #[test]
    fn zwei_puffer_teilen_sich_keine_seite() {
        // Beim Passwortwechsel leben zwei gleichzeitig, das alte und das
        // neue. Laegen sie auf derselben Seite, loeste das Wegwerfen des
        // einen die Seite des anderen gleich mit.
        //
        // Sie muessen dabei gleichzeitig LEBEN. Einzeln nacheinander
        // erzeugt, gaebe der Allokator jedem denselben Platz, und der Test
        // pruefte nichts. Am Leben gehalten werden sie in einem `Vec` und
        // nicht mit `mem::forget`: Vergessene Puffer blieben bis zum
        // Prozessende festgenagelt, und unter Linux ist die Menge, die ein
        // Prozess festnageln darf, durch `RLIMIT_MEMLOCK` begrenzt.
        let puffer: Vec<Festgenagelt> = (0..8).map(|_| Festgenagelt::neu(32)).collect();
        let alle: Vec<(usize, usize)> = puffer.iter().map(genagelter_bereich).collect();

        for (i, a) in alle.iter().enumerate() {
            for b in alle.iter().skip(i + 1) {
                assert!(
                    a.1 <= b.0 || b.1 <= a.0,
                    "zwei Puffer ueberlappen: {a:?} und {b:?}"
                );
            }
        }
    }

    #[test]
    fn kapazitaet_null_geht_nicht_in_panik() {
        let mut p = Festgenagelt::neu(0);
        assert!(p.ist_leer());
        assert_eq!(p.freier_platz(), 0);
        assert_eq!(p.anhaengen("x"), Err(Voll));
        p.letztes_zeichen_loeschen();
        p.leeren();
    }

    #[test]
    fn debug_zeigt_den_inhalt_nicht() {
        let mut p = Festgenagelt::neu(64);
        p.anhaengen("streng-geheim").unwrap();
        let text = format!("{p:?}");
        assert!(
            !text.contains("geheim"),
            "Debug hat das Passwort ausgedruckt: {text}"
        );
    }

    #[test]
    fn auf_diesem_system_wird_wirklich_festgenagelt() {
        // Kein weiches Urteil: Eine einzelne Seite festzunageln muss auf
        // jedem System gelingen, auf dem dieses Programm laeuft. Schlaegt
        // es fehl, ist das eine Erkenntnis und kein Grund, den Test
        // nachgiebig zu machen.
        let p = Festgenagelt::neu(64);
        assert!(
            p.ist_festgenagelt(),
            "Festnageln fehlgeschlagen -- unter Linux zuerst RLIMIT_MEMLOCK ansehen"
        );
    }

    /// Der Beweis von aussen, nicht aus unserem eigenen Rueckgabewert.
    ///
    /// `VmLck` in `/proc/self/status` ist die Zahl des Kerns selbst. Ohne
    /// diesen Test bewiese der vorige nur, dass unsere Funktion `true`
    /// zurueckgibt -- nicht, dass irgendetwas festgenagelt wurde.
    #[cfg(target_os = "linux")]
    #[test]
    fn der_kern_bestaetigt_das_festnageln() {
        fn vmlck_kib() -> u64 {
            let status = std::fs::read_to_string("/proc/self/status").unwrap();
            status
                .lines()
                .find_map(|zeile| zeile.strip_prefix("VmLck:"))
                .and_then(|rest| rest.trim().trim_end_matches(" kB").trim().parse().ok())
                .unwrap()
        }

        let vorher = vmlck_kib();
        // Klein halten: `RLIMIT_MEMLOCK` ist auf manchen Systemen nur 64 KiB,
        // und die Tests laufen als Faeden EINES Prozesses -- das Kontingent
        // teilen sie sich also.
        let p = Festgenagelt::neu(16 * 1024);
        let waehrend = vmlck_kib();
        assert!(
            waehrend > vorher,
            "der Kern meldet keinen Zuwachs: {vorher} -> {waehrend}"
        );
        drop(p);
        assert_eq!(vmlck_kib(), vorher, "beim Wegwerfen nicht wieder geloest");
    }
}
