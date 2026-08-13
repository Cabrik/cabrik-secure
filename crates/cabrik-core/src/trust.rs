//! Trust Store nach `spec/trust-store.md`.
//!
//! Der wichtigste konzeptionelle Fix gegenüber v1.
//!
//! # Das Problem, das dieses Modul löst
//!
//! In v1 stammte der Signaturprüfschlüssel aus dem Header **derselben
//! Nachricht**. Ein Angreifer erzeugte ein eigenes Ed25519-Paar, signierte
//! damit und legte den passenden Public Key bei — v1 meldete
//! `signature_valid: true`. Die Prüfung belegte ausschließlich, dass
//! Signatur und mitgelieferter Schlüssel zusammenpassten. Über die **Person**
//! sagte sie nichts.
//!
//! Kryptographie kann das nicht lösen. Die Zuordnung Schlüssel ↔ Mensch
//! entsteht ausschließlich durch einen Vorgang **außerhalb** des Kanals.
//! Genau den bildet dieses Modul ab.
//!
//! # Kein Wahrheitswert
//!
//! [`Authenticity`] hat sechs Ausprägungen und **keine** Reduktion auf `bool`.
//! Eine gültige Signatur eines unbekannten Schlüssels ist keine
//! Authentizität — sie besagt nur, dass derselbe Schlüssel die Nachricht
//! erzeugt hat.

use crate::error::{Error, Result};
use crate::fingerprint::Fingerprint;
use crate::keyfile::Identity;
use crate::tlv::{TlvReader, TlvWriter, expect_len};
use crate::{base32, kem};

use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop};

const NAME_MAX: usize = 128;
const NOTE_MAX: usize = 512;
const REVOCATION_NOTE_MAX: usize = 256;
/// Länge eines ML-KEM-768-Anteils in Bytes.
///
/// **Öffentlich, weil `Contact::new_seen` sie im Typ führt.** Ohne sie
/// lässt sich von außerhalb dieser Crate kein Kontakt mit
/// Post-Quantum-Schlüssel anlegen — der gesamte Pfad wäre über die
/// Crate-Grenze hinweg unerreichbar, und das fiel erst auf, als es jemand
/// versuchte.
pub const PQ_PUB_LEN: usize = 1216;

// Dieselbe Länge steht in `xwing` und in `fingerprint`. Drei Kopien einer
// Zahl, die übereinstimmen MUSS -- ab jetzt sagt das der Übersetzer und
// nicht die Hoffnung.
const _: () = assert!(PQ_PUB_LEN == crate::xwing::PK_LEN);

/// Höchstzahl früherer Schlüssel je Kontakt.
///
/// Begrenzt den Speicherbedarf beim Parsen einer präparierten Datei.
const MAX_HISTORY: usize = 64;

/// TLV-Typen eines Kontakteintrags (`spec/trust-store.md` §6).
mod tag {
    pub(super) const ENC_PUB: u8 = 0x01;
    pub(super) const SIG_PUB: u8 = 0x02;
    pub(super) const NAME: u8 = 0x03;
    pub(super) const STATE: u8 = 0x04;
    pub(super) const FIRST_SEEN: u8 = 0x05;
    pub(super) const VERIFIED_AT: u8 = 0x06;
    pub(super) const VERIFIED_VIA: u8 = 0x07;
    pub(super) const NOTE: u8 = 0x08;
    pub(super) const PREVIOUS_KEYS: u8 = 0x09;
    pub(super) const XWING_PUB: u8 = 0x0A;
    pub(super) const REVOKED_AT: u8 = 0x0B;
    pub(super) const REVOCATION_NOTE: u8 = 0x0C;
}

// ---------------------------------------------------------------------------
// Zustände
// ---------------------------------------------------------------------------

/// Vertrauenszustand eines Kontakts (`spec/trust-store.md` §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustState {
    /// Im Speicher, aber nie verifiziert (Trust on First Use).
    ///
    /// Erlaubt es, wiederkehrende Absender wiederzuerkennen, **ohne
    /// Sicherheit vorzutäuschen**.
    Seen,
    /// Fingerprint oder Safety Number wurde außerhalb des Kanals abgeglichen.
    Verified,
    /// **Warnzustand.** Der Kontakt tritt mit anderem Schlüssel auf als zuvor.
    Changed,
    /// Schlüssel als kompromittiert markiert.
    ///
    /// Rein **lokal** — ein Widerruf ohne Verteilweg erreicht niemanden sonst
    /// (`spec/trust-store.md` §4.3).
    Revoked,
}

impl TrustState {
    const fn to_byte(self) -> u8 {
        match self {
            Self::Seen => 1,
            Self::Verified => 2,
            Self::Changed => 3,
            Self::Revoked => 4,
        }
    }

    const fn from_byte(b: u8) -> Result<Self> {
        match b {
            1 => Ok(Self::Seen),
            2 => Ok(Self::Verified),
            3 => Ok(Self::Changed),
            4 => Ok(Self::Revoked),
            _ => Err(Error::Malformed("trust: unknown state")),
        }
    }
}

/// Auf welchem Weg verifiziert wurde (`spec/trust-store.md` §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifiedVia {
    /// QR-Code, erfordert physische Nähe.
    QrCode,
    /// Safety Number vorgelesen.
    SafetyNumber,
    /// Fingerprint abgetippt.
    Fingerprint,
}

impl VerifiedVia {
    const fn to_byte(self) -> u8 {
        match self {
            Self::QrCode => 1,
            Self::SafetyNumber => 2,
            Self::Fingerprint => 3,
        }
    }

    const fn from_byte(b: u8) -> Result<Self> {
        match b {
            1 => Ok(Self::QrCode),
            2 => Ok(Self::SafetyNumber),
            3 => Ok(Self::Fingerprint),
            _ => Err(Error::Malformed("trust: unknown verification method")),
        }
    }
}

/// Ein früherer Schlüsselsatz eines Kontakts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviousKey {
    /// Fingerprint des abgelösten Schlüsselsatzes.
    pub fingerprint: Fingerprint,
    /// Der Ed25519-Signierschlüssel von damals, sofern vorhanden.
    ///
    /// Wird zum Nachschlagen gebraucht: Trifft eine Nachricht mit diesem
    /// Schlüssel ein, ist das ein Warnfall (`spec/trust-store.md` §7.2).
    pub sig_pub: Option<[u8; 32]>,
    /// Wann er abgelöst wurde, Unix-Sekunden.
    pub replaced_at: u64,
    /// Ob er damals **verifiziert** war.
    ///
    /// Der Wechsel eines verifizierten Schlüssels wiegt schwerer als der
    /// eines nie verifizierten und muss deutlicher gewarnt werden.
    pub was_verified: bool,
}

// ---------------------------------------------------------------------------
// Kontakt
// ---------------------------------------------------------------------------

/// Ein Eintrag im Kontaktspeicher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contact {
    /// X25519-Public-Key.
    pub enc_pub: [u8; 32],
    /// Ed25519-Signierschlüssel. Fehlt bei Anonymitätsidentitäten.
    pub sig_pub: Option<[u8; 32]>,
    /// X-Wing-Public-Key.
    ///
    /// Fehlt bei aus v1 übernommenen Kontakten. Dann ist nur Suite `0x0001`
    /// möglich — die Oberfläche sollte das anzeigen.
    pub xwing_pub: Option<Box<[u8; PQ_PUB_LEN]>>,
    /// Anzeigename.
    pub name: String,
    /// Vertrauenszustand.
    pub state: TrustState,
    /// Erstkontakt, Unix-Sekunden.
    pub first_seen: u64,
    /// Zeitpunkt der Verifikation.
    pub verified_at: Option<u64>,
    /// Weg der Verifikation.
    pub verified_via: Option<VerifiedVia>,
    /// Freie Notiz.
    pub note: Option<String>,
    /// Schlüsselhistorie. **Wird nie überschrieben.**
    pub previous_keys: Vec<PreviousKey>,
    /// Zeitpunkt des lokalen Widerrufs.
    pub revoked_at: Option<u64>,
    /// Begründung des Widerrufs.
    pub revocation_note: Option<String>,
}

