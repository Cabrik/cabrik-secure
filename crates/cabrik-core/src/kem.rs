//! Schlüsselverpackung nach `spec/envelope-v2.md` §5.1.
//!
//! Ein zufälliger Content Encryption Key (CEK) wird pro Empfänger mit HPKE
//! verpackt. Die Nutzdaten werden dadurch **einmal** verschlüsselt,
//! unabhängig von der Empfängerzahl — der Grund, warum Mehrfachempfänger
//! ohne Mehrfachaufwand möglich sind.
//!
//! # Warum Base-Modus und nicht HPKE-Auth
//!
//! HPKE-Auth bindet die statische Absenderidentität in die KEM-Kapsel. Das
//! wäre bei mehreren Empfängern unhandlich, gibt keine Nichtabstreitbarkeit
//! und — entscheidend — würde den Absender gegenüber jedem preisgeben, der
//! die Kapsel sieht. Cabrik Secure signiert stattdessen mit Ed25519 im
//! **verschlüsselten** Trailer. Erst dadurch sind Authentizität und
//! Anonymität gegenüber Dritten gleichzeitig erreichbar.
//!
//! # Eingekapselte Abhängigkeit
//!
//! Dieses Modul ist die einzige Stelle, die die HPKE-Bibliothek kennt. Für
//! die Post-Quantum-Suite in Schritt 2.6 muss nur hier etwas geschehen.

use crate::error::{Error, Result};
use crate::rng::Randomness;
use crate::suite::Suite;
use crate::xwing::{self, XWing};

use hpke::aead::ChaCha20Poly1305;
use hpke::kdf::HkdfSha256;
use hpke::kem::X25519HkdfSha256;
use hpke::{Deserializable, Kem as KemTrait, OpModeR, OpModeS, Serializable};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Länge des Content Encryption Key.
pub const CEK_LEN: usize = 32;

type Kem = X25519HkdfSha256;
type Kdf = HkdfSha256;
type Aead = ChaCha20Poly1305;

/// Content Encryption Key. Wird beim Verwerfen zeroisiert.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Cek(pub [u8; CEK_LEN]);

impl Cek {
    /// Erzeugt einen frischen CEK.
    ///
    /// Reihenfolge des Zufallsverbrauchs: 32 Bytes, als **erstes** einer
    /// Envelope-Operation (`spec/envelope-v2.md` §11).
    ///
    /// # Fehler
    ///
    /// Gibt den Fehler der Zufallsquelle weiter.
    pub fn generate<R: Randomness>(rng: &mut R) -> Result<Self> {
        let mut cek = [0u8; CEK_LEN];
        rng.fill(&mut cek)?;
        Ok(Self(cek))
    }
}

impl core::fmt::Debug for Cek {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Cek(<redacted>)")
    }
}

// ---------------------------------------------------------------------------
// Brücke zur Zufallsquelle
// ---------------------------------------------------------------------------

/// Reicht eine feste Bytefolge an die HPKE-Bibliothek weiter.
///
/// Die Spezifikation legt je Suite fest, wie viele Bytes für den ephemeren
/// Schlüssel verbraucht werden. Statt das zu hoffen, wird es hier
/// erzwungen: Fordert die Bibliothek mehr an, bleibt der Vorrat leer und
/// [`FixedBytes::exhausted`] meldet es. Der Aufrufer prüft das, bevor er das
/// Ergebnis verwendet.
///
/// Das ist zugleich die Voraussetzung für bit-genaue Verschlüsselungsvektoren
/// (`spec/test-vectors.md` §3).
struct FixedBytes<'a> {
    bytes: &'a [u8],
    pos: usize,
    exhausted: bool,
}

impl<'a> FixedBytes<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            pos: 0,
            exhausted: false,
        }
    }

    const fn exhausted(&self) -> bool {
        self.exhausted
    }
}

impl FixedBytes<'_> {
    fn take(&mut self, dest: &mut [u8]) {
        let end = self.pos.saturating_add(dest.len());
        match self.bytes.get(self.pos..end) {
            Some(src) => {
                dest.copy_from_slice(src);
                self.pos = end;
            }
            None => {
                // Mehr angefordert als vorgesehen. Nullen füllen und
                // vormerken — der Aufrufer verwirft das Ergebnis.
                dest.fill(0);
                self.exhausted = true;
            }
        }
    }
}

