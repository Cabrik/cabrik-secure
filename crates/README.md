# Rust-Crates

Entstehen in **Phase 2** (Core und CLI) sowie **Phase 4** (Tauri-Backend).

| Crate | Rolle | Lizenz (geplant) |
|---|---|---|
| `cabrik-core` | Krypto-Kern: HPKE, Streaming, Keyfiles, Trust Store, Metadaten | quelloffen |
| `cabrik-cli` | Referenz-CLI, deckt den Core vollständig ab | quelloffen |
| `cabrik-app` | Tauri-Backend, dünne Brücke zum Core | ggf. proprietär |
| `cabrik-v1` | Lesezugriff auf das alte Format v1 (Migration) | quelloffen |

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