impl Contact {
    /// Legt einen Kontakt nach Trust on First Use an — als `Seen`, **nicht**
    /// als verifiziert.
    ///
    /// # Fehler
    ///
    /// [`Error::Malformed`] bei zu langem Namen.
    pub fn new_seen(
        name: &str,
        enc_pub: [u8; 32],
        sig_pub: Option<[u8; 32]>,
        xwing_pub: Option<Box<[u8; PQ_PUB_LEN]>>,
        first_seen: u64,
    ) -> Result<Self> {
        if name.len() > NAME_MAX {
            return Err(Error::Malformed("trust: name too long"));
        }
        Ok(Self {
            enc_pub,
            sig_pub,
            xwing_pub,
            name: name.to_owned(),
            state: TrustState::Seen,
            first_seen,
            verified_at: None,
            verified_via: None,
            note: None,
            previous_keys: Vec::new(),
            revoked_at: None,
            revocation_note: None,
        })
    }

    /// Fingerprint dieses Kontakts (`spec/trust-store.md` §2).
    #[must_use]
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::compute(
            &self.enc_pub,
            self.sig_pub.as_ref(),
            self.xwing_pub.as_deref(),
        )
    }

    /// Ob an diesen Kontakt post-quantum verschlüsselt werden kann.
    #[must_use]
    pub const fn supports_post_quantum(&self) -> bool {
        self.xwing_pub.is_some()
    }

    /// Markiert den Kontakt als verifiziert.
    ///
    /// Ein **widerrufener** Kontakt lässt sich nicht ohne Weiteres wieder
    /// verifizieren — der Widerruf ist monoton (`spec/trust-store.md` §4.3).
    ///
    /// # Fehler
    ///
    /// [`Error::Malformed`], wenn der Kontakt widerrufen ist.
    pub fn verify(&mut self, via: VerifiedVia, at: u64) -> Result<()> {
        if self.state == TrustState::Revoked {
            return Err(Error::Malformed("trust: contact is revoked"));
        }
        self.state = TrustState::Verified;
        self.verified_at = Some(at);
        self.verified_via = Some(via);
        Ok(())
    }

    /// Nimmt eine Verifikation zurück — der Kontakt gilt wieder als
    /// **gesehen**.
    ///
    /// Für den Fall, dass ein Abgleich der Safety Number **nicht**
    /// übereinstimmt. Das ist ausdrücklich **kein Widerruf**: Widerrufen
    /// hieße „dieser Schlüssel ist kompromittiert“, und das weiß niemand.
    /// Bekannt ist nur, dass die Prüfung fehlgeschlagen ist — und der
    /// ehrliche Zustand dafür ist derselbe wie vor der Prüfung.
    ///
    /// Ein **widerrufener** Kontakt bleibt widerrufen: Der Widerruf ist
    /// monoton (`spec/trust-store.md` §4.3), und ihn über diesen Weg
    /// aufzuheben wäre eine Hintertür in genau die Sperre, die schützen
    /// soll.
    ///
    /// # Fehler
    ///
    /// [`Error::Malformed`], wenn der Kontakt widerrufen ist.
    pub fn unverify(&mut self) -> Result<()> {
        if self.state == TrustState::Revoked {
            return Err(Error::Malformed("trust: contact is revoked"));
        }
        self.state = TrustState::Seen;
        self.verified_at = None;
        self.verified_via = None;
        Ok(())
    }

    /// Markiert den Kontakt lokal als widerrufen.
    ///
    /// Erreicht **niemanden sonst**. Die Oberfläche muss das klarstellen.
    ///
    /// # Fehler
    ///
    /// [`Error::Malformed`] bei zu langer Begründung.
    pub fn revoke(&mut self, at: u64, note: Option<&str>) -> Result<()> {
        if let Some(n) = note
            && n.len() > REVOCATION_NOTE_MAX
        {
            return Err(Error::Malformed("trust: revocation note too long"));
        }
        self.state = TrustState::Revoked;
        self.revoked_at = Some(at);
        self.revocation_note = note.map(str::to_owned);
        Ok(())
    }

    /// Trägt einen neuen Schlüsselsatz ein und schiebt den alten in die
    /// Historie.
    ///
    /// Der Zustand fällt auf [`TrustState::Changed`] — **nicht** auf `Seen`.
    /// Eine frühere Verifikation gilt für den alten Schlüssel und darf sich
    /// nicht stillschweigend auf den neuen übertragen.
    ///
    /// # Fehler
    ///
    /// - [`Error::Malformed`], wenn der Kontakt widerrufen ist
    /// - [`Error::Malformed`], wenn die Historie überläuft
    pub fn replace_keys(
        &mut self,
        enc_pub: [u8; 32],
        sig_pub: Option<[u8; 32]>,
        xwing_pub: Option<Box<[u8; PQ_PUB_LEN]>>,
        at: u64,
    ) -> Result<()> {
        if self.state == TrustState::Revoked {
            return Err(Error::Malformed("trust: contact is revoked"));
        }
        if self.previous_keys.len() >= MAX_HISTORY {
            return Err(Error::Malformed("trust: key history full"));
        }

        self.previous_keys.push(PreviousKey {
            fingerprint: self.fingerprint(),
            sig_pub: self.sig_pub,
            replaced_at: at,
            was_verified: self.state == TrustState::Verified,
        });

        self.enc_pub = enc_pub;
        self.sig_pub = sig_pub;
        self.xwing_pub = xwing_pub;
        self.state = TrustState::Changed;
        self.verified_at = None;
        self.verified_via = None;
        Ok(())
    }

    fn to_tlv(&self) -> Result<Vec<u8>> {
        let mut w = TlvWriter::new();
        w.push(tag::ENC_PUB, &self.enc_pub)?;
        if let Some(k) = &self.sig_pub {
            w.push(tag::SIG_PUB, k)?;
        }
        let name = self.name.as_bytes();
        if name.len() > NAME_MAX {
            return Err(Error::Malformed("trust: name too long"));
        }
        w.push(tag::NAME, name)?;
        w.push(tag::STATE, &[self.state.to_byte()])?;
        w.push(tag::FIRST_SEEN, &self.first_seen.to_be_bytes())?;
        if let Some(t) = self.verified_at {
            w.push(tag::VERIFIED_AT, &t.to_be_bytes())?;
        }
        if let Some(v) = self.verified_via {
            w.push(tag::VERIFIED_VIA, &[v.to_byte()])?;
        }
        if let Some(n) = &self.note {
            if n.len() > NOTE_MAX {
                return Err(Error::Malformed("trust: note too long"));
            }
            w.push(tag::NOTE, n.as_bytes())?;
        }
        if !self.previous_keys.is_empty() {
            w.push(tag::PREVIOUS_KEYS, &encode_history(&self.previous_keys)?)?;
        }
        if let Some(k) = &self.xwing_pub {
            w.push(tag::XWING_PUB, k.as_slice())?;
        }
        if let Some(t) = self.revoked_at {
            w.push(tag::REVOKED_AT, &t.to_be_bytes())?;
        }
        if let Some(n) = &self.revocation_note {
            if n.len() > REVOCATION_NOTE_MAX {
                return Err(Error::Malformed("trust: revocation note too long"));
            }
            w.push(tag::REVOCATION_NOTE, n.as_bytes())?;
        }
        Ok(w.finish())
    }

    fn from_tlv(data: &[u8]) -> Result<Self> {
        let mut enc_pub = None;
        let mut sig_pub = None;
        let mut xwing_pub = None;
        let mut name = None;
        let mut state = None;
        let mut first_seen = None;
        let mut verified_at = None;
        let mut verified_via = None;
        let mut note = None;
        let mut previous_keys = Vec::new();
        let mut revoked_at = None;
        let mut revocation_note = None;

        let mut r = TlvReader::new(data);
        while let Some((ty, value)) = r.next_field()? {
            match ty {
                tag::ENC_PUB => enc_pub = Some(expect_len::<32>(value, "trust: enc_pub length")?),
                tag::SIG_PUB => sig_pub = Some(expect_len::<32>(value, "trust: sig_pub length")?),
                tag::NAME => name = Some(text(value, NAME_MAX, "trust: name")?),
                tag::STATE => {
                    let b = expect_len::<1>(value, "trust: state length")?;
                    state = Some(TrustState::from_byte(b[0])?);
                }
                tag::FIRST_SEEN => {
                    first_seen = Some(u64::from_be_bytes(expect_len::<8>(
                        value,
                        "trust: first_seen length",
                    )?));
                }
                tag::VERIFIED_AT => {
                    verified_at = Some(u64::from_be_bytes(expect_len::<8>(
                        value,
                        "trust: verified_at length",
                    )?));
                }
                tag::VERIFIED_VIA => {
                    let b = expect_len::<1>(value, "trust: verified_via length")?;
                    verified_via = Some(VerifiedVia::from_byte(b[0])?);
                }
                tag::NOTE => note = Some(text(value, NOTE_MAX, "trust: note")?),
                tag::PREVIOUS_KEYS => previous_keys = decode_history(value)?,
                tag::XWING_PUB => {
                    let arr: [u8; PQ_PUB_LEN] = value
                        .try_into()
                        .map_err(|_| Error::Malformed("trust: xwing_pub length"))?;
                    xwing_pub = Some(Box::new(arr));
                }
                tag::REVOKED_AT => {
                    revoked_at = Some(u64::from_be_bytes(expect_len::<8>(
                        value,
                        "trust: revoked_at length",
                    )?));
                }
                tag::REVOCATION_NOTE => {
                    revocation_note =
                        Some(text(value, REVOCATION_NOTE_MAX, "trust: revocation note")?);
                }
                _ => return Err(Error::Malformed("trust: unknown contact TLV type")),
            }
        }

        Ok(Self {
            enc_pub: enc_pub.ok_or(Error::Malformed("trust: enc_pub missing"))?,
            sig_pub,
            xwing_pub,
            name: name.ok_or(Error::Malformed("trust: name missing"))?,
            state: state.ok_or(Error::Malformed("trust: state missing"))?,
            first_seen: first_seen.ok_or(Error::Malformed("trust: first_seen missing"))?,
            verified_at,
            verified_via,
            note,
            previous_keys,
            revoked_at,
            revocation_note,
        })
    }
}