// rand_core 0.10 hat die Traits umgebaut: `TryRng` ist die Basis, `Rng` und
// `CryptoRng` entstehen daraus über Blanket-Implementierungen, sobald der
// Fehlertyp `Infallible` ist.
impl rand_core::TryRng for FixedBytes<'_> {
    type Error = core::convert::Infallible;

    fn try_next_u32(&mut self) -> core::result::Result<u32, Self::Error> {
        let mut b = [0u8; 4];
        self.take(&mut b);
        Ok(u32::from_le_bytes(b))
    }

    fn try_next_u64(&mut self) -> core::result::Result<u64, Self::Error> {
        let mut b = [0u8; 8];
        self.take(&mut b);
        Ok(u64::from_le_bytes(b))
    }

    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> core::result::Result<(), Self::Error> {
        self.take(dst);
        Ok(())
    }
}

impl rand_core::TryCryptoRng for FixedBytes<'_> {}

// ---------------------------------------------------------------------------
// Öffentliche Schnittstelle
// ---------------------------------------------------------------------------

/// Leitet den öffentlichen X25519-Schlüssel aus dem privaten ab.
///
/// v2 speichert öffentliche Schlüssel nicht, sondern berechnet sie —
/// siehe `spec/keyfile-v2.md` §1.
///
/// # Fehler
///
/// [`Error::Malformed`], wenn `enc_sk` kein gültiger X25519-Schlüssel ist.
pub fn public_key(enc_sk: &[u8; 32]) -> Result<[u8; 32]> {
    let sk = <Kem as KemTrait>::PrivateKey::from_bytes(enc_sk)
        .map_err(|_| Error::Malformed("kem: invalid private key"))?;
    let pk = <Kem as KemTrait>::sk_to_pk(&sk);
    let bytes = pk.to_bytes();
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| Error::Malformed("kem: unexpected public key length"))
}

/// Leitet den öffentlichen X-Wing-Schlüssel aus dem Post-Quantum-Seed ab.
///
/// Auch hier wird nichts gespeichert: Das Keyfile führt 32 Bytes Seed, der
/// 1216-Byte-Schlüssel entsteht bei Bedarf (`spec/keyfile-v2.md` §3.2).
#[must_use]
pub fn pq_public_key(pq_seed: &[u8; 32]) -> [u8; xwing::PK_LEN] {
    let sk = xwing::PrivateKey::from_seed(*pq_seed);
    *<XWing as KemTrait>::sk_to_pk(&sk).as_bytes()
}

/// Der zum Öffnen einer Kapsel nötige private Schlüsselsatz.
///
/// Welcher Teil gebraucht wird, entscheidet die Suite des Envelopes.
#[derive(Debug, Clone, Copy)]
pub struct RecipientKeys<'a> {
    /// X25519-Privatschlüssel für Suite `0x0001`.
    pub enc_sk: &'a [u8; 32],
    /// X-Wing-Seed für Suite `0x0002`.
    pub pq_seed: &'a [u8; 32],
}

