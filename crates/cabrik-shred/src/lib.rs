//! Sicheres Löschen mit ehrlichen Garantien (`spec/shredding.md`).
//!
//! # Was v1 tat
//!
//! ```python
//! except Exception:
//!     pass
//! ...
//! try:
//!     os.remove(path)
//! except Exception:
//!     pass
//! ```
//!
//! `secure_delete` verschluckte jeden Fehler. Die Oberfläche meldete
//! anschließend „Gelöscht" — auch dann, wenn kein einziges Byte überschrieben
//! wurde, etwa weil die Datei schreibgeschützt oder von einem anderen Prozess
//! geöffnet war.
//!
//! Das ist die schlechteste denkbare Eigenschaft für ein Sicherheitswerkzeug:
//! Es erzeugt Vertrauen, wo keines gerechtfertigt ist.
//!
//! [`ShredOutcome`] meldet deshalb einzeln, was tatsächlich gelungen ist.
//!
//! # Die eigentliche Lösung liegt woanders
//!
//! Die Frage „wie lösche ich Klartext von der SSD" ist die falsche. Die
//! richtige lautet: **warum liegt dort überhaupt Klartext?**
//!
//! v1 schrieb beim Verschlüsseln mehrerer Anhänge ein **unverschlüsseltes
//! ZIP** nach `%TEMP%`. Sämtliche Anhänge lagen damit im Klartext auf dem
//! Datenträger, bevor überhaupt verschlüsselt wurde. v2 kennt kein solches
//! Zwischenprodukt — mehrere Dateien gehen über den Archiv-Index direkt in
//! den verschlüsselten Strom.
//!
//! Was nie im Klartext geschrieben wurde, muss nicht gelöscht werden. Dieses
//! Modul ist die zweite Verteidigungslinie, nicht die erste.

pub mod capability;
pub mod dir;
pub mod file;

pub use capability::{Assessment, ShredCapability, Warning, assess};
pub use dir::{DirOutcome, Preview, Refusal, preview, shred_dir};
pub use file::{ShredOptions, ShredOutcome, shred_file};

/// Voreingestellte Zahl der Überschreibdurchgänge.
///
/// **Einer genügt** bei jedem Datenträger, der nach 2001 gebaut wurde. Die
/// verbreitete Annahme, 35 Durchgänge (Gutmann) seien nötig, bezieht sich auf
/// MFM- und RLL-Kodierung der frühen 1990er und ist auf heutige Laufwerke
/// nicht übertragbar.
///
/// v1 hatte 3 voreingestellt und suggerierte damit einen Nutzen, den
/// zusätzliche Durchgänge nicht haben.
pub const DEFAULT_PASSES: u8 = 1;

/// Obergrenze der Durchgänge.
pub const MAX_PASSES: u8 = 7;

/// Dateien unterhalb dieser Größe werden vor dem Überschreiben vergrößert.
///
/// Unter NTFS liegen Dateien unter etwa 700 Bytes **resident im
/// MFT-Eintrag** — das Überschreiben der „Datei" erreicht diese Kopie nicht.
/// Die genaue Schwelle hängt von der Eintragsgröße und der Zahl der Attribute
/// ab und ließe sich über `FSCTL_GET_NTFS_VOLUME_DATA` ermitteln.
///
/// Das wird bewusst **nicht** getan: Jede Datei unter 8 KiB pauschal zu
/// vergrößern braucht keine Sonderrechte, funktioniert auf jedem Dateisystem
/// und liegt immer über jeder denkbaren Residenzgrenze. Die einfache Lösung
/// ist hier zugleich die robustere (`spec/shredding.md` §5.1).
pub const GROW_BELOW: u64 = 8 * 1024;