fn text(value: &[u8], max: usize, feld: &'static str) -> Result<String> {
    if value.len() > max {
        return Err(Error::Malformed(feld));
    }
    core::str::from_utf8(value)
        .map(str::to_owned)
        .map_err(|_| Error::Malformed(feld))
}

fn encode_history(keys: &[PreviousKey]) -> Result<Vec<u8>> {
    let count =
        u16::try_from(keys.len()).map_err(|_| Error::Malformed("trust: history too long"))?;
    let mut out = Vec::new();
    out.extend_from_slice(&count.to_be_bytes());
    for k in keys {
        out.extend_from_slice(k.fingerprint.as_bytes());
        out.extend_from_slice(&k.replaced_at.to_be_bytes());
        out.push(u8::from(k.was_verified));
        out.push(u8::from(k.sig_pub.is_some()));
        out.extend_from_slice(&k.sig_pub.unwrap_or([0u8; 32]));
    }
    Ok(out)
}

/// Länge eines Historieneintrags: 32 + 8 + 1 + 1 + 32.
const HISTORY_ENTRY_LEN: usize = 74;

fn decode_history(data: &[u8]) -> Result<Vec<PreviousKey>> {
    let count = usize::from(u16::from_be_bytes(
        data.get(0..2)
            .and_then(|s| s.try_into().ok())
            .ok_or(Error::Malformed("trust: history truncated"))?,
    ));
    if count > MAX_HISTORY {
        return Err(Error::Malformed("trust: history too long"));
    }

    // Kein Vorabreservieren anhand der Zählerangabe.
    let mut out = Vec::new();
    let mut pos = 2usize;
    for _ in 0..count {
        let ende = pos
            .checked_add(HISTORY_ENTRY_LEN)
            .ok_or(Error::Malformed("trust: history overflow"))?;
        let e = data
            .get(pos..ende)
            .ok_or(Error::Malformed("trust: history truncated"))?;

        let fp: [u8; 32] = e
            .get(0..32)
            .and_then(|s| s.try_into().ok())
            .ok_or(Error::Malformed("trust: history entry"))?;
        let at = u64::from_be_bytes(
            e.get(32..40)
                .and_then(|s| s.try_into().ok())
                .ok_or(Error::Malformed("trust: history entry"))?,
        );
        let was_verified = *e.get(40).ok_or(Error::Malformed("trust: history entry"))? == 1;
        let has_sig = *e.get(41).ok_or(Error::Malformed("trust: history entry"))? == 1;
        let sig: [u8; 32] = e
            .get(42..74)
            .and_then(|s| s.try_into().ok())
            .ok_or(Error::Malformed("trust: history entry"))?;

        out.push(PreviousKey {
            fingerprint: Fingerprint::from_bytes(fp),
            sig_pub: has_sig.then_some(sig),
            replaced_at: at,
            was_verified,
        });
        pos = ende;
    }
    if pos != data.len() {
        return Err(Error::Malformed("trust: trailing bytes in history"));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Authentizität
// ---------------------------------------------------------------------------

/// Wie die Signatur einer Nachricht einzuordnen ist
/// (`spec/trust-store.md` §7).
///
/// **Bewusst kein Wahrheitswert.** Es gibt absichtlich keine Methode, die
/// das auf `bool` reduziert — genau diese Einebnung war der Fehler in v1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Authenticity {
    /// Nicht signiert. Ein legitimer Modus, kein Mangel.
    Unsigned,
    /// Gültige Signatur eines Schlüssels, der nicht im Speicher steht.
    ///
    /// Trägt den **Signierschlüssel**, nicht einen Fingerprint: Aus einer
    /// Signatur allein lässt sich keiner bilden (`spec/trust-store.md` §7.1).
    SignedUnknown {
        /// Der Ed25519-Signierschlüssel.
        sig_pub: [u8; 32],
    },
    /// Bekannter Kontakt, aber **nie verifiziert**.
    SignedSeen {
        /// Fingerprint des Kontakts.
        fingerprint: Fingerprint,
        /// Anzeigename.
        name: String,
    },
    /// Verifizierter Kontakt. Der einzige Fall, der Grün verdient.
    SignedVerified {
        /// Fingerprint des Kontakts.
        fingerprint: Fingerprint,
        /// Anzeigename.
        name: String,
        /// Wann verifiziert wurde.
        verified_at: Option<u64>,
        /// **Auf welchem Weg** verifiziert wurde.
        ///
        /// §5 stellt fest, dass die Wege nicht gleichwertig sind, und
        /// verlangt, dass die Oberfläche die schwächste Zeile der Tabelle
        /// benennt: Ein Fingerprint, der über denselben Kanal kam wie die
        /// Nachricht, beweist nichts. Ohne dieses Feld stünde bei jeder
        /// verifizierten Nachricht derselbe Satz — und der schwächste Weg
        /// sähe aus wie der stärkste.
        ///
        /// Der Wert liegt im Kontakt bereits vor; er wurde bisher nur nicht
        /// mitgenommen.
        verified_via: Option<VerifiedVia>,
    },
    /// **Warnfall.** Der Schlüssel ist nicht der aktuelle des Kontakts.
    SignedChanged {
        /// Fingerprint des Kontakts, wie er jetzt ist.
        fingerprint: Fingerprint,
        /// Anzeigename.
        name: String,
        /// Fingerprint des abgelösten Schlüsselsatzes, sofern bekannt.
        previous_fingerprint: Option<Fingerprint>,
        /// Ob der abgelöste Schlüssel damals verifiziert war.
        ///
        /// Wiegt schwerer und muss deutlicher gewarnt werden.
        previous_was_verified: bool,
    },
    /// **Warnfall.** Der Schlüssel wurde lokal als kompromittiert markiert.
    SignedRevoked {
        /// Fingerprint des Kontakts.
        fingerprint: Fingerprint,
        /// Anzeigename.
        name: String,
    },
}

impl Authenticity {
    /// Ob die Darstellung eine Warnung sein muss (`spec/trust-store.md` §8).
    #[must_use]
    pub const fn is_warning(&self) -> bool {
        matches!(
            self,
            Self::SignedChanged { .. } | Self::SignedRevoked { .. }
        )
    }

    /// Ob grün dargestellt werden darf.
    ///
    /// **Nur** bei [`Authenticity::SignedVerified`]. Eine gültige Signatur
    /// eines unbekannten Schlüssels ist keine Authentizität.
    #[must_use]
    pub const fn may_show_green(&self) -> bool {
        matches!(self, Self::SignedVerified { .. })
    }
}

// ---------------------------------------------------------------------------
// Speicher
// ---------------------------------------------------------------------------

/// Kontaktspeicher.
#[derive(Debug, Default, Clone)]
pub struct TrustStore {
    contacts: Vec<Contact>,
}

impl TrustStore {
    /// Leerer Speicher.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            contacts: Vec::new(),
        }
    }

    /// Alle Kontakte.
    #[must_use]
    pub fn contacts(&self) -> &[Contact] {
        &self.contacts
    }

    /// Zahl der Kontakte.
    #[must_use]
    pub fn len(&self) -> usize {
        self.contacts.len()
    }

    /// Ob der Speicher leer ist.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.contacts.is_empty()
    }

    /// Fügt einen Kontakt hinzu.
    ///
    /// # Fehler
    ///
    /// [`Error::Malformed`], wenn bereits ein Kontakt mit demselben
    /// Signierschlüssel existiert. Zwei Einträge mit gleichem Schlüssel
    /// machten das Nachschlagen mehrdeutig.
    pub fn add(&mut self, contact: Contact) -> Result<()> {
        if let Some(k) = contact.sig_pub
            && self.find_by_sig_pub(&k).is_some()
        {
            return Err(Error::Malformed("trust: signing key already present"));
        }
        self.contacts.push(contact);
        Ok(())
    }

    /// Alle Kontakte, veränderbar.
    ///
    /// Zum Bearbeiten eines Eintrags, den der Aufrufer über seine Position
    /// gefunden hat — etwa nach einer Namenssuche, die dieses Modul bewusst
    /// nicht anbietet: Namen sind frei wählbar, nicht eindeutig und damit
    /// kein Schlüssel. Wer sie als solche verwenden will, muss die
    /// Mehrdeutigkeit selbst behandeln.
    pub fn contacts_mut(&mut self) -> &mut [Contact] {
        &mut self.contacts
    }

    /// Entfernt einen Kontakt an seiner Position.
    ///
    /// # Was dabei verloren geht
    ///
    /// Mit dem Eintrag verschwindet die **Schlüsselhistorie**. Meldet sich
    /// derselbe Mensch später mit einem anderen Schlüssel, ist das danach
    /// nicht mehr als Wechsel erkennbar — der Zustand `Changed` (§4.2) kann
    /// nicht mehr entstehen. Bei Verdacht auf Kompromittierung ist
    /// [`Contact::revoke`] deshalb das richtige Mittel, nicht das Entfernen.
    ///
    /// # Fehler
    ///
    /// [`Error::Malformed`], wenn die Position nicht belegt ist.
    pub fn remove(&mut self, index: usize) -> Result<Contact> {
        if index >= self.contacts.len() {
            return Err(Error::Malformed("trust: contact index out of range"));
        }
        Ok(self.contacts.remove(index))
    }

    /// Sucht einen Kontakt über seinen **aktuellen** Signierschlüssel.
    #[must_use]
    pub fn find_by_sig_pub(&self, sig_pub: &[u8; 32]) -> Option<&Contact> {
        self.contacts
            .iter()
            .find(|c| c.sig_pub.as_ref() == Some(sig_pub))
    }

    /// Wie [`TrustStore::find_by_sig_pub`], veränderbar.
    pub fn find_by_sig_pub_mut(&mut self, sig_pub: &[u8; 32]) -> Option<&mut Contact> {
        self.contacts
            .iter_mut()
            .find(|c| c.sig_pub.as_ref() == Some(sig_pub))
    }

    /// Sucht einen Kontakt über seinen Fingerprint.
    #[must_use]
    pub fn find_by_fingerprint(&self, fp: &Fingerprint) -> Option<&Contact> {
        self.contacts.iter().find(|c| &c.fingerprint() == fp)
    }

    /// Ordnet die Signaturlage einer Nachricht ein
    /// (`spec/trust-store.md` §7.2).
    ///
    /// Nachgeschlagen wird über den Signierschlüssel — aus einer Signatur
    /// allein lässt sich kein Fingerprint bilden.
    #[must_use]
    pub fn resolve(&self, signer: &crate::envelope::Signer) -> Authenticity {
        let sig_pub = match signer {
            crate::envelope::Signer::None => return Authenticity::Unsigned,
            crate::envelope::Signer::Key(k) => *k,
        };

        // 1. Aktueller Schlüssel eines Kontakts?
        if let Some(c) = self.find_by_sig_pub(&sig_pub) {
            let fingerprint = c.fingerprint();
            let name = c.name.clone();
            return match c.state {
                TrustState::Seen => Authenticity::SignedSeen { fingerprint, name },
                TrustState::Verified => Authenticity::SignedVerified {
                    fingerprint,
                    name,
                    verified_at: c.verified_at,
                    verified_via: c.verified_via,
                },
                TrustState::Changed => Authenticity::SignedChanged {
                    fingerprint,
                    name,
                    previous_fingerprint: c.previous_keys.last().map(|p| p.fingerprint),
                    previous_was_verified: c.previous_keys.last().is_some_and(|p| p.was_verified),
                },
                TrustState::Revoked => Authenticity::SignedRevoked { fingerprint, name },
            };
        }

        // 2. Ausgemusterter Schlüssel eines Kontakts? Ebenfalls Warnfall —
        //    entweder wurde gewechselt und der alte noch benutzt, oder
        //    jemand anderes verwendet einen ausrangierten Schlüssel.
        for c in &self.contacts {
            if let Some(alt) = c
                .previous_keys
                .iter()
                .find(|p| p.sig_pub.as_ref() == Some(&sig_pub))
            {
                return Authenticity::SignedChanged {
                    fingerprint: c.fingerprint(),
                    name: c.name.clone(),
                    previous_fingerprint: Some(alt.fingerprint),
                    previous_was_verified: alt.was_verified,
                };
            }
        }

        // 3. Unbekannt.
        Authenticity::SignedUnknown { sig_pub }
    }
}

