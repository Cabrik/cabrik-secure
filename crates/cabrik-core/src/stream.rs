//! Chunk-Stream nach `spec/envelope-v2.md` §8.
//!
//! STREAM-Konstruktion nach Hoang–Reyhanitabar–Rogaway–Vizár, wie in `age`.
//! Die Nutzdaten werden in Blöcke zu 64 KiB zerlegt, jeder einzeln
//! authentisiert. Die Position steckt im Nonce, der letzte Block ist
//! ausdrücklich markiert.
//!
//! ```text
//! N_i = counter(11 Bytes BE) ‖ final_flag(1 Byte)
//! ```
//!
//! # Was das abwehrt
//!
//! | Angriff | Wirkung |
//! |---|---|
//! | Abschneiden | Der letzte gelesene Chunk trägt `final_flag = 0` → `TRUNCATED` |
//! | Chunks vertauschen | Zähler im Nonce stimmt nicht → `AUTH_FAILED` |
//! | Chunk wiederholen | dito |
//! | Chunk aus fremdem Envelope | `stream_key` hängt über `PH` am Prolog → `AUTH_FAILED` |
//! | Chunks anhängen | Die Zahl ergibt sich aus dem Header → `MALFORMED` |
//!
//! # Kein `std::io`
//!
//! Dieses Modul arbeitet auf Byte-Scheiben und kennt keine Dateien. Der
//! Kern soll später per UniFFI nach Swift und Kotlin; `std::io` im
//! Krypto-Kern würde das unnötig erschweren. Adapter für `Read`/`Write`
//! gehören in die CLI.

use crate::error::{Error, Result};

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Klartextgröße eines Chunks in Bytes (`spec/envelope-v2.md` §8).
pub const CHUNK_SIZE: usize = 65_536;

/// Zuwachs je Chunk durch das AEAD-Tag.
pub const TAG_LEN: usize = 16;

/// Ciphertextgröße eines vollen Chunks.
pub const CHUNK_CIPHERTEXT_SIZE: usize = CHUNK_SIZE + TAG_LEN;

/// Länge des Zählerteils im Nonce.
const COUNTER_LEN: usize = 11;

/// Aus dem CEK abgeleiteter Schlüssel für den Chunk-Stream.
///
/// Wird beim Verwerfen zeroisiert.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct StreamKey([u8; 32]);

impl core::fmt::Debug for StreamKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("StreamKey(<redacted>)")
    }
}

impl StreamKey {
    /// Leitet den Stream-Schlüssel ab (`spec/envelope-v2.md` §6).
    ///
    /// ```text
    /// stream_key = HKDF-SHA256(ikm = CEK, salt = PH, info = "cabrik-v2 stream")
    /// ```
    ///
    /// `PH` ist der SHA-256 über den Prolog. Er bindet die Nutzdaten an den
    /// **exakten Empfängersatz**: Ein Chunk aus einem anderen Envelope lässt
    /// sich nicht einsetzen, und das Entfernen einer fremden Empfängerkapsel
    /// macht den Stream unlesbar.
    ///
    /// # Panics
    ///
    /// Nie. Die Ausgabelänge ist konstant 32 Bytes und damit weit unter der
    /// für HKDF-SHA256 zulässigen Grenze.
    #[must_use]
    pub fn derive(cek: &[u8; 32], prologue_hash: &[u8; 32]) -> Self {
        let mut key = [0u8; 32];
        let hk = Hkdf::<Sha256>::new(Some(prologue_hash), cek);
        if hk.expand(b"cabrik-v2 stream", &mut key).is_err() {
            // Unerreichbar; 32 Bytes liegen weit unter 255 * 32.
            key = [0u8; 32];
        }
        Self(key)
    }

