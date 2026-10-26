//! Keyfile-Format v2 nach `spec/keyfile-v2.md`.
//!
//! ```text
//! ┌─ Klartext ─────────────────────────────────────┐
//! │  magic, version, Argon2id-Parameter, salt      │
//! ├─ Verschlüsselter Geheimnisblock ───────────────┤
//! │  enc_sk, sig_sk, pq_seed, Erstellungszeit      │
//! └────────────────────────────────────────────────┘
//! ```
//!
//! Der unverschlüsselte Teil enthält **nur**, was zum Ableiten des Schlüssels
//! aus dem Passwort nötig ist. v1 legte `enc_pub` und `sig_pub` im Klartext
//! ab — wer ein Keyfile fand, konnte damit belegen, *welche* Identität dem
//! Gerät gehört, ohne das Passwort zu kennen. Beide Public Keys lassen sich
//! aus den privaten berechnen; sie zu speichern war reiner Komfort und
//! kostete Schutzwirkung.

use crate::error::{Error, Result};
use crate::rng::Randomness;
use crate::tlv::{TlvReader, TlvWriter, expect_len};

use argon2::{Algorithm, Argon2, Block, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use ed25519_dalek::SigningKey;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

/// Magic-Bytes eines v2-Keyfiles.
pub const MAGIC: [u8; 2] = [0xCA, 0x4B];
/// Formatversion.
pub const VERSION: u8 = 0x02;

const SALT_LEN: usize = 16;
const KEK_LEN: usize = 32;

/// Länge des Klartextkopfes bis einschließlich `secret_len`.
const HEADER_LEN: usize = 32;

/// Länge des Teils, der als AEAD-AAD dient: Magic bis einschließlich Salz
/// (`spec/keyfile-v2.md` §2, „Bytes 0..28").
///
/// **Nicht** eingeschlossen ist `secret_len` — es beschreibt den
/// Ciphertext und ist über dessen Länge ohnehin festgelegt.
const AAD_LEN: usize = 28;

/// TLV-Typen des Geheimnisblocks (`spec/keyfile-v2.md` §3).
mod tag {
    pub(super) const ENC_SK: u8 = 0x01;
    pub(super) const SIG_SK: u8 = 0x02;
    pub(super) const CREATED: u8 = 0x03;
    pub(super) const LABEL: u8 = 0x04;
    pub(super) const PQ_SEED: u8 = 0x05;
}

const LABEL_MAX: usize = 64;

// ---------------------------------------------------------------------------
// Argon2id-Parameter
// ---------------------------------------------------------------------------

/// Wie stark die Passwortableitung sein soll.
///
/// # Warum das hier steht und nicht in der Oberfläche
///
/// Weil sonst jede Oberfläche ihre eigene Auslegung von „empfohlen“ hätte.
/// Die Zuordnung stand bis eben in `cabrik-cli`; das Fenster hätte sie ein
/// zweites Mal bekommen, und beim nächsten Anheben der Empfehlung wäre eine
/// der beiden stehengeblieben. Dann schriebe dasselbe Wort zwei verschieden
/// starke Dateien — ohne dass es jemandem auffiele, denn beide ließen sich
/// öffnen.
///
/// Dieselbe Überlegung wie beim Dateiformat des Kontaktspeichers und bei
/// den Ablagepfaden, die aus demselben Grund hierher gewandert sind.
///
/// Die Namen sind eine **Empfehlung, keine Messgröße**. Was sie kosten,
/// hängt vom Gerät ab; deshalb sagt die Oberfläche die Dauer dazu, die sie
/// tatsächlich gemessen hat, statt eine zu versprechen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KdfStufe {
    /// Untergrenze der Spezifikation: 64 MiB. Nur für schwache Geräte.
    Min,
    /// 256 MiB — spürbar, aber erträglich.
    #[default]
    Empfohlen,
    /// 1 GiB. Deutlich langsam, auch beim eigenen Entsperren.
    Stark,
}

impl KdfStufe {
    /// Zu welcher Stufe diese Parameter gehören — falls zu einer.
    ///
    /// `None` heißt: eigene Werte. Das ist kein Fehler; die CLI lässt sie
    /// zu, und eine übernommene Datei kann sie tragen.
    ///
    /// **Genau oder gar nicht.** Die nächstgelegene Stufe zu nennen wäre
    /// bequem und falsch: Eine Datei mit 200 MiB als „Empfohlen“ zu
    /// bezeichnen behauptete 256. Wer eigene Werte gewählt hat, soll sie
    /// sehen, nicht ein Etikett, das ungefähr passt.
    #[must_use]
    pub fn von_params(p: &KdfParams) -> Option<Self> {
        [Self::Min, Self::Empfohlen, Self::Stark]
            .into_iter()
            .find(|s| s.params() == *p)
    }

    /// Die Parameter zu dieser Stufe.
    #[must_use]
    pub const fn params(self) -> KdfParams {
        match self {
            // `p_cost: 1` und nicht 4: Wer diese Stufe wählt, hat ein
            // schwaches Geraet, und auf einem einzelnen Kern kosten vier
            // Bahnen nur Verwaltung.
            Self::Min => KdfParams {
                m_cost: KdfParams::M_COST_MIN,
                t_cost: KdfParams::T_COST_MIN,
                p_cost: 1,
            },
            Self::Empfohlen => KdfParams::recommended(),
            Self::Stark => KdfParams {
                m_cost: 1_048_576,
                t_cost: 4,
                p_cost: 4,
            },
        }
    }
}

/// Argon2id-Parameter, wie sie im Keyfile mitgeführt werden.
///
/// Sie stehen in der Datei, damit sie später erhöht werden können, ohne das
/// Format zu brechen. v1 hatte sie fest im Code — angemessen gewählt, aber
/// nirgends festgehalten und damit nicht änderbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    /// Speicherkosten in KiB.
    pub m_cost: u32,
    /// Zahl der Durchgänge.
    pub t_cost: u32,
    /// Parallelität.
    pub p_cost: u8,
}

impl KdfParams {
    /// Untergrenze: 64 MiB.
    pub const M_COST_MIN: u32 = 65_536;
    /// Obergrenze: 4 GiB.
    pub const M_COST_MAX: u32 = 4_194_304;
    /// Untergrenze der Durchgänge.
    pub const T_COST_MIN: u32 = 3;
    /// Obergrenze der Durchgänge (`spec/keyfile-v2.md` §4.1).
    ///
    /// # Warum es sie gibt
    ///
    /// Sie fehlte, und das Fuzzing hat es gefunden: `t_cost` ist ein `u32`,
    /// und die Parameter stehen im **Kopf des Envelopes** — also in einer
    /// Datei, die der Absender wählt. Ein Wert von 4 294 901 763 ließ
    /// Argon2id Jahrhunderte laufen, bei völlig legalem `m_cost`.
    ///
    /// Der Absender ist laut `threat-model.md` nicht vertrauenswürdig. Ohne
    /// diese Grenze hätte ein Empfänger ein Programm vor sich, das nie
    /// zurückkehrt: kein Fehler, keine Meldung, nur ein Fortschrittstext,
    /// der für immer stehen bleibt.
    ///
    /// # Warum 16
    ///
    /// RFC 9106 empfiehlt einen Durchgang bei hohem Speicher und drei bei
    /// mittlerem; Cabrik schreibt drei. Sechzehn lässt mehr als das
    /// Fünffache Luft und begrenzt den ungünstigsten erlaubten Fall — 4 GiB
    /// bei 16 Durchgängen — auf Minuten.
    pub const T_COST_MAX: u32 = 16;

