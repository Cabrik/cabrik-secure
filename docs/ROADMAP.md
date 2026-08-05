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

Reihenfolge ist verbindlich — jedes Dokument hängt vom vorherigen ab.

### 1. `spec/threat-model.md`

Legt fest, wogegen geschützt wird und wogegen ausdrücklich nicht. Bestimmt
alle folgenden Entscheidungen.

### 2. `spec/test-vectors.md`

Steht **vor** dem Formatdokument, weil Bit-Genauigkeit eine Anforderung an die
Architektur ist, nicht an die Tests.

- [ ] Verschlüsselung ist randomisiert — bit-genaue Verschlüsselungsvektoren
      erfordern eine **injizierbare Zufallsquelle**, die die Spec vorschreiben
      muss (Vorbild: RFC 9180 fixiert `ikmE` in seinen eigenen Vektoren)
- [ ] Drei Ebenen: Entschlüsselungsvektoren (von Natur aus deterministisch),
      Verschlüsselungsvektoren (nur mit fixiertem RNG), Kreuzmatrix
- [ ] `cabrik-core` muss zusätzlich die offiziellen RFC-9180-Vektoren bestehen

### 3. `spec/envelope-v2.md`

- [ ] **HPKE nach RFC 9180** statt eigenem Key-Agreement
      → Suite `0x0001`: `DHKEM(X25519, HKDF-SHA256)` + `ChaCha20-Poly1305`
      → behebt das fehlende Transcript-Binding in v1 (`_derive_session_key`)
      → auditierte Implementierungen existieren in Rust *und* Swift/Kotlin
- [ ] **Post-Quantum: Suite `0x0002`** (X-Wing = X25519 + ML-KEM-768)
      → wehrt „heute mitschneiden, später entschlüsseln" ab
      → verbindlich zu implementieren, Voreinstellung vorerst `0x0001`
        wegen der Schlüsselgröße (~1 620 Zeichen im Austausch)
      → **jede Identität trägt den ML-KEM-Schlüssel ab Tag 1**, sonst wird der
        spätere Umstieg zur teuersten denkbaren Migration
      → Rust: `libcrux-ml-kem` (formal verifiziert, in Firefox produktiv)
- [ ] **Header-Leck schließen.** Aus einem v1-Envelope liest jeder ohne
      Schlüssel: Dateiname, Klartextgröße, Empfänger-Fingerprint, Zeitstempel,
      verwendetes Programm — und in nicht-anonymen Nachrichten den
      **persistenten Signatur-Public-Key des Absenders**. Der ephemere
      Schlüsselaustausch macht den Absender unsichtbar, der Header hebt das
      sofort wieder auf. In v1 gibt es Authentizität *oder* Anonymität, nie
      beides.
      → Absender-Authentifizierung und Dateimetadaten wandern in den
        **verschlüsselten** Teil. Im Klartext bleibt nur, was zum
        Entschlüsseln zwingend nötig ist.
- [ ] **Binärformat mit Chunked Streaming** (STREAM-Konstruktion wie `age`,
      bzw. libsodium `secretstream`)
      → behebt: 78,1 % Größen-Overhead (empirisch bestätigt via `smoke_test.py`)
      → behebt: komplette Datei im RAM, Peak bei ~4–5× Dateigröße
      → Base64-„Armor" bleibt als *optionaler* Modus für Copy-Paste
- [ ] **Mehrere Empfänger** — pro Empfänger gewrappter Content-Key
- [ ] **Passwort-Modus** — symmetrisch, ohne Schlüsselaustausch
- [ ] **Strikte Versions- und Algorithmus-Validierung**, unbekannte Versionen
      werden abgelehnt (v1 liest den Header, prüft ihn aber nie)
- [ ] **Abwärtskompatibilität:** v2 liest v1-Envelopes, schreibt nur v2

### 4. `spec/keyfile-v2.md`

- [ ] Argon2id-Parameter explizit im Keyfile versioniert
- [ ] Migration von v1-Keyfiles

### 5. `spec/trust-store.md` — der wichtigste konzeptionelle Fix

In v1 kommt der Signaturprüfschlüssel aus dem Header derselben Nachricht.
`signature_valid: true` beweist damit nur „konsistent signiert", nicht *wer*.

- [ ] Lokaler, verschlüsselter Kontaktspeicher: Name ↔ `enc_pub` ↔ `sig_pub`
- [ ] Verifikation out-of-band per Fingerprint-Vergleich oder QR-Code
      (Vorbild: Signal Safety Numbers)
- [ ] **Fingerprint: 256 Bit intern**, Anzeige in Crockford-Base32,
      **mindestens 32 Zeichen** sichtbar (= 160 Bit, 80 Bit Kollisionsschutz).
      v1 hatte 8 Hex-Zeichen = 32 Bit.
- [ ] Zusätzlich **Safety Number** als paarweise Ableitung beider Fingerprints,
      damit beide Seiten *eine* Zeichenfolge vergleichen
