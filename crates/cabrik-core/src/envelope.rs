//! Envelope-Format v2 — Zusammensetzung und Zerlegung.
//!
//! Setzt `spec/envelope-v2.md` um. Hier laufen die Bausteine aus [`kem`],
//! [`stream`] und [`tlv`] zusammen.
//!
//! ```text
//! ┌─ Prolog (Klartext) ────────────────────────────────────┐
//! │  Magic, Version, Suite, Empfängerkapseln               │
//! ├─ Verschlüsselter Header ───────────────────────────────┤
//! │  Dateiname, Größe, Zeitstempel, Absenderschlüssel      │
//! ├─ Chunk-Stream ─────────────────────────────────────────┤
//! │  Nutzdaten, 64-KiB-Chunks, jeder einzeln authentisiert │
//! ├─ Verschlüsselter Trailer (nur bei Signatur) ───────────┤
//! │  Ed25519-Signatur über das gesamte Transkript          │
//! └────────────────────────────────────────────────────────┘
//! ```
//!
//! [`kem`]: crate::kem
//! [`stream`]: crate::stream
//! [`tlv`]: crate::tlv

use crate::error::{Error, Result};
use crate::kem::{self, CEK_LEN, Cek, RecipientKeys};
use crate::keyfile::{Identity, KdfParams};
use crate::padme;
use crate::rng::Randomness;
use crate::stream::{self, StreamKey};
use crate::suite::Suite;
use crate::tlv::{TlvReader, TlvWriter, expect_len};

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use ed25519_dalek::{Signature, Signer as _, SigningKey, Verifier as _, VerifyingKey};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Magic-Bytes eines v2-Envelopes.
pub const MAGIC: [u8; 2] = [0xCA, 0x02];

/// Höchstzahl echter Empfänger je Envelope (`spec/envelope-v2.md` §5.3).
pub const MAX_RECIPIENTS: usize = 32;

/// Obergrenze für Attrappen-Auffüllung.
const DUMMY_CAP: usize = 16;

/// Höchstlänge einer Kapsel (`spec/envelope-v2.md` §5).
const MAX_STANZA_LEN: usize = 4096;

/// Vielfaches, auf das der Header aufgefüllt wird (`§7.5`).
const HEADER_PAD_TO: usize = 256;

/// Länge des verschlüsselten Trailers: 64 Bytes Signatur + 16 Bytes Tag.
const TRAILER_LEN: usize = 80;

const FILENAME_MAX: usize = 255;

/// Kapseltypen (`spec/envelope-v2.md` §5).
mod stanza {
    pub(super) const HPKE: u8 = 0x01;
    pub(super) const PASSWORD: u8 = 0x02;
    pub(super) const DUMMY: u8 = 0xFF;
}

/// TLV-Typen des verschlüsselten Headers (`spec/envelope-v2.md` §7.2).
mod tag {
    pub(super) const PADDING: u8 = 0x00;
    pub(super) const CONTENT_TYPE: u8 = 0x01;
    pub(super) const PLAINTEXT_SIZE: u8 = 0x02;
    pub(super) const PADDING_LEN: u8 = 0x03;
    pub(super) const SIGNED: u8 = 0x04;
    pub(super) const SENDER_SIG_PUB: u8 = 0x05;
    pub(super) const FILENAME: u8 = 0x06;
    pub(super) const TIMESTAMP: u8 = 0x07;
    /// Reserviert für in-band-Widerruf, in 2.0 nicht geschrieben.
    pub(super) const REVOCATION_RESERVED: u8 = 0x09;
}

// ---------------------------------------------------------------------------
// Öffentliche Typen
// ---------------------------------------------------------------------------

/// Art der Nutzdaten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    /// Textnachricht. Padding ist voreingestellt an.
    Text,
    /// Einzelne Datei.
    File,
}

impl ContentType {
    const fn to_byte(self) -> u8 {
        match self {
            Self::Text => 0,
            Self::File => 1,
        }
    }

    const fn from_byte(b: u8) -> Result<Self> {
        match b {
            0 => Ok(Self::Text),
            1 => Ok(Self::File),
            // 2 = Archiv ist im Format vorgesehen, aber noch nicht umgesetzt.
            _ => Err(Error::Malformed("envelope: unknown content type")),
        }
    }
}

/// Wer die Nachricht signiert hat — soweit die Kryptographie das hergibt.
///
/// **Bewusst kein Wahrheitswert.** `spec/trust-store.md` §7 verlangt das:
/// Eine gültige Signatur belegt nur, dass der Inhaber eines bestimmten
/// Schlüssels die Nachricht erzeugt hat. **Wer** das ist, entscheidet
/// ausschließlich der Trust Store — der in Schritt 2.8 dazukommt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signer {
    /// Nicht signiert. Anonymer Versand ist ein legitimer Modus, kein Mangel.
    None,
    /// Signatur ist gültig und stammt vom Inhaber dieses Ed25519-Schlüssels.
    Key([u8; 32]),
}

/// Einstellungen beim Verschlüsseln.
#[derive(Debug, Clone)]
pub struct SealOptions<'a> {
    /// Art der Nutzdaten.
    pub content_type: ContentType,
    /// Ursprünglicher Dateiname. Liegt **verschlüsselt** im Header.
    pub filename: Option<&'a str>,
    /// Sendezeitpunkt. `None` lässt ihn ganz weg (`§7.3`).
    pub timestamp: Option<u64>,
    /// Längenauffüllung nach Padmé. `None` folgt der Voreinstellung:
    /// an bei Text, aus bei Dateien (`§10.3`).
    pub padding: Option<bool>,
    /// Empfängerzahl mit Attrappen verschleiern (`§5.3`).
    pub dummy_stanzas: bool,
}

impl Default for SealOptions<'_> {
    fn default() -> Self {
        Self {
            content_type: ContentType::Text,
            filename: None,
            timestamp: None,
            padding: None,
            dummy_stanzas: false,
        }
    }
}

/// Ergebnis des Entschlüsselns.
#[derive(Debug)]
pub struct Opened {
    /// Die Nutzdaten, Füllbytes bereits entfernt.
    pub plaintext: Vec<u8>,
    /// Art der Nutzdaten.
    pub content_type: ContentType,
    /// Dateiname, bereits auf Unbedenklichkeit geprüft.
    pub filename: Option<String>,
    /// Sendezeitpunkt, sofern der Absender ihn mitgeschickt hat.
    pub timestamp: Option<u64>,
    /// Signaturlage. Siehe [`Signer`].
    pub signer: Signer,
}