    /// Empfohlene Werte beim Schreiben: 256 MiB, 3 Durchgänge, 4 Lanes.
    #[must_use]
    pub const fn recommended() -> Self {
        Self {
            m_cost: 262_144,
            t_cost: 3,
            p_cost: 4,
        }
    }

    /// Prüft die Grenzen aus `spec/keyfile-v2.md` §4.
    ///
    /// # Fehler
    ///
    /// [`Error::Malformed`] außerhalb der Grenzen.
    ///
    /// Die Untergrenze schützt davor, dass ein Angreifer ein Keyfile mit
    /// absichtlich schwachen Parametern unterschiebt und damit einen
    /// billigen Rateangriff ermöglicht.
    ///
    /// Die Obergrenzen schützen davor, dass eine präparierte Datei den
    /// Rechner beim Öffnen lahmlegt — `m_cost` über den Speicher,
    /// `t_cost` über die **Zeit**. Die zweite fehlte anfangs: Der Satz hier
    /// nannte nur den Speicherüberlauf, und `t_cost` blieb unbegrenzt.
    pub fn validate(&self) -> Result<()> {
        if self.m_cost < Self::M_COST_MIN {
            return Err(Error::Malformed("keyfile: m_cost below minimum"));
        }
        if self.m_cost > Self::M_COST_MAX {
            return Err(Error::Malformed("keyfile: m_cost above maximum"));
        }
        if self.t_cost < Self::T_COST_MIN {
            return Err(Error::Malformed("keyfile: t_cost below minimum"));
        }
        if self.t_cost > Self::T_COST_MAX {
            return Err(Error::Malformed("keyfile: t_cost above maximum"));
        }
        if self.p_cost < 1 {
            return Err(Error::Malformed("keyfile: p_cost below minimum"));
        }
        Ok(())
    }

    /// Wie viel Arbeitsspeicher die Ableitung belegt, in Bytes.
    ///
    /// `m_cost` zählt in KiB. Argon2 rundet auf ein Vielfaches von
    /// `4 · p_cost` Blöcken **ab** — die Angabe ist also eine Obergrenze,
    /// keine Schätzung.
    #[must_use]
    pub const fn speicherbedarf(&self) -> u64 {
        (self.m_cost as u64).saturating_mul(1024)
    }

    /// Ob die Werte über das hinausgehen, was Cabrik selbst je schreibt.
    ///
    /// Die stärkste eigene Stufe ist [`KdfStufe::Stark`]: 1 GiB, vier
    /// Durchgänge. Mehr Speicher **oder** mehr Arbeit — Speicher mal
    /// Durchgänge — trägt keine Datei, die dieses Programm angelegt hat.
    ///
    /// # Warum das kein Fehler ist
    ///
    /// Weil die Datei trotzdem echt sein kann: von einer anderen Umsetzung
    /// derselben Spezifikation, oder von Hand verschärft. Sie abzuweisen
    /// hieße, jemanden von seinem eigenen Schlüssel auszusperren. Sie
    /// wortlos zu öffnen hieße, einer fremden Datei zu erlauben, den
    /// Rechner minutenlang zu belegen. Dazwischen liegt die Rückfrage
    /// (`spec/entsperrung.md` §4.5) — und die stellt die Oberfläche, nicht
    /// der Kern.
    ///
    /// # Warum `p_cost` nicht zählt
    ///
    /// Es verteilt die Arbeit auf Bahnen, statt sie zu vermehren — derselbe
    /// Grund, aus dem es keine Obergrenze hat (`spec/keyfile-v2.md` §4.1).
    #[must_use]
    pub fn ist_ungewoehnlich(&self) -> bool {
        let stark = KdfStufe::Stark.params();
        let arbeit = |p: &Self| u64::from(p.m_cost).saturating_mul(u64::from(p.t_cost));
        self.m_cost > stark.m_cost || arbeit(self) > arbeit(&stark)
    }

    /// Prüft, ob die Ableitung in die Grenze passt — **bevor** etwas
    /// angelegt wird.
    ///
    /// # Fehler
    ///
    /// [`Error::InsufficientMemory`], wenn nicht.
    pub fn pruefe_speicher(&self, grenze: Speichergrenze) -> Result<()> {
        match grenze.in_bytes() {
            Some(erlaubt) if self.speicherbedarf() > erlaubt => Err(Error::InsufficientMemory {
                needed: self.speicherbedarf(),
                limit: Some(erlaubt),
            }),
            _ => Ok(()),
        }
    }
}

/// Wie viel Arbeitsspeicher eine Passwortableitung **auf diesem Rechner**
/// belegen darf (`spec/keyfile-v2.md` §4.2).
///
/// # Warum der Kern das nicht selbst ermittelt
///
/// Weil er dafür das Betriebssystem fragen müsste, und das geht in Rust
/// nicht ohne `unsafe` — das in dieser Kiste verboten ist. Gefragt wird in
/// `cabrik-speicher`, und der Aufrufer reicht die Antwort herein.
///
/// Dasselbe Muster wie bei der Uhrzeit: Die Sitzung bekommt `jetzt`
/// gesagt, statt die Uhr zu lesen — und lässt sich deshalb prüfen, ohne
/// den Rechner zu kennen, auf dem die Prüfung läuft.
///
/// # Warum eine eigene Art und keine Zahl
///
/// Damit an jeder Aufrufstelle steht, **was** die Zahl ist. Ein nacktes
/// `u64` neben einem Passwort wäre eine Einladung, Bytes und KiB zu
/// verwechseln — `m_cost` zählt in KiB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Speichergrenze(Option<u64>);

impl Speichergrenze {
    /// Keine Grenze außer denen der Spezifikation.
    ///
    /// Für Aufrufer, die den freien Speicher nicht ermitteln können, und für
    /// Tests, die nicht vom Rechner abhängen sollen. Auch dann bricht das
    /// Programm nicht ab, wenn der Speicher nicht reicht: [`argon2id`] legt
    /// fehlbar an.
    pub const UNBEKANNT: Self = Self(None);

    /// Höchstens so viele Bytes.
    #[must_use]
    pub const fn bytes(n: u64) -> Self {
        Self(Some(n))
    }

    /// Die Grenze in Bytes, sofern bekannt.
    #[must_use]
    pub const fn in_bytes(self) -> Option<u64> {
        self.0
    }
}

