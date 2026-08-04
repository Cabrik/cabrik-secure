# Cabrik Secure 2.0 — Roadmap

**Stand:** 2026-08-04
**Ziel:** Professionell gebautes, quelloffenes Krypto-Werkzeug. Desktop zuerst, Mobile später.
Kein Live-Termin — Qualität vor Geschwindigkeit. Alles wird so gebaut, dass eine
spätere kommerzielle Verwertung möglich bleibt.

---

## Entscheidungen (getroffen)

| Frage | Entscheidung |
|---|---|
| Architektur | Rust-Core + Tauri (Desktop), später UniFFI → Swift/Kotlin (Mobile) |
| Web-App | Nein. Gehostete Krypto bricht das Vertrauensmodell. Website nur für Doku/Downloads |
| Produktziel | Werkzeug für Profis zuerst; Format so bauen, dass Breite später möglich bleibt |
| Vorgehen | Lernprojekt ohne Termindruck, aber mit Produktionsstandards von Tag 1 |
| Python v1 | Bleibt als Referenzimplementierung erhalten (`legacy/`) |

## Leitprinzipien

1. **Die Spezifikation ist das Produkt.** Der Code wird zweimal neu geschrieben,
   das Envelope-Format muss über Desktop, iOS und Android identisch und
   jahrelang stabil sein. Deshalb: Spec vor Implementierung.
2. **Nie zwei Unbekannte gleichzeitig.** Rust wird ohne UI gelernt, Frontend ohne Rust.
3. **Keine eigene Krypto-Konstruktion.** Wo ein Standard existiert (HPKE, Argon2id,
   STREAM), wird der Standard genommen — nicht selbst gebaut.
4. **Geheimnisse verlassen Rust nicht.** Schlüsselmaterial wird niemals an das
   Frontend gereicht. Das ist der eigentliche Grund für Tauri statt Electron.
5. **Keine Telemetrie. Nie.** Kein Crash-Reporting mit Inhalten, keine Analytics,
   keine Netzwerkverbindung außer für signierte Updates.

---

## Phase 0 — Fundament

*Kein Rust nötig. Aufräumen und Weichen stellen.*

- [ ] **Git-Repository anlegen** — existiert bisher nicht
- [ ] Monorepo-Struktur aufsetzen (siehe unten)
- [ ] `.gitignore` für Rust, Node, Python, PyInstaller
- [ ] Aufräumen:
  - Testschlüssel aus dem Root entfernen (`t.json`, `AonTestKey1.json`,
    `TestKeySignatur1.json`, `TestKeyAnnonym1.json`, `myid.json`)
  - Build-Artefakte entfernen (`cabrik_secure/gui/build/`, `cabrik_secure/gui/dist/`,
    `build/`, `dist/`, `Output/`, `cabrik_secure.egg-info/`)
  - Vier konkurrierende `.spec`-Dateien auf die eine aktive reduzieren
  - `myid.json` enthält versehentlich `pyproject.toml`-Inhalt
- [ ] Python v1 nach `legacy/python-v1/` verschieben, `dependencies` in
      `pyproject.toml` nachtragen, damit sie als Referenz lauffähig bleibt