// ---------------------------------------------------------------------------
// Verschlüsselte Ablage (§6)
// ---------------------------------------------------------------------------

/// Schlüssel für den Kontaktspeicher. Wird beim Verwerfen zeroisiert.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct ContactsKey([u8; 32]);

impl core::fmt::Debug for ContactsKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("ContactsKey(<redacted>)")
    }
}

impl ContactsKey {
    /// Leitet den Schlüssel aus der Identität ab (`spec/trust-store.md` §6).
    ///
    /// ```text
    /// contacts_key = HKDF-SHA256(ikm = enc_sk, salt = "", info = "cabrik-v2 contacts")
    /// ```
    ///
    /// Damit ist der Kontaktspeicher nur bei **entsperrter** Identität
    /// lesbar. Ein Angreifer mit Dateisystemzugriff sieht nicht, mit wem
    /// kommuniziert wird — eine der aussagekräftigsten Metadaten überhaupt.
    #[must_use]
    pub fn derive(identity: &Identity) -> Self {
        let mut key = [0u8; 32];
        let hk = Hkdf::<Sha256>::new(None, &identity.enc_sk);
        if hk.expand(b"cabrik-v2 contacts", &mut key).is_err() {
            key = [0u8; 32];
        }
        Self(key)
    }

    /// Die rohen Bytes — für die Ablage in `cabrik-app`.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Serialisiert den Speicher als Klartext-Byteblock.
///
/// Die Verschlüsselung geschieht in der aufrufenden Schicht mit
/// [`ContactsKey`]; dieses Modul kennt keine Dateien.
///
/// # Fehler
///
/// [`Error::Malformed`] bei überlangen Feldern.
pub fn serialize(store: &TrustStore) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let count = u32::try_from(store.contacts.len())
        .map_err(|_| Error::Malformed("trust: too many contacts"))?;
    out.extend_from_slice(&count.to_be_bytes());
    for c in &store.contacts {
        let tlv = c.to_tlv()?;
        let len =
            u32::try_from(tlv.len()).map_err(|_| Error::Malformed("trust: contact too large"))?;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(&tlv);
    }
    Ok(out)
}

