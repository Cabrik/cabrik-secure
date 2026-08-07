# Rust-Crates

Entstehen in **Phase 2** (Core und CLI) sowie **Phase 4** (Tauri-Backend).

| Crate | Rolle | Lizenz (geplant) |
|---|---|---|
| `cabrik-core` | Krypto-Kern: HPKE, Streaming, Keyfiles, Trust Store | quelloffen |
| `cabrik-cli` | Referenz-CLI (`cabrik`), deckt den Core vollständig ab | quelloffen |
| `cabrik-app` | Tauri-Backend, dünne Brücke zum Core | ggf. proprietär |
| `cabrik-v1` | Lesezugriff auf das alte Format v1 (Migration) | quelloffen |
| `cabrik-metadata` | Erkennen und Entfernen von Metadaten in Nutzdateien | quelloffen |
| `cabrik-shred` | Sicheres Löschen mit ehrlichen Garantien | quelloffen |

`cabrik-metadata` ist aus demselben Grund eigenständig wie `cabrik-v1`, nur
mit noch mehr Gewicht: Metadaten-Bereinigung heißt Parser für viele
Dateiformate, und Parser sind Angriffsfläche.

`cabrik-shred` ist eigenständig, weil es als einziger Teil des Systems
**Dateien zerstört**. Diese Trennung hält den Kern frei von einer Fähigkeit,
die dort nichts zu suchen hat, und macht die Prüfung des heiklen Codes
überschaubar. Es hat keine Abhängigkeit auf Krypto — nur auf `getrandom`.

`cabrik-cli` ist mehr als ein Werkzeug: Sie ist der **erste echte Aufrufer**
des Kerns und deckt ihn deshalb vollständig ab. Modultests rufen Funktionen so
auf, wie die Autorin sie gedacht hat; eine Bedienoberfläche muss sie so
aufrufen, wie ein Mensch sie braucht. Jeder schwere Entwurfsfehler dieses
Projekts kam an genau dieser Naht heraus — zuletzt vier Stück auf einmal,
siehe `docs/ROADMAP.md` 2.11. Sie kommt deshalb bewusst **vor** dem Frontend.

Als einzige Kiste kennt sie Dateien und Pfade: Der Kern verarbeitet nur Bytes,
damit er per UniFFI nach Swift und Kotlin gehen kann. Auch die Verschlüsselung
des Kontaktspeichers liegt hier, nicht im Kern.

`cabrik-v1` ist bewusst eigenständig: v1 ist JSON über Base64, und beides
soll nicht in den auditierten Kern, der per UniFFI nach iOS und Android
geht. Der Inhalt ist eingefroren — einmal geschrieben, gegen die
Referenzimplementierung geprüft, danach unverändert.

## Verbindliche Standards

- `#![forbid(unsafe_code)]` im Core
- `zeroize` für sämtliches Schlüsselmaterial
- `cargo fuzz` auf dem Envelope-Parser
- `cargo deny` für Lizenz- und CVE-Prüfung
- Differenztests gegen `legacy/python-v1` in CI

## Reihenfolge der Implementierung

Folgt bewusst der Rust-Lernkurve, nicht der fachlichen Logik:
Helfer → Keyfile → HPKE Single-Shot → Streaming → Mehrfachempfänger.
Streaming kommt spät, weil dort Lifetimes und Iteratoren zusammenkommen.