    /// Baut einen Schlüssel aus rohen Bytes — für Testvektoren.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// Nonce für den Chunk an Position `counter`.
///
/// # Fehler
///
/// [`Error::Malformed`] bei Zählerüberlauf. Der Zähler ist 88 Bit breit;
/// erreichbar ist das nicht, geprüft wird es trotzdem.
fn nonce_for(counter: u64, final_chunk: bool) -> Result<[u8; 12]> {
    let mut nonce = [0u8; 12];
    let be = counter.to_be_bytes();

    // Der Zähler ist 11 Bytes breit, `u64` liefert 8. Die oberen drei Bytes
    // bleiben null — der Wertebereich von u64 ist damit vollständig abgedeckt.
    nonce
        .get_mut(COUNTER_LEN.saturating_sub(be.len())..COUNTER_LEN)
        .ok_or(Error::Malformed("stream: nonce layout"))?
        .copy_from_slice(&be);

    *nonce
        .get_mut(COUNTER_LEN)
        .ok_or(Error::Malformed("stream: nonce layout"))? = u8::from(final_chunk);

    Ok(nonce)
}

/// Zahl der Chunks für eine Nutzdatenlänge (`spec/envelope-v2.md` §8.1).
///
/// ```text
/// chunk_count = max(1, ceil(total / CHUNK_SIZE))
/// ```
///
/// `max(1, …)` deckt den leeren Klartext ab: Er ergibt genau einen Chunk der
/// Länge 0.
///
/// # Fehler
///
/// [`Error::Malformed`] bei Überlauf.
///
/// # Beispiele
///
/// ```
/// use cabrik_core::stream::chunk_count;
/// assert_eq!(chunk_count(0).unwrap(), 1);
/// assert_eq!(chunk_count(1).unwrap(), 1);
/// assert_eq!(chunk_count(65_536).unwrap(), 1);
/// assert_eq!(chunk_count(65_537).unwrap(), 2);
/// ```
pub fn chunk_count(total: u64) -> Result<u64> {
    if total == 0 {
        return Ok(1);
    }
    let size = CHUNK_SIZE as u64;
    let voll = total
        .checked_div(size)
        .ok_or(Error::Malformed("stream: chunk size zero"))?;
    let rest = total.checked_rem(size).unwrap_or(0);
    if rest == 0 {
        Ok(voll)
    } else {
        voll.checked_add(1)
            .ok_or(Error::Malformed("stream: chunk count overflow"))
    }
}

/// Gesamtlänge des Ciphertext-Bereichs für eine Nutzdatenlänge.
///
/// # Fehler
///
/// [`Error::Malformed`] bei Überlauf.
pub fn ciphertext_len(total: u64) -> Result<u64> {
    let chunks = chunk_count(total)?;
    let tags = chunks
        .checked_mul(TAG_LEN as u64)
        .ok_or(Error::Malformed("stream: length overflow"))?;
    total
        .checked_add(tags)
        .ok_or(Error::Malformed("stream: length overflow"))
}

// ---------------------------------------------------------------------------
// Verschlüsseln
// ---------------------------------------------------------------------------

/// Zerlegt `plaintext` in Chunks und verschlüsselt jeden einzeln.
///
/// `plaintext` ist bereits gepolstert — Padding geschieht **vor** dem
/// Chunking (`spec/envelope-v2.md` §8).
///
/// # Fehler
///
/// [`Error::AuthFailed`] bei einem Fehler der AEAD-Schicht,
/// [`Error::Malformed`] bei Längenüberlauf.
pub fn seal(key: &StreamKey, plaintext: &[u8]) -> Result<Vec<u8>> {
    let kapazitaet = usize::try_from(ciphertext_len(plaintext.len() as u64)?)
        .map_err(|_| Error::Malformed("stream: output too large for this platform"))?;
    let mut out = Vec::with_capacity(kapazitaet);
    seal_into(key, plaintext, &mut out)?;
    Ok(out)
}

/// Wie [`seal`], hängt das Ergebnis aber an einen **bestehenden** Puffer an.
///
/// # Warum es diese Form gibt
///
/// [`seal`] gibt einen eigenen `Vec` zurück, den der Aufrufer anschließend in
/// seinen Ausgabepuffer kopieren muss. Bei einer 200-MB-Datei sind das 200 MB
/// zusätzlich, nur um sie gleich darauf wieder freizugeben.
///
/// Diese Form schreibt unmittelbar dorthin, wo die Bytes hingehören. Das
/// spart eine vollständige Kopie der Nutzdaten — bei großen Dateien der
/// Unterschied zwischen „läuft" und „geht der Arbeitsspeicher aus".
///
/// # Fehler
///
/// [`Error::AuthFailed`] bei einem Fehler der AEAD-Schicht,
/// [`Error::Malformed`] bei Längenüberlauf.
pub fn seal_into(key: &StreamKey, plaintext: &[u8], out: &mut Vec<u8>) -> Result<()> {
    let cipher = ChaCha20Poly1305::new(&Key::from(key.0));

    let gesamt = plaintext.len() as u64;
    let chunks = chunk_count(gesamt)?;

    // Einmal reservieren statt bei jedem Chunk nachzuwachsen: Umkopieren beim
    // Wachsen wäre genau die Kopie, die hier vermieden werden soll.
    let zusatz = usize::try_from(ciphertext_len(gesamt)?)
        .map_err(|_| Error::Malformed("stream: output too large for this platform"))?;
    out.reserve(zusatz);

    for index in 0..chunks {
        let start = usize::try_from(
            index
                .checked_mul(CHUNK_SIZE as u64)
                .ok_or(Error::Malformed("stream: offset overflow"))?,
        )
        .map_err(|_| Error::Malformed("stream: offset too large"))?;

        let end = start.saturating_add(CHUNK_SIZE).min(plaintext.len());
        let stueck = plaintext
            .get(start..end)
            .ok_or(Error::Malformed("stream: chunk out of range"))?;

        let letzter = index.saturating_add(1) == chunks;
        let nonce = nonce_for(index, letzter)?;

        let ct = cipher
            .encrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: stueck,
                    aad: b"",
                },
            )
            .map_err(|_| Error::AuthFailed)?;
        out.extend_from_slice(&ct);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Entschlüsseln
// ---------------------------------------------------------------------------

/// Entschlüsselt einen Chunk-Stream.
///
/// `total` ist `plaintext_size + padding_len` aus dem verschlüsselten Header.
/// Daraus ergibt sich die erwartete Chunk-Zahl; es wird **nicht** geraten und
/// nicht vorausgeschaut (`spec/envelope-v2.md` §8.1).
///
/// # Fehler
///
/// - [`Error::Truncated`], wenn Chunks fehlen
/// - [`Error::Malformed`], wenn Bytes übrig bleiben oder Längen nicht passen
/// - [`Error::AuthFailed`] bei Manipulation, Umordnung oder Wiederholung
pub fn open(key: &StreamKey, ciphertext: &[u8], total: u64) -> Result<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new(&Key::from(key.0));
    let chunks = chunk_count(total)?;