/// Höchstzahl Kontakte beim Lesen — Speicherschutz gegen präparierte Dateien.
const MAX_CONTACTS: usize = 100_000;

/// Liest einen serialisierten Speicher.
///
/// # Fehler
///
/// [`Error::Malformed`] bei kaputter Struktur.
pub fn deserialize(data: &[u8]) -> Result<TrustStore> {
    let count = usize::try_from(u32::from_be_bytes(
        data.get(0..4)
            .and_then(|s| s.try_into().ok())
            .ok_or(Error::Malformed("trust: truncated header"))?,
    ))
    .map_err(|_| Error::Malformed("trust: count overflow"))?;

    if count > MAX_CONTACTS {
        return Err(Error::Malformed("trust: too many contacts"));
    }

    // Kein Vorabreservieren anhand der Zählerangabe.
    let mut store = TrustStore::new();
    let mut pos = 4usize;
    for _ in 0..count {
        let len_ende = pos
            .checked_add(4)
            .ok_or(Error::Malformed("trust: offset overflow"))?;
        let len = usize::try_from(u32::from_be_bytes(
            data.get(pos..len_ende)
                .and_then(|s| s.try_into().ok())
                .ok_or(Error::Malformed("trust: truncated entry length"))?,
        ))
        .map_err(|_| Error::Malformed("trust: length overflow"))?;

        let ende = len_ende
            .checked_add(len)
            .ok_or(Error::Malformed("trust: offset overflow"))?;
        let tlv = data
            .get(len_ende..ende)
            .ok_or(Error::Malformed("trust: truncated entry"))?;

        store.contacts.push(Contact::from_tlv(tlv)?);
        pos = ende;
    }
    if pos != data.len() {
        return Err(Error::Malformed("trust: trailing bytes"));
    }
    Ok(store)
}

// ---------------------------------------------------------------------------
// QR-Nutzlast (§5.1)
// ---------------------------------------------------------------------------

/// Baut die Austausch-Nutzlast für eine Identität.
///
/// ```text
/// cabrik:v2:<enc_pub>:<sig_pub>:<xwing_pub>:<fingerprint[0..8]>
/// ```
///
/// Der Fingerprint-Anfang ist **nur eine Prüfsumme** gegen
/// Übertragungsfehler.
///
/// # Warum der Post-Quantum-Schlüssel mitmuss
///
/// Ein früherer Entwurf führte hier nur `enc_pub` und `sig_pub`. Das war aus
/// zwei Gründen falsch, und beide fielen erst beim Verdrahten der CLI auf:
///
/// 1. §2 nimmt `xwing_pub` **zwingend** in den Fingerprint. Wer die Nutzlast
///    ohne ihn einliest, legt einen Kontakt mit `xwing_pub = None` an — und
///    berechnet damit einen **anderen** Fingerprint als den, den die
///    Gegenseite anzeigt. Zwei ehrliche Beteiligte hätten sich nie
///    verifizieren können.
/// 2. Ohne den Schlüssel ist Suite `0x0002` für diesen Kontakt unerreichbar.
///    Der gesamte Post-Quantum-Pfad wäre totes Gewicht gewesen.
///
/// Das Feld bleibt **optional**, weil aus v1 migrierte Identitäten
/// tatsächlich keinen X-Wing-Schlüssel haben (`§6`). Dann steht dort ein
/// leeres Feld, und der Fingerprint wird korrekt mit `None` gebildet.
///
/// Die Nutzlast wird dadurch rund 2000 Zeichen lang. Als QR-Code ist das
/// etwa Version 29 — dicht, aber lesbar. Wo ein QR-Code unpraktisch ist,
/// wird dieselbe Zeichenfolge als Datei ausgetauscht; das Format ist
/// dasselbe.
#[must_use]
pub fn qr_payload(
    enc_pub: &[u8; 32],
    sig_pub: Option<&[u8; 32]>,
    xwing_pub: Option<&[u8; PQ_PUB_LEN]>,
) -> String {
    let fp = Fingerprint::compute(enc_pub, sig_pub, xwing_pub);
    format!(
        "cabrik:v2:{}:{}:{}:{}",
        base32::encode(enc_pub),
        sig_pub.map_or_else(String::new, |k| base32::encode(k)),
        xwing_pub.map_or_else(String::new, |k| base32::encode(k)),
        fp.short()
    )
}

/// Aus einer Austausch-Nutzlast gelesene Schlüssel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrIdentity {
    /// X25519-Public-Key.
    pub enc_pub: [u8; 32],
    /// Ed25519-Signierschlüssel, sofern vorhanden.
    pub sig_pub: Option<[u8; 32]>,
    /// X-Wing-Public-Key. Fehlt bei aus v1 stammenden Identitäten.
    pub xwing_pub: Option<Box<[u8; PQ_PUB_LEN]>>,
}

/// Warum eine Austausch-Nutzlast nicht gelesen werden konnte.
///
/// **Zwei Faelle, und sie verlangen verschiedene Ratschlaege.** Wer etwas
/// Falsches eingefuegt hat, muss die richtige Zeichenfolge holen. Wer die
/// richtige eingefuegt hat und sie kam beschaedigt an, muss sie sich noch
/// einmal schicken lassen -- am besten als Datei statt ueber die
/// Zwischenablage.
///
/// Vorher waren beide `Error::Malformed` mit verschiedenen Texten. Wer sie
/// unterscheiden wollte, musste auf Meldungen pruefen; eine Umformulierung
/// haette die Anzeige stumm veraendert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum QrFehler {
    /// Keine Cabrik-Austausch-Nutzlast.
    ///
    /// Der Aufbau stimmt nicht oder das Praefix fehlt -- es wurde etwas
    /// anderes eingefuegt.
    Fremd,
    /// Erkennbar eine Cabrik-Nutzlast, aber unbrauchbar.
    ///
    /// Ein Schluesselfeld laesst sich nicht dekodieren, hat die falsche
    /// Laenge, oder die Pruefsumme passt nicht zu den Schluesseln. In allen
    /// drei Faellen ist die Zeichenfolge unterwegs beschaedigt worden.
    ///
    /// **Das ist kein Angriff.** Die Pruefsumme schuetzt gegen
    /// Uebertragungsfehler, nicht gegen Faelschung -- wer die Nutzlast
    /// austauscht, rechnet sie neu.
    Beschaedigt,
}