/// Argon2id — mit Arbeitsspeicher, den **dieser Kern** anlegt und **dieser
/// Kern** überschreibt.
///
/// Jede Passwortableitung des Programms läuft hier durch: Schlüsseldatei,
/// Passwortkapsel, und der Leser für Version 1.
///
/// # Warum nicht `Argon2::hash_password_into`
///
/// Aus zwei Gründen, und beide waren Lücken:
///
/// 1. **Es bricht ab, statt zu melden.** Die Kiste legt ihren Speicher mit
///    `vec!` an. Reicht er nicht, endet das ganze Programm — ohne Meldung,
///    mitten im Entsperren. Hier wird fehlbar angelegt, und aus dem Abbruch
///    wird [`Error::InsufficientMemory`].
/// 2. **Es überschreibt nichts.** Die Blöcke halten den ganzen
///    Zwischenzustand der Ableitung, und aus den letzten folgt der
///    Schlüssel unmittelbar. `argon2` gibt sie ungelöscht frei, auch mit
///    seinem Merkmal `zeroize`, das nur die kleinen Zwischenwerte erfasst.
///    Bis zu 4 GiB Passwortableitung im freigegebenen Speicher passen nicht
///    zu einem Kern, der jeden 32-Byte-Schlüssel beim Freigeben
///    überschreibt.
///
/// # Reihenfolge
///
/// Erst die Grenzen der Spezifikation, dann die des Rechners, dann das
/// Anlegen. Eine präparierte Datei soll abgewiesen sein, bevor auch nur
/// ein Block existiert.
///
/// # Fehler
///
/// - [`Error::Malformed`] bei Parametern außerhalb der Spezifikation
/// - [`Error::InsufficientMemory`], wenn `grenze` überschritten wird oder
///   der Rechner das Anlegen verweigert
pub fn argon2id(
    passwort: &[u8],
    salz: &[u8],
    params: &KdfParams,
    grenze: Speichergrenze,
    ausgabe: &mut [u8],
) -> Result<()> {
    params.validate()?;
    params.pruefe_speicher(grenze)?;

    let p = Params::new(
        params.m_cost,
        params.t_cost,
        u32::from(params.p_cost),
        Some(ausgabe.len()),
    )
    .map_err(|_| Error::Malformed("kdf: invalid argon2 parameters"))?;

    let anzahl = p.block_count();
    let mut bloecke: Zeroizing<Vec<Block>> = Zeroizing::new(Vec::new());
    // Verweigert der Rechner den Speicher, ist ueber die Grenze nichts
    // bekannt -- sie wurde ja gerade eingehalten. `limit: None` sagt genau
    // das, statt eine Zahl zu nennen, die groesser waere als der Bedarf.
    bloecke
        .try_reserve_exact(anzahl)
        .map_err(|_| Error::InsufficientMemory {
            needed: params.speicherbedarf(),
            limit: None,
        })?;
    bloecke.resize(anzahl, Block::new());

    Argon2::new(Algorithm::Argon2id, Version::V0x13, p)
        .hash_password_into_with_memory(passwort, salz, ausgabe, bloecke.as_mut_slice())
        .map_err(|_| Error::Malformed("kdf: argon2 failed"))
}

impl Default for KdfParams {
    fn default() -> Self {
        Self::recommended()
    }
}

// ---------------------------------------------------------------------------
// Identität
// ---------------------------------------------------------------------------

/// Entsperrte Identität. Enthält privates Schlüsselmaterial.
///
/// Wird beim Verwerfen zeroisiert. Kopien vermeiden — jede `clone` erzeugt
/// eine weitere Stelle im Speicher, die überschrieben werden muss.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct Identity {
    /// X25519-Privatschlüssel.
    pub enc_sk: [u8; 32],
    /// Ed25519-Seed. `None` bei Anonymitäts-Identitäten.
    pub sig_sk: Option<[u8; 32]>,
    /// X-Wing-Seed für Post-Quantum. Pflicht ab v2.
    pub pq_seed: [u8; 32],
    /// Erstellungszeitpunkt, Unix-Sekunden.
    #[zeroize(skip)]
    pub created: u64,
    /// Freie Bezeichnung.
    #[zeroize(skip)]
    pub label: Option<String>,
}

impl core::fmt::Debug for Identity {
    /// Gibt **kein** Schlüsselmaterial aus.
    ///
    /// Ein versehentliches `dbg!` oder ein Protokolleintrag darf keine
    /// privaten Schlüssel preisgeben.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Identity")
            .field("enc_sk", &"<redacted>")
            .field("sig_sk", &self.sig_sk.map(|_| "<redacted>"))
            .field("pq_seed", &"<redacted>")
            .field("created", &self.created)
            .field("label", &self.label)
            .finish()
    }
}

impl Identity {
    /// Erzeugt eine neue Identität.
    ///
    /// Ist `signing` falsch, entsteht eine **Anonymitäts-Identität**: Sie
    /// kann empfangen, aber nie dauerhaft signieren.
    ///
    /// Der Post-Quantum-Seed wird **immer** erzeugt, auch wenn Envelopes
    /// zunächst mit der klassischen Suite geschrieben werden. Ohne ihn wäre
    /// der spätere Umstieg eine Neuverteilung aller Schlüssel — siehe
    /// `spec/keyfile-v2.md` §3.1.
    ///
    /// Reihenfolge des Zufallsverbrauchs: `enc_sk`, dann `sig_sk` (falls
    /// verlangt), dann `pq_seed`.
    ///
    /// # Fehler
    ///
    /// Gibt den Fehler der Zufallsquelle weiter.
    pub fn generate<R: Randomness>(rng: &mut R, signing: bool, created: u64) -> Result<Self> {
        let mut enc_sk = [0u8; 32];
        rng.fill(&mut enc_sk)?;

        let sig_sk = if signing {
            let mut s = [0u8; 32];
            rng.fill(&mut s)?;
            Some(s)
        } else {
            None
        };

        let mut pq_seed = [0u8; 32];
        rng.fill(&mut pq_seed)?;

        Ok(Self {
            enc_sk,
            sig_sk,
            pq_seed,
            created,
            label: None,
        })
    }

    /// Ob dauerhaft signiert werden kann.
    #[must_use]
    pub const fn can_sign(&self) -> bool {
        self.sig_sk.is_some()
    }

    /// Der öffentliche Signierschlüssel, sofern die Identität signieren kann.
    ///
    /// v2 speichert öffentliche Schlüssel nicht, sondern berechnet sie
    /// (`spec/keyfile-v2.md` §1). Ohne diese Methode müsste jede aufrufende
    /// Schicht `ed25519-dalek` selbst einbinden, nur um an 32 öffentliche
    /// Bytes zu kommen — genau die Art von Krypto-Abhängigkeit, die außerhalb
    /// des Kerns nichts zu suchen hat.
    #[must_use]
    pub fn sig_pub(&self) -> Option<[u8; 32]> {
        self.sig_sk
            .as_ref()
            .map(|s| SigningKey::from_bytes(s).verifying_key().to_bytes())
    }

    /// Der öffentliche X25519-Schlüssel.
    ///
    /// # Fehler
    ///
    /// [`Error::Malformed`], wenn `enc_sk` kein gültiger Schlüssel ist.
    pub fn enc_pub(&self) -> Result<[u8; 32]> {
        crate::kem::public_key(&self.enc_sk)
    }