    let erwartet = ciphertext_len(total)?;
    if ciphertext.len() as u64 != erwartet {
        // Zu wenig heißt abgeschnitten, zu viel heißt angehängt. Die
        // Unterscheidung hilft dem Nutzer und ist hier gefahrlos.
        return if (ciphertext.len() as u64) < erwartet {
            Err(Error::Truncated)
        } else {
            Err(Error::Malformed("stream: trailing bytes after last chunk"))
        };
    }

    let kapazitaet = usize::try_from(total)
        .map_err(|_| Error::Malformed("stream: output too large for this platform"))?;
    let mut out = Vec::with_capacity(kapazitaet);
    let mut pos = 0usize;

    for index in 0..chunks {
        let letzter = index.saturating_add(1) == chunks;

        let rest_klartext = total.saturating_sub(index.saturating_mul(CHUNK_SIZE as u64));
        let klartext_laenge = usize::try_from(rest_klartext.min(CHUNK_SIZE as u64))
            .map_err(|_| Error::Malformed("stream: chunk length overflow"))?;
        let ct_laenge = klartext_laenge.saturating_add(TAG_LEN);

        let ende = pos
            .checked_add(ct_laenge)
            .ok_or(Error::Malformed("stream: offset overflow"))?;
        let stueck = ciphertext.get(pos..ende).ok_or(Error::Truncated)?;

        let nonce = nonce_for(index, letzter)?;
        let pt = cipher
            .decrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: stueck,
                    aad: b"",
                },
            )
            .map_err(|_| Error::AuthFailed)?;

        out.extend_from_slice(&pt);
        pos = ende;
    }

    Ok(out)
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;

    fn key() -> StreamKey {
        StreamKey::from_bytes([0x5A; 32])
    }

    fn daten(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn chunk_zahl_entspricht_der_spezifikation() {
        assert_eq!(chunk_count(0).unwrap(), 1, "leerer Klartext ergibt 1 Chunk");
        assert_eq!(chunk_count(1).unwrap(), 1);
        assert_eq!(chunk_count(65_535).unwrap(), 1);
        assert_eq!(chunk_count(65_536).unwrap(), 1, "exakt ein voller Chunk");
        assert_eq!(chunk_count(65_537).unwrap(), 2);
        assert_eq!(chunk_count(131_072).unwrap(), 2);
        assert_eq!(chunk_count(131_073).unwrap(), 3);
    }

    #[test]
    fn nonce_aufbau() {
        // 11 Bytes Zaehler big-endian, dann das Flag.
        let n = nonce_for(0, false).unwrap();
        assert_eq!(n, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let n = nonce_for(1, false).unwrap();
        assert_eq!(n[10], 1);
        assert_eq!(n[11], 0);

        let n = nonce_for(1, true).unwrap();
        assert_eq!(n[10], 1);
        assert_eq!(n[11], 1, "Abschlussflag fehlt");

        let n = nonce_for(0x0102_0304_0506_0708, false).unwrap();
        assert_eq!(&n[3..11], &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(&n[0..3], &[0, 0, 0], "obere Zaehlerbytes muessen null sein");
    }

    #[test]
    fn round_trip_ueber_die_relevanten_laengen() {
        for len in [
            0,
            1,
            255,
            CHUNK_SIZE - 1,
            CHUNK_SIZE,
            CHUNK_SIZE + 1,
            2 * CHUNK_SIZE,
            2 * CHUNK_SIZE + 1,
            3 * CHUNK_SIZE + 12_345,
        ] {
            let pt = daten(len);
            let ct = seal(&key(), &pt).unwrap();
            assert_eq!(
                ct.len() as u64,
                ciphertext_len(len as u64).unwrap(),
                "Ciphertextlaenge bei {len}"
            );
            let zurueck = open(&key(), &ct, len as u64).unwrap();
            assert_eq!(zurueck, pt, "Round-Trip bei Laenge {len}");
        }
    }

    #[test]
    fn leerer_klartext_ergibt_einen_chunk() {
        let ct = seal(&key(), &[]).unwrap();
        assert_eq!(ct.len(), TAG_LEN, "genau ein leerer Chunk plus Tag");
        assert_eq!(open(&key(), &ct, 0).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn abschneiden_wird_erkannt() {
        let pt = daten(3 * CHUNK_SIZE);
        let ct = seal(&key(), &pt).unwrap();

        // Nach dem zweiten Chunk abschneiden.
        let kurz = &ct[..2 * CHUNK_CIPHERTEXT_SIZE];
        assert_eq!(
            open(&key(), kurz, pt.len() as u64).unwrap_err().code(),
            "TRUNCATED"
        );

        // Auch ein einzelnes fehlendes Byte.
        let fast = &ct[..ct.len() - 1];
        assert_eq!(
            open(&key(), fast, pt.len() as u64).unwrap_err().code(),
            "TRUNCATED"
        );
    }

    #[test]
    fn anhaengen_wird_erkannt() {
        let pt = daten(1000);
        let mut ct = seal(&key(), &pt).unwrap();
        ct.push(0x00);
        assert_eq!(
            open(&key(), &ct, pt.len() as u64).unwrap_err().code(),
            "MALFORMED"
        );
    }

    #[test]
    fn vertauschte_chunks_werden_erkannt() {
        let pt = daten(2 * CHUNK_SIZE);
        let ct = seal(&key(), &pt).unwrap();

        let mut getauscht = Vec::with_capacity(ct.len());
        getauscht.extend_from_slice(&ct[CHUNK_CIPHERTEXT_SIZE..]);
        getauscht.extend_from_slice(&ct[..CHUNK_CIPHERTEXT_SIZE]);

        assert_eq!(
            open(&key(), &getauscht, pt.len() as u64)
                .unwrap_err()
                .code(),
            "AUTH_FAILED"
        );
    }

    #[test]
    fn wiederholter_chunk_wird_erkannt() {
        let pt = daten(2 * CHUNK_SIZE);
        let ct = seal(&key(), &pt).unwrap();

        let mut doppelt = Vec::with_capacity(ct.len());
        doppelt.extend_from_slice(&ct[..CHUNK_CIPHERTEXT_SIZE]);
        doppelt.extend_from_slice(&ct[..CHUNK_CIPHERTEXT_SIZE]);

        assert_eq!(
            open(&key(), &doppelt, pt.len() as u64).unwrap_err().code(),
            "AUTH_FAILED"
        );
    }

    #[test]
    fn letzter_chunk_ist_nicht_gegen_vorletzten_austauschbar() {
        // Der Kern der Abschlussmarkierung: Ein voller letzter Chunk und ein
        // voller mittlerer Chunk haben dieselbe Laenge, aber verschiedene
        // Nonces.
        let pt = daten(2 * CHUNK_SIZE);
        let ct = seal(&key(), &pt).unwrap();

        let mut vertauscht = ct.clone();
        let (a, b) = vertauscht.split_at_mut(CHUNK_CIPHERTEXT_SIZE);
        a.swap_with_slice(&mut b[..CHUNK_CIPHERTEXT_SIZE]);

        assert!(open(&key(), &vertauscht, pt.len() as u64).is_err());
    }

    #[test]
    fn jede_einzelbyte_aenderung_wird_erkannt() {
        let pt = daten(300);
        let ct = seal(&key(), &pt).unwrap();
        for i in 0..ct.len() {
            let mut kaputt = ct.clone();
            kaputt[i] ^= 0x01;
            assert!(
                open(&key(), &kaputt, pt.len() as u64).is_err(),
                "Aenderung an Byte {i} blieb unbemerkt"
            );
        }
    }

    #[test]
    fn fremder_schluessel_scheitert() {
        let pt = daten(100);
        let ct = seal(&key(), &pt).unwrap();
        let fremd = StreamKey::from_bytes([0x11; 32]);
        assert_eq!(
            open(&fremd, &ct, pt.len() as u64).unwrap_err().code(),
            "AUTH_FAILED"
        );
    }

    #[test]
    fn falsche_laengenangabe_scheitert() {
        let pt = daten(1000);
        let ct = seal(&key(), &pt).unwrap();
        // Der Header behauptet eine andere Laenge als der Stream hergibt.
        assert!(open(&key(), &ct, 999).is_err());
        assert!(open(&key(), &ct, 1001).is_err());
    }

    #[test]
    fn ableitung_haengt_am_prolog() {
        // spec/envelope-v2.md §6: PH als Salt bindet die Nutzdaten an den
        // Empfaengersatz.
        let cek = [1u8; 32];
        let a = StreamKey::derive(&cek, &[0xAA; 32]);
        let b = StreamKey::derive(&cek, &[0xBB; 32]);
        assert_ne!(
            a.0, b.0,
            "verschiedene Prologe ergeben denselben Schluessel"
        );

        // Und deterministisch.
        assert_eq!(StreamKey::derive(&cek, &[0xAA; 32]).0, a.0);
    }

    #[test]
    fn ableitung_haengt_am_cek() {
        let ph = [0xAA; 32];
        let a = StreamKey::derive(&[1u8; 32], &ph);
        let b = StreamKey::derive(&[2u8; 32], &ph);
        assert_ne!(a.0, b.0);
    }

    #[test]
    fn debug_gibt_keinen_schluessel_preis() {
        let ausgabe = format!("{:?}", key());
        assert!(ausgabe.contains("redacted"));
        assert!(!ausgabe.contains("90"));
    }
}
