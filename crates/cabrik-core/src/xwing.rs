//! X-Wing als HPKE-KEM (`spec/envelope-v2.md` §4.1).
//!
//! X-Wing kombiniert X25519 und ML-KEM-768 zu einem einzigen KEM. Die
//! Konstruktion ist mindestens so sicher wie ihr stärkerer Bestandteil: Sie
//! bricht erst, wenn **beide** Verfahren brechen.
//!
//! Quelle: `draft-connolly-cfrg-xwing-kem`, HPKE-Kennung `0x647a`.
//!
//! # Warum als HPKE-KEM und nicht direkt
//!
//! Der bequeme Weg wäre, das gemeinsame Geheimnis von X-Wing zu nehmen und
//! selbst einen Schlüssel daraus abzuleiten. Das wäre eine **eigene
//! Krypto-Konstruktion** — und genau die verbieten die Leitprinzipien des
//! Projekts. Stattdessen wird X-Wing in den `Kem`-Trait der HPKE-Bibliothek
//! eingehängt; der genormte Schlüsselplan aus RFC 9180 bleibt unangetastet
//! und die Kennung `0x647a` geht korrekt in seine Labels ein.
//!
//! # Entwurfsstand
//!
//! Die zugrunde liegende Crate setzt Draft 06 um, der Entwurf steht bei
//! Revision 10. Die Unterschiede sind redaktionell; belegt wird das durch
//! die Vektoren aus Anhang C des Entwurfs, die dieses Modul prüft.

use hpke::kem::SharedSecret;
use hpke::{Deserializable, HpkeError, Kem as KemTrait, Serializable};
use hybrid_array::Array;
use hybrid_array::sizes::{U32, U64, U1120, U1216};
use rand_core::CryptoRng;
use shake::{ExtendableOutput, Shake256, Update as _, XofReader};
use subtle::{Choice, ConstantTimeEq};
use x_wing::kem::{Decapsulate as _, Decapsulator as _};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Länge des privaten Schlüssels: der Seed selbst.
pub const SK_LEN: usize = 32;
/// Länge des öffentlichen Schlüssels.
pub const PK_LEN: usize = 1216;
/// Länge einer Kapsel.
pub const ENC_LEN: usize = 1120;
/// Zufallsbedarf einer Kapselung.
pub const ESEED_LEN: usize = 64;

/// X-Wing als HPKE-KEM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XWing;

// ---------------------------------------------------------------------------
// Schlüsseltypen
// ---------------------------------------------------------------------------

/// Öffentlicher X-Wing-Schlüssel (1216 Bytes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicKey([u8; PK_LEN]);

/// Privater X-Wing-Schlüssel — der 32-Byte-Seed.
///
/// Der Entwurf legt fest, dass der private Schlüssel **der Seed ist**
/// (`Nsk = 32`). Das eigentliche Schlüsselpaar wird bei Bedarf daraus
/// abgeleitet; gespeichert werden 32 statt 2432 Bytes
/// (`spec/keyfile-v2.md` §3.2).
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct PrivateKey([u8; SK_LEN]);

/// Kapsel (1120 Bytes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncappedKey([u8; ENC_LEN]);

impl core::fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("PrivateKey(<redacted>)")
    }
}

impl ConstantTimeEq for PrivateKey {
    fn ct_eq(&self, other: &Self) -> Choice {
        self.0.ct_eq(&other.0)
    }
}

impl PrivateKey {
    /// Baut den Schlüssel aus dem Seed.
    #[must_use]
    pub const fn from_seed(seed: [u8; SK_LEN]) -> Self {
        Self(seed)
    }

    fn decapsulation_key(&self) -> x_wing::DecapsulationKey {
        x_wing::DecapsulationKey::from(self.0)
    }
}