    /// Der öffentliche X-Wing-Schlüssel, aus dem Seed berechnet.
    #[must_use]
    pub fn xwing_pub(&self) -> [u8; crate::xwing::PK_LEN] {
        crate::kem::pq_public_key(&self.pq_seed)
    }

    /// Serialisiert den Geheimnisblock als TLV.
    fn to_secret_block(&self) -> Result<Vec<u8>> {
        let mut w = TlvWriter::new();
        w.push(tag::ENC_SK, &self.enc_sk)?;
        if let Some(sig) = &self.sig_sk {
            w.push(tag::SIG_SK, sig)?;
        }
        w.push(tag::CREATED, &self.created.to_be_bytes())?;
        if let Some(label) = &self.label {
            let bytes = label.as_bytes();
            if bytes.len() > LABEL_MAX {
                return Err(Error::Malformed("keyfile: label too long"));
            }
            w.push(tag::LABEL, bytes)?;
        }
        w.push(tag::PQ_SEED, &self.pq_seed)?;
        Ok(w.finish())
    }

    /// Liest den Geheimnisblock.
    fn from_secret_block(block: &[u8]) -> Result<Self> {
        let mut enc_sk = None;
        let mut sig_sk = None;
        let mut created = None;
        let mut label = None;
        let mut pq_seed = None;

        let mut r = TlvReader::new(block);
        while let Some((ty, value)) = r.next_field()? {
            match ty {
                tag::ENC_SK => enc_sk = Some(expect_len::<32>(value, "keyfile: enc_sk length")?),
                tag::SIG_SK => sig_sk = Some(expect_len::<32>(value, "keyfile: sig_sk length")?),
                tag::CREATED => {
                    let b = expect_len::<8>(value, "keyfile: created length")?;
                    created = Some(u64::from_be_bytes(b));
                }
                tag::LABEL => {
                    if value.len() > LABEL_MAX {
                        return Err(Error::Malformed("keyfile: label too long"));
                    }
                    let s = core::str::from_utf8(value)
                        .map_err(|_| Error::Malformed("keyfile: label is not valid UTF-8"))?;
                    label = Some(s.to_owned());
                }
                tag::PQ_SEED => pq_seed = Some(expect_len::<32>(value, "keyfile: pq_seed length")?),
                // Kein Ueberlesen: neue Felder erfordern eine neue Version.
                _ => return Err(Error::Malformed("keyfile: unknown TLV type")),
            }
        }

        Ok(Self {
            enc_sk: enc_sk.ok_or(Error::Malformed("keyfile: enc_sk missing"))?,
            sig_sk,
            pq_seed: pq_seed.ok_or(Error::Malformed("keyfile: pq_seed missing"))?,
            created: created.ok_or(Error::Malformed("keyfile: created missing"))?,
            label,
        })
    }
}

// ---------------------------------------------------------------------------
// Schlüsselableitung
// ---------------------------------------------------------------------------

/// Argon2id über Passwort und Salz.
fn derive_kek(
    password: &[u8],
    salt: &[u8; SALT_LEN],
    params: &KdfParams,
    grenze: Speichergrenze,
) -> Result<Kek> {
    let mut kek = Kek([0u8; KEK_LEN]);
    argon2id(password, salt, params, grenze, &mut kek.0)?;
    Ok(kek)
}

/// Key Encryption Key. Wird beim Verwerfen zeroisiert.
#[derive(Zeroize, ZeroizeOnDrop)]
struct Kek([u8; KEK_LEN]);

// ---------------------------------------------------------------------------
// Schreiben und Lesen
// ---------------------------------------------------------------------------