/// Verpackt einen CEK für einen Empfänger.
///
/// Ergebnis ist der `body` einer HPKE-Kapsel: `enc ‖ wrapped_cek`.
///
/// Reihenfolge des Zufallsverbrauchs: [`Suite::kem_randomness_len`] Bytes.
///
/// # Fehler
///
/// - Fehler der Zufallsquelle
/// - [`Error::Malformed`], wenn `recipient_pk` ungültig ist
/// - [`Error::AuthFailed`], wenn die Verpackung fehlschlägt
pub fn wrap_cek<R: Randomness>(
    suite: Suite,
    recipient_pk: &[u8],
    cek: &Cek,
    rng: &mut R,
) -> Result<Vec<u8>> {
    if recipient_pk.len() != suite.pk_len() {
        return Err(Error::Malformed(
            "kem: recipient key length does not match suite",
        ));
    }

    // Genau so viele Bytes, wie `spec/envelope-v2.md` §11 für die Suite
    // festlegt — 32 für DHKEM(X25519), 64 für X-Wing.
    let mut ikm = vec![0u8; suite.kem_randomness_len()];
    rng.fill(&mut ikm)?;

    // Das Ergebnis wird erst nach dem Zeroisieren zurückgegeben, damit das
    // Eingangsmaterial auch im Fehlerfall nicht liegen bleibt.
    let sealed = match suite {
        Suite::Classical => seal_once::<Kem>(&suite.hpke_info(), recipient_pk, &ikm, &cek.0, b""),
        Suite::Hybrid => seal_once::<XWing>(&suite.hpke_info(), recipient_pk, &ikm, &cek.0, b""),
    };
    ikm.zeroize();
    let (enc, wrapped) = sealed?;

    let mut body = Vec::with_capacity(suite.stanza_len());
    body.extend_from_slice(&enc);
    body.extend_from_slice(&wrapped);

    if body.len() != suite.stanza_len() {
        return Err(Error::Malformed("kem: unexpected stanza length"));
    }
    Ok(body)
}

/// Führt HPKE `SetupBaseS` mit festgelegtem `ikm_e` aus und versiegelt
/// **einmal** (Sequenznummer 0).
///
/// Gibt `(enc, ciphertext)` zurück.
///
/// `info`, `pt` und `aad` sind Parameter, damit die offiziellen
/// RFC-9180-Vektoren genau diesen Codepfad prüfen können und nicht eine
/// Nachbildung davon.
fn seal_once<K: KemTrait>(
    info: &[u8],
    recipient_pk: &[u8],
    ikm_e: &[u8],
    pt: &[u8],
    aad: &[u8],
) -> Result<(Vec<u8>, Vec<u8>)> {
    let pk = <K as KemTrait>::PublicKey::from_bytes(recipient_pk)
        .map_err(|_| Error::Malformed("kem: invalid recipient public key"))?;

    let mut source = FixedBytes::new(ikm_e);

    // Scheitert hier vor allem an einem unbrauchbaren Empfängerschlüssel —
    // Punkt niedriger Ordnung, lauter Nullen. Das ist ein Fehler des
    // *Verschlüsselns* und darf nicht als Entschlüsselungsfehler erscheinen.
    let (encapped, mut ctx) =
        hpke::setup_sender_with_rng::<Aead, Kdf, K>(&OpModeS::Base, &pk, info, &mut source)
            .map_err(|_| Error::InvalidRecipientKey)?;

    // Die Spezifikation legt den Verbrauch auf genau IKM_E_LEN Bytes fest.
    // Hätte die Bibliothek mehr angefordert, wären die zusätzlichen Bytes
    // Nullen gewesen — das Ergebnis wäre still schwach statt fehlerhaft.
    if source.exhausted() {
        return Err(Error::Malformed(
            "kem: hpke requested more randomness than the spec allows",
        ));
    }

    let ct = ctx.seal(pt, aad).map_err(|_| Error::AuthFailed)?;
    Ok((encapped.to_bytes().as_slice().to_vec(), ct))
}

/// Öffnet eine HPKE-Kapsel.
///
/// # Fehler
///
/// - [`Error::Malformed`] bei falscher Kapsellänge
/// - [`Error::NoMatchingRecipient`], wenn die Kapsel nicht zu diesem
///   Schlüssel gehört
pub fn unwrap_cek(suite: Suite, keys: RecipientKeys<'_>, body: &[u8]) -> Result<Cek> {
    if body.len() != suite.stanza_len() {
        return Err(Error::Malformed("kem: wrong stanza length"));
    }
    let (enc_bytes, wrapped) = body.split_at(suite.enc_len());

    let plain = match suite {
        Suite::Classical => open_once::<Kem>(&suite.hpke_info(), keys.enc_sk, enc_bytes, wrapped),
        Suite::Hybrid => open_once::<XWing>(&suite.hpke_info(), keys.pq_seed, enc_bytes, wrapped),
    }?;

    let cek: [u8; CEK_LEN] = plain
        .as_slice()
        .try_into()
        .map_err(|_| Error::Malformed("kem: wrong CEK length"))?;
    Ok(Cek(cek))
}