/// Womit ein Envelope geöffnet wird.
pub enum Opener<'a> {
    /// Mit einer entsperrten Identität.
    Identity(&'a Identity),
    /// Mit einem Passwort (`§5.2`).
    Password(&'a [u8]),
}

// ---------------------------------------------------------------------------
// Schlüsselableitung (§6)
// ---------------------------------------------------------------------------

#[derive(Zeroize, ZeroizeOnDrop)]
struct DerivedKeys {
    header: [u8; 32],
    stream: [u8; 32],
    trailer: [u8; 32],
}

impl DerivedKeys {
    fn derive(cek: &Cek, prologue_hash: &[u8; 32]) -> Self {
        let mut keys = Self {
            header: [0u8; 32],
            stream: [0u8; 32],
            trailer: [0u8; 32],
        };
        // Der Header nutzt kein Salt: sein AAD ist PH, die Bindung an den
        // Prolog entsteht also dort. Stream und Trailer binden über das Salt.
        let hk = Hkdf::<Sha256>::new(None, &cek.0);
        let _ = hk.expand(b"cabrik-v2 header", &mut keys.header);

        let hk = Hkdf::<Sha256>::new(Some(prologue_hash), &cek.0);
        let _ = hk.expand(b"cabrik-v2 stream", &mut keys.stream);
        let _ = hk.expand(b"cabrik-v2 trailer", &mut keys.trailer);

        keys
    }
}

fn aead(key: &[u8; 32]) -> ChaCha20Poly1305 {
    ChaCha20Poly1305::new(&Key::from(*key))
}

/// Nonce aus lauter Nullen.
///
/// Zulässig, weil der jeweilige Schlüssel über den pro Envelope zufälligen
/// CEK eindeutig ist und genau einmal verwendet wird.
fn zero_nonce() -> Nonce {
    Nonce::from([0u8; 12])
}

// ---------------------------------------------------------------------------
// Kapseln (§5)
// ---------------------------------------------------------------------------

fn password_stanza_body(
    password: &[u8],
    cek: &Cek,
    params: &KdfParams,
    salt: &[u8; 16],
) -> Result<Vec<u8>> {
    params.validate()?;

    let p = Params::new(
        params.m_cost,
        params.t_cost,
        u32::from(params.p_cost),
        Some(32),
    )
    .map_err(|_| Error::Malformed("envelope: invalid argon2 parameters"))?;

    let mut kek = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, p)
        .hash_password_into(password, salt, &mut kek)
        .map_err(|_| Error::Malformed("envelope: argon2 failed"))?;

    let wrapped = aead(&kek).encrypt(
        &zero_nonce(),
        Payload {
            msg: &cek.0,
            aad: b"cabrik-v2 pwrap",
        },
    );
    kek.zeroize();
    let wrapped = wrapped.map_err(|_| Error::AuthFailed)?;

    let mut body = Vec::with_capacity(73);
    body.extend_from_slice(salt);
    body.extend_from_slice(&params.m_cost.to_be_bytes());
    body.extend_from_slice(&params.t_cost.to_be_bytes());
    body.push(params.p_cost);
    body.extend_from_slice(&wrapped);
    Ok(body)
}

fn password_unwrap(body: &[u8], password: &[u8]) -> Result<Cek> {
    let salt: [u8; 16] = body
        .get(0..16)
        .and_then(|s| s.try_into().ok())
        .ok_or(Error::Malformed("envelope: password stanza too short"))?;
    let m_cost = read_u32(body, 16)?;
    let t_cost = read_u32(body, 20)?;
    let p_cost = *body
        .get(24)
        .ok_or(Error::Malformed("envelope: password stanza too short"))?;
    let wrapped = body
        .get(25..)
        .ok_or(Error::Malformed("envelope: password stanza too short"))?;

    let params = KdfParams {
        m_cost,
        t_cost,
        p_cost,
    };
    // Grenzen VOR der teuren Ableitung. Sonst könnte eine präparierte Datei
    // mit m_cost = 4 GiB den Rechner lahmlegen, bevor etwas geprüft ist.
    params.validate()?;

    let p = Params::new(m_cost, t_cost, u32::from(p_cost), Some(32))
        .map_err(|_| Error::Malformed("envelope: invalid argon2 parameters"))?;

    let mut kek = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, p)
        .hash_password_into(password, &salt, &mut kek)
        .map_err(|_| Error::Malformed("envelope: argon2 failed"))?;

    let plain = aead(&kek).decrypt(
        &zero_nonce(),
        Payload {
            msg: wrapped,
            aad: b"cabrik-v2 pwrap",
        },
    );
    kek.zeroize();

    let plain = plain.map_err(|_| Error::NoMatchingRecipient)?;
    let cek: [u8; CEK_LEN] = plain
        .as_slice()
        .try_into()
        .map_err(|_| Error::Malformed("envelope: wrong CEK length"))?;
    Ok(Cek(cek))
}

fn read_u32(data: &[u8], at: usize) -> Result<u32> {
    let end = at
        .checked_add(4)
        .ok_or(Error::Malformed("envelope: offset overflow"))?;
    let b: [u8; 4] = data
        .get(at..end)
        .and_then(|s| s.try_into().ok())
        .ok_or(Error::Malformed("envelope: truncated field"))?;
    Ok(u32::from_be_bytes(b))
}

/// Zahl der Kapseln nach Attrappen-Auffüllung (`§5.3`).
///
/// Rundet auf die nächste Zweierpotenz, gedeckelt bei [`DUMMY_CAP`]. Ab
/// mehr echten Empfängern als der Deckel entfällt die Auffüllung — die Zahl
/// ist dann ohnehin wenig aussagekräftig.
fn padded_stanza_count(real: usize) -> usize {
    if real >= DUMMY_CAP {
        return real;
    }
    let mut n = 1usize;
    while n < real {
        n = n.saturating_mul(2);
    }
    // Untergrenze 2: Eine einzige Kapsel verschleiert nichts — sie sagt
    // "genau ein Empfänger". Erst ab zwei entsteht überhaupt eine Gruppe,
    // in der sich der Einzelfall verbergen kann.
    n.max(2)
}

// ---------------------------------------------------------------------------
// Header (§7)
// ---------------------------------------------------------------------------