/// Schreibt eine Identität als v2-Keyfile.
///
/// Reihenfolge des Zufallsverbrauchs: 16 Bytes Salz.
///
/// `grenze` gilt auch hier, obwohl die Werte vom Programm selbst kommen:
/// Wer „Stark“ wählt, auf einem Rechner mit weniger als einem freien
/// Gigabyte, soll das gesagt bekommen, statt minutenlang auf eine
/// auslagernde Ableitung zu warten.
///
/// # Fehler
///
/// - Fehler der Zufallsquelle
/// - [`Error::Malformed`] bei ungültigen Parametern oder zu langem Label
/// - [`Error::InsufficientMemory`], wenn die Ableitung nicht in `grenze`
///   passt
pub fn write<R: Randomness>(
    identity: &Identity,
    password: &[u8],
    params: &KdfParams,
    grenze: Speichergrenze,
    rng: &mut R,
) -> Result<Vec<u8>> {
    params.validate()?;
    // Vor dem Zufall: Wer gleich abgewiesen wird, soll keinen verbrauchen.
    params.pruefe_speicher(grenze)?;

    let mut salt = [0u8; SALT_LEN];
    rng.fill(&mut salt)?;

    // Klartextkopf. Die ersten AAD_LEN Bytes dienen zugleich als AEAD-AAD,
    // damit manipulierte Parameter zu KEYFILE_AUTH_FAILED führen statt zu
    // einer schwaecheren Ableitung.
    let mut out = Vec::with_capacity(HEADER_LEN);
    out.extend_from_slice(&MAGIC);
    out.push(VERSION);
    out.extend_from_slice(&params.m_cost.to_be_bytes());
    out.extend_from_slice(&params.t_cost.to_be_bytes());
    out.push(params.p_cost);
    out.extend_from_slice(&salt);
    debug_assert_eq!(out.len(), AAD_LEN, "Kopfaufbau weicht von der Spec ab");

    let mut secret = identity.to_secret_block()?;

    let kek = derive_kek(password, &salt, params, grenze)?;
    let cipher = ChaCha20Poly1305::new(&Key::from(kek.0));
    let nonce = &Nonce::from([0u8; 12]);

    let aad = out.clone();
    let ct = cipher
        .encrypt(
            nonce,
            Payload {
                msg: &secret,
                aad: &aad,
            },
        )
        .map_err(|_| Error::Malformed("keyfile: encryption failed"));

    // Der Klartext des Geheimnisblocks darf nicht im Speicher zurueckbleiben,
    // auch wenn die Verschluesselung fehlschlug.
    secret.zeroize();
    let ct = ct?;

    let ct_len =
        u32::try_from(ct.len()).map_err(|_| Error::Malformed("keyfile: secret block too large"))?;
    out.extend_from_slice(&ct_len.to_be_bytes());
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Liest die Ableitungsparameter aus dem Klartextkopf.
///
/// **Ohne Passwort.** Sie stehen unverschlüsselt in der Datei, weil man sie
/// braucht, um überhaupt abzuleiten. Sie verraten nichts über den Inhaber —
/// nur, wie teuer ein Rateversuch wäre.
///
/// Die Grenzen werden geprüft, bevor der Wert herausgeht: Eine präparierte
/// Datei soll nicht mit „4 GiB“ in einer Anzeige landen und dort jemanden
/// dazu bringen, sie zu öffnen.
///
/// # Fehler
///
/// [`Error::Malformed`] bei falscher Kennung oder abgeschnittenem Kopf,
/// [`Error::UnsupportedVersion`] bei fremder Fassung.
pub fn params_of(data: &[u8]) -> Result<KdfParams> {
    let head = data
        .get(..HEADER_LEN)
        .ok_or(Error::Malformed("keyfile: truncated header"))?;
    if head.get(..2) != Some(&MAGIC[..]) {
        return Err(Error::Malformed("keyfile: bad magic"));
    }
    if head.get(2) != Some(&VERSION) {
        return Err(Error::UnsupportedVersion);
    }
    let params = KdfParams {
        m_cost: read_u32(head, 3)?,
        t_cost: read_u32(head, 7)?,
        p_cost: *head.get(11).ok_or(Error::Malformed("keyfile: truncated"))?,
    };
    params.validate()?;
    Ok(params)
}

/// Liest ein v2-Keyfile.
///
/// Ob die Werte **ungewöhnlich** sind, prüft diese Funktion nicht — das
/// ist eine Frage an den Menschen, und die stellt der Aufrufer vorher, mit
/// [`params_of`] und [`KdfParams::ist_ungewoehnlich`]. Was sie prüft, ist
/// `grenze`: Passt die Ableitung nicht in den Speicher, wird nichts
/// angelegt.
///
/// # Fehler
///
/// - [`Error::UnsupportedVersion`] bei fremder Formatversion
/// - [`Error::Malformed`] bei kaputter Struktur oder Parametern außerhalb
///   der Grenzen
/// - [`Error::InsufficientMemory`], wenn die Ableitung nicht in `grenze`
///   passt — **nie** als falsches Passwort gemeldet
/// - [`Error::KeyfileAuthFailed`] bei falschem Passwort oder Manipulation
pub fn read(data: &[u8], password: &[u8], grenze: Speichergrenze) -> Result<Identity> {
    let head = data
        .get(..HEADER_LEN)
        .ok_or(Error::Malformed("keyfile: truncated header"))?;

    if head.get(..2) != Some(&MAGIC[..]) {
        return Err(Error::Malformed("keyfile: bad magic"));
    }
    if head.get(2) != Some(&VERSION) {
        return Err(Error::UnsupportedVersion);
    }

    let m_cost = read_u32(head, 3)?;
    let t_cost = read_u32(head, 7)?;
    let p_cost = *head.get(11).ok_or(Error::Malformed("keyfile: truncated"))?;
    let salt: [u8; SALT_LEN] = head
        .get(12..28)
        .and_then(|s| s.try_into().ok())
        .ok_or(Error::Malformed("keyfile: truncated salt"))?;
    let secret_len = read_u32(head, 28)? as usize;

    let params = KdfParams {
        m_cost,
        t_cost,
        p_cost,
    };
    // Vor jeder teuren Operation: Grenzen pruefen. Sonst koennte eine
    // praeparierte Datei mit m_cost = 4 GiB den Rechner lahmlegen, bevor
    // ueberhaupt etwas geprueft wurde.
    params.validate()?;

    let ct_end = HEADER_LEN
        .checked_add(secret_len)
        .ok_or(Error::Malformed("keyfile: length overflow"))?;
    let ct = data
        .get(HEADER_LEN..ct_end)
        .ok_or(Error::Malformed("keyfile: truncated secret block"))?;
    if ct_end != data.len() {
        return Err(Error::Malformed("keyfile: trailing bytes"));
    }

    let aad = head
        .get(..AAD_LEN)
        .ok_or(Error::Malformed("keyfile: truncated header"))?;

    let kek = derive_kek(password, &salt, &params, grenze)?;
    let cipher = ChaCha20Poly1305::new(&Key::from(kek.0));
    let nonce = &Nonce::from([0u8; 12]);

    let mut plain = cipher
        .decrypt(nonce, Payload { msg: ct, aad })
        .map_err(|_| Error::KeyfileAuthFailed)?;

    let identity = Identity::from_secret_block(&plain);
    plain.zeroize();
    identity
}

fn read_u32(head: &[u8], at: usize) -> Result<u32> {
    let end = at
        .checked_add(4)
        .ok_or(Error::Malformed("keyfile: offset overflow"))?;
    let b: [u8; 4] = head
        .get(at..end)
        .and_then(|s| s.try_into().ok())
        .ok_or(Error::Malformed("keyfile: truncated field"))?;
    Ok(u32::from_be_bytes(b))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;
    use crate::rng::OsRandom;

    /// Argon2id mit den empfohlenen Werten braucht 256 MiB und spürbar Zeit.
    /// In Tests genügen die Mindestwerte.
    fn schnelle_params() -> KdfParams {
        KdfParams {
            m_cost: KdfParams::M_COST_MIN,
            t_cost: KdfParams::T_COST_MIN,
            p_cost: 1,
        }
    }

    fn identitaet(signing: bool) -> Identity {
        Identity::generate(&mut OsRandom, signing, 1_700_000_000).unwrap()
    }

    #[test]
    fn round_trip_mit_signierschluessel() {
        let id = identitaet(true);
        let (enc, sig, pq) = (id.enc_sk, id.sig_sk, id.pq_seed);

        let data = write(
            &id,
            b"passwort",
            &schnelle_params(),
            Speichergrenze::UNBEKANNT,
            &mut OsRandom,
        )
        .unwrap();
        let back = read(&data, b"passwort", Speichergrenze::UNBEKANNT).unwrap();

        assert_eq!(back.enc_sk, enc);
        assert_eq!(back.sig_sk, sig);
        assert_eq!(back.pq_seed, pq);
        assert_eq!(back.created, 1_700_000_000);
        assert!(back.can_sign());
    }

    #[test]
    fn round_trip_anonymitaets_keyfile() {
        let id = identitaet(false);
        assert!(!id.can_sign());

        let data = write(
            &id,
            b"pw",
            &schnelle_params(),
            Speichergrenze::UNBEKANNT,
            &mut OsRandom,
        )
        .unwrap();
        let back = read(&data, b"pw", Speichergrenze::UNBEKANNT).unwrap();

        assert!(back.sig_sk.is_none(), "Signierschluessel aufgetaucht");
        assert!(!back.can_sign());
    }

    #[test]
    fn pq_seed_wird_immer_erzeugt() {
        // spec/keyfile-v2.md §3.1 -- auch ohne Signierschluessel.
        for signing in [true, false] {
            let id = identitaet(signing);
            assert_ne!(id.pq_seed, [0u8; 32], "pq_seed fehlt bei signing={signing}");
        }
    }

    #[test]
    fn falsches_passwort_wird_abgelehnt() {
        let id = identitaet(true);
        let data = write(
            &id,
            b"richtig",
            &schnelle_params(),
            Speichergrenze::UNBEKANNT,
            &mut OsRandom,
        )
        .unwrap();
        let e = read(&data, b"falsch", Speichergrenze::UNBEKANNT).unwrap_err();
        assert_eq!(e.code(), "KEYFILE_AUTH_FAILED");
    }

    #[test]
    fn label_ueberlebt_den_round_trip() {
        let mut id = identitaet(true);
        id.label = Some("Arbeitsidentität".to_owned());
        let data = write(
            &id,
            b"pw",
            &schnelle_params(),
            Speichergrenze::UNBEKANNT,
            &mut OsRandom,
        )
        .unwrap();
        let back = read(&data, b"pw", Speichergrenze::UNBEKANNT).unwrap();
        assert_eq!(back.label.as_deref(), Some("Arbeitsidentität"));
    }

    #[test]
    fn zu_langes_label_wird_abgelehnt() {
        let mut id = identitaet(true);
        id.label = Some("x".repeat(LABEL_MAX + 1));
        assert!(
            write(
                &id,
                b"pw",
                &schnelle_params(),
                Speichergrenze::UNBEKANNT,
                &mut OsRandom
            )
            .is_err()
        );
    }

    #[test]
    fn manipulierte_kdf_parameter_werden_erkannt() {
        // Der Kern der AAD-Entscheidung aus spec/keyfile-v2.md §2: Wer die
        // Parameter herabsetzt, um einen billigen Rateangriff zu fahren,
        // bekommt KEYFILE_AUTH_FAILED statt einer schwaecheren Ableitung.
        let id = identitaet(true);
        let mut data = write(
            &id,
            b"pw",
            &KdfParams::recommended(),
            Speichergrenze::UNBEKANNT,
            &mut OsRandom,
        )
        .unwrap();

        // t_cost von 3 auf 3 lassen, aber m_cost auf das Minimum druecken.
        data[3..7].copy_from_slice(&KdfParams::M_COST_MIN.to_be_bytes());

        let e = read(&data, b"pw", Speichergrenze::UNBEKANNT).unwrap_err();
        assert_eq!(e.code(), "KEYFILE_AUTH_FAILED");
    }

    #[test]
    fn schwache_parameter_werden_vor_der_ableitung_abgelehnt() {
        let id = identitaet(true);
        let mut data = write(
            &id,
            b"pw",
            &schnelle_params(),
            Speichergrenze::UNBEKANNT,
            &mut OsRandom,
        )
        .unwrap();

        // m_cost = 8 KiB -- weit unter der Untergrenze.
        data[3..7].copy_from_slice(&8u32.to_be_bytes());
        assert_eq!(
            read(&data, b"pw", Speichergrenze::UNBEKANNT)
                .unwrap_err()
                .code(),
            "MALFORMED"
        );

        // Und die Obergrenze, gegen Speicher-Erschoepfung.
        let mut data2 = write(
            &id,
            b"pw",
            &schnelle_params(),
            Speichergrenze::UNBEKANNT,
            &mut OsRandom,
        )
        .unwrap();
        data2[3..7].copy_from_slice(&(KdfParams::M_COST_MAX + 1).to_be_bytes());
        assert_eq!(
            read(&data2, b"pw", Speichergrenze::UNBEKANNT)
                .unwrap_err()
                .code(),
            "MALFORMED"
        );
    }

    #[test]
    fn fremde_version_wird_abgelehnt() {
        let id = identitaet(true);
        let mut data = write(
            &id,
            b"pw",
            &schnelle_params(),
            Speichergrenze::UNBEKANNT,
            &mut OsRandom,
        )
        .unwrap();
        data[2] = 0x03;
        assert_eq!(
            read(&data, b"pw", Speichergrenze::UNBEKANNT)
                .unwrap_err()
                .code(),
            "UNSUPPORTED_VERSION"
        );
    }

    #[test]
    fn falsches_magic_wird_abgelehnt() {
        let id = identitaet(true);
        let mut data = write(
            &id,
            b"pw",
            &schnelle_params(),
            Speichergrenze::UNBEKANNT,
            &mut OsRandom,
        )
        .unwrap();
        data[0] = 0x00;
        assert_eq!(
            read(&data, b"pw", Speichergrenze::UNBEKANNT)
                .unwrap_err()
                .code(),
            "MALFORMED"
        );
    }

    #[test]
    fn jede_einzelbyte_aenderung_wird_erkannt() {
        let id = identitaet(true);
        let data = write(
            &id,
            b"pw",
            &schnelle_params(),
            Speichergrenze::UNBEKANNT,
            &mut OsRandom,
        )
        .unwrap();

        // Stichprobe ueber Kopf und Ciphertext -- vollstaendig waere zu teuer,
        // weil jeder Versuch eine Argon2id-Ableitung ausloest.
        for i in [0, 2, 3, 12, 27, 28, HEADER_LEN, data.len() - 1] {
            let mut kaputt = data.clone();
            kaputt[i] ^= 0x01;
            assert!(
                read(&kaputt, b"pw", Speichergrenze::UNBEKANNT).is_err(),
                "Aenderung an Byte {i} blieb unbemerkt"
            );
        }
    }

    #[test]
    fn angehaengte_bytes_werden_abgelehnt() {
        let id = identitaet(true);
        let mut data = write(
            &id,
            b"pw",
            &schnelle_params(),
            Speichergrenze::UNBEKANNT,
            &mut OsRandom,
        )
        .unwrap();
        data.push(0xFF);
        assert_eq!(
            read(&data, b"pw", Speichergrenze::UNBEKANNT)
                .unwrap_err()
                .code(),
            "MALFORMED"
        );
    }

    #[test]
    fn abgeschnittene_datei_wird_abgelehnt() {
        let id = identitaet(true);
        let data = write(
            &id,
            b"pw",
            &schnelle_params(),
            Speichergrenze::UNBEKANNT,
            &mut OsRandom,
        )
        .unwrap();
        for len in [0, 1, 10, HEADER_LEN - 1, HEADER_LEN, data.len() - 1] {
            assert!(
                read(&data[..len], b"pw", Speichergrenze::UNBEKANNT).is_err(),
                "Laenge {len} haette abgelehnt werden muessen"
            );
        }
    }

    #[test]
    fn oeffentliche_schluessel_stehen_nicht_im_klartext() {
        // spec/keyfile-v2.md §1 -- der Grund, warum v2 sie nicht speichert.
        let id = identitaet(true);
        let enc_sk = id.enc_sk;
        let data = write(
            &id,
            b"pw",
            &schnelle_params(),
            Speichergrenze::UNBEKANNT,
            &mut OsRandom,
        )
        .unwrap();

        // Weder der private Schluessel noch irgendein Teil davon darf
        // in der Datei auffindbar sein.
        assert!(
            !data.windows(32).any(|w| w == enc_sk),
            "enc_sk steht im Klartext in der Datei"
        );
    }

    #[test]
    fn kopf_hat_die_spezifizierte_laenge() {
        // spec/keyfile-v2.md §2: secret_ct beginnt bei Offset 32.
        let id = identitaet(true);
        let data = write(
            &id,
            b"pw",
            &schnelle_params(),
            Speichergrenze::UNBEKANNT,
            &mut OsRandom,
        )
        .unwrap();
        assert_eq!(&data[0..2], &MAGIC);
        assert_eq!(data[2], VERSION);
        assert_eq!(HEADER_LEN, 32);

        let angekuendigt = u32::from_be_bytes(data[28..32].try_into().unwrap()) as usize;
        assert_eq!(angekuendigt, data.len() - HEADER_LEN);
    }

    #[test]
    fn debug_gibt_kein_schluesselmaterial_preis() {
        let id = identitaet(true);
        let ausgabe = format!("{id:?}");
        assert!(ausgabe.contains("redacted"));
        assert!(!ausgabe.contains(&format!("{:?}", id.enc_sk)));
    }

    #[test]
    fn unbekannter_tlv_typ_wird_abgelehnt() {
        let mut w = TlvWriter::new();
        w.push(tag::ENC_SK, &[1u8; 32]).unwrap();
        w.push(tag::CREATED, &0u64.to_be_bytes()).unwrap();
        w.push(tag::PQ_SEED, &[2u8; 32]).unwrap();
        w.push(0x77, b"unbekannt").unwrap();
        let e = Identity::from_secret_block(&w.finish()).unwrap_err();
        assert_eq!(e.code(), "MALFORMED");
    }

    #[test]
    fn fehlende_pflichtfelder_werden_erkannt() {
        // pq_seed fehlt.
        let mut w = TlvWriter::new();
        w.push(tag::ENC_SK, &[1u8; 32]).unwrap();
        w.push(tag::CREATED, &0u64.to_be_bytes()).unwrap();
        assert!(Identity::from_secret_block(&w.finish()).is_err());

        // enc_sk fehlt.
        let mut w = TlvWriter::new();
        w.push(tag::CREATED, &0u64.to_be_bytes()).unwrap();
        w.push(tag::PQ_SEED, &[2u8; 32]).unwrap();
        assert!(Identity::from_secret_block(&w.finish()).is_err());
    }

    #[test]
    fn parametergrenzen_entsprechen_der_spezifikation() {
        assert_eq!(KdfParams::M_COST_MIN, 64 * 1024);
        assert_eq!(KdfParams::M_COST_MAX, 4 * 1024 * 1024);
        assert_eq!(KdfParams::T_COST_MIN, 3);
        assert_eq!(KdfParams::T_COST_MAX, 16);

        let r = KdfParams::recommended();
        assert_eq!(r.m_cost, 256 * 1024);
        assert!(r.validate().is_ok());
    }

    #[test]
    fn ein_absurd_hohes_t_cost_wird_abgewiesen() {
        /*
         * Der Fund des Fuzzings, als Regel statt als Wirkung.
         *
         * Der Korpus-Test in `tests/robustheit.rs` prueft dasselbe an der
         * echten Datei -- aber er prueft, DASS es schnell zurueckkommt.
         * Dieser hier prueft, WARUM: weil die Grenze greift. Ohne ihn
         * koennte jemand die Grenze anheben und den Korpus-Test durch
         * Geduld gruen bekommen.
         *
         * `t_cost` ist ein `u32`, und die Parameter stehen im Kopf des
         * Envelopes -- also in einer Datei, die der ABSENDER waehlt.
         */
        let boese = KdfParams {
            // Innerhalb der Grenzen. Der Speicher war nie das Problem.
            m_cost: 299_775,
            t_cost: 4_294_901_763,
            p_cost: 4,
        };

        let fehler = boese.validate().expect_err("muss abgewiesen werden");

        // Auf die VARIANTE pruefen, nicht auf `Display`: Der angezeigte
        // Satz ist der fuer den Nutzer ("Die Datei ist beschaedigt ...")
        // und nennt die technische Ursache bewusst nicht. Sie steht im
        // Inneren der Variante -- und dort gehoert das Feld benannt, sonst
        // sucht bei einer Fehlersuche niemand an der richtigen Stelle.
        let Error::Malformed(grund) = fehler else {
            panic!("erwartet wird Malformed, war: {fehler:?}");
        };
        assert!(
            grund.contains("t_cost"),
            "Grund nennt das Feld nicht: {grund}"
        );
    }

    #[test]
    fn genau_an_der_grenze_ist_noch_erlaubt() {
        // Die Gegenprobe. Eine Grenze, die schon bei 16 abweist, waere um
        // eins zu streng -- und niemand faende den Grund.
        let gerade_noch = KdfParams {
            m_cost: KdfParams::M_COST_MIN,
            t_cost: KdfParams::T_COST_MAX,
            p_cost: 1,
        };
        assert!(gerade_noch.validate().is_ok());

        let eins_zu_viel = KdfParams {
            t_cost: KdfParams::T_COST_MAX + 1,
            ..gerade_noch
        };
        assert!(eins_zu_viel.validate().is_err());
    }
}

#[cfg(test)]
mod stufen {
    use super::{KdfParams, KdfStufe};

    /// Alle drei Stufen müssen die Grenzen der Spezifikation einhalten.
    ///
    /// Sonst scheiterte das Schreiben erst **nach** der Passworteingabe —
    /// und zwar an einer Stelle, an der der Nutzer nichts falsch gemacht hat.
    #[test]
    fn jede_stufe_ist_gueltig() {
        for stufe in [KdfStufe::Min, KdfStufe::Empfohlen, KdfStufe::Stark] {
            assert!(
                stufe.params().validate().is_ok(),
                "{stufe:?} liegt ausserhalb der Spezifikation"
            );
        }
    }

    #[test]
    fn die_stufen_steigen_wirklich() {
        // Wenn hier je zwei gleich stark würden, wäre die Auswahl eine
        // Beruhigung ohne Wirkung.
        let m = KdfStufe::Min.params().m_cost;
        let e = KdfStufe::Empfohlen.params().m_cost;
        let s = KdfStufe::Stark.params().m_cost;

        assert_eq!(m, KdfParams::M_COST_MIN);
        assert!(m < e, "{m} < {e}");
        assert!(e < s, "{e} < {s}");
    }

    #[test]
    fn eigene_werte_bekommen_kein_etikett() {
        // Eine Datei mit 200 MiB als „Empfohlen“ zu bezeichnen behauptete
        // 256. Wer eigene Werte gewaehlt hat, soll sie sehen.
        let eigen = KdfParams {
            m_cost: 200_000,
            t_cost: 3,
            p_cost: 4,
        };
        assert_eq!(KdfStufe::von_params(&eigen), None);
    }

    #[test]
    fn jede_stufe_erkennt_sich_selbst_wieder() {
        for stufe in [KdfStufe::Min, KdfStufe::Empfohlen, KdfStufe::Stark] {
            assert_eq!(KdfStufe::von_params(&stufe.params()), Some(stufe));
        }
    }

    #[test]
    fn empfohlen_ist_die_voreinstellung() {
        // Weder das Schwächste noch das Langsamste: Beides waere eine
        // Entscheidung, die der Voreinstellung nicht zusteht.
        assert_eq!(KdfStufe::default(), KdfStufe::Empfohlen);
    }
}

#[cfg(test)]
#[expect(clippy::expect_used, reason = "Fehlschlag soll den Test abbrechen")]
#[expect(
    clippy::indexing_slicing,
    reason = "feste Kopfversaetze aus der Spezifikation"
)]
mod kopf {
    use super::{KdfParams, KdfStufe, Speichergrenze, params_of, write};
    use crate::{Identity, OsRandom};

