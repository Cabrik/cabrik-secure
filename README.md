# Cabrik Secure

Offline-Werkzeug für asymmetrisch verschlüsselte Nachrichten und Dateien.
Kein Server, keine Accounts, keine Telemetrie.

**Status:** v2.0 in Planung. Die ausgelieferte v1.0 (Python/Tkinter) liegt
eingefroren unter [`legacy/python-v1/`](legacy/python-v1/) und dient als
Referenzimplementierung.

Der Weg zu v2.0 ist in [`docs/ROADMAP.md`](docs/ROADMAP.md) beschrieben.

## Struktur

| Verzeichnis | Inhalt |
|---|---|
| `spec/` | Formatspezifikation und Threat Model — entsteht in Phase 1 |
| `crates/` | Rust: `cabrik-core`, `cabrik-cli`, `cabrik-app` — Phase 2 und 4 |
| `app/` | Frontend (Svelte + TypeScript) — Phase 3 |
| `testvectors/` | Sprachunabhängige Testvektoren für alle Implementierungen |
| `legacy/python-v1/` | Eingefrorene v1.0, Referenz für Differenztests |
| `docs/` | Roadmap und Projektdokumentation |
| `assets/` | Icons und Markenmaterial |

## Leitprinzipien

1. **Die Spezifikation ist das Produkt.** Das Envelope-Format muss über
   Desktop, iOS und Android identisch und jahrelang stabil sein — deshalb
   wird es vor der Implementierung geschrieben.
2. **Keine eigene Krypto-Konstruktion.** Wo ein Standard existiert
   (HPKE, Argon2id, STREAM), wird der Standard verwendet.
3. **Geheimnisse verlassen Rust nicht.** Schlüsselmaterial wird niemals an
   das Frontend gereicht.
4. **Keine Telemetrie.** Kein Analytics, kein Crash-Reporting mit Inhalten,
   keine Netzwerkverbindung außer für signierte Updates.

## Sicherheitshinweis

Cabrik Secure v1.0 ist **nicht auditiert**. Bis eine unabhängige Prüfung
vorliegt, sollte die Software nicht für Daten eingesetzt werden, deren
Offenlegung ernsthaften Schaden verursachen würde.

Bekannte konzeptionelle Schwäche in v1: der Signaturprüfschlüssel stammt aus
dem Header derselben Nachricht. `signature_valid: true` belegt daher nur, dass
konsistent signiert wurde — nicht, *wer* signiert hat. Ein Vertrauensmodell mit
verifizierten Kontakten kommt in v2 (siehe Roadmap, Phase 1).