/// Baut den Header-TLV-Block. Die Parameter entsprechen 1:1 den Typen aus §7.2.
fn build_header(
    content_type: ContentType,
    plaintext_size: u64,
    padding_len: u64,
    sender_sig_pub: Option<[u8; 32]>,
    filename: Option<&str>,
    timestamp: Option<u64>,
) -> Result<Vec<u8>> {
    let mut w = TlvWriter::new();
    w.push(tag::CONTENT_TYPE, &[content_type.to_byte()])?;
    w.push(tag::PLAINTEXT_SIZE, &plaintext_size.to_be_bytes())?;
    w.push(tag::PADDING_LEN, &padding_len.to_be_bytes())?;
    w.push(tag::SIGNED, &[u8::from(sender_sig_pub.is_some())])?;
    if let Some(pk) = &sender_sig_pub {
        w.push(tag::SENDER_SIG_PUB, pk)?;
    }
    if let Some(name) = filename {
        let bytes = name.as_bytes();
        if bytes.len() > FILENAME_MAX {
            return Err(Error::Malformed("envelope: filename too long"));
        }
        w.push(tag::FILENAME, bytes)?;
    }
    if let Some(ts) = timestamp {
        w.push(tag::TIMESTAMP, &ts.to_be_bytes())?;
    }
    let body = w.finish();

    // §7.5: Der Header muss selbst gepolstert werden. `header_len` steht im
    // Klartext, und der Header enthält den Dateinamen — ohne Auffüllung
    // verriete die Länge, wie lang der Name ist.
    //
    // Das Padding-TLV steht als erstes Feld (Typ 0x00 ist der kleinste).
    let mit_kopf = body
        .len()
        .checked_add(3)
        .ok_or(Error::Malformed("envelope: header too large"))?;
    let ziel = mit_kopf
        .checked_next_multiple_of(HEADER_PAD_TO)
        .ok_or(Error::Malformed("envelope: header too large"))?;
    let pad_len = ziel.saturating_sub(mit_kopf);

    let mut padded = TlvWriter::new();
    padded.push(tag::PADDING, &vec![0u8; pad_len])?;
    let mut out = padded.finish();
    out.extend_from_slice(&body);

    debug_assert_eq!(out.len() % HEADER_PAD_TO, 0, "Header nicht ausgerichtet");
    Ok(out)
}

#[derive(Debug)]
struct HeaderFields {
    content_type: ContentType,
    plaintext_size: u64,
    padding_len: u64,
    sender_sig_pub: Option<[u8; 32]>,
    filename: Option<String>,
    timestamp: Option<u64>,
}

fn parse_header(plain: &[u8]) -> Result<HeaderFields> {
    let (mut ct, mut size, mut pad, mut signed) = (None, None, None, None);
    let (mut sig_pub, mut filename, mut timestamp) = (None, None, None);

    let mut r = TlvReader::new(plain);
    while let Some((ty, value)) = r.next_field()? {
        match ty {
            tag::PADDING => {
                if value.iter().any(|&b| b != 0) {
                    return Err(Error::Malformed("envelope: header padding not zero"));
                }
            }
            tag::CONTENT_TYPE => {
                let b = expect_len::<1>(value, "envelope: content_type length")?;
                ct = Some(ContentType::from_byte(b[0])?);
            }
            tag::PLAINTEXT_SIZE => {
                size = Some(u64::from_be_bytes(expect_len::<8>(
                    value,
                    "envelope: plaintext_size length",
                )?));
            }
            tag::PADDING_LEN => {
                pad = Some(u64::from_be_bytes(expect_len::<8>(
                    value,
                    "envelope: padding_len length",
                )?));
            }
            tag::SIGNED => {
                let b = expect_len::<1>(value, "envelope: signed length")?;
                signed = Some(match b[0] {
                    0 => false,
                    1 => true,
                    _ => return Err(Error::Malformed("envelope: signed is not 0 or 1")),
                });
            }
            tag::SENDER_SIG_PUB => {
                sig_pub = Some(expect_len::<32>(value, "envelope: sender_sig_pub length")?);
            }
            tag::FILENAME => {
                if value.len() > FILENAME_MAX {
                    return Err(Error::Malformed("envelope: filename too long"));
                }
                let s = core::str::from_utf8(value)
                    .map_err(|_| Error::Malformed("envelope: filename is not valid UTF-8"))?;
                filename = Some(sanitize_filename(s)?);
            }
            tag::TIMESTAMP => {
                timestamp = Some(u64::from_be_bytes(expect_len::<8>(
                    value,
                    "envelope: timestamp length",
                )?));
            }
            tag::REVOCATION_RESERVED => {
                return Err(Error::Malformed("envelope: reserved TLV type 0x09"));
            }
            _ => return Err(Error::Malformed("envelope: unknown header TLV type")),
        }
    }

    let signed = signed.ok_or(Error::Malformed("envelope: signed missing"))?;
    if signed != sig_pub.is_some() {
        return Err(Error::Malformed("envelope: signed flag and key disagree"));
    }

    Ok(HeaderFields {
        content_type: ct.ok_or(Error::Malformed("envelope: content_type missing"))?,
        plaintext_size: size.ok_or(Error::Malformed("envelope: plaintext_size missing"))?,
        padding_len: pad.ok_or(Error::Malformed("envelope: padding_len missing"))?,
        sender_sig_pub: sig_pub,
        filename,
        timestamp,
    })
}

/// Prüft einen Dateinamen auf Unbedenklichkeit (`spec/envelope-v2.md` §7.2).
///
/// Ein Dateiname aus einem Envelope ist Angreifereingabe. Er darf weder aus
/// dem Zielverzeichnis herausführen noch anders aussehen, als er ist.
fn sanitize_filename(name: &str) -> Result<String> {
    if name.is_empty() {
        return Err(Error::Malformed("envelope: empty filename"));
    }
    if name.contains('/') || name.contains('\\') || name.contains(':') {
        return Err(Error::Malformed(
            "envelope: filename contains path separator",
        ));
    }
    if name == "." || name == ".." {
        return Err(Error::Malformed("envelope: filename is a path element"));
    }
    if name.chars().any(|c| c.is_control()) {
        return Err(Error::Malformed(
            "envelope: filename contains control chars",
        ));
    }
    // Bidi-Overrides: sonst lässt sich `harmlos<U+202E>fdp.exe` als
    // `harmlos exe.pdf` darstellen.
    const BIDI: [char; 7] = [
        '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}', '\u{2069}',
    ];
    if name.chars().any(|c| BIDI.contains(&c)) {
        return Err(Error::Malformed(
            "envelope: filename contains bidi override",
        ));
    }
    // Reservierte Windows-Namen, auch mit Endung.
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let stamm = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    if RESERVED.contains(&stamm.as_str()) {
        return Err(Error::Malformed("envelope: reserved filename"));
    }
    Ok(name.to_owned())
}

// ---------------------------------------------------------------------------
// Transkript und Trailer (§9)
// ---------------------------------------------------------------------------

fn transcript(prologue_hash: &[u8; 32], header_ct: &[u8], chunks: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"cabrik-transcript-v2");
    h.update(prologue_hash);
    h.update(Sha256::digest(header_ct));
    h.update(Sha256::digest(chunks));
    h.finalize().into()
}

// ---------------------------------------------------------------------------
// Verschlüsseln
// ---------------------------------------------------------------------------

