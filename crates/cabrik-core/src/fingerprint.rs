//! Fingerprints und Safety Numbers nach `spec/trust-store.md` §2 und §3.
//!
//! Ein Fingerprint identifiziert eine Identität. Er ersetzt keine Prüfung —
//! erst der Abgleich über einen **zweiten Kanal** macht aus einem Schlüssel
//! einen bekannten Absender. Siehe `spec/threat-model.md` §8.

use crate::base32;
use hkdf::Hkdf;
use sha2::{Digest, Sha256};

/// Längen der Schlüsselbestandteile in Bytes.
const ENC_PUB_LEN: usize = 32;
const SIG_PUB_LEN: usize = 32;
const PQ_PUB_LEN: usize = 1216;

/// Zeichen, die eine Anzeige mindestens umfassen muss (= 160 Bit).
pub const MIN_DISPLAY_CHARS: usize = 32;

/// Zeichen der vollständigen Anzeige (= 256 Bit).
pub const FULL_DISPLAY_CHARS: usize = 52;

/// 256-Bit-Fingerprint einer Identität.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    /// Berechnet den Fingerprint aus den öffentlichen Schlüsseln.
    ///
    /// Fehlende Bestandteile — `sig_pub` bei Anonymitäts-Identitäten,
    /// `pq_pub` bei aus v1 übernommenen Kontakten — gehen als
    /// Präsenz-Byte `0x00` plus Nullbytes in voller Länge ein.
    ///
    /// Der Post-Quantum-Schlüssel geht **zwingend** ein. Ohne ihn hätten zwei
    /// Identitäten mit gleichen klassischen, aber verschiedenen
    /// Post-Quantum-Schlüsseln denselben Fingerprint — ein Angreifer könnte
    /// einen eigenen Post-Quantum-Schlüssel unterschieben, ohne dass die
    /// Verifikation es bemerkt.
    ///
    /// Das Präsenz-Byte trennt „kein Schlüssel" von „Schlüssel aus lauter
    /// Nullen". Ohne es könnte ein Angreifer eine Identität mit einem
    /// Null-Post-Quantum-Schlüssel anlegen, deren Fingerprint mit dem eines
    /// migrierten Kontakts ohne PQ-Schlüssel übereinstimmt. Siehe
    /// `spec/trust-store.md` §2.1.
    #[must_use]
    pub fn compute(
        enc_pub: &[u8; ENC_PUB_LEN],
        sig_pub: Option<&[u8; SIG_PUB_LEN]>,
        pq_pub: Option<&[u8; PQ_PUB_LEN]>,
    ) -> Self {
        let mut h = Sha256::new();
        h.update(b"cabrik-fp-v2");
        h.update(enc_pub);

        h.update([u8::from(sig_pub.is_some())]);
        h.update(sig_pub.map_or(&[0u8; SIG_PUB_LEN], |k| k));

        h.update([u8::from(pq_pub.is_some())]);
        match pq_pub {
            Some(k) => h.update(k),
            None => h.update([0u8; PQ_PUB_LEN]),
        }

        Self(h.finalize().into())
    }

    /// Die 32 rohen Bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Baut einen Fingerprint aus bereits berechneten Bytes.
    ///
    /// Für gespeicherte Kontakte und Testvektoren. Es findet **keine**
    /// Prüfung statt, ob die Bytes zu einem Schlüsselsatz passen — dafür
    /// gibt es [`Fingerprint::compute`].
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Vollständige Anzeige: 52 Zeichen in Vierergruppen.
    #[must_use]
    pub fn display_full(&self) -> String {
        base32::encode_grouped(&self.0, 4)
    }

    /// Anzeige zur Verifikation: 32 Zeichen (160 Bit) in Vierergruppen.
    ///
    /// Das ist die **Untergrenze** aus `spec/trust-store.md` §2.2. 160 Bit
    /// ergeben 80 Bit Kollisionsschutz — ausreichend gegen einen Angreifer,
    /// der zwei Schlüssel mit gleichem Fingerprint sucht.
    #[must_use]
    pub fn display(&self) -> String {
        let full = base32::encode(&self.0);
        let kurz: String = full.chars().take(MIN_DISPLAY_CHARS).collect();
        gruppiere(&kurz, 4)
    }

    /// Achtstellige Kurzform — **ausschließlich** zur Unterscheidung in Listen.
    ///
    /// Sie umfasst 40 Bit und damit nur 20 Bit Kollisionsschutz. Sie darf
    /// niemals als Grundlage einer Verifikation angeboten werden.
    #[must_use]
    pub fn short(&self) -> String {
        base32::encode(&self.0).chars().take(8).collect()
    }
}

fn gruppiere(s: &str, group: usize) -> String {
    if group == 0 {
        return s.to_owned();
    }
    let chars: Vec<char> = s.chars().collect();
    chars
        .chunks(group)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-")
}