    #[test]
    fn die_parameter_stehen_ohne_passwort_im_kopf() {
        // Sie muessen lesbar sein, bevor abgeleitet wird -- sonst wuesste
        // niemand, wie. Ein Geheimnis sind sie nicht: Sie sagen nur, was
        // ein Rateversuch kostet.
        let id = Identity::generate(&mut OsRandom, true, 1_700_000_000).expect("Identität");
        let stufe = KdfStufe::Min;
        let datei = write(
            &id,
            b"passwort",
            &stufe.params(),
            Speichergrenze::UNBEKANNT,
            &mut OsRandom,
        )
        .expect("schreiben");

        assert_eq!(params_of(&datei).expect("lesen"), stufe.params());
    }

    #[test]
    fn ein_praeparierter_kopf_kommt_nicht_durch() {
        // Sonst landete „4 GiB" in einer Anzeige und braechte jemanden
        // dazu, die Datei zu oeffnen.
        let id = Identity::generate(&mut OsRandom, true, 1_700_000_000).expect("Identität");
        let mut datei = write(
            &id,
            b"passwort",
            &KdfParams::recommended(),
            Speichergrenze::UNBEKANNT,
            &mut OsRandom,
        )
        .expect("schreiben");

        // m_cost auf 1 KiB setzen -- weit unter der Untergrenze.
        datei[3..7].copy_from_slice(&1_u32.to_le_bytes());

        assert!(params_of(&datei).is_err());
    }