impl PublicKey {
    /// Die rohen 1216 Bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; PK_LEN] {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Serialisierung
// ---------------------------------------------------------------------------

macro_rules! serde_impl {
    ($typ:ty, $size:ty, $len:expr) => {
        impl Serializable for $typ {
            type OutputSize = $size;

            fn write_exact(&self, buf: &mut [u8]) {
                // Der Trait sichert zu, dass buf genau size() lang ist.
                if buf.len() == $len {
                    buf.copy_from_slice(&self.0);
                }
            }
        }

        impl Deserializable for $typ {
            fn from_bytes(encoded: &[u8]) -> Result<Self, HpkeError> {
                let arr: [u8; $len] = encoded
                    .try_into()
                    .map_err(|_| HpkeError::IncorrectInputLength($len, encoded.len()))?;
                Ok(Self(arr))
            }
        }
    };
}

serde_impl!(PublicKey, U1216, PK_LEN);
serde_impl!(PrivateKey, U32, SK_LEN);
serde_impl!(EncappedKey, U1120, ENC_LEN);

// ---------------------------------------------------------------------------
// KEM
// ---------------------------------------------------------------------------

impl KemTrait for XWing {
    type PublicKey = PublicKey;
    type PrivateKey = PrivateKey;
    type EncappedKey = EncappedKey;
    type NSecret = U32;

    /// Kennung aus `draft-connolly-cfrg-xwing-kem` §7: 25519 + 203.
    const KEM_ID: u16 = 0x647a;

    fn sk_to_pk(sk: &Self::PrivateKey) -> Self::PublicKey {
        let dk = sk.decapsulation_key();
        let mut out = [0u8; PK_LEN];
        out.copy_from_slice(&x_wing::kem::KeyExport::to_bytes(dk.encapsulation_key()));
        PublicKey(out)
    }

    /// Leitet ein Schlüsselpaar aus beliebig langem Eingangsmaterial ab.
    ///
    /// Der Entwurf schreibt `sk = SHAKE256(ikm, 32*8)` vor — die Angabe in
    /// Bit ist reine Schreibweise nach FIPS 202, das Ergebnis sind 32 Bytes.
    fn derive_keypair(ikm: &[u8]) -> (Self::PrivateKey, Self::PublicKey) {
        let mut hasher = Shake256::default();
        hasher.update(ikm);
        let mut seed = [0u8; SK_LEN];
        hasher.finalize_xof().read(&mut seed);

        let sk = PrivateKey(seed);
        seed.zeroize();
        let pk = Self::sk_to_pk(&sk);
        (sk, pk)
    }

    fn encap_with_rng(
        pk_recip: &Self::PublicKey,
        sender_id_keypair: Option<(&Self::PrivateKey, &Self::PublicKey)>,
        csprng: &mut impl CryptoRng,
    ) -> Result<(SharedSecret<Self>, Self::EncappedKey), HpkeError> {
        // X-Wing kennt keinen Auth-Modus. Das Format nutzt ihn auch nicht —
        // die Absenderauthentifizierung läuft über eine Ed25519-Signatur im
        // verschlüsselten Trailer (`spec/envelope-v2.md` §2).
        if sender_id_keypair.is_some() {
            return Err(HpkeError::EncapError);
        }

        let ek = x_wing::EncapsulationKey::try_from(&pk_recip.0[..])
            .map_err(|_| HpkeError::ValidationError)?;

        // Genau ESEED_LEN Bytes, wie `spec/envelope-v2.md` §11 festlegt.
        // Die Aufteilung nimmt X-Wing selbst vor: die vorderen 32 Bytes
        // speisen ML-KEM, die hinteren den ephemeren X25519-Schlüssel.
        let mut eseed = Array::<u8, U64>::default();
        csprng.fill_bytes(&mut eseed);

        let (ct, ss) = ek.encapsulate_deterministic(&eseed);
        eseed.zeroize();

        let mut enc = [0u8; ENC_LEN];
        enc.copy_from_slice(&ct);
        Ok((SharedSecret(ss), EncappedKey(enc)))
    }

    fn decap(
        sk_recip: &Self::PrivateKey,
        pk_sender_id: Option<&Self::PublicKey>,
        encapped_key: &Self::EncappedKey,
    ) -> Result<SharedSecret<Self>, HpkeError> {
        if pk_sender_id.is_some() {
            return Err(HpkeError::DecapError);
        }
        let ct = x_wing::Ciphertext::try_from(&encapped_key.0[..])
            .map_err(|_| HpkeError::ValidationError)?;
        let ss = sk_recip.decapsulation_key().decapsulate(&ct);
        Ok(SharedSecret(ss))
    }
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

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    fn vektoren() -> serde_json::Value {
        let pfad = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testvectors/xwing/draft10.json");
        serde_json::from_str(&std::fs::read_to_string(pfad).unwrap()).unwrap()
    }