impl core::fmt::Display for QrFehler {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Fremd => "keine Cabrik-Austausch-Nutzlast",
            Self::Beschaedigt => "Austausch-Nutzlast beschaedigt",
        })
    }
}

impl From<QrFehler> for Error {
    fn from(e: QrFehler) -> Self {
        Self::Malformed(match e {
            QrFehler::Fremd => "trust: qr payload is not cabrik v2",
            QrFehler::Beschaedigt => "trust: qr payload is damaged",
        })
    }
}

/// Liest eine QR-Nutzlast.
///
/// Der Fingerprint wird aus den Schlüsseln **neu berechnet**; dem
/// übertragenen Wert wird nicht vertraut. Er dient allein dazu,
/// Übertragungsfehler zu erkennen.
///
/// # Fehler
///
/// [`QrFehler::Fremd`], wenn es keine Cabrik-Nutzlast ist;
/// [`QrFehler::Beschaedigt`], wenn sie es ist, aber unbrauchbar ankam.
pub fn parse_qr(payload: &str) -> core::result::Result<QrIdentity, QrFehler> {
    let teile: Vec<&str> = payload.split(':').collect();
    // Der Aufbau entscheidet, ob es ueberhaupt eine Cabrik-Nutzlast sein
    // will. Alles danach ist Beschaedigung.
    if teile.len() != 6 {
        return Err(QrFehler::Fremd);
    }
    if teile.first() != Some(&"cabrik") || teile.get(1) != Some(&"v2") {
        return Err(QrFehler::Fremd);
    }

    let enc_pub: [u8; 32] = base32::decode(teile.get(2).unwrap_or(&""))
        .map_err(|_| QrFehler::Beschaedigt)?
        .as_slice()
        .try_into()
        .map_err(|_| QrFehler::Beschaedigt)?;

    let sig_feld = teile.get(3).unwrap_or(&"");
    let sig_pub: Option<[u8; 32]> = if sig_feld.is_empty() {
        None
    } else {
        Some(
            base32::decode(sig_feld)
                .map_err(|_| QrFehler::Beschaedigt)?
                .as_slice()
                .try_into()
                .map_err(|_| QrFehler::Beschaedigt)?,
        )
    };

    let pq_feld = teile.get(4).unwrap_or(&"");
    let xwing_pub: Option<Box<[u8; PQ_PUB_LEN]>> = if pq_feld.is_empty() {
        None
    } else {
        Some(
            base32::decode(pq_feld)
                .map_err(|_| QrFehler::Beschaedigt)?
                .into_boxed_slice()
                .try_into()
                .map_err(|_| QrFehler::Beschaedigt)?,
        )
    };

    // Prüfsumme gegen Übertragungsfehler. Sie wird über **denselben**
    // Schlüsselsatz gebildet, der auch in den Kontakt geht — sonst zeigten
    // beide Seiten verschiedene Fingerprints an.
    let erwartet = Fingerprint::compute(&enc_pub, sig_pub.as_ref(), xwing_pub.as_deref()).short();
    if teile.get(5) != Some(&erwartet.as_str()) {
        return Err(QrFehler::Beschaedigt);
    }

    Ok(QrIdentity {
        enc_pub,
        sig_pub,
        xwing_pub,
    })
}

/// Austausch-Nutzlast der eigenen Identität.
///
/// Enthält den Post-Quantum-Schlüssel, weil die Gegenseite sonst einen
/// anderen Fingerprint berechnet als den hier angezeigten. Siehe
/// [`qr_payload`].
#[must_use]
pub fn own_qr_payload(identity: &Identity) -> String {
    let enc_pub = kem::public_key(&identity.enc_sk).unwrap_or([0u8; 32]);
    let sig_pub = identity.sig_sk.as_ref().map(|s| {
        ed25519_dalek::SigningKey::from_bytes(s)
            .verifying_key()
            .to_bytes()
    });
    let xwing_pub = kem::pq_public_key(&identity.pq_seed);
    qr_payload(&enc_pub, sig_pub.as_ref(), Some(&xwing_pub))
}