- [ ] **UI-Regel:** drei klar getrennte Zustände
  - „Signiert von **Alice** ✓" (verifizierter Kontakt)
  - „Signiert von unbekanntem Schlüssel `abcd…`" (⚠ neutral, kein grüner Haken)
  - „Nicht signiert / anonym"

### 6. `spec/metadata.md`

- [ ] **Fähigkeitsmodell** mit drei Zuständen: `Vollständig bereinigt` /
      `Teilweise bereinigt (Rest: …)` / `Unbekanntes Format, nicht prüfbar`.
      v1 kopiert unbekannte Formate stillschweigend durch und suggeriert damit
      Sauberkeit — v2 behauptet sie für unverstandene Formate **nie**.
- [ ] Abdeckung erweitern: vollständiges OOXML (docx/xlsx/pptx inkl. `app.xml`
      und `custom.xml`, die v1 gar nicht anfasst), ODF, HEIC/HEIF, AVIF, GIF,
      BMP, SVG (Metadaten im XML)
- [ ] Dateizeitstempel normalisieren — v1 nutzt `shutil.copy2` und *erhält* sie
- [ ] Palette-PNGs (Mode `P`) korrekt behandeln (v1-Bug: Farbpalette geht verloren)
- [ ] **Nicht in 2.0:** Video- und Audio-Container (MP4-Atome, MKV, ID3)

### 7. `spec/shredding.md`

- [ ] **Ehrliche Garantien.** Überschreiben löst das SSD-Problem nicht:
      Wear-Leveling schreibt jeden Vorgang auf eine neue physische Seite,
      dazu Over-Provisioning, NTFS-Journal, Shadow Copies, Pagefile.
      Dateien unter ~700 Bytes liegen resident im MFT-Eintrag.
- [ ] **Crypto-Shredding als eigentliche Lösung:** Klartext berührt die Platte
      nie. v1 schreibt beim Mehrfach-Anhang ein ZIP im Klartext nach
      `tempfile.mkdtemp()` — ein echtes Leck. In v2 wird gestreamt; temporäre
      Daten liegen nur in einem Container, dessen Schlüssel ausschließlich im
      RAM existiert und danach zeroisiert wird.
- [ ] Laufwerkstyp erkennen (Windows: `IOCTL_STORAGE_QUERY_PROPERTY`,
      Seek-Penalty) und dem Nutzer sagen, was tatsächlich erreichbar ist
- [ ] MFT-residente Kleindateien vor dem Überschreiben über die Residenzgrenze
      aufblasen; Dateinamen vor dem Löschen mehrfach zufällig umbenennen
- [ ] Rückgabewert meldet ehrlich, was gelungen ist — v1 verschluckt alle
      Fehler und meldet trotzdem Erfolg
- [ ] **Nicht in 2.0:** ATA Secure Erase / NVMe Sanitize (nur laufwerksweit)

**Ergebnis:** Eingefrorene Spec. Ab hier ist die Sprache austauschbar.

---

## Phase 2 — Rust lernen am Core

*Kein UI. Nur Bibliothek und CLI. Der Python-Core ist das Orakel.*

Implementierungsreihenfolge folgt bewusst der Rust-Lernkurve:

- [ ] **2.1** Helfer: Encoding, Fingerprints, `PADME`, Fehlertypen
      → *Ownership, `Result`, `thiserror`*
- [ ] **2.2** Keyfile v2: Argon2id, TLV-Parser → *Structs, Traits, Serialisierung*
- [ ] **2.3** HPKE Single-Shot seal/open, Suite `0x0001` → *Generics, Trait Bounds*
- [ ] **2.4** Streaming/Chunking → *Lifetimes, Iteratoren — der härteste Teil*
- [ ] **2.5** Mehrere Empfänger, Passwort-Modus
- [ ] **2.6** Suite `0x0002` (X-Wing) — bewusst nach dem klassischen Pfad,
      damit die Schnittstelle bereits steht und nur das KEM getauscht wird
- [ ] **2.7** v1-Kompatibilitätsleser (gegen Python-Testvektoren)
- [ ] **2.8** Trust Store: Fingerprints, Safety Numbers, Vertrauenszustände
- [ ] **2.9** Metadaten: Bildformate, OOXML, ODF, PDF, SVG
      → aufwendiger als in Python; `kamadak-exif`, `img-parts`, `lopdf`,
        OOXML und ODF sind ZIP mit XML
      → v1-Bug mitnehmen: Palette-PNGs (Mode `P`) verlieren beim Strippen die Farbpalette
      → eingebettete Vorschaubilder und zugeschnittene Office-Bilder als
        `Critical` erkennen
- [ ] **2.10** Secure Delete mit **ehrlichem Rückgabewert** — v1 verschluckt alle
      Fehler und meldet trotzdem Erfolg
- [ ] **2.11** `cabrik-cli`: deckt den Core vollständig ab

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