/// Baut einen Envelope.
///
/// Reihenfolge des Zufallsverbrauchs nach `spec/envelope-v2.md` §11:
/// zuerst der CEK, dann je Kapsel in **Eingabereihenfolge** der Empfänger.
/// Die Sortierung der Kapseln geschieht danach und verbraucht keinen Zufall.
///
/// # Fehler
///
/// - [`Error::Malformed`] bei zu vielen Empfängern, zu langem Dateinamen
///   oder wenn weder Empfänger noch Passwort angegeben sind
/// - Fehler der Zufallsquelle
pub fn seal<R: Randomness>(
    suite: Suite,
    recipients: &[&[u8]],
    password: Option<&[u8]>,
    plaintext: &[u8],
    sender: Option<&Identity>,
    opts: &SealOptions<'_>,
    rng: &mut R,
) -> Result<Vec<u8>> {
    if recipients.len() > MAX_RECIPIENTS {
        return Err(Error::Malformed("envelope: too many recipients"));
    }
    if recipients.is_empty() && password.is_none() {
        return Err(Error::Malformed("envelope: no recipients and no password"));
    }

    // --- 1. CEK ----------------------------------------------------------
    let cek = Cek::generate(rng)?;

    // --- 2. Kapseln in Eingabereihenfolge erzeugen ------------------------
    let echte = recipients
        .len()
        .checked_add(usize::from(password.is_some()))
        .ok_or(Error::Malformed("envelope: stanza count overflow"))?;

    let gesamt = if opts.dummy_stanzas {
        padded_stanza_count(echte)
    } else {
        echte
    };
    let attrappen = gesamt.saturating_sub(echte);

    let mut bodies: Vec<(u8, Vec<u8>)> = Vec::with_capacity(gesamt);

    for pk in recipients {
        bodies.push((stanza::HPKE, kem::wrap_cek(suite, pk, &cek, rng)?));
    }
    if let Some(pw) = password {
        let mut salt = [0u8; 16];
        rng.fill(&mut salt)?;
        bodies.push((
            stanza::PASSWORD,
            password_stanza_body(pw, &cek, &KdfParams::recommended(), &salt)?,
        ));
    }
    for _ in 0..attrappen {
        // Attrappen sehen aus wie HPKE-Kapseln der gewählten Suite.
        let mut body = vec![0u8; suite.stanza_len()];
        rng.fill(&mut body)?;
        bodies.push((stanza::DUMMY, body));
    }

    // --- 3. Sortieren (§5), verbraucht keinen Zufall ----------------------
    bodies.sort_by(|a, b| a.1.cmp(&b.1));

    // --- 4. Prolog --------------------------------------------------------
    let anzahl =
        u8::try_from(bodies.len()).map_err(|_| Error::Malformed("envelope: too many stanzas"))?;
    let mut prologue = Vec::new();
    prologue.extend_from_slice(&MAGIC);
    prologue.extend_from_slice(&suite.id().to_be_bytes());
    prologue.push(anzahl);
    for (ty, body) in &bodies {
        let len =
            u16::try_from(body.len()).map_err(|_| Error::Malformed("envelope: stanza too long"))?;
        prologue.push(*ty);
        prologue.extend_from_slice(&len.to_be_bytes());
        prologue.extend_from_slice(body);
    }
    let ph: [u8; 32] = Sha256::digest(&prologue).into();

    // --- 5. Padding (§10) -------------------------------------------------
    let padding_an = opts
        .padding
        .unwrap_or(opts.content_type == ContentType::Text);
    let klartext_len = plaintext.len() as u64;
    let gepolstert = if padding_an {
        padme::padme(klartext_len)?
    } else {
        klartext_len
    };
    let padding_len = gepolstert.saturating_sub(klartext_len);

    let mut nutzdaten = Vec::with_capacity(
        usize::try_from(gepolstert).map_err(|_| Error::Malformed("envelope: payload too large"))?,
    );
    nutzdaten.extend_from_slice(plaintext);
    nutzdaten.resize(
        usize::try_from(gepolstert).map_err(|_| Error::Malformed("envelope: payload too large"))?,
        0,
    );

    // --- 6. Header --------------------------------------------------------
    let sig_key = sender.and_then(|id| id.sig_sk.as_ref().map(SigningKey::from_bytes));
    let sig_pub = sig_key.as_ref().map(|k| k.verifying_key().to_bytes());

    let keys = DerivedKeys::derive(&cek, &ph);

    let header_plain = build_header(
        opts.content_type,
        klartext_len,
        padding_len,
        sig_pub,
        opts.filename,
        opts.timestamp,
    )?;
    let header_ct = aead(&keys.header)
        .encrypt(
            &zero_nonce(),
            Payload {
                msg: &header_plain,
                aad: &ph,
            },
        )
        .map_err(|_| Error::AuthFailed)?;

    // --- 7. Chunk-Stream --------------------------------------------------
    let stream_key = StreamKey::from_bytes(keys.stream);
    let chunks = stream::seal(&stream_key, &nutzdaten)?;
    nutzdaten.zeroize();

    // --- 8. Zusammensetzen ------------------------------------------------
    let header_len = u32::try_from(header_ct.len())
        .map_err(|_| Error::Malformed("envelope: header too large"))?;
    let mut out = prologue;
    out.extend_from_slice(&header_len.to_be_bytes());
    out.extend_from_slice(&header_ct);
    out.extend_from_slice(&chunks);

    // --- 9. Trailer (§9) --------------------------------------------------
    if let Some(key) = sig_key {
        let t = transcript(&ph, &header_ct, &chunks);
        let sig = key.sign(&t);
        let trailer = aead(&keys.trailer)
            .encrypt(
                &zero_nonce(),
                Payload {
                    msg: &sig.to_bytes(),
                    aad: &t,
                },
            )
            .map_err(|_| Error::AuthFailed)?;
        debug_assert_eq!(trailer.len(), TRAILER_LEN);
        out.extend_from_slice(&trailer);
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// Entschlüsseln
// ---------------------------------------------------------------------------

/// Öffnet einen Envelope.
///
/// Ablauf nach `spec/envelope-v2.md` §12. Der Klartext wird **erst
/// zurückgegeben, wenn der Trailer geprüft ist** — bei Fehlschlag gibt es
/// kein Teilergebnis (§8.4).
///
/// # Fehler
///
/// Siehe [`Error`]. `AUTH_FAILED` und `NO_MATCHING_RECIPIENT` sind nach
/// außen ununterscheidbar formuliert.
pub fn open(opener: &Opener<'_>, data: &[u8], require_signature: bool) -> Result<Opened> {
    // --- 1./2. Magic und Suite -------------------------------------------
    if data.get(..2) != Some(&MAGIC[..]) {
        return Err(Error::Malformed("envelope: bad magic"));
    }
    let suite = Suite::from_id(u16::from_be_bytes(
        data.get(2..4)
            .and_then(|s| s.try_into().ok())
            .ok_or(Error::Malformed("envelope: truncated suite id"))?,
    ))?;

    // --- 3. Kapseln parsen, PH berechnen ---------------------------------
    let anzahl = usize::from(
        *data
            .get(4)
            .ok_or(Error::Malformed("envelope: truncated stanza count"))?,
    );
    let mut pos = 5usize;
    let mut stanzas: Vec<(u8, &[u8])> = Vec::with_capacity(anzahl);

    for _ in 0..anzahl {
        let ty = *data
            .get(pos)
            .ok_or(Error::Malformed("envelope: truncated stanza"))?;
        let len_at = pos
            .checked_add(1)
            .ok_or(Error::Malformed("envelope: offset overflow"))?;
        let len = usize::from(u16::from_be_bytes(
            data.get(len_at..len_at.saturating_add(2))
                .and_then(|s| s.try_into().ok())
                .ok_or(Error::Malformed("envelope: truncated stanza length"))?,
        ));
        if len > MAX_STANZA_LEN {
            return Err(Error::Malformed("envelope: stanza too long"));
        }
        let body_at = len_at
            .checked_add(2)
            .ok_or(Error::Malformed("envelope: offset overflow"))?;
        let ende = body_at
            .checked_add(len)
            .ok_or(Error::Malformed("envelope: offset overflow"))?;
        let body = data
            .get(body_at..ende)
            .ok_or(Error::Malformed("envelope: truncated stanza body"))?;
        stanzas.push((ty, body));
        pos = ende;
    }

    let prologue = data
        .get(..pos)
        .ok_or(Error::Malformed("envelope: truncated prologue"))?;
    let ph: [u8; 32] = Sha256::digest(prologue).into();

    // --- 4./5. Trial Decryption ------------------------------------------
    let header_len = usize::try_from(read_u32(data, pos)?)
        .map_err(|_| Error::Malformed("envelope: header too large"))?;
    let header_at = pos
        .checked_add(4)
        .ok_or(Error::Malformed("envelope: offset overflow"))?;
    let header_ende = header_at
        .checked_add(header_len)
        .ok_or(Error::Malformed("envelope: offset overflow"))?;
    let header_ct = data
        .get(header_at..header_ende)
        .ok_or(Error::Malformed("envelope: truncated header"))?;

    let (cek, keys) = find_recipient(opener, &stanzas, suite, &ph, header_ct)?;
    let _ = cek;

    let header_plain = aead(&keys.header)
        .decrypt(
            &zero_nonce(),
            Payload {
                msg: header_ct,
                aad: &ph,
            },
        )
        .map_err(|_| Error::NoMatchingRecipient)?;

    // --- 6. Header strikt validieren -------------------------------------
    let fields = parse_header(&header_plain)?;

    // --- 7. Signaturpflicht VOR jeder Nutzdatenverarbeitung ---------------
    if require_signature && fields.sender_sig_pub.is_none() {
        return Err(Error::SignatureMissing);
    }

    // --- 8. Chunks --------------------------------------------------------
    let gesamt = fields
        .plaintext_size
        .checked_add(fields.padding_len)
        .ok_or(Error::Malformed("envelope: length overflow"))?;
    let chunk_bytes = usize::try_from(stream::ciphertext_len(gesamt)?)
        .map_err(|_| Error::Malformed("envelope: payload too large"))?;
    let chunk_ende = header_ende
        .checked_add(chunk_bytes)
        .ok_or(Error::Malformed("envelope: offset overflow"))?;
    let chunks = data.get(header_ende..chunk_ende).ok_or(Error::Truncated)?;

    let stream_key = StreamKey::from_bytes(keys.stream);
    let mut nutzdaten = stream::open(&stream_key, chunks, gesamt)?;

    // --- 9. Trailer -------------------------------------------------------
    let signer = match fields.sender_sig_pub {
        None => {
            if chunk_ende != data.len() {
                nutzdaten.zeroize();
                return Err(Error::Malformed("envelope: trailing bytes"));
            }
            Signer::None
        }
        Some(pk) => {
            let trailer = data.get(chunk_ende..).ok_or(Error::Truncated)?;
            if trailer.len() != TRAILER_LEN {
                nutzdaten.zeroize();
                return Err(if trailer.len() < TRAILER_LEN {
                    Error::Truncated
                } else {
                    Error::Malformed("envelope: trailing bytes after trailer")
                });
            }
            let t = transcript(&ph, header_ct, chunks);
            let sig_bytes = aead(&keys.trailer)
                .decrypt(
                    &zero_nonce(),
                    Payload {
                        msg: trailer,
                        aad: &t,
                    },
                )
                .map_err(|_| Error::SignatureInvalid)?;

            let sig: [u8; 64] = sig_bytes
                .as_slice()
                .try_into()
                .map_err(|_| Error::Malformed("envelope: wrong signature length"))?;
            let vk = VerifyingKey::from_bytes(&pk).map_err(|_| Error::SignatureInvalid)?;
            vk.verify(&t, &Signature::from_bytes(&sig))
                .map_err(|_| Error::SignatureInvalid)?;
            Signer::Key(pk)
        }
    };

    // --- 10. Padding prüfen und abschneiden (§10.4) -----------------------
    let echte_len = usize::try_from(fields.plaintext_size)
        .map_err(|_| Error::Malformed("envelope: plaintext too large"))?;
    let fuell = nutzdaten
        .get(echte_len..)
        .ok_or(Error::Malformed("envelope: padding beyond payload"))?;
    if fuell.iter().any(|&b| b != 0) {
        nutzdaten.zeroize();
        return Err(Error::Malformed("envelope: padding bytes are not zero"));
    }
    nutzdaten.truncate(echte_len);

    // --- 11. Freigeben ----------------------------------------------------
    Ok(Opened {
        plaintext: nutzdaten,
        content_type: fields.content_type,
        filename: fields.filename,
        timestamp: fields.timestamp,
        signer,
    })
}

/// Probiert alle Kapseln durch (`§7.1`).
///
/// Schlägt jede fehl, gibt es [`Error::NoMatchingRecipient`] — nach außen
/// ununterscheidbar von einem Header, der sich nicht entschlüsseln ließ.
fn find_recipient(
    opener: &Opener<'_>,
    stanzas: &[(u8, &[u8])],
    suite: Suite,
    ph: &[u8; 32],
    header_ct: &[u8],
) -> Result<(Cek, DerivedKeys)> {
    for (ty, body) in stanzas {
        let kandidat = match (opener, *ty) {
            (Opener::Identity(id), stanza::HPKE) => kem::unwrap_cek(
                suite,
                RecipientKeys {
                    enc_sk: &id.enc_sk,
                    pq_seed: &id.pq_seed,
                },
                body,
            )
            .ok(),
            (Opener::Password(pw), stanza::PASSWORD) => password_unwrap(body, pw).ok(),
            _ => None,
        };
        let Some(cek) = kandidat else { continue };

        let keys = DerivedKeys::derive(&cek, ph);
        // Erst wenn auch der Header aufgeht, ist es wirklich unsere Kapsel.
        if aead(&keys.header)
            .decrypt(
                &zero_nonce(),
                Payload {
                    msg: header_ct,
                    aad: ph,
                },
            )
            .is_ok()
        {
            return Ok((cek, keys));
        }
    }
    Err(Error::NoMatchingRecipient)
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;
    use crate::rng::OsRandom;
    use crate::stream::CHUNK_SIZE;

    fn identitaet(signing: bool) -> Identity {
        Identity::generate(&mut OsRandom, signing, 1_700_000_000).unwrap()
    }

    fn pk_von(id: &Identity) -> [u8; 32] {
        kem::public_key(&id.enc_sk).unwrap()
    }

    fn opts() -> SealOptions<'static> {
        SealOptions::default()
    }

    #[test]
    fn round_trip_ein_empfaenger_unsigniert() {
        let empf = identitaet(false);
        let env = seal(
            Suite::Classical,
            &[&pk_von(&empf)[..]],
            None,
            b"Hallo Welt",
            None,
            &opts(),
            &mut OsRandom,
        )
        .unwrap();

        let auf = open(&Opener::Identity(&empf), &env, false).unwrap();
        assert_eq!(auf.plaintext, b"Hallo Welt");
        assert_eq!(auf.signer, Signer::None);
        assert_eq!(auf.content_type, ContentType::Text);
    }

    #[test]
    fn round_trip_signiert() {
        let empf = identitaet(false);
        let abs = identitaet(true);
        let env = seal(
            Suite::Classical,
            &[&pk_von(&empf)[..]],
            None,
            b"signiert",
            Some(&abs),
            &opts(),
            &mut OsRandom,
        )
        .unwrap();

        let auf = open(&Opener::Identity(&empf), &env, true).unwrap();
        assert_eq!(auf.plaintext, b"signiert");
        let erwartet = SigningKey::from_bytes(abs.sig_sk.as_ref().unwrap())
            .verifying_key()
            .to_bytes();
        assert_eq!(auf.signer, Signer::Key(erwartet));
    }

    #[test]
    fn signaturpflicht_wird_vor_den_nutzdaten_geprueft() {
        let empf = identitaet(false);
        let env = seal(
            Suite::Classical,
            &[&pk_von(&empf)[..]],
            None,
            b"anonym",
            None,
            &opts(),
            &mut OsRandom,
        )
        .unwrap();
        assert_eq!(
            open(&Opener::Identity(&empf), &env, true)
                .unwrap_err()
                .code(),
            "SIGNATURE_MISSING"
        );
    }

    #[test]
    fn mehrere_empfaenger_koennen_alle_oeffnen() {
        let empfaenger: Vec<Identity> = (0..5).map(|_| identitaet(false)).collect();
        let pks: Vec<[u8; 32]> = empfaenger.iter().map(pk_von).collect();
        let pk_refs: Vec<&[u8]> = pks.iter().map(|k| &k[..]).collect();

        let env = seal(
            Suite::Classical,
            &pk_refs,
            None,
            b"an alle",
            None,
            &opts(),
            &mut OsRandom,
        )
        .unwrap();

        for e in &empfaenger {
            let auf = open(&Opener::Identity(e), &env, false).unwrap();
            assert_eq!(auf.plaintext, b"an alle");
        }

        // Ein Unbeteiligter nicht.
        let fremd = identitaet(false);
        assert_eq!(
            open(&Opener::Identity(&fremd), &env, false)
                .unwrap_err()
                .code(),
            "NO_MATCHING_RECIPIENT"
        );
    }

    #[test]
    fn passwort_modus() {
        let env = seal(
            Suite::Classical,
            &[],
            Some(b"geheim"),
            b"per Passwort",
            None,
            &opts(),
            &mut OsRandom,
        )
        .unwrap();

        let auf = open(&Opener::Password(b"geheim"), &env, false).unwrap();
        assert_eq!(auf.plaintext, b"per Passwort");

        assert_eq!(
            open(&Opener::Password(b"falsch"), &env, false)
                .unwrap_err()
                .code(),
            "NO_MATCHING_RECIPIENT"
        );
    }

    #[test]
    fn empfaenger_und_passwort_gleichzeitig() {
        let empf = identitaet(false);
        let env = seal(
            Suite::Classical,
            &[&pk_von(&empf)[..]],
            Some(b"pw"),
            b"beides",
            None,
            &opts(),
            &mut OsRandom,
        )
        .unwrap();

        assert_eq!(
            open(&Opener::Identity(&empf), &env, false)
                .unwrap()
                .plaintext,
            b"beides"
        );
        assert_eq!(
            open(&Opener::Password(b"pw"), &env, false)
                .unwrap()
                .plaintext,
            b"beides"
        );
    }

    #[test]
    fn ohne_empfaenger_und_ohne_passwort_wird_abgelehnt() {
        assert!(
            seal(
                Suite::Classical,
                &[],
                None,
                b"x",
                None,
                &opts(),
                &mut OsRandom
            )
            .is_err()
        );
    }

    #[test]
    fn zu_viele_empfaenger_werden_abgelehnt() {
        let keys = vec![[1u8; 32]; MAX_RECIPIENTS + 1];
        let pks: Vec<&[u8]> = keys.iter().map(|k| &k[..]).collect();
        assert!(
            seal(
                Suite::Classical,
                &pks,
                None,
                b"x",
                None,
                &opts(),
                &mut OsRandom
            )
            .is_err()
        );
    }

    #[test]
    fn dateiname_und_zeitstempel_liegen_verschluesselt() {
        let empf = identitaet(false);
        let o = SealOptions {
            content_type: ContentType::File,
            filename: Some("Kuendigung_vertraulich.pdf"),
            timestamp: Some(1_700_000_042),
            ..SealOptions::default()
        };
        let env = seal(
            Suite::Classical,
            &[&pk_von(&empf)[..]],
            None,
            b"Inhalt",
            None,
            &o,
            &mut OsRandom,
        )
        .unwrap();

        // Der Kernbefund aus v1: der Name darf nirgends im Klartext stehen.
        assert!(
            !env.windows(9)
                .any(|w| w == b"Kuendigu"[..8].to_vec().as_slice()),
            "Dateiname im Klartext auffindbar"
        );

        let auf = open(&Opener::Identity(&empf), &env, false).unwrap();
        assert_eq!(auf.filename.as_deref(), Some("Kuendigung_vertraulich.pdf"));
        assert_eq!(auf.timestamp, Some(1_700_000_042));
        assert_eq!(auf.content_type, ContentType::File);
    }

    #[test]
    fn absenderschluessel_liegt_verschluesselt() {
        // In v1 stand sender_sig_pub im Klartext-Header und hob die
        // Anonymitaet des ephemeren Austauschs sofort wieder auf.
        let empf = identitaet(false);
        let abs = identitaet(true);
        let sig_pub = SigningKey::from_bytes(abs.sig_sk.as_ref().unwrap())
            .verifying_key()
            .to_bytes();

        let env = seal(
            Suite::Classical,
            &[&pk_von(&empf)[..]],
            None,
            b"x",
            Some(&abs),
            &opts(),
            &mut OsRandom,
        )
        .unwrap();

        assert!(
            !env.windows(32).any(|w| w == sig_pub),
            "Absenderschluessel im Klartext auffindbar"
        );
    }

    #[test]
    fn header_ist_auf_256_ausgerichtet() {
        // §7.5: sonst verriete header_len die Laenge des Dateinamens.
        let empf = identitaet(false);
        let pk = pk_von(&empf);

        let mut laengen = std::collections::HashSet::new();
        for name in ["a.txt", "ein_sehr_viel_laengerer_dateiname.pdf", "x"] {
            let o = SealOptions {
                content_type: ContentType::File,
                filename: Some(name),
                ..SealOptions::default()
            };
            let env = seal(
                Suite::Classical,
                &[&pk[..]],
                None,
                b"gleich",
                None,
                &o,
                &mut OsRandom,
            )
            .unwrap();
            // header_len steht direkt nach dem Prolog.
            let prolog_len = 5 + 3 + Suite::Classical.stanza_len();
            let hl = u32::from_be_bytes(env[prolog_len..prolog_len + 4].try_into().unwrap());
            laengen.insert(hl);
        }
        assert_eq!(
            laengen.len(),
            1,
            "verschiedene Dateinamenlaengen ergaben verschiedene header_len"
        );
    }

    #[test]
    fn padding_ist_bei_text_voreingestellt_an() {
        let empf = identitaet(false);
        let pk = pk_von(&empf);

        let kurz = seal(
            Suite::Classical,
            &[&pk[..]],
            None,
            b"Ja",
            None,
            &opts(),
            &mut OsRandom,
        )
        .unwrap();
        let laenger = seal(
            Suite::Classical,
            &[&pk[..]],
            None,
            b"Treffen 14 Uhr Hauptbahnhof",
            None,
            &opts(),
            &mut OsRandom,
        )
        .unwrap();
        assert_eq!(
            kurz.len(),
            laenger.len(),
            "kurze Texte sind an der Laenge unterscheidbar"
        );
    }

    #[test]
    fn round_trip_ueber_mehrere_chunks() {
        let empf = identitaet(false);
        let daten: Vec<u8> = (0..(2 * CHUNK_SIZE + 1234))
            .map(|i| (i % 251) as u8)
            .collect();
        let o = SealOptions {
            content_type: ContentType::File,
            ..SealOptions::default()
        };
        let env = seal(
            Suite::Classical,
            &[&pk_von(&empf)[..]],
            None,
            &daten,
            None,
            &o,
            &mut OsRandom,
        )
        .unwrap();
        let auf = open(&Opener::Identity(&empf), &env, false).unwrap();
        assert_eq!(auf.plaintext, daten);
    }

    #[test]
    fn leerer_klartext() {
        let empf = identitaet(false);
        let o = SealOptions {
            content_type: ContentType::File,
            ..SealOptions::default()
        };
        let env = seal(
            Suite::Classical,
            &[&pk_von(&empf)[..]],
            None,
            b"",
            None,
            &o,
            &mut OsRandom,
        )
        .unwrap();
        assert!(
            open(&Opener::Identity(&empf), &env, false)
                .unwrap()
                .plaintext
                .is_empty()
        );
    }

    #[test]
    fn jede_einzelbyte_aenderung_wird_erkannt() {
        let empf = identitaet(false);
        let abs = identitaet(true);
        let env = seal(
            Suite::Classical,
            &[&pk_von(&empf)[..]],
            None,
            b"unveraendert",
            Some(&abs),
            &opts(),
            &mut OsRandom,
        )
        .unwrap();

        for i in 0..env.len() {
            let mut kaputt = env.clone();
            kaputt[i] ^= 0x01;
            assert!(
                open(&Opener::Identity(&empf), &kaputt, false).is_err(),
                "Aenderung an Byte {i} blieb unbemerkt"
            );
        }
    }

    #[test]
    fn abgeschnittener_envelope_wird_erkannt() {
        let empf = identitaet(false);
        let env = seal(
            Suite::Classical,
            &[&pk_von(&empf)[..]],
            None,
            b"vollstaendig",
            None,
            &opts(),
            &mut OsRandom,
        )
        .unwrap();
        for len in [0, 4, 20, env.len() / 2, env.len() - 1] {
            assert!(
                open(&Opener::Identity(&empf), &env[..len], false).is_err(),
                "Laenge {len} haette abgelehnt werden muessen"
            );
        }
    }

    #[test]
    fn angehaengte_bytes_werden_erkannt() {
        let empf = identitaet(false);
        let mut env = seal(
            Suite::Classical,
            &[&pk_von(&empf)[..]],
            None,
            b"x",
            None,
            &opts(),
            &mut OsRandom,
        )
        .unwrap();
        env.push(0xFF);
        assert!(open(&Opener::Identity(&empf), &env, false).is_err());
    }

    #[test]
    fn unbekannte_suite_wird_abgelehnt() {
        let empf = identitaet(false);
        let mut env = seal(
            Suite::Classical,
            &[&pk_von(&empf)[..]],
            None,
            b"x",
            None,
            &opts(),
            &mut OsRandom,
        )
        .unwrap();
        env[3] = 0x03; // es gibt keine Suite 0x0003
        assert_eq!(
            open(&Opener::Identity(&empf), &env, false)
                .unwrap_err()
                .code(),
            "UNSUPPORTED_SUITE"
        );
    }

    #[test]
    fn attrappen_verschleiern_die_empfaengerzahl() {
        let empf = identitaet(false);
        let o = SealOptions {
            dummy_stanzas: true,
            ..SealOptions::default()
        };
        let env = seal(
            Suite::Classical,
            &[&pk_von(&empf)[..]],
            None,
            b"x",
            None,
            &o,
            &mut OsRandom,
        )
        .unwrap();
        assert_eq!(
            env[4], 2,
            "ein Empfaenger sollte auf zwei Kapseln aufrunden"
        );
        // Und es laesst sich trotzdem oeffnen.
        assert_eq!(
            open(&Opener::Identity(&empf), &env, false)
                .unwrap()
                .plaintext,
            b"x"
        );
    }

    #[test]
    fn attrappen_auffuellung_entspricht_der_spezifikation() {
        // Untergrenze 2: eine einzelne Kapsel verschleiert nichts.
        assert_eq!(padded_stanza_count(1), 2);
        assert_eq!(padded_stanza_count(2), 2);
        assert_eq!(padded_stanza_count(3), 4);
        assert_eq!(padded_stanza_count(5), 8);
        assert_eq!(padded_stanza_count(9), 16);
        assert_eq!(padded_stanza_count(16), 16, "ab dem Deckel keine Attrappen");
        assert_eq!(padded_stanza_count(20), 20);
    }

    #[test]
    fn kapselreihenfolge_verraet_die_eingabereihenfolge_nicht() {
        // §5: Kapseln werden lexikographisch nach ihren Bytes sortiert.
        let a = identitaet(false);
        let b = identitaet(false);
        let env = seal(
            Suite::Classical,
            &[&pk_von(&a)[..], &pk_von(&b)[..]],
            None,
            b"x",
            None,
            &opts(),
            &mut OsRandom,
        )
        .unwrap();

        let s = Suite::Classical.stanza_len();
        let erste = &env[5 + 3..5 + 3 + s];
        let zweite = &env[5 + 3 + s + 3..5 + 3 + s + 3 + s];
        assert!(erste < zweite, "Kapseln sind nicht sortiert");
    }

    #[test]
    fn gefaehrliche_dateinamen_werden_abgelehnt() {
        for name in [
            "../etc/passwd",
            "a/b.txt",
            "a\\b.txt",
            "C:evil",
            "..",
            ".",
            "CON",
            "nul.txt",
            "harmlos\u{202E}fdp.exe",
            "mit\u{0007}Glocke",
            "",
        ] {
            assert!(
                sanitize_filename(name).is_err(),
                "{name:?} haette abgelehnt werden muessen"
            );
        }
        for name in ["normal.pdf", "Bericht 2026.docx", "ümlaut.txt"] {
            assert!(sanitize_filename(name).is_ok(), "{name:?} ist harmlos");
        }
    }

    #[test]
    fn reservierter_tlv_typ_wird_abgelehnt() {
        // 0x09 ist fuer in-band-Widerruf reserviert (trust-store.md §4.3)
        // und darf in 2.0 nicht vorkommen.
        let mut w = TlvWriter::new();
        w.push(tag::CONTENT_TYPE, &[0]).unwrap();
        w.push(tag::PLAINTEXT_SIZE, &0u64.to_be_bytes()).unwrap();
        w.push(tag::PADDING_LEN, &0u64.to_be_bytes()).unwrap();
        w.push(tag::SIGNED, &[0]).unwrap();
        w.push(tag::REVOCATION_RESERVED, b"x").unwrap();
        assert_eq!(parse_header(&w.finish()).unwrap_err().code(), "MALFORMED");
    }

    // ---------------------------------------------------------------
    // Post-Quantum-Suite (§4.1)
    // ---------------------------------------------------------------

    #[test]
    fn round_trip_mit_post_quantum_suite() {
        let empf = identitaet(false);
        let pq_pk = kem::pq_public_key(&empf.pq_seed);

        let env = seal(
            Suite::Hybrid,
            &[&pq_pk[..]],
            None,
            b"quantensicher",
            None,
            &opts(),
            &mut OsRandom,
        )
        .unwrap();

        assert_eq!(env[2..4], [0x00, 0x02], "falsche Suite im Prolog");
        let auf = open(&Opener::Identity(&empf), &env, false).unwrap();
        assert_eq!(auf.plaintext, b"quantensicher");
    }

    #[test]
    fn post_quantum_signiert_und_mit_mehreren_empfaengern() {
        let empfaenger: Vec<Identity> = (0..3).map(|_| identitaet(false)).collect();
        let pks: Vec<[u8; 1216]> = empfaenger
            .iter()
            .map(|e| kem::pq_public_key(&e.pq_seed))
            .collect();
        let refs: Vec<&[u8]> = pks.iter().map(|k| &k[..]).collect();
        let abs = identitaet(true);

        let env = seal(
            Suite::Hybrid,
            &refs,
            None,
            b"an alle, quantensicher",
            Some(&abs),
            &opts(),
            &mut OsRandom,
        )
        .unwrap();

        for e in &empfaenger {
            let auf = open(&Opener::Identity(e), &env, true).unwrap();
            assert_eq!(auf.plaintext, b"an alle, quantensicher");
            assert!(matches!(auf.signer, Signer::Key(_)));
        }

        let fremd = identitaet(false);
        assert_eq!(
            open(&Opener::Identity(&fremd), &env, false)
                .unwrap_err()
                .code(),
            "NO_MATCHING_RECIPIENT"
        );
    }

    #[test]
    fn post_quantum_kapsel_hat_die_spezifizierte_groesse() {
        let empf = identitaet(false);
        let pq_pk = kem::pq_public_key(&empf.pq_seed);
        let env = seal(
            Suite::Hybrid,
            &[&pq_pk[..]],
            None,
            b"x",
            None,
            &opts(),
            &mut OsRandom,
        )
        .unwrap();

        // Prolog: magic(2) + suite(2) + count(1) + typ(1) + len(2) + body
        let len = u16::from_be_bytes(env[6..8].try_into().unwrap()) as usize;
        assert_eq!(len, 1168, "Kapsellaenge weicht von §4.1 ab");
        assert_eq!(len, Suite::Hybrid.stanza_len());
    }

    #[test]
    fn schluessel_der_falschen_suite_wird_abgelehnt() {
        // Ein 32-Byte-X25519-Schluessel taugt nicht fuer die Hybrid-Suite
        // und umgekehrt. Ohne diese Pruefung entstuende ein Envelope, den
        // niemand oeffnen kann.
        let empf = identitaet(false);
        let klassisch = pk_von(&empf);
        let pq = kem::pq_public_key(&empf.pq_seed);

        assert!(
            seal(
                Suite::Hybrid,
                &[&klassisch[..]],
                None,
                b"x",
                None,
                &opts(),
                &mut OsRandom
            )
            .is_err()
        );
        assert!(
            seal(
                Suite::Classical,
                &[&pq[..]],
                None,
                b"x",
                None,
                &opts(),
                &mut OsRandom
            )
            .is_err()
        );
    }

    #[test]
    fn suiten_sind_nicht_gegeneinander_austauschbar() {
        // §5.1: `info` bindet die Suite in die Ableitung. Eine
        // umetikettierte Suite-Kennung darf nicht aufgehen.
        let empf = identitaet(false);
        let mut env = seal(
            Suite::Classical,
            &[&pk_von(&empf)[..]],
            None,
            b"x",
            None,
            &opts(),
            &mut OsRandom,
        )
        .unwrap();
        env[3] = 0x02;
        assert!(open(&Opener::Identity(&empf), &env, false).is_err());
    }

    #[test]
    fn zwei_envelopes_desselben_inhalts_unterscheiden_sich() {
        let empf = identitaet(false);
        let pk = pk_von(&empf);
        let a = seal(
            Suite::Classical,
            &[&pk[..]],
            None,
            b"x",
            None,
            &opts(),
            &mut OsRandom,
        )
        .unwrap();
        let b = seal(
            Suite::Classical,
            &[&pk[..]],
            None,
            b"x",
            None,
            &opts(),
            &mut OsRandom,
        )
        .unwrap();
        assert_ne!(a, b, "Verschluesselung ist nicht randomisiert");
        assert_eq!(a.len(), b.len());
    }
}
