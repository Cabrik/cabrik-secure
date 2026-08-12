# Cabrik Secure 2.0 — Roadmap

**Stand:** 2026-08-06
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

- [x] **Git-Repository anlegen** — existiert bisher nicht
- [x] Monorepo-Struktur aufsetzen (siehe unten)
- [x] `.gitignore` für Rust, Node, Python, PyInstaller
- [x] Aufräumen:
  - [x] Testschlüssel aus dem Root entfernen (`t.json`, `AonTestKey1.json`,
    `TestKeySignatur1.json`, `TestKeyAnnonym1.json`, `myid.json`)
  - [x] Build-Artefakte entfernen (`cabrik_secure/gui/build/`, `cabrik_secure/gui/dist/`,
    `build/`, `dist/`, `Output/`, `cabrik_secure.egg-info/`)
  - [x] Vier konkurrierende `.spec`-Dateien auf die eine aktive reduzieren
  - [x] `myid.json` enthält versehentlich `pyproject.toml`-Inhalt
- [x] Python v1 nach `legacy/python-v1/` verschieben, `dependencies` in
      `pyproject.toml` nachtragen, damit sie als Referenz lauffähig bleibt
- [x] **Lizenzmodell festlegen** (siehe „Offene Entscheidungen")
- [x] Rust-Toolchain installieren, `rustup`, `cargo`, VS Code + rust-analyzer

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

- [x] Verschlüsselung ist randomisiert — bit-genaue Verschlüsselungsvektoren
      erfordern eine **injizierbare Zufallsquelle**, die die Spec vorschreiben
      muss (Vorbild: RFC 9180 fixiert `ikmE` in seinen eigenen Vektoren)
- [x] Drei Ebenen: Entschlüsselungsvektoren (von Natur aus deterministisch),
      Verschlüsselungsvektoren (nur mit fixiertem RNG), Kreuzmatrix
- [x] `cabrik-core` muss zusätzlich die offiziellen RFC-9180-Vektoren bestehen

### 3. `spec/envelope-v2.md`

- [x] **HPKE nach RFC 9180** statt eigenem Key-Agreement
      → Suite `0x0001`: `DHKEM(X25519, HKDF-SHA256)` + `ChaCha20-Poly1305`
      → behebt das fehlende Transcript-Binding in v1 (`_derive_session_key`)
      → auditierte Implementierungen existieren in Rust *und* Swift/Kotlin
- [x] **Post-Quantum: Suite `0x0002`** (X-Wing = X25519 + ML-KEM-768)
      → wehrt „heute mitschneiden, später entschlüsseln" ab
      → verbindlich zu implementieren, Voreinstellung vorerst `0x0001`
        wegen der Schlüsselgröße (~1 620 Zeichen im Austausch)
      → **jede Identität trägt den ML-KEM-Schlüssel ab Tag 1**, sonst wird der
        spätere Umstieg zur teuersten denkbaren Migration
      → Rust: `libcrux-ml-kem` (formal verifiziert, in Firefox produktiv)
- [x] **Header-Leck schließen.** Aus einem v1-Envelope liest jeder ohne
      Schlüssel: Dateiname, Klartextgröße, Empfänger-Fingerprint, Zeitstempel,
      verwendetes Programm — und in nicht-anonymen Nachrichten den
      **persistenten Signatur-Public-Key des Absenders**. Der ephemere
      Schlüsselaustausch macht den Absender unsichtbar, der Header hebt das
      sofort wieder auf. In v1 gibt es Authentizität *oder* Anonymität, nie
      beides.
      → Absender-Authentifizierung und Dateimetadaten wandern in den
        **verschlüsselten** Teil. Im Klartext bleibt nur, was zum
        Entschlüsseln zwingend nötig ist.
- [x] **Binärformat mit Chunked Streaming** (STREAM-Konstruktion wie `age`,
      bzw. libsodium `secretstream`)
      → behebt: 78,1 % Größen-Overhead (empirisch bestätigt via `smoke_test.py`)
      → behebt: komplette Datei im RAM, Peak bei ~4–5× Dateigröße
      → Base64-„Armor" bleibt als *optionaler* Modus für Copy-Paste
- [x] **Mehrere Empfänger** — pro Empfänger gewrappter Content-Key
- [x] **Passwort-Modus** — symmetrisch, ohne Schlüsselaustausch
- [x] **Strikte Versions- und Algorithmus-Validierung**, unbekannte Versionen
      werden abgelehnt (v1 liest den Header, prüft ihn aber nie)
- [x] **Abwärtskompatibilität:** v2 liest v1-Envelopes, schreibt nur v2

### 4. `spec/keyfile-v2.md`

- [x] Argon2id-Parameter explizit im Keyfile versioniert
- [x] Migration von v1-Keyfiles

### 5. `spec/trust-store.md` — der wichtigste konzeptionelle Fix

In v1 kommt der Signaturprüfschlüssel aus dem Header derselben Nachricht.
`signature_valid: true` beweist damit nur „konsistent signiert", nicht *wer*.

- [x] Lokaler, verschlüsselter Kontaktspeicher: Name ↔ `enc_pub` ↔ `sig_pub`
- [x] Verifikation out-of-band per Fingerprint-Vergleich oder QR-Code
      (Vorbild: Signal Safety Numbers)
- [x] **Fingerprint: 256 Bit intern**, Anzeige in Crockford-Base32,
      **mindestens 32 Zeichen** sichtbar (= 160 Bit, 80 Bit Kollisionsschutz).
      v1 hatte 8 Hex-Zeichen = 32 Bit.
- [x] Zusätzlich **Safety Number** als paarweise Ableitung beider Fingerprints,
      damit beide Seiten *eine* Zeichenfolge vergleichen
- [x] **UI-Regel:** drei klar getrennte Zustände
  - [x] „Signiert von **Alice** ✓" (verifizierter Kontakt)
  - [x] „Signiert von unbekanntem Schlüssel `abcd…`" (⚠ neutral, kein grüner Haken)
  - [x] „Nicht signiert / anonym"

### 6. `spec/metadata.md`

- [x] **Fähigkeitsmodell** mit drei Zuständen: `Vollständig bereinigt` /
      `Teilweise bereinigt (Rest: …)` / `Unbekanntes Format, nicht prüfbar`.
      v1 kopiert unbekannte Formate stillschweigend durch und suggeriert damit
      Sauberkeit — v2 behauptet sie für unverstandene Formate **nie**.
- [x] Abdeckung erweitern: vollständiges OOXML (docx/xlsx/pptx inkl. `app.xml`
      und `custom.xml`, die v1 gar nicht anfasst), ODF, HEIC/HEIF, AVIF, GIF,
      BMP, SVG (Metadaten im XML)
- [x] Dateizeitstempel normalisieren — v1 nutzt `shutil.copy2` und *erhält* sie
- [x] Palette-PNGs (Mode `P`) korrekt behandeln (v1-Bug: Farbpalette geht verloren)
- [x] **Nicht in 2.0:** Video- und Audio-Container (MP4-Atome, MKV, ID3)

### 7. `spec/shredding.md`

- [x] **Ehrliche Garantien.** Überschreiben löst das SSD-Problem nicht:
      Wear-Leveling schreibt jeden Vorgang auf eine neue physische Seite,
      dazu Over-Provisioning, NTFS-Journal, Shadow Copies, Pagefile.
      Dateien unter ~700 Bytes liegen resident im MFT-Eintrag.
- [x] **Crypto-Shredding als eigentliche Lösung:** Klartext berührt die Platte
      nie. v1 schreibt beim Mehrfach-Anhang ein ZIP im Klartext nach
      `tempfile.mkdtemp()` — ein echtes Leck. In v2 wird gestreamt; temporäre
      Daten liegen nur in einem Container, dessen Schlüssel ausschließlich im
      RAM existiert und danach zeroisiert wird.
- [x] Laufwerkstyp erkennen (Windows: `IOCTL_STORAGE_QUERY_PROPERTY`,
      Seek-Penalty) und dem Nutzer sagen, was tatsächlich erreichbar ist
- [x] MFT-residente Kleindateien vor dem Überschreiben über die Residenzgrenze
      aufblasen; Dateinamen vor dem Löschen mehrfach zufällig umbenennen
- [x] Rückgabewert meldet ehrlich, was gelungen ist — v1 verschluckt alle
      Fehler und meldet trotzdem Erfolg
- [x] **Nicht in 2.0:** ATA Secure Erase / NVMe Sanitize (nur laufwerksweit)

**Ergebnis:** Eingefrorene Spec. Ab hier ist die Sprache austauschbar.

---

## Phase 2 — Rust lernen am Core

*Kein UI. Nur Bibliothek und CLI. Der Python-Core ist das Orakel.*

Implementierungsreihenfolge folgt bewusst der Rust-Lernkurve:

- [x] **2.1** Helfer: Encoding, Fingerprints, `PADME`, Fehlertypen
      → *Ownership, `Result`, `thiserror`*
- [x] **2.2** Keyfile v2: Argon2id, TLV-Parser → *Structs, Traits, Serialisierung*
- [x] **2.3** HPKE Single-Shot seal/open, Suite `0x0001` → *Generics, Trait Bounds*
- [x] **2.4** Streaming/Chunking → *Lifetimes, Iteratoren — der härteste Teil*
- [x] **2.5** Mehrere Empfänger, Passwort-Modus, Signatur — erster vollständiger Envelope
- [x] **2.6** Suite `0x0002` (X-Wing) — bewusst nach dem klassischen Pfad,
      damit die Schnittstelle bereits steht und nur das KEM getauscht wird
- [x] **2.7** v1-Kompatibilitätsleser (gegen Python-Testvektoren)
- [x] **2.8** Trust Store: Fingerprints, Safety Numbers, Vertrauenszustände
- [x] **2.9b** Metadaten, restliche Formate — **vollständig**
      → ZIP-Grundlage mit normalisierten Zeitstempeln (ZIP-Epoche, nicht
        „jetzt" — sonst verriete schon der Unterschied, wann bereinigt wurde)
      → `zip` bewusst ohne Vorgabemerkmale: nur `deflate`, kein bzip2/LZMA/
        zstd/XZ/AES. Parser sind die teuerste Art von Abhängigkeit
      → vier Fundstellen kamen erst an **echten** Word-Dateien heraus:
        Vorschaubild, feste GUID in `customXml/`, `dc:description`, und
        EXIF **in eingebetteten Bildern**
      → Kommentare und nachverfolgte Änderungen bleiben und machen das
        Ergebnis `Partial` — sie zu entfernen wäre eine inhaltliche
        Entscheidung. Auf **ausdrückliche Anweisung** werden sie aufgelöst
        (`spec/metadata.md` §4.2.2)
      → ODF: Bearbeitungsdauer und Speicherzyklen, Druckername in
        `settings.xml`, Vorlagenpfad mit Benutzernamen. `mimetype` muss
        erster und unkomprimierter Eintrag bleiben — ein Test hält das fest
      → ZIP: Zeitstempel normalisiert, enthaltene Dateien bereinigt, Namen
        bleiben zwangsläufig. Office-Dokumente im Archiv werden behandelt,
        bloße Archive nur gemeldet — sonst gäbe es keine Schachtelungsgrenze
      → EPUB, JAR und Android-Pakete werden **benannt**: Die ZIP-Erkennung
        hätte sie sonst als gewöhnliches Archiv gemeldet und ihre eigenen
        Metadaten stillschweigend übergangen
      → WebP: RIFF-Chunks. Die Merkmalsbits in `VP8X` müssen mitgelöscht
        werden, sonst kündigt die Datei Metadaten an, die es nicht mehr gibt
      → GIF: `NETSCAPE` steuert die Wiederholung einer Animation und bleibt,
        `XMP Data` trägt den Verfassernamen und fällt weg
      → BMP: trägt kaum Metadaten — wird aber geprüft statt durchgewunken.
        Die Erkennung übersah zunächst ausgerechnet Dateien mit Anhängsel
      → TIFF: der aufwendigste Bildfall. Die Bilddaten hängen an Versätzen,
        also wird die Datei **neu gebaut** statt gestrichen — ein Fehler dabei
        erzeugt eine Datei, die keinen Fehler meldet und Müll anzeigt.
        Mehrseitige Scans behalten alle Seiten, Vorschau-Verzeichnisse
        verschwinden; unterschieden wird an `NewSubfileType`. BigTIFF wird
        erkannt und ehrlich abgelehnt
      → HEIC/AVIF: hier wäre der Neubau der falsche Tausch. Exif und XMP
        werden **an Ort und Stelle** durch gültige leere Blöcke gleicher
        Länge ersetzt — kein Versatz ändert sich, die Dateilänge bleibt auf
        das Byte gleich. Farbprofil und Vorschaubilder bleiben und werden
        benannt. Video wird bewusst nicht beansprucht
      → SVG: Erlaubnisliste für Elemente, Regeln für Attribute. Der
        schwerwiegendste Fund ist kein Metadatum, sondern der **Verweis nach
        außen** — ein Zählpixel, das dem Absender Zeitpunkt und IP-Adresse
        des Empfängers meldet. Eingebettete Bilder werden rekursiv bereinigt.
        Bleibt immer `Partial`
      → PDF: der folgenreichste Fund des ganzen Moduls ist die
        **Änderungshistorie**. Eine „geschwärzte" Stelle steht vollständig
        lesbar in der Datei; kein Leser zeigt sie an. Neuschreiben beseitigt
        sie. Jede Fassung ist ein gültiges PDF für sich — deshalb
        `metadata revisions` als Vorschau und `--revision N` zur Wahl.
        Signierte Dateien werden abgelehnt, rechtebeschränkte ohne Nachfrage
        geöffnet, Passwörter niemals geraten
- [~] **2.9a** Metadaten: Fähigkeitsmodell, PNG, JPEG
      → aufwendiger als in Python; `kamadak-exif`, `img-parts`, `lopdf`,
        OOXML und ODF sind ZIP mit XML
      → v1-Bug mitnehmen: Palette-PNGs (Mode `P`) verlieren beim Strippen die Farbpalette
      → eingebettete Vorschaubilder und zugeschnittene Office-Bilder als
        `Critical` erkennen
- [x] **2.10** Secure Delete mit **ehrlichem Rückgabewert** — v1 verschluckt alle
      Fehler und meldet trotzdem Erfolg
      → `ShredOutcome` meldet jeden Schritt einzeln; ein pauschales „Gelöscht"
        gibt es nicht mehr, und ein Test hält das fest
      → Fähigkeit wird nur auf `Overwrite` gesetzt, wenn positiv festgestellt
        (rotierende Platte **und** kein Copy-on-Write); sonst `BestEffort`
      → echte Erkennung unter Linux über sysfs; unter Windows bliebe nur
        `DeviceIoControl` und damit `unsafe` — bewusst nicht getan
      → Verzeichnisse: Vorschau, Eintippen des Namens, kategorische
        Verweigerung bei Wurzeln/Systempfaden/`.git`, Links nie verfolgt
- [x] **2.11** `cabrik-cli`: deckt den Core vollständig ab
      → **Der ergiebigste Schritt der ganzen Phase.** Der erste echte Aufrufer
        hat vier Fehler freigelegt, die kein Modultest finden konnte:
      → Die Austausch-Nutzlast trug den Post-Quantum-Schlüssel nicht, der
        Fingerprint aber schon. **Zwei ehrliche Beteiligte hätten sich nie
        verifizieren können**, und Suite `0x0002` war für jeden so angelegten
        Kontakt unerreichbar
      → `SignedUnknown` automatisch als Kontakt anzulegen erzeugt einen
        Fingerprint, den die Gegenseite nie sieht — aus einer Signatur allein
        entsteht kein Kontakt (`trust-store.md` §7.1.1)
      → `AUTH_FAILED` beim **Verschlüsseln** meldete „konnte nicht
        entschlüsselt werden"; neu ist `INVALID_RECIPIENT_KEY`
      → `shred --dir ordner` hielt jeden relativen Pfad für eine
        Laufwerkswurzel; alle Tests hatten absolute Pfade übergeben
      → Passwörter nie als Argument (Prozessliste, Shell-History), sondern
        Abfrage, `--password-file` oder `--password-stdin`
      → `--json` von Anfang an: dieselbe Datenform, die Phase 4 braucht
      → 14 Durchlauftests gegen das gebaute Programm, darunter „beide Seiten
        sehen dieselbe Safety Number"

- [x] **2.12** Große Dateien: gemessen, verbessert, ehrlich begrenzt
      → Spitzenbedarf beim Verschlüsseln lag beim **4-fachen** der
        Dateigröße — vier vollständige Kopien im Speicher
      → zwei davon beseitigt: `stream::seal_into` schreibt unmittelbar in
        den Ausgabepuffer, und ohne Füllung entfällt die Kopie der
        Nutzdaten (bei Dateien ist Padding voreingestellt aus)
      → gemessen **4,0 → 2,3**; für 200 MB jetzt 460 statt 804 MB
      → darunter geht es nicht ohne `std::io` im Kryptopfad — das widerspräche
        der Portierung nach iOS und Android und bleibt deshalb offen
      → `--max-size` mit 2 GB Voreinstellung: statt eines Speicherfehlers
        mitten im Vorgang eine Auskunft, die den Bedarf nennt und den Ausweg

- [x] **2.13** Videoformate: MP4/MOV, Matroska/WebM, AVI
      → alle drei führen Verzeichnisse mit **absoluten Byte-Positionen**
        (`stco`, `SeekHead`/`Cues`, `idx1`) — es darf sich kein Byte
        verschieben, sonst geht die Datei auf und spielt nicht
      → alle drei bringen dafür einen **eigenen Platzhalter** mit: `free`,
        `Void` und `JUNK`. Ersetzen an Ort und Stelle ist damit der im
        Format vorgesehene Weg, kein Kunstgriff
      → schwerwiegendster Fund bei MP4: `moov/udta/©xyz`, die
        **GPS-Koordinaten** der Aufnahme. Jedes Mobiltelefon schreibt sie
      → schwerwiegendster Fund bei Matroska: `SegmentFilename`, der
        **ursprüngliche Dateiname** — dasselbe Leck, das v1 im Umschlag
        hatte. Dafür gibt es jetzt `FindingKind::FileName`
      → `MuxingApp` und `WritingApp` sind Pflichtelemente und werden
        geleert statt entfernt; die `Seek`-Einträge auf entfernte
        Abschnitte und betroffene `CRC-32` fallen mit weg
      → Kapitel bleiben stehen — Navigation ist Inhalt, dieselbe Grenze wie
        bei Kommentaren in Word. Das Ergebnis ist dann `Partial`
      → die MKV-, WebM- und AVI-Vorlagen erzeugt **ffmpeg** (über PyAV), und
        derselbe ffmpeg prüft danach, dass alle 25 Bilder noch dekodieren.
        In der handgebauten Matroska stand zunächst ein falsches Byte in der
        Kennung des `Info`-Elements — Leser und Vorlage waren sich einig und
        lagen beide daneben. Erst die echte Datei zeigte es

- [x] **2.14** Tonformate: MP3, FLAC, Ogg/Opus, WAV — und M4A gab es umsonst
      → **hier endet die Byte-Regel.** Sie hieß nie „nichts verschieben",
        sondern „nichts verschieben, worauf etwas zeigt". Ein MP3 hat keine
        einzige Tabelle mit Byte-Positionen, also wird der Tag wirklich
        abgeschnitten und die Datei kleiner
      → FLAC nennt seinen Platzhalter wörtlich `PADDING`; seine Sprungmarken
        zählen ab dem ersten Tonrahmen und bleiben deshalb richtig
      → **Ogg ist der einzige Fall, in dem gerechnet werden muss**: Jede
        Seite trägt eine CRC über sich selbst — und zwar nach einer anderen
        Spielart als ZIP oder PNG. Kommentarpaket ersetzen heißt Seiten neu
        aufteilen, neu nummerieren, neu prüfsummen
      → **der Fund des Tages**: Das Identifikationspaket muss ALLEIN auf der
        ersten Seite stehen (Vorbis I §4.2, RFC 7845 §3). Ein erster Entwurf
        packte alle Kopfpakete in eine Seite, weil sie hineinpassten. ffmpeg
        spielte die Datei weiter ab — mutagen las die Tondaten als Kommentar
      → **WAV ist nicht nackt.** Der `bext`-Block eines Feldrekorders trägt
        Aufnehmenden, Gerätekennung, Uhrzeit der Aufnahme und eine weltweit
        eindeutige Materialkennung. Für ein anonymes Interview der
        schwerwiegendste Fund des ganzen Formats
      → **die ehrliche Grenze**: LAME schreibt seinen Namen in die
        Zusatzdaten der Tonrahmen. Im `Xing`-Kopf lässt er sich nullen, im
        Tonstrom nur durch Neuberechnen — deshalb bleibt ein MP3 aus einem
        Schnittprogramm `Partial`, und das wird auch so gesagt
      → ein ID3-Tag vor einer FLAC-Datei wird gefunden: FLAC kennt kein ID3,
        ein reiner FLAC-Reiniger übersieht ihn vollständig
      → der Vorbis-Kommentarleser und der RIFF-Läufer sind je **einmal**
        geschrieben und werden von zwei bis drei Formaten benutzt. Ein Parser
        ist die teuerste Art von Code, die man doppelt haben kann
      → `verify_medien_stripped.py` prüft jedes Ergebnis mit **ffmpeg und
        mutagen** und vergleicht eine Prüfsumme über die dekodierten
        Abtastwerte — nicht die Spieldauer, die bei MP3 nur geschätzt ist

- [x] **2.15** Rohdateien aus Kameras: erkannt und unangetastet gelassen
      → **der gefährlichste Fund des ganzen Moduls.** DNG, NEF, ARW und CR2
        SIND TIFF und wurden deshalb hier behandelt — sie sind aber umgekehrt
        aufgebaut: erstes Verzeichnis Vorschau, `SubIFD` das eigentliche Bild
      → das Modul entfernt `SubIFDs` als Vorschaubilder und entfernte damit
        **das Foto**. Im Versuch: 1368 Bytes -> 198 Bytes, Meldung
        „vollständig bereinigt". Kein Fehler, keine Warnung
      → dasselbe Versagen wie in v1, nur andersherum: dort stille Kopie mit
        behaupteter Sauberkeit, hier stille Vernichtung mit behaupteter
        Sauberkeit
      → auch richtig erkannte SubIFDs hätten nicht genügt: Der `MakerNote`
        zählt seine Versätze ab Dateianfang, und dieses Modul vergibt beim
        Neubau alle Versätze neu. Teile davon sind herstellereigen
        verschlüsselt und werden zugleich vom Rohentwickler gebraucht
      → erkannt wird **strukturell**: Sensormarken (`DNGVersion`, `CFAPattern`,
        `PhotometricInterpretation` 32803/34892) oder ein erstes Verzeichnis,
        das sich selbst als Vorschau ausweist und ein `SubIFD` führt. Eine
        Liste von Herstellern und Endungen wäre immer unvollständig
      → die Datei bleibt byteweise unverändert, das Ergebnis ist `Partial`,
        die Funde werden trotzdem gemeldet, und die Begründung nennt den
        Ausweg: als JPEG oder TIFF exportieren, das wird dann vollständig
        bereinigt
      → gefunden, weil die Frage gestellt wurde, warum RAW ausgelassen wird.
        Die eigene Begründung („herstellerspezifisch") hielt der Prüfung
        nicht stand — und dahinter lag ein Datenverlustfehler

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