fn open_once<K: KemTrait>(
    info: &[u8],
    recipient_sk: &[u8; 32],
    enc_bytes: &[u8],
    wrapped: &[u8],
) -> Result<Vec<u8>> {
    let sk = <K as KemTrait>::PrivateKey::from_bytes(recipient_sk)
        .map_err(|_| Error::Malformed("kem: invalid private key"))?;
    let encapped = <K as KemTrait>::EncappedKey::from_bytes(enc_bytes)
        .map_err(|_| Error::Malformed("kem: invalid encapsulated key"))?;

    let mut ctx = hpke::setup_receiver::<Aead, Kdf, K>(&OpModeR::Base, &sk, &encapped, info)
        .map_err(|_| Error::NoMatchingRecipient)?;

    ctx.open(wrapped, b"")
        .map_err(|_| Error::NoMatchingRecipient)
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "Fehlschlag soll den Test abbrechen"
)]
mod tests {
    use super::*;
    use crate::rng::OsRandom;

    /// Zufallsquelle mit fest vorgegebenen Bytes.
    struct Fixed(Vec<u8>, usize);
    impl Randomness for Fixed {
        fn fill(&mut self, dest: &mut [u8]) -> Result<()> {
            let end = self.1 + dest.len();
            assert!(end <= self.0.len(), "Fixed erschoepft");
            dest.copy_from_slice(&self.0[self.1..end]);
            self.1 = end;
            Ok(())
        }
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    /// Beide Schluesselarten aus demselben Bytesatz -- fuer Tests der
    /// klassischen Suite genuegt der X25519-Teil.
    fn keys(sk: &[u8; 32]) -> RecipientKeys<'_> {
        RecipientKeys {
            enc_sk: sk,
            pq_seed: sk,
        }
    }

    fn schluesselpaar() -> ([u8; 32], [u8; 32]) {
        let mut sk = [0u8; 32];
        OsRandom.fill(&mut sk).unwrap();
        let pk = public_key(&sk).unwrap();
        (sk, pk)
    }

    #[test]
    fn round_trip() {
        let (sk, pk) = schluesselpaar();
        let cek = Cek::generate(&mut OsRandom).unwrap();

        let body = wrap_cek(Suite::Classical, &pk, &cek, &mut OsRandom).unwrap();
        assert_eq!(body.len(), Suite::Classical.stanza_len());

        let zurueck = unwrap_cek(Suite::Classical, keys(&sk), &body).unwrap();
        assert_eq!(zurueck.0, cek.0);
    }

    #[test]
    fn falscher_empfaenger_scheitert() {
        let (_, pk) = schluesselpaar();
        let (fremder_sk, _) = schluesselpaar();
        let cek = Cek::generate(&mut OsRandom).unwrap();

        let body = wrap_cek(Suite::Classical, &pk, &cek, &mut OsRandom).unwrap();
        let e = unwrap_cek(Suite::Classical, keys(&fremder_sk), &body).unwrap_err();
        assert_eq!(e.code(), "NO_MATCHING_RECIPIENT");
    }

    #[test]
    fn jede_einzelbyte_aenderung_wird_erkannt() {
        let (sk, pk) = schluesselpaar();
        let cek = Cek::generate(&mut OsRandom).unwrap();
        let body = wrap_cek(Suite::Classical, &pk, &cek, &mut OsRandom).unwrap();

        for i in 0..body.len() {
            let mut kaputt = body.clone();
            kaputt[i] ^= 0x01;
            assert!(
                unwrap_cek(Suite::Classical, keys(&sk), &kaputt).is_err(),
                "Aenderung an Byte {i} blieb unbemerkt"
            );
        }
    }

    #[test]
    fn falsche_kapsellaenge_wird_abgelehnt() {
        let (sk, _) = schluesselpaar();
        for len in [0, 1, 79, 81, 200] {
            let body = vec![0u8; len];
            assert_eq!(
                unwrap_cek(Suite::Classical, keys(&sk), &body)
                    .unwrap_err()
                    .code(),
                "MALFORMED",
                "Laenge {len} haette abgelehnt werden muessen"
            );
        }
    }

