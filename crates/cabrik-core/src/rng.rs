//! Zufallsquelle nach `spec/test-vectors.md` §3.
//!
//! Jede Operation, die Zufall verbraucht, bezieht ihn über [`Randomness`] —
//! **nie** über einen direkten Aufruf des Betriebssystems. Das ist keine
//! Stilfrage, sondern Voraussetzung für bit-genaue Verschlüsselungsvektoren:
//! Nur mit einer austauschbaren Quelle lässt sich prüfen, dass Desktop, iOS
//! und Android aus derselben Eingabe denselben Envelope erzeugen.
//!
//! Vorbild ist RFC 9180 selbst — die offiziellen HPKE-Testvektoren fixieren
//! `ikmE`, das Eingangsmaterial des ephemeren Schlüssels.

/// Quelle für Zufallsbytes.
///
/// Im Produktivbetrieb ist das ausschließlich [`OsRandom`].
pub trait Randomness {
    /// Füllt `dest` vollständig mit Zufallsbytes.
    ///
    /// # Fehler
    ///
    /// [`crate::Error::Malformed`], wenn die Quelle versagt. Bei
    /// [`OsRandom`] bedeutet das, dass das Betriebssystem keinen Zufall
    /// liefern kann — ein Zustand, in dem nichts Sinnvolles mehr getan
    /// werden kann.
    fn fill(&mut self, dest: &mut [u8]) -> crate::Result<()>;
}

/// Zufall des Betriebssystems. Die einzige im Produktivbetrieb zulässige Quelle.
#[derive(Debug, Clone, Copy, Default)]
pub struct OsRandom;

impl Randomness for OsRandom {
    fn fill(&mut self, dest: &mut [u8]) -> crate::Result<()> {
        getrandom::fill(dest).map_err(|_| crate::Error::Malformed("rng: os entropy unavailable"))
    }
}

/// Deterministische Zufallsquelle — **ausschließlich für Testvektoren**.
///
/// Steht nur unter dem Cargo-Feature `testing` zur Verfügung. Ohne das
/// Feature existiert dieses Modul nicht; das ist vom Übersetzer garantiert
/// und nicht bloß eine Laufzeitprüfung.
#[cfg(feature = "testing")]
pub mod testing {
    use super::Randomness;
    use chacha20::ChaCha20;
    use chacha20::cipher::{KeyIvInit, StreamCipher};

    /// Erzeugt reproduzierbare Bytes aus einem 32-Byte-Seed.
    ///
    /// Konstruktion: ChaCha20-Schlüsselstrom mit dem Seed als Schlüssel und
    /// einer Nonce aus Nullbytes. Der Zähler läuft über die Aufrufe hinweg
    /// weiter, sodass die **Reihenfolge** der Anforderungen das Ergebnis
    /// bestimmt — genau die Eigenschaft, die `spec/envelope-v2.md` §11
    /// normativ festlegt.
    ///
    /// # Sicherheit
    ///
    /// Niemals im Produktivbetrieb verwenden. Wer den Seed kennt, kennt jeden
    /// erzeugten Schlüssel.
    pub struct DeterministicRng {
        cipher: ChaCha20,
    }

    impl DeterministicRng {
        /// Neue Quelle aus einem Seed.
        #[must_use]
        pub fn new(seed: [u8; 32]) -> Self {
            Self {
                cipher: ChaCha20::new(&seed.into(), &[0u8; 12].into()),
            }
        }
    }

    impl Randomness for DeterministicRng {
        fn fill(&mut self, dest: &mut [u8]) -> crate::Result<()> {
            dest.fill(0);
            self.cipher.apply_keystream(dest);
            Ok(())
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Fehlschlag soll den Test abbrechen")]
mod tests {
    use super::*;

    #[test]
    fn os_random_liefert_unterschiedliche_bytes() {
        let mut a = [0u8; 32];
        let mut b = [0u8; 32];
        OsRandom.fill(&mut a).unwrap();
        OsRandom.fill(&mut b).unwrap();
        assert_ne!(a, b, "zwei Aufrufe lieferten identische Bytes");
        assert_ne!(a, [0u8; 32], "Quelle lieferte nur Nullen");
    }

    #[cfg(feature = "testing")]
    #[test]
    fn deterministischer_rng_ist_reproduzierbar() {
        use super::testing::DeterministicRng;

        let mut a = DeterministicRng::new([7u8; 32]);
        let mut b = DeterministicRng::new([7u8; 32]);
        let (mut x, mut y) = ([0u8; 64], [0u8; 64]);
        a.fill(&mut x).unwrap();
        b.fill(&mut y).unwrap();
        assert_eq!(x, y);

        // Anderer Seed, anderes Ergebnis.
        let mut c = DeterministicRng::new([8u8; 32]);
        let mut z = [0u8; 64];
        c.fill(&mut z).unwrap();
        assert_ne!(x, z);
    }

    #[cfg(feature = "testing")]
    #[test]
    fn reihenfolge_der_anforderungen_bestimmt_das_ergebnis() {
        use super::testing::DeterministicRng;

        // Einmal 64 Bytes am Stueck ...
        let mut a = DeterministicRng::new([1u8; 32]);
        let mut ganz = [0u8; 64];
        a.fill(&mut ganz).unwrap();

        // ... muss dasselbe ergeben wie zweimal 32 Bytes nacheinander.
        let mut b = DeterministicRng::new([1u8; 32]);
        let (mut erst, mut dann) = ([0u8; 32], [0u8; 32]);
        b.fill(&mut erst).unwrap();
        b.fill(&mut dann).unwrap();

        assert_eq!(&ganz[..32], &erst[..]);
        assert_eq!(&ganz[32..], &dann[..]);
    }

    // Fuer die Abwesenheit von `testing::DeterministicRng` gibt es bewusst
    // keinen Laufzeittest: Ohne das Feature existiert das Modul nicht, und
    // jeder Code, der es benutzt, laesst sich nicht uebersetzen. Das ist eine
    // staerkere Zusicherung als jede Behauptung zur Laufzeit -- ein Test
    // koennte nur pruefen, was ohnehin schon feststeht.
    //
    // Abgesichert wird es stattdessen in CI: `cargo build` ohne Feature muss
    // erfolgreich sein, `cargo build` mit einem Testprogramm, das den Typ
    // benutzt, muss fehlschlagen.
}