/// Fingerprint der eigenen Identität — das, was zur Verifikation angezeigt
/// und vorgelesen wird.
///
/// Bildet **denselben** Schlüsselsatz ab, den [`own_qr_payload`] überträgt.
#[must_use]
pub fn own_fingerprint(identity: &Identity) -> Fingerprint {
    let enc_pub = kem::public_key(&identity.enc_sk).unwrap_or([0u8; 32]);
    let sig_pub = identity.sig_sk.as_ref().map(|s| {
        ed25519_dalek::SigningKey::from_bytes(s)
            .verifying_key()
            .to_bytes()
    });
    let xwing_pub = kem::pq_public_key(&identity.pq_seed);
    Fingerprint::compute(&enc_pub, sig_pub.as_ref(), Some(&xwing_pub))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;
    use crate::envelope::Signer;

    fn kontakt(name: &str, sig: u8) -> Contact {
        Contact::new_seen(
            name,
            [sig.wrapping_add(1); 32],
            Some([sig; 32]),
            Some(Box::new([sig.wrapping_add(2); PQ_PUB_LEN])),
            1_700_000_000,
        )
        .unwrap()
    }

    // -----------------------------------------------------------------
    // Der Kernbefund aus v1
    // -----------------------------------------------------------------

    /// In v1 meldete jede selbst mitgelieferte Signatur `valid: true`.
    /// Hier muss dieselbe Lage „unbekannt" ergeben — und **nicht** grün.
    #[test]
    fn gueltige_signatur_eines_unbekannten_schluessels_ist_keine_authentizitaet() {
        let store = TrustStore::new();
        let auth = store.resolve(&Signer::Key([0x99; 32]));

        assert_eq!(
            auth,
            Authenticity::SignedUnknown {
                sig_pub: [0x99; 32]
            }
        );
        assert!(
            !auth.may_show_green(),
            "unbekannter Schluessel darf nicht gruen erscheinen"
        );
        assert!(
            !auth.is_warning(),
            "anonymer Versand ist kein Fehler, nur ein anderer Modus"
        );
    }

    #[test]
    fn nur_verifiziert_darf_gruen_sein() {
        let mut store = TrustStore::new();
        let mut c = kontakt("Alice", 0x11);
        store.add(c.clone()).unwrap();

        // Gesehen, aber nicht verifiziert.
        assert!(!store.resolve(&Signer::Key([0x11; 32])).may_show_green());

        // Nach der Verifikation.
        c.verify(VerifiedVia::QrCode, 1_700_000_100).unwrap();
        let mut store2 = TrustStore::new();
        store2.add(c).unwrap();
        let auth = store2.resolve(&Signer::Key([0x11; 32]));
        assert!(auth.may_show_green());
        assert!(matches!(auth, Authenticity::SignedVerified { .. }));
    }

    #[test]
    fn nicht_signiert_ist_kein_mangel() {
        let store = TrustStore::new();
        let auth = store.resolve(&Signer::None);
        assert_eq!(auth, Authenticity::Unsigned);
        assert!(!auth.is_warning());
        assert!(!auth.may_show_green());
    }

    // -----------------------------------------------------------------
    // Trust on First Use und Schluesselwechsel
    // -----------------------------------------------------------------

    #[test]
    fn tofu_legt_als_gesehen_an_nicht_als_verifiziert() {
        let c = kontakt("Bob", 0x22);
        assert_eq!(c.state, TrustState::Seen);
        assert!(c.verified_at.is_none());
    }

    #[test]
    fn schluesselwechsel_setzt_warnzustand_und_verwirft_die_verifikation() {
        let mut c = kontakt("Alice", 0x11);
        c.verify(VerifiedVia::SafetyNumber, 1_700_000_100).unwrap();
        let alter_fp = c.fingerprint();

        c.replace_keys([0x33; 32], Some([0x44; 32]), None, 1_700_000_200)
            .unwrap();

        assert_eq!(c.state, TrustState::Changed, "Warnzustand fehlt");
        assert!(
            c.verified_at.is_none(),
            "alte Verifikation gilt weiter -- sie betraf den alten Schluessel"
        );
        assert_eq!(c.previous_keys.len(), 1);
        assert_eq!(c.previous_keys[0].fingerprint, alter_fp);
        assert!(
            c.previous_keys[0].was_verified,
            "dass der alte Schluessel verifiziert war, wiegt schwerer"
        );
    }

    #[test]
    fn geaenderter_kontakt_ergibt_warnung() {
        let mut c = kontakt("Alice", 0x11);
        c.verify(VerifiedVia::QrCode, 1).unwrap();
        c.replace_keys([0x33; 32], Some([0x44; 32]), None, 2)
            .unwrap();

        let mut store = TrustStore::new();
        store.add(c).unwrap();

        let auth = store.resolve(&Signer::Key([0x44; 32]));
        assert!(auth.is_warning());
        assert!(!auth.may_show_green());
        match auth {
            Authenticity::SignedChanged {
                previous_was_verified,
                previous_fingerprint,
                ..
            } => {
                assert!(previous_was_verified);
                assert!(previous_fingerprint.is_some());
            }
            other => panic!("erwartete SignedChanged, bekam {other:?}"),
        }
    }

    #[test]
    fn ausgemusterter_schluessel_wird_als_warnfall_erkannt() {
        // spec/trust-store.md §7.2, mittlerer Fall: Die Nachricht kommt mit
        // einem Schluessel, den der Kontakt nicht mehr benutzt.
        let mut c = kontakt("Alice", 0x11);
        c.replace_keys([0x33; 32], Some([0x44; 32]), None, 2)
            .unwrap();

        let mut store = TrustStore::new();
        store.add(c).unwrap();

        // 0x11 ist der ALTE Schluessel.
        let auth = store.resolve(&Signer::Key([0x11; 32]));
        assert!(
            auth.is_warning(),
            "ausgemusterter Schluessel blieb unbemerkt"
        );
        assert!(matches!(auth, Authenticity::SignedChanged { .. }));
    }

    #[test]
    fn historie_wird_nicht_ueberschrieben() {
        let mut c = kontakt("Alice", 0x11);
        c.replace_keys([1; 32], Some([2; 32]), None, 10).unwrap();
        c.replace_keys([3; 32], Some([4; 32]), None, 20).unwrap();
        assert_eq!(c.previous_keys.len(), 2);
        assert_eq!(c.previous_keys[0].replaced_at, 10);
        assert_eq!(c.previous_keys[1].replaced_at, 20);
    }

    // -----------------------------------------------------------------
    // Widerruf
    // -----------------------------------------------------------------

    #[test]
    fn widerruf_ist_monoton() {
        let mut c = kontakt("Mallory", 0x55);
        c.revoke(1_700_000_300, Some("Laptop gestohlen")).unwrap();
        assert_eq!(c.state, TrustState::Revoked);

        // Weder Verifikation noch Schluesselwechsel heben ihn auf.
        assert!(c.verify(VerifiedVia::QrCode, 1_700_000_400).is_err());
        assert!(c.replace_keys([9; 32], Some([9; 32]), None, 1).is_err());
        assert_eq!(c.state, TrustState::Revoked);
    }

    #[test]
    fn widerrufener_kontakt_ergibt_warnung() {
        let mut c = kontakt("Mallory", 0x55);
        c.revoke(1, None).unwrap();
        let mut store = TrustStore::new();
        store.add(c).unwrap();

        let auth = store.resolve(&Signer::Key([0x55; 32]));
        assert!(auth.is_warning());
        assert!(matches!(auth, Authenticity::SignedRevoked { .. }));
    }

    // -----------------------------------------------------------------
    // Speicher
    // -----------------------------------------------------------------

    #[test]
    fn doppelter_signierschluessel_wird_abgelehnt() {
        let mut store = TrustStore::new();
        store.add(kontakt("Alice", 0x11)).unwrap();
        assert!(
            store.add(kontakt("Falsche Alice", 0x11)).is_err(),
            "zwei Kontakte mit gleichem Schluessel machen das Nachschlagen mehrdeutig"
        );
    }

    #[test]
    fn round_trip_ueber_die_serialisierung() {
        let mut store = TrustStore::new();
        let mut a = kontakt("Alice", 0x11);
        a.verify(VerifiedVia::SafetyNumber, 1_700_000_100).unwrap();
        a.note = Some("Redaktion".to_owned());
        store.add(a).unwrap();

        let mut b = kontakt("Bob", 0x22);
        b.replace_keys([7; 32], Some([8; 32]), None, 1_700_000_200)
            .unwrap();
        store.add(b).unwrap();

        let mut c = kontakt("Anonym", 0x33);
        c.sig_pub = None;
        c.xwing_pub = None;
        store.add(c).unwrap();

        let bytes = serialize(&store).unwrap();
        let zurueck = deserialize(&bytes).unwrap();

        assert_eq!(zurueck.len(), 3);
        assert_eq!(zurueck.contacts(), store.contacts());
    }

    #[test]
    fn kaputte_ablage_wird_abgelehnt() {
        let mut store = TrustStore::new();
        store.add(kontakt("Alice", 0x11)).unwrap();
        let bytes = serialize(&store).unwrap();

        for len in [0, 2, 4, bytes.len() - 1] {
            assert!(
                deserialize(&bytes[..len]).is_err(),
                "Laenge {len} haette abgelehnt werden muessen"
            );
        }
        let mut zuviel = bytes.clone();
        zuviel.push(0);
        assert!(deserialize(&zuviel).is_err());
    }

    #[test]
    fn uebergrosse_zaehlerangabe_reserviert_keinen_speicher() {
        // Behauptet 4 Milliarden Kontakte, liefert keine.
        let data = [0xFF, 0xFF, 0xFF, 0xFF];
        assert_eq!(deserialize(&data).unwrap_err().code(), "MALFORMED");
    }

    #[test]
    fn unbekannter_tlv_typ_wird_abgelehnt() {
        let mut w = TlvWriter::new();
        w.push(tag::ENC_PUB, &[1u8; 32]).unwrap();
        w.push(tag::NAME, b"X").unwrap();
        w.push(tag::STATE, &[1]).unwrap();
        w.push(tag::FIRST_SEEN, &0u64.to_be_bytes()).unwrap();
        w.push(0x7F, b"unbekannt").unwrap();
        assert_eq!(
            Contact::from_tlv(&w.finish()).unwrap_err().code(),
            "MALFORMED"
        );
    }

    // -----------------------------------------------------------------
    // Ableitung und QR
    // -----------------------------------------------------------------

    #[test]
    fn contacts_key_haengt_an_der_identitaet() {
        let a = Identity::generate(&mut crate::OsRandom, true, 0).unwrap();
        let b = Identity::generate(&mut crate::OsRandom, true, 0).unwrap();
        assert_ne!(
            ContactsKey::derive(&a).as_bytes(),
            ContactsKey::derive(&b).as_bytes()
        );
        assert_eq!(
            ContactsKey::derive(&a).as_bytes(),
            ContactsKey::derive(&a).as_bytes()
        );
    }

    #[test]
    fn contacts_key_gibt_sich_in_debug_nicht_preis() {
        let id = Identity::generate(&mut crate::OsRandom, true, 0).unwrap();
        assert!(format!("{:?}", ContactsKey::derive(&id)).contains("redacted"));
    }

    #[test]
    fn qr_round_trip() {
        let enc = [0x21; 32];
        let sig = [0x22; 32];
        let pq = Box::new([0x23; PQ_PUB_LEN]);
        let payload = qr_payload(&enc, Some(&sig), Some(&pq));
        assert!(payload.starts_with("cabrik:v2:"));

        let gelesen = parse_qr(&payload).unwrap();
        assert_eq!(gelesen.enc_pub, enc);
        assert_eq!(gelesen.sig_pub, Some(sig));
        assert_eq!(gelesen.xwing_pub, Some(pq));
    }

    #[test]
    fn qr_ohne_signierschluessel() {
        let enc = [0x31; 32];
        let payload = qr_payload(&enc, None, None);
        let gelesen = parse_qr(&payload).unwrap();
        assert_eq!(gelesen.sig_pub, None);
    }

    /// Aus v1 migrierte Identitaeten haben keinen X-Wing-Schluessel. Das
    /// Feld bleibt deshalb optional — und der Fingerprint wird dann korrekt
    /// mit `None` gebildet, nicht mit einem Nullschluessel (§2.1).
    #[test]
    fn qr_ohne_post_quantum_schluessel_bleibt_moeglich() {
        let enc = [0x35; 32];
        let payload = qr_payload(&enc, None, None);
        let gelesen = parse_qr(&payload).unwrap();
        assert_eq!(gelesen.xwing_pub, None);

        let kontakt = Contact::new_seen("Aus v1", gelesen.enc_pub, None, None, 0).unwrap();
        assert!(!kontakt.supports_post_quantum());
        assert_eq!(
            kontakt.fingerprint(),
            Fingerprint::compute(&enc, None, None)
        );
    }

    #[test]
    fn qr_pruefsumme_wird_neu_berechnet_nicht_geglaubt() {
        // §5.1: Dem uebertragenen Wert wird nicht vertraut. Wer die
        // Schluessel austauscht und die Pruefsumme stehen laesst, faellt auf.
        let payload = qr_payload(&[0x41; 32], Some(&[0x42; 32]), Some(&[0x43; PQ_PUB_LEN]));
        let teile: Vec<&str> = payload.split(':').collect();
        let gefaelscht = format!(
            "cabrik:v2:{}:{}:{}:{}",
            base32::encode(&[0x51; 32]),
            teile[3],
            teile[4],
            teile[5]
        );
        assert_eq!(
            parse_qr(&gefaelscht).unwrap_err(),
            QrFehler::Beschaedigt,
            "vertauschter Schluessel blieb unbemerkt"
        );
    }

    /// Der Angriff, gegen den die Pruefsumme ueber **alle** Schluessel
    /// schuetzt: Ein untergeschobener Post-Quantum-Schluessel darf nicht
    /// unbemerkt bleiben.
    #[test]
    fn untergeschobener_post_quantum_schluessel_faellt_auf() {
        let payload = qr_payload(&[0x41; 32], Some(&[0x42; 32]), Some(&[0x43; PQ_PUB_LEN]));
        let teile: Vec<&str> = payload.split(':').collect();
        let gefaelscht = format!(
            "cabrik:v2:{}:{}:{}:{}",
            teile[2],
            teile[3],
            base32::encode(&[0x99; PQ_PUB_LEN]),
            teile[5]
        );
        assert_eq!(
            parse_qr(&gefaelscht).unwrap_err(),
            QrFehler::Beschaedigt,
            "vertauschter Post-Quantum-Schluessel blieb unbemerkt"
        );
    }

    #[test]
    fn qr_lehnt_fremde_formate_ab() {
        for bad in [
            "",
            "cabrik:v1:a:b:c:d",
            "andere:v2:a:b:c:d",
            "cabrik:v2:a:b",
            "cabrik:v2:a:b:c",
            "cabrik:v2:a:b:c:d:e",
        ] {
            assert!(parse_qr(bad).is_err(), "{bad:?} haette scheitern muessen");
        }
    }

    #[test]
    fn eigene_qr_nutzlast_ist_lesbar() {
        let id = Identity::generate(&mut crate::OsRandom, true, 0).unwrap();
        let payload = own_qr_payload(&id);
        let gelesen = parse_qr(&payload).unwrap();
        assert_eq!(gelesen.enc_pub, kem::public_key(&id.enc_sk).unwrap());
        assert!(gelesen.sig_pub.is_some());
    }

    /// Der Kern der Sache: Was Alice als **ihren** Fingerprint anzeigt, muss
    /// dasselbe sein, was Bob nach dem Einlesen ihrer Nutzlast sieht.
    ///
    /// Andernfalls scheitert die Verifikation zwischen zwei ehrlichen
    /// Beteiligten — und genau die ist der Zweck des ganzen Trust Stores.
    #[test]
    fn was_alice_anzeigt_sieht_bob_nach_dem_einlesen() {
        let alice = Identity::generate(&mut crate::OsRandom, true, 0).unwrap();

        // Was Alices Oberflaeche anzeigt: der Fingerprint ihrer Identitaet,
        // inklusive Post-Quantum-Schluessel (spec/trust-store.md §2).
        let alice_zeigt = Fingerprint::compute(
            &kem::public_key(&alice.enc_sk).unwrap(),
            Some(
                &ed25519_dalek::SigningKey::from_bytes(alice.sig_sk.as_ref().unwrap())
                    .verifying_key()
                    .to_bytes(),
            ),
            Some(&kem::pq_public_key(&alice.pq_seed)),
        );

        // Was Bob nach dem Einlesen der Nutzlast sieht.
        let gelesen = parse_qr(&own_qr_payload(&alice)).unwrap();
        let bob_sieht = Contact::new_seen(
            "Alice",
            gelesen.enc_pub,
            gelesen.sig_pub,
            gelesen.xwing_pub,
            0,
        )
        .unwrap()
        .fingerprint();

        assert_eq!(
            alice_zeigt.display(),
            bob_sieht.display(),
            "Alice und Bob sehen verschiedene Fingerprints — Verifikation unmoeglich"
        );
    }

    /// Ohne den Post-Quantum-Schluessel im Austauschformat waere Suite
    /// `0x0002` fuer jeden ueber diesen Weg angelegten Kontakt unerreichbar —
    /// die gesamte Post-Quantum-Arbeit liefe ins Leere.
    #[test]
    fn eingelesener_kontakt_ist_post_quantum_faehig() {
        let alice = Identity::generate(&mut crate::OsRandom, true, 0).unwrap();
        let gelesen = parse_qr(&own_qr_payload(&alice)).unwrap();

        assert_eq!(
            gelesen.xwing_pub.as_deref(),
            Some(&kem::pq_public_key(&alice.pq_seed)),
            "Post-Quantum-Schluessel ging beim Austausch verloren"
        );

        let kontakt = Contact::new_seen(
            "Alice",
            gelesen.enc_pub,
            gelesen.sig_pub,
            gelesen.xwing_pub,
            0,
        )
        .unwrap();
        assert!(kontakt.supports_post_quantum());
    }

    #[test]
    fn post_quantum_faehigkeit_wird_gemeldet() {
        let mit = kontakt("Neu", 0x11);
        assert!(mit.supports_post_quantum());

        let mut ohne = kontakt("Aus v1", 0x22);
        ohne.xwing_pub = None;
        assert!(
            !ohne.supports_post_quantum(),
            "migrierte Kontakte koennen nur Suite 0x0001"
        );
    }
}