    /// Die Vektoren aus Anhang C des Entwurfs — Revision 10, während die
    /// Crate Draft 06 umsetzt. Stimmen sie, ist der Abstand nachweislich
    /// redaktionell.
    #[test]
    fn draft10_vektoren() {
        let doc = vektoren();
        let vs = doc["vectors"].as_array().unwrap();
        assert!(!vs.is_empty());

        for v in vs {
            let id = v["id"].as_str().unwrap();

            let seed: [u8; 32] = hex(v["seed"].as_str().unwrap()).try_into().unwrap();
            let sk = PrivateKey::from_seed(seed);

            // sk IST der Seed (Nsk = 32).
            assert_eq!(
                sk.0.to_vec(),
                hex(v["sk"].as_str().unwrap()),
                "{id}: sk weicht ab"
            );

            // Schluesselerzeugung.
            let pk = XWing::sk_to_pk(&sk);
            assert_eq!(
                pk.0.to_vec(),
                hex(v["pk"].as_str().unwrap()),
                "{id}: oeffentlicher Schluessel weicht ab"
            );

            // Kapselung mit festgelegtem eseed.
            let eseed = hex(v["eseed"].as_str().unwrap());
            let mut arr = Array::<u8, U64>::default();
            arr.copy_from_slice(&eseed);
            let ek = x_wing::EncapsulationKey::try_from(&pk.0[..]).unwrap();
            let (ct, ss) = ek.encapsulate_deterministic(&arr);

            assert_eq!(
                ct.to_vec(),
                hex(v["ct"].as_str().unwrap()),
                "{id}: Kapsel weicht ab"
            );
            assert_eq!(
                ss.to_vec(),
                hex(v["ss"].as_str().unwrap()),
                "{id}: gemeinsames Geheimnis weicht ab"
            );

            // Entkapselung ergibt dasselbe Geheimnis.
            let enc = EncappedKey(ct.as_slice().try_into().unwrap());
            let zurueck = XWing::decap(&sk, None, &enc).unwrap();
            assert_eq!(
                zurueck.0.to_vec(),
                hex(v["ss"].as_str().unwrap()),
                "{id}: Entkapselung weicht ab"
            );
        }
    }

    #[test]
    fn groessen_entsprechen_dem_entwurf() {
        assert_eq!(SK_LEN, 32);
        assert_eq!(PK_LEN, 1216);
        assert_eq!(ENC_LEN, 1120);
        assert_eq!(ESEED_LEN, 64);
        assert_eq!(XWing::KEM_ID, 0x647a);
        assert_eq!(0x647a, 25519 + 203);
    }

    #[test]
    fn serialisierung_ist_verlustfrei() {
        let sk = PrivateKey::from_seed([0x42; 32]);
        let pk = XWing::sk_to_pk(&sk);

        let pk_bytes = pk.to_bytes();
        assert_eq!(pk_bytes.len(), PK_LEN);
        assert_eq!(PublicKey::from_bytes(&pk_bytes).unwrap(), pk);

        let sk_bytes = sk.to_bytes();
        assert_eq!(sk_bytes.len(), SK_LEN);
        assert!(bool::from(
            PrivateKey::from_bytes(&sk_bytes).unwrap().ct_eq(&sk)
        ));
    }

    #[test]
    fn falsche_laengen_werden_abgelehnt() {
        assert!(PublicKey::from_bytes(&[0u8; 1215]).is_err());
        assert!(PublicKey::from_bytes(&[0u8; 1217]).is_err());
        assert!(EncappedKey::from_bytes(&[0u8; 1119]).is_err());
        assert!(PrivateKey::from_bytes(&[0u8; 31]).is_err());
    }

    #[test]
    fn auth_modus_wird_abgelehnt() {
        // X-Wing kennt keinen Auth-Modus, und das Format nutzt ihn nicht.
        let sk = PrivateKey::from_seed([1; 32]);
        let pk = XWing::sk_to_pk(&sk);
        let enc = EncappedKey([0u8; ENC_LEN]);
        assert!(XWing::decap(&sk, Some(&pk), &enc).is_err());
    }

    #[test]
    fn debug_gibt_keinen_schluessel_preis() {
        let sk = PrivateKey::from_seed([0xAB; 32]);
        let ausgabe = format!("{sk:?}");
        assert!(ausgabe.contains("redacted"));
        assert!(!ausgabe.contains("171"));
    }
}