- [ ] **Lizenzmodell festlegen** (siehe „Offene Entscheidungen")
- [ ] Rust-Toolchain installieren, `rustup`, `cargo`, VS Code + rust-analyzer

**Ergebnis:** Sauberes Repo mit lauffähiger Referenzimplementierung.

### Ziel-Struktur

```
cabrik-secure/
├── spec/                       # Formatspezifikation + Threat Model
│   ├── envelope-v2.md
│   ├── keyfile-v2.md
│   ├── trust-store.md
│   └── threat-model.md
├── crates/
│   ├── cabrik-core/            # Krypto-Kern (quelloffen)
│   ├── cabrik-cli/             # Referenz-CLI (quelloffen)
│   └── cabrik-app/             # Tauri-Backend
├── app/                        # Frontend (Svelte/TS)
├── testvectors/                # Sprachunabhängige Testvektoren (JSON)
├── legacy/python-v1/           # Referenzimplementierung
└── docs/
```

---

## Phase 1 — Spezifikation

*Kein Rust nötig. Kann parallel zum Rust-Lernen laufen.*

Das inhaltliche Herzstück. Jeder Punkt behebt einen konkreten Befund aus v1.

### `spec/envelope-v2.md`

- [ ] **HPKE nach RFC 9180** statt eigenem Key-Agreement
      → Ciphersuite: `DHKEM(X25519, HKDF-SHA256)` + `ChaCha20-Poly1305`
      → behebt das fehlende Transcript-Binding in v1 (`_derive_session_key`)
      → auditierte Implementierungen existieren in Rust *und* Swift/Kotlin
- [ ] **Binärformat mit Chunked Streaming** (STREAM-Konstruktion wie `age`,
      bzw. libsodium `secretstream`)
      → behebt: 78 % Größen-Overhead durch Base64-über-JSON-über-Base64
      → behebt: komplette Datei im RAM, Peak bei ~4–5× Dateigröße
      → Base64-„Armor" bleibt als *optionaler* Modus für Copy-Paste
- [ ] **Mehrere Empfänger** — pro Empfänger gewrappter Content-Key
- [ ] **Passwort-Modus** — symmetrisch, ohne Schlüsselaustausch
- [ ] **Strikte Versions- und Algorithmus-Validierung**, unbekannte Versionen
      werden abgelehnt (v1 liest den Header, prüft ihn aber nie)
- [ ] **Abwärtskompatibilität:** v2 liest v1-Envelopes, schreibt nur v2

### `spec/keyfile-v2.md`

- [ ] Argon2id-Parameter explizit im Keyfile versioniert
- [ ] Migration von v1-Keyfiles

### `spec/trust-store.md` — der wichtigste konzeptionelle Fix

In v1 kommt der Signaturprüfschlüssel aus dem Header derselben Nachricht.
`signature_valid: true` beweist damit nur „konsistent signiert", nicht *wer*.

- [ ] Lokaler, verschlüsselter Kontaktspeicher: Name ↔ `enc_pub` ↔ `sig_pub`
- [ ] Verifikation out-of-band per Fingerprint-Vergleich oder QR-Code
      (Vorbild: Signal Safety Numbers)
- [ ] Fingerprint auf **mindestens 16 Zeichen** (v1: 8 Hex = 32 Bit, kollisionsanfällig)
- [ ] **UI-Regel:** drei klar getrennte Zustände
  - „Signiert von **Alice** ✓" (verifizierter Kontakt)
  - „Signiert von unbekanntem Schlüssel `abcd…`" (⚠ neutral, kein grüner Haken)
  - „Nicht signiert / anonym"

### `spec/threat-model.md`

Wogegen schützt Cabrik Secure — und wogegen ausdrücklich nicht:
kompromittiertes Endgerät, Verkehrsanalyse, Secure Delete auf SSDs
(Wear-Leveling), Metadaten außerhalb der unterstützten Formate.

### `testvectors/`

- [ ] Format definieren: feste Eingaben → feste Envelopes als JSON
- [ ] Diese Vektoren sind später die Prüfung, dass iOS/Android/Desktop
      bitgenau dasselbe tun

**Ergebnis:** Eingefrorene Spec. Ab hier ist die Sprache austauschbar.

---

## Phase 2 — Rust lernen am Core

*Kein UI. Nur Bibliothek und CLI. Der Python-Core ist das Orakel.*

Implementierungsreihenfolge folgt bewusst der Rust-Lernkurve:

- [ ] **2.1** Helfer: Encoding, Fingerprints, Fehlertypen → *Ownership, `Result`, `thiserror`*
- [ ] **2.2** Keyfile v2: Argon2id, `serde` → *Structs, Traits, Serialisierung*
- [ ] **2.3** HPKE Single-Shot seal/open → *Generics, Trait Bounds*
- [ ] **2.4** Streaming/Chunking → *Lifetimes, Iteratoren — der härteste Teil*
- [ ] **2.5** Mehrere Empfänger, Passwort-Modus
- [ ] **2.6** v1-Kompatibilitätsleser (gegen Python-Testvektoren)
- [ ] **2.7** Metadaten: EXIF, PDF, DOCX
      → aufwendiger als in Python; `kamadak-exif`, `img-parts`, `lopdf`,
        DOCX ist ein ZIP mit XML
      → v1-Bug mitnehmen: Palette-PNGs (Mode `P`) verlieren beim Strippen die Farbpalette
- [ ] **2.8** Secure Delete mit **ehrlichem Rückgabewert** — v1 verschluckt alle
      Fehler und meldet trotzdem Erfolg
- [ ] **2.9** `cabrik-cli`: deckt den Core vollständig ab

**Professionelle Standards, die hier nicht übersprungen werden:**
- [ ] `#![forbid(unsafe_code)]` im Core
- [ ] `zeroize` für sämtliches Schlüsselmaterial
- [ ] `cargo fuzz` auf dem Envelope-Parser
- [ ] `cargo deny` für Lizenz- und CVE-Prüfung der Abhängigkeiten
- [ ] Differenztests gegen die Python-Referenz in CI

**Ergebnis:** CLI kann alles, was v1 konnte — plus Streaming, Mehrfachempfänger,
Passwort-Modus. Ab hier ist das Projekt technisch bereits wertvoll.

---

## Phase 3 — Frontend lernen

*Kein Rust. Gegen Mock-Daten, echte Anbindung erst in Phase 4.*

- [ ] Stack: **Svelte 5 + TypeScript + Tailwind**
      (sanftere Lernkurve und weniger Boilerplate als React; React hat mehr
      Tutorials — falls das schwerer wiegt, ist es die gleichwertige Alternative)
- [ ] Wireframes vor Code: Onboarding, Identität/Schlüssel, Kontakte + Verifikation,
      Senden, Empfangen, Werkzeuge
- [ ] Designsystem: Farben, Typografie, Zustände, Fehlermeldungen in Klartext
- [ ] Prototyp mit Fake-Daten, komplett ohne Backend

**Hier entsteht das, was das Produkt „ansprechend" macht.** Nicht in der Krypto.

---

## Phase 4 — Tauri-Integration

- [ ] Tauri-Commands als dünne Brücke zu `cabrik-core`
- [ ] **Architekturregel:** Schlüsselmaterial bleibt in Rust. Das Frontend
      erhält ausschließlich Handles, Status und Fortschritt — nie Secrets.
- [ ] Session-Entsperrung über OS-Keychain
      (v1 hielt das Passwort dauerhaft im Klartext in `STATE`)
- [ ] Drag & Drop, Fortschrittsereignisse aus dem Streaming
- [ ] `.enc`-Dateizuordnung (wie in v1 bereits gelöst)
- [ ] **Fehlerbehandlung ernst nehmen** — v1 stürzt bei falschem Keyfile mit
      Traceback ab, statt eine verständliche Meldung zu zeigen

---

## Phase 5 — Produktreife

- [ ] CI: GitHub Actions, Builds für Windows/macOS/Linux
- [ ] Reproduzierbare Builds
- [ ] Signierter Auto-Updater (Tauri Updater)
- [ ] Dokumentation + Website (statisch, ohne Krypto, mit Prüfsummen)
- [ ] **Core quelloffen stellen** — ohne das ist keine Sicherheitsaussage überprüfbar
- [ ] Erst bei kommerzieller Absicht: Code Signing, Notarisierung, Audit

---

## Phase 6 — Mobile

*Erst wenn Desktop läuft und die Spec eingefroren ist.*

- [ ] UniFFI-Bindings aus `cabrik-core` → Swift + Kotlin
- [ ] Testvektoren beweisen Bit-Gleichheit zum Desktop
- [ ] Natives UI pro Plattform
- [ ] iOS: Share-Sheet und Dateiweitergabe sind der eigentliche Knackpunkt,
      nicht die Krypto

---

## Zeitrahmen (nebenberuflich, mit Lernkurve)

| Phase | Dauer |
|---|---|
| 0 — Fundament | 2–3 Wochen |
| 1 — Spezifikation | 3–4 Wochen |
| 2 — Rust-Core | 3–5 Monate |
| 3 — Frontend | 1–2 Monate *(parallel zu Phase 2 möglich)* |
| 4 — Tauri | 1–2 Monate |
| 5 — Produktreife | 2–3 Monate |
| **Bis vertriebsfähiges Desktop-Produkt** | **~10–14 Monate** |
| 6 — Mobile | +4–6 Monate |

Ohne Termindruck ist das entspannt machbar. Die Krypto ist der kleinste Posten —
Rust-Lernkurve und UI fressen die Zeit.

---

## Kosten (erst bei kommerziellem Vertrieb)

| Posten | Kosten |
|---|---|
| Code Signing (Azure Trusted Signing) | ~10 $/Monat |
| Code Signing (EV-Zertifikat, Alternative) | 300–500 €/Jahr |
| Apple Developer Program | 99 $/Jahr |
| Security-Audit (kleiner Scope) | 5.000–15.000 € |

Ohne Code Signing blockt Windows SmartScreen den Installer — bei einer
Verschlüsselungssoftware installiert das niemand.

**Compliance bei Store-Vertrieb:** Verschlüsselungs-Deklaration
(`ITSAppUsesNonExemptEncryption`), US-Selbstklassifizierung als Mass-Market
(ECCN 5D992), separate Erklärung für Frankreich. Routine, aber notwendig.

**DSGVO:** kein Server, keine Accounts, keine Daten — die beste denkbare
Ausgangslage. So halten.

---

## Offene Entscheidungen

- [ ] **Lizenzmodell.** Empfehlung: `cabrik-core` + `cabrik-cli` + `spec/`
      unter Apache-2.0, App-Schicht später proprietär. Niemand vertraut
      Closed-Source-Krypto von einem unbekannten Anbieter; die UI-Schicht
      darf geschlossen bleiben.
- [ ] **Markenrecherche** „Cabrik Secure", falls kommerzieller Vertrieb.
      Billig jetzt, teuer später.
- [ ] **Transport-Layer ja/nein.** Nicht jetzt, aber die Spec darf ihn nicht
      unmöglich machen.

---

## Nächster Schritt

Phase 0: Git-Repository anlegen, Repo aufräumen, Python v1 nach `legacy/` sichern.