/// Paarweise Safety Number nach `spec/trust-store.md` §3.
///
/// Beide Seiten vergleichen **eine** Zeichenfolge statt zweier Fingerprints.
/// Die Sortierung sorgt dafür, dass beide dasselbe sehen, unabhängig davon,
/// wer fragt.
///
/// Ausgabe: 60 Dezimalziffern als 12 Gruppen zu 5, vorlesbar und
/// sprachunabhängig.
///
/// # Panics
///
/// Nie. Die HKDF-Ausgabelänge ist konstant und liegt weit unter dem für
/// SHA-256 zulässigen Höchstwert.
#[must_use]
pub fn safety_number(a: &Fingerprint, b: &Fingerprint) -> String {
    // Lexikografisch sortieren, damit die Reihenfolge der Gesprächspartner
    // keine Rolle spielt.
    let (first, second) = if a.0 <= b.0 {
        (&a.0, &b.0)
    } else {
        (&b.0, &a.0)
    };

    let mut h = Sha256::new();
    h.update(b"cabrik-sn-v2");
    h.update(first);
    h.update(second);
    let base = h.finalize();

    // 8 Bytes je Gruppe statt der bei Signal üblichen 5. Der Modulo-Bias
    // sinkt damit von rund 2,5e-8 auf 2,8e-15. Rejection Sampling wäre exakt,
    // würde die Ableitung aber datenabhängig und damit nicht mehr
    // bit-reproduzierbar machen — siehe spec/test-vectors.md §3.
    let mut material = [0u8; 96];
    let hk = Hkdf::<Sha256>::new(None, &base);
    // expand schlägt nur fehl, wenn die Länge > 255 * 32 ist; 96 ist konstant.
    if hk.expand(b"cabrik-sn-digits", &mut material).is_err() {
        // Unerreichbar; ein leerer String wäre hier immer noch besser als ein Absturz.
        return String::new();
    }

    let mut gruppen = Vec::with_capacity(12);
    for chunk in material.chunks_exact(8) {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(chunk);
        let value = u64::from_be_bytes(buf) % 100_000;
        gruppen.push(format!("{value:05}"));
    }
    gruppen.join(" ")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn fp(seed: u8) -> Fingerprint {
        Fingerprint::compute(&[seed; 32], Some(&[seed ^ 0xFF; 32]), None)
    }

    #[test]
    fn anzeigelaengen_entsprechen_der_spezifikation() {
        let f = fp(1);
        assert_eq!(f.display_full().replace('-', "").len(), FULL_DISPLAY_CHARS);
        assert_eq!(f.display().replace('-', "").len(), MIN_DISPLAY_CHARS);
        assert_eq!(f.short().len(), 8);
    }

    #[test]
    fn pq_schluessel_veraendert_den_fingerprint() {
        // Der Kern der Entscheidung aus spec/trust-store.md §2: ohne diesen
        // Anteil koennte ein Angreifer den PQ-Schluessel unterschieben.
        let enc = [7u8; 32];
        let sig = [9u8; 32];
        let ohne = Fingerprint::compute(&enc, Some(&sig), None);
        let mit = Fingerprint::compute(&enc, Some(&sig), Some(&[3u8; 1216]));
        assert_ne!(ohne, mit);
    }

    #[test]
    fn fehlender_schluessel_kollidiert_nicht_mit_null_schluessel() {
        // spec/trust-store.md §2.1 — der Grund fuer die Praesenz-Bytes.
        let enc = [4u8; 32];

        assert_ne!(
            Fingerprint::compute(&enc, None, None),
            Fingerprint::compute(&enc, Some(&[0u8; 32]), None),
            "fehlender Signierschluessel darf nicht wie ein Null-Schluessel wirken"
        );

        // Der sicherheitsrelevante Fall: ein Null-PQ-Schluessel ist
        // syntaktisch gueltig und waere sonst nicht von "kein PQ-Schluessel"
        // zu unterscheiden.
        assert_ne!(
            Fingerprint::compute(&enc, None, None),
            Fingerprint::compute(&enc, None, Some(&[0u8; 1216])),
            "fehlender PQ-Schluessel darf nicht wie ein Null-Schluessel wirken"
        );
    }

    #[test]
    fn ist_deterministisch() {
        assert_eq!(fp(42), fp(42));
        assert_ne!(fp(42), fp(43));
    }

    #[test]
    fn safety_number_ist_reihenfolgeunabhaengig() {
        let (a, b) = (fp(1), fp(2));
        assert_eq!(safety_number(&a, &b), safety_number(&b, &a));
    }

    #[test]
    fn safety_number_hat_60_ziffern_in_12_gruppen() {
        let sn = safety_number(&fp(1), &fp(2));
        let gruppen: Vec<&str> = sn.split(' ').collect();
        assert_eq!(gruppen.len(), 12);
        for g in &gruppen {
            assert_eq!(g.len(), 5, "Gruppe {g} ist nicht fuenfstellig");
            assert!(g.chars().all(|c| c.is_ascii_digit()));
        }
        assert_eq!(sn.replace(' ', "").len(), 60);
    }

    #[test]
    fn safety_number_unterscheidet_verschiedene_paare() {
        assert_ne!(safety_number(&fp(1), &fp(2)), safety_number(&fp(1), &fp(3)));
    }

    #[test]
    fn fingerprint_ist_ueber_base32_wiederherstellbar() {
        let f = fp(77);
        let decoded = crate::base32::decode(&f.display_full()).unwrap();
        assert_eq!(decoded.as_slice(), f.as_bytes().as_slice());
    }
}