    #[test]
    fn eine_fremde_datei_wird_abgelehnt() {
        assert!(params_of(b"das ist gar keine Schluesseldatei ueberhaupt nicht").is_err());
        assert!(params_of(b"kurz").is_err());
    }
}

/// Speichergrenze und Rückfrage (`spec/keyfile-v2.md` §4.2,
/// `spec/entsperrung.md` §4.5).
#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod speichergrenze {
    use super::{KdfParams, KdfStufe, Speichergrenze, argon2id, read, write};
    use crate::{Error, Identity, OsRandom};

    const MIB: u64 = 1 << 20;

    /// Keine eigene Stufe ist ungewöhnlich — sonst fragte das Programm bei
    /// seinen eigenen Dateien nach, und eine Rückfrage, die bei jedem
    /// Entsperren kommt, wird weggeklickt, ohne gelesen zu werden.
    #[test]
    fn die_eigenen_stufen_sind_nicht_ungewoehnlich() {
        for stufe in [KdfStufe::Min, KdfStufe::Empfohlen, KdfStufe::Stark] {
            assert!(!stufe.params().ist_ungewoehnlich(), "{stufe:?}");
        }
    }

    #[test]
    fn mehr_speicher_oder_mehr_arbeit_als_stark_ist_ungewoehnlich() {
        let stark = KdfStufe::Stark.params();
        // Mehr Speicher, aber weniger Durchgänge.
        let speicher = KdfParams {
            m_cost: 1_049_600,
            t_cost: KdfParams::T_COST_MIN,
            ..stark
        };
        // Weniger Speicher, aber mehr Arbeit: 512 MiB × 16 > 1 GiB × 4.
        let arbeit = KdfParams {
            m_cost: 524_288,
            t_cost: KdfParams::T_COST_MAX,
            ..stark
        };
        // Genau dieselbe Arbeit: 512 MiB × 8 = 1 GiB × 4. Nicht mehr.
        let gleich = KdfParams {
            m_cost: 524_288,
            t_cost: 8,
            ..stark
        };
        assert!(speicher.ist_ungewoehnlich());
        assert!(arbeit.ist_ungewoehnlich());
        assert!(!gleich.ist_ungewoehnlich());
    }

    /// 4 GiB Bedarf gegen 64 MiB Grenze kommt **sofort** zurück. Griffe die
    /// Prüfung erst nach dem Anlegen, liefe dieser Test minutenlang — oder
    /// gar nicht zu Ende.
    #[test]
    fn ueber_der_grenze_wird_nichts_angelegt() {
        let riesig = KdfParams {
            m_cost: KdfParams::M_COST_MAX,
            t_cost: KdfParams::T_COST_MAX,
            p_cost: 1,
        };
        let mut aus = [0_u8; 32];
        let e = argon2id(
            b"passwort",
            &[0_u8; 16],
            &riesig,
            Speichergrenze::bytes(64 * MIB),
            &mut aus,
        )
        .unwrap_err();
        assert_eq!(
            e,
            Error::InsufficientMemory {
                needed: 4096 * MIB,
                limit: Some(64 * MIB),
            }
        );
        assert_eq!(aus, [0_u8; 32], "es wurde trotzdem abgeleitet");
    }

    /// Beim Entsperren darf Speichermangel nie wie ein falsches Passwort
    /// aussehen — und dieselbe Datei geht auf, sobald genug da ist.
    #[test]
    fn speichermangel_beim_lesen_ist_kein_falsches_passwort() {
        let id = Identity::generate(&mut OsRandom, false, 1_700_000_000).expect("Identität");
        let min = KdfStufe::Min.params();
        let datei = write(
            &id,
            b"passwort",
            &min,
            Speichergrenze::UNBEKANNT,
            &mut OsRandom,
        )
        .expect("schreiben");

        let e = read(&datei, b"passwort", Speichergrenze::bytes(MIB)).unwrap_err();
        assert_eq!(e.code(), "INSUFFICIENT_MEMORY");

        let genug = Speichergrenze::bytes(min.speicherbedarf());
        assert!(read(&datei, b"passwort", genug).is_ok());
    }

    /// Auch beim Anlegen: Wer „Stark“ wählt und nicht genug frei hat, soll
    /// das erfahren, statt auf eine auslagernde Ableitung zu warten.
    #[test]
    fn auch_das_schreiben_haelt_die_grenze() {
        let id = Identity::generate(&mut OsRandom, false, 1_700_000_000).expect("Identität");
        let e = write(
            &id,
            b"passwort",
            &KdfStufe::Stark.params(),
            Speichergrenze::bytes(512 * MIB),
            &mut OsRandom,
        )
        .unwrap_err();
        assert_eq!(e.code(), "INSUFFICIENT_MEMORY");
    }

    /// Der eigene Speicher ändert am Ergebnis **nichts**. Sonst ginge keine
    /// bestehende Schlüsseldatei mehr auf — und kein Envelope.
    #[test]
    fn eigener_speicher_rechnet_wie_die_kiste_selbst() {
        use argon2::{Algorithm, Argon2, Params, Version};

        let params = KdfStufe::Min.params();
        let salz = b"salzsalzsalzsalz";

        let mut unser = [0_u8; 32];
        argon2id(
            b"passwort",
            salz,
            &params,
            Speichergrenze::UNBEKANNT,
            &mut unser,
        )
        .unwrap();

        let mut ihr = [0_u8; 32];
        let p = Params::new(
            params.m_cost,
            params.t_cost,
            u32::from(params.p_cost),
            Some(32),
        )
        .unwrap();
        Argon2::new(Algorithm::Argon2id, Version::V0x13, p)
            .hash_password_into(b"passwort", salz, &mut ihr)
            .unwrap();

        assert_eq!(unser, ihr);
    }
}