    #[test]
    fn zwei_kapseln_unterscheiden_sich() {
        // Der ephemere Schluessel ist pro Kapsel neu -- sonst waeren
        // Nachrichten an denselben Empfaenger verknuepfbar.
        let (_, pk) = schluesselpaar();
        let cek = Cek::generate(&mut OsRandom).unwrap();
        let a = wrap_cek(Suite::Classical, &pk, &cek, &mut OsRandom).unwrap();
        let b = wrap_cek(Suite::Classical, &pk, &cek, &mut OsRandom).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn festgelegter_zufall_ergibt_festgelegte_kapsel() {
        // Voraussetzung fuer bit-genaue Vektoren (test-vectors.md §3).
        let (_, pk) = schluesselpaar();
        let cek = Cek(*b"01234567890123456789012345678901");

        let a = wrap_cek(Suite::Classical, &pk, &cek, &mut Fixed(vec![7u8; 32], 0)).unwrap();
        let b = wrap_cek(Suite::Classical, &pk, &cek, &mut Fixed(vec![7u8; 32], 0)).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn verbraucht_genau_32_bytes_zufall() {
        // spec/envelope-v2.md §11. Mehr Vorrat als noetig darf nicht
        // aufgebraucht werden.
        let (_, pk) = schluesselpaar();
        let cek = Cek([0u8; 32]);
        let mut quelle = Fixed(vec![3u8; 64], 0);
        wrap_cek(Suite::Classical, &pk, &cek, &mut quelle).unwrap();
        assert_eq!(
            quelle.1,
            Suite::Classical.kem_randomness_len(),
            "Verbrauch weicht von der Spec ab"
        );
    }

    /// Die offiziellen RFC-9180-Vektoren.
    ///
    /// Das ist die einzige Prüfung, die nicht gegen eigenen Code läuft. Ohne
    /// sie testet die Implementierung nur ihre eigene Auffassung von HPKE.
    #[test]
    fn rfc9180_vektoren() {
        let pfad = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testvectors/hpke/rfc9180-x25519-chacha.json");
        let raw = std::fs::read_to_string(&pfad).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&raw).unwrap();

        let vectors = doc["vectors"].as_array().unwrap();
        assert!(!vectors.is_empty(), "keine RFC-Vektoren vorhanden");

        for v in vectors {
            let id = v["id"].as_str().unwrap();
            assert_eq!(v["mode"], 0, "{id}: nur Base-Modus wird unterstuetzt");
            assert_eq!(v["kem_id"], 0x0020);
            assert_eq!(v["kdf_id"], 0x0001);
            assert_eq!(v["aead_id"], 0x0003);

            let info = hex(v["info"].as_str().unwrap());
            let ikm_e = hex(v["ikmE"].as_str().unwrap());
            let pk_rm: [u8; 32] = hex(v["pkRm"].as_str().unwrap()).try_into().unwrap();
            let erwartet_enc = hex(v["enc"].as_str().unwrap());

            let erste = &v["encryptions"][0];
            let pt = hex(erste["pt"].as_str().unwrap());
            let aad = hex(erste["aad"].as_str().unwrap());
            let erwartet_ct = hex(erste["ct"].as_str().unwrap());

            let (enc, ct) = seal_once::<Kem>(&info, &pk_rm, &ikm_e, &pt, &aad).unwrap();

            // `enc` haengt allein am ephemeren Schluessel. Stimmt es, ist
            // belegt, dass genau ikmE verbraucht und daraus das Schluesselpaar
            // nach RFC abgeleitet wurde.
            assert_eq!(
                enc, erwartet_enc,
                "{id}: enc weicht ab -- Zufallsverbrauch stimmt nicht"
            );
            assert_eq!(
                ct, erwartet_ct,
                "{id}: Ciphertext weicht ab -- Key Schedule oder AEAD stimmt nicht"
            );
        }
    }
}
