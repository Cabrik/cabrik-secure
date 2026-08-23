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

- [x] **2.16** Systematische Nachschau: Live Photo, CR3, und was nur benannt wird
      → nach dem Rohdatei-Fund geprüft, welche Formate die Erkennung
        beansprucht, ohne sie zu verstehen. PSD, JPEG 2000, JPEG XL und AAC:
        sauber unbeansprucht. **Canons CR3 nicht** — ISO-BMFF mit `isom` in
        der Markenliste, also als Video behandelt
      → CR3 verlor zwar kein Bild (es wird nichts verschoben), meldete aber
        „vollständig bereinigt", obwohl `THMB` und `PRVW` zweite Kopien der
        Aufnahme enthalten. Jetzt dieselbe Entscheidung wie bei TIFF-RAW:
        erkennen, benennen, unangetastet lassen
      → **ein iPhone benutzt die iTunes-Marken nicht.** Es legt seine Angaben
        im `keys`-Verzeichnis ab, wo der Kastentyp im `ilst` nur noch ein
        Index ist. Ein Leser, der auf `©`-Codes prüft, sieht dort gar nichts
      → entfernt wurde trotzdem alles (`udta` wird ganz zu `free`), gemeldet
        aber nur „614 Bytes Benutzerdaten" — bei einem echten Handyvideo wäre
        der **Aufnahmeort** damit unbenannt geblieben. Und `inspect` ist
        gerade das Werkzeug, mit dem man entscheidet
      → **das Live Photo besteht aus zwei Dateien**, verknüpft durch einen
        gemeinsamen Kennzeichner: `content.identifier` im `.MOV`, Apples
        MakerNote-Marke 0x0011 im `.HEIC`. Wer nur eine bereinigt, lässt die
        Verbindung bestehen. Er wird jetzt als `Critical` benannt
      → zwei kleinere Felder aus derselben Gegenprobe: das Namensfeld in
        `hdlr` und die Herstellerkennung in `stsd` (`FFMP`, `appl`). Beide
        fielen auf, weil ffmpeg sie noch las, als alles andere leer war
      → PSD, JPEG 2000, JPEG XL und gzip werden seither beim Namen genannt
        statt als „unbekannt" abgetan

- [x] **Prüfdurchgang vor der CI** — sechs Punkte, die nie nachgemessen wurden
      → **v1-Leser**: Meine Sorge war unbegründet. Die Vektoren stammen schon
        aus `legacy/python-v1`. Neu erzeugt und wieder durchlaufen lassen:
        14 Tests grün gegen frische Ausgaben der Referenz
      → **Trust Store mit mehreren Kontakten**: kein Fehler gefunden. Die
        wichtigste Eigenschaft — zwei Kontakte dürfen nicht denselben
        Signierschlüssel führen — war da, aber ungetestet. Jetzt festgehalten:
        Ohne sie könnte jemand den Schlüssel eines Dritten unter eigenem Namen
        eintragen, und jede Nachricht löste **still** auf den falschen Namen auf
      → **`shred` am echten Dateisystem**: Der v1-Fehlerfall ist behoben und
        nachgewiesen. Eine von einem anderen Prozess offen gehaltene Datei
        ergibt „Fehlgeschlagen", Rückgabewert 1, Datei bleibt. v1 meldete hier
        „Gelöscht". Schreibschutz wird aufgehoben **und gemeldet**
      → **Rundlauf über alle 31 Formate** im Release-Build: bereinigen,
        verschlüsseln, entschlüsseln — **alle bytegleich**. Als
        `testvectors/tools/pruefe_rundlauf.py` dauerhaft im Projekt, weil er
        das Zusammenspiel über die Crate-Grenzen prüft, das kein Einzeltest
        abdeckt
      → **Argon2 im Release-Build**: 0,39 s statt 6,8 s im Debug — Faktor 17
      → **Der eine echte Fund: der Speicherfaktor 2,3 war falsch begründet.**
        Gemessen ergibt sich `Spitze = 2,0 x Dateigröße + 4 MB`, und im
        Passwortmodus zusätzlich rund 250 MB für Argon2. Die ursprüngliche
        Messung nahm einen einzigen Punkt (200 MB → 460 MB) und hielt den
        **Argon2-Sockel für einen Faktor**. Sichtbar wird der Fehler bei
        kleinen Dateien: 50 MB mit Passwort ergeben Faktor **6,2**, nicht 2,3.
        Bei 400 MB fällt der Unterschied ganz weg, weil Argon2 seinen Speicher
        freigibt, bevor die großen Puffer ihre Spitze erreichen — die Anteile
        stapeln sich nicht. Für die Grenze selbst ändert das nichts (2,3 liegt
        bewusst über den gemessenen 2,08); falsch war nicht die Zahl, sondern
        ihre Begründung

**Professionelle Standards, die hier nicht übersprungen werden:**
- [x] `#![forbid(unsafe_code)]` im Core — werkbankweit gesetzt, alle fuenf
      Crates erben es; mit einer Wegwerfdatei nachgewiesen, dass es greift
- [x] `zeroize` für sämtliches Schlüsselmaterial
      → sieben von acht geheimnistragenden Typen waren abgedeckt. Der achte
        war ausgerechnet `Opened` — der Typ mit dem **entschlüsselten
        Klartext**, also dem eigentlichen Gegenstand des Schutzes
      → `Zeroizing<Vec<u8>>` statt `ZeroizeOnDrop` auf der Struktur: So
        wandert der Schutz **mit den Daten**, wenn der Aufrufer den Puffer
        herausnimmt. Ein `Drop` auf `Opened` hätte das Herausnehmen verboten
      → es wirkt auch tatsächlich: `stream::open` legt den Puffer einmal mit
        voller Kapazität an, es gibt unterwegs keine freigegebenen Umkopien
      → dieselbe Behandlung für die Leseseite: Beim Verschlüsseln ist der
        eingelesene Puffer der Klartext
      → eine Prüfung zur **Übersetzungszeit** hält fest, was entschieden
        wurde — wer eine Ableitung entfernt, kann den Test nicht mehr
        übersetzen
- [x] `cargo fuzz` auf dem Envelope-Parser — **und auf den Formatlesern**
      → fünf Ziele: Umschlag mit Identität, Umschlag mit Passwort,
        `metadata::inspect`, `metadata::strip` (zweimal hintereinander) und
        der v1-Leser. Der Umschlag stand im Auftrag; die siebzehn
        Formatleser sind die größere Fläche und kamen dazu
      → `fuzz/` ist eine **eigene Werkbank**. `cargo fuzz` braucht nightly,
        die Hauptwerkbank ist auf 1.97.1 festgenagelt — beides in einem Baum
        hieße, die Festlegung aufzuweichen
      → **auf Windows bauen die Ziele, starten aber nicht**: Es fehlt die
        Laufzeit des Adressprüfers, ein optionaler Visual-Studio-Bestandteil.
        `--sanitizer=none` hilft nicht, dann fehlen die Abdeckungssymbole.
        Fuzzing gehört ohnehin in die CI auf Linux
      → **deshalb die zweite Hälfte**, und die läuft überall: deterministische
        Verstümmelung mit festem Startwert in `tests/robustheit.rs`. Sechs
        Angriffsarten (Bit kippen, Byte ersetzen, abschneiden, Länge
        aufblähen, Länge nullen, Stück verdoppeln), je Fehlschlag die Saat in
        der Meldung — also nachstellbar
      → Ergebnis: **13 000 verstümmelte Umschläge und 3 100 Mediendateien**
        je Testlauf, kein einziger Absturz, zusammen unter zwanzig Sekunden
      → gemessen statt geraten: Ein Passwortlauf kostet im Debug-Build
        **6,8 Sekunden** (Argon2, 256 MiB, drei Durchgänge). Zwei Zuschnitte
        liefen deshalb in die Zeitgrenze. Der dritte trifft: ein Umschlag
        **ohne** Passwortkapsel, mit Passwort-Öffner geöffnet — er durchläuft
        die ganze Kapselsuche und kommt nie zur Ableitung
      → was das Fuzzing findet, gehört nach `testvectors/fuzz/` und wird von
        `korpus_bleibt_beherrschbar` bei jedem Lauf erneut geprüft.
        **Fuzzing findet, der Korpus hält fest**
- [x] `cargo deny` für Lizenz- und CVE-Prüfung der Abhängigkeiten
      → 141 fremde Crates im Baum, alle mit Lizenzangabe, **kein Copyleft** —
        die kommerzielle Weitergabe bleibt möglich
      → gegen 1223 Einträge der RustSec-Datenbank geprüft: **null Treffer**
      → `md-5` steckt im Baum und ist gebrochen. Zurückverfolgt: Es kommt aus
        `lopdf`, weil die PDF-Norm ihn für ihre eigene Alt-Verschlüsselung
        vorschreibt — außerhalb unseres Kryptopfads. Die Ausnahme ist
        **auf `lopdf` beschränkt**; käme MD5 über einen anderen Weg herein,
        schlüge die Prüfung an
      → vorbeugend verboten: `sha1`, `rc4`, `des`, `openssl*`, `native-tls`.
        Sie sind nicht da und sollen es nicht werden — der Kern bleibt reines
        Rust, sonst wären UniFFI und `forbid(unsafe_code)` dahin
      → **ein echter Fund nebenbei**: Die eigenen Crates hingen als reine
        Pfadabhängigkeiten ohne Fassungsangabe zusammen. Formal sind das
        Wildcards, und veröffentlichen ließe sich so nichts. Sie werden jetzt
        wie die fremden zentral in `[workspace.dependencies]` geführt
- [x] Differenztests gegen die Python-Referenz in CI
      → vier Arbeitsläufe unter `.github/workflows/`: **Prüfung** (Linux und
        Windows: fmt, clippy, Tests), **Gegenprobe** (fremde Werkzeuge),
        **Abhängigkeiten** (`cargo deny`), **Fuzzing** (nightly auf Linux)
      → die Differenzprüfung erzeugt die v1-Vektoren bei jedem Lauf **neu aus
        `legacy/python-v1`** und lässt den Rust-Leser darauf los. Ohne das
        prüften wir nur, ob wir unsere eigene Lesart richtig aufgeschrieben
        haben
      → auch die Vorlagen werden neu gebaut statt benutzt: Nur so fällt auf,
        wenn eine neue Fassung von Pillow oder ffmpeg etwas anders schreibt
      → `cargo deny` und die Gegenprobe laufen **wöchentlich nach Plan**. Eine
        Schwachstelle wird veröffentlicht, ohne dass jemand hier eine Zeile
        anfasst; ein Projekt, das nur beim Bauen prüft, erfährt davon zufällig
      → nur `actions/checkout` und `actions/cache` von GitHub selbst. Ein
        Projekt, das seine Rust-Abhängigkeiten prüft, sollte daneben nicht
        beliebige fremde Actions einbinden
      → **zwei Funde beim Vorbereiten**, beide hätten die CI beim ersten Lauf
        angehalten: Das Bild-Prüfskript ging jeden Manifest-Eintrag durch und
        scheiterte an der ersten MP3 — und es meldete HEIC als „lässt sich
        nicht öffnen", weil es den HEIF-Leser nie angemeldet hatte
      → dadurch kam heraus: **die unabhängige Pillow-Prüfung deckte nur JPEG
        und PNG ab**. Das Manifest war auf ein Dutzend Bildformate gewachsen,
        das Ablegen der Ergebnisse nicht. Jetzt werden **12 statt 4** Dateien
        geöffnet und Pixel für Pixel verglichen

**Ergebnis:** CLI kann alles, was v1 konnte — plus Streaming, Mehrfachempfänger,
Passwort-Modus. Ab hier ist das Projekt technisch bereits wertvoll.

---

## Phase 3 — Frontend lernen

*Kein Rust. Gegen Mock-Daten, echte Anbindung erst in Phase 4.*

- [x] **Anzeigevertrag** (`spec/anzeige.md`) — steht vor den Wireframes
      → **vier Zustände, nicht drei.** Grün, Gelb, Rot — und *Keine Aussage*.
        Der vierte ist der wichtigste: Er entspricht der Flagge am künstlichen
        Horizont, die erscheint, wenn das Instrument seine Eingangsdaten
        verliert. Die gefährlichste Anzeige ist die, die etwas Plausibles
        zeigt, während sie nichts weiß — genau das war v1
      → **Grün heißt nicht „sicher"**, sondern „dieses System meldet
        Normalbetrieb". Bei den Metadaten also: alle Träger entfernt, die
        dieses Programm *für dieses Format* kennt
      → **Farbe steht nie allein.** Rund acht Prozent der Männer unterscheiden
        Rot und Grün schlecht. Für ein Werkzeug, dessen ganzer Zweck eine
        Einschätzung ist, kein Randfall. Farbe **und** Zeichen **und** Wort,
        und die Bedeutung steckt im Wort
      → `Unsigned` wird **neutral**, nicht gelb: Anonymer Versand ist ein
        legitimer Modus. Ihn zu warnen drängte jeden zum Signieren und träfe
        ausgerechnet die, die es nicht dürfen. Wer eine Signatur *verlangt*,
        bekommt bei ihrem Fehlen einen Fehler
      → Liste verbotener Formulierungen: „sicher", „garantiert
        metadatenfrei", „anonym" als Zusicherung
- [x] Stack: **Svelte 5 + TypeScript + Tailwind 4** unter `app/oberflaeche/`
- [x] **Bildschirm „Empfangen"** — zuerst gebaut, weil dort alle vier
      Zustände zusammentreffen
      → Absender und Metadaten werden **nebeneinander** bewertet, nicht zu
        einem Urteil verrechnet. Ein verifizierter Absender macht eine
        unbereinigte Datei nicht sauber, und umgekehrt
      → bei „teilweise bereinigt" steht **Geblieben zuerst und aufgeklappt**.
        Das ist die Nachricht, nicht die Liste des Entfernten
      → **statt Wireframes gleich ein lauffähiger Prototyp**, mit einer
        Fallauswahl über acht Beispiele: Nur so lassen sich auch die
        seltenen Zustände ansehen, die sonst schlecht gestaltet werden
- [x] Designsystem: die vier Zustände als Bausteine, die den Vertrag
      durchsetzen
      → `kern/typen.ts` bildet die Rust-Aufzählungen **eins zu eins** nach
        statt bequemer eigener Typen. Damit ist die Datei zugleich der
        Entwurf des Brückenvertrags für Phase 4
      → `anzeige/zustand.ts` hat **Tests**: 35 insgesamt, davon sechs, die
        jeden Beispielfall serverseitig darstellen und nachlesen, was
        tatsächlich dasteht. Ein Vertrag, der nur im Dokument steht, wird
        beim dritten Bildschirm gebrochen
      → die Beispieldaten stammen aus der wirklichen Arbeit am Kern — der
        Aufnahmeort in `©xyz`, der Kodierername in den MP3-Tonrahmen, der
        Kennzeichner des Live Photo, das Hauptbild im `SubIFD`. „Datei1.txt"
        prüft nichts
      → in der CI: Typprüfung, Anzeigevertrag und Bau
- [x] **Dunkler Modus in der Logopalette** und die zweite Farbachse
      → das Logo ist bereits Cyan auf Schwarz: 45 % Schwarz, 25 % Weiß,
        10 % `#00E8FF`. Der dunkle Modus nimmt das auf — mit einer
        Abweichung: **kein reines Schwarz**, weil leuchtendes Cyan darauf
        Halation erzeugt. Cockpitanzeigen sind aus demselben Grund dunkelgrau
      → **Cyan und Magenta sind keine fünften und sechsten Zustände**,
        sondern eine eigene Achse. Im Glascockpit gibt es zwei getrennte
        Farbsysteme: Rot/Gelb/Grün ist die Warnhierarchie, Weiß/Cyan/Magenta
        die Informationshierarchie. Magenta ist dort nicht „schlimmer als
        grün", sondern der am Autopiloten **eingestellte** Sollwert
      → hier also: **Cyan** für Werte, die das Programm gelesen hat — Format,
        Größe, Fingerprint, Fundstelle. **Magenta** für das, was der Nutzer
        verlangt hat: „Sie haben eine Signatur verlangt"
      → Regel: beide erscheinen **nie in einer Zustandsmarke**. Dort wären
        sie doch wieder Zustände. Ein Test belegt, dass alle Zuordnungen
        zusammen genau vier Werte zurückgeben können
      → Systemeinstellung als Voreinstellung, Umschalten trotzdem möglich:
        Wer in dunklem Raum mit hellem System arbeitet, soll nicht das
        Betriebssystem umstellen müssen
- [x] **Bildschirm „Senden"** — die Metadatenvorschau steht **vor** dem
      Verschlüsseln, nicht danach
      → wer erst verschlüsselt und dann berichtet, was drin war, stellt den
        Nutzer vor eine Datei, die er nicht mehr ändern kann, ohne von vorn
        zu beginnen
      → **kein Überspringen bei vielen Dateien**, obwohl es naheliegt. Dort
        ist die Prüfung am wichtigsten: Wer vierzig Dateien schickt und drei
        sind nur teilweise bereinigt, übersieht beim Überspringen genau die
        drei. Stattdessen wird eine Ebene höher zusammengefasst — das
        Unauffällige zu einer Zeile, das Auffällige einzeln
      → aber **zugeklappt heißt nicht weggeworfen**: Die Sammelzeile lässt
        sich aufklappen und nennt jede bereinigte Datei mit der Zahl ihrer
        Funde. „Stör nur, wenn du wirklich etwas zu sagen hast" darf nicht
        „nicht nachsehen können" bedeuten
      → eine Datei, aus der alles entfernt wurde, fällt in die Sammelzeile —
        **auch wenn der Fund kritisch war**. Was weg ist, ist keine
        Entscheidung mehr. Sonst gewöhnt man sich das Wegklicken an, und
        dann wirkt die Bestätigung auch dort nicht, wo sie zählt
      → ein Empfänger ohne Post-Quantum-Schlüssel zieht die **ganze**
        Nachricht auf die klassische Suite herunter. Das steht dabei, mit
        Namen und Grund — sonst hält jemand eine Nachricht für
        quantensicher, die es nicht ist
      → **einzeln abwählbar, und das ist der Kern.** Der erste Entwurf ließ
        nur zwei Wege: alles senden oder die Dateiauswahl von vorn machen.
        Bei einundvierzig Dateien ist „von vorn" so teuer, dass praktisch
        jeder das Bestätigungshäkchen setzt — damit erzieht die Bestätigung
        genau zu dem Wegklicken, gegen das sie gebaut ist. **Der sichere Weg
        muss der bequemste sein**, deshalb nimmt ein Klick alle auffälligen
        heraus
      → das Ausgenommene verschwindet **nicht**, sondern bleibt mit Grund
        stehen — sonst hielte man das Problem für gelöst statt für umgangen
      → Farbe dafür: **Magenta**, nicht Grau. Eine ausgenommene Datei ist
        kein Systemzustand, sondern ein eingestellter Sollwert
      → die Zählung nennt immer **beide** Zahlen („38 von 41"). „38 Dateien"
        allein verschwiege die drei anderen
- [x] **Bildschirm „Kontakte"** — Vertrauen ist hier eine *Handlung*, beim
      Empfangen nur eine *Anzeige*
      → daraus folgt die unbequemste Regel des ganzen Entwurfs:
        **derselbe Sachverhalt, zwei Bewertungen.** Ein nie verifizierter
        Kontakt ist im Verzeichnis **grau**, als Absender einer Nachricht
        **gelb**. Als Eintrag ist er erwartbar — so fängt jeder an. Erst wenn
        man sich auf den Namen verlassen soll, wird daraus eine Warnung
      → das ist genau die Art Unterscheidung, die beim Umbauen still
        verschwindet. Deshalb steht sie als Test da, nicht nur als Absatz
      → die Safety Number steht **groß, einfarbig cyan, in zwölf Gruppen zu
        fünf Ziffern** — zum Vorlesen am Telefon, sprachunabhängig
      → der Vergleich verlangt ausdrücklich einen Weg, „den Sie nicht über
        dieses Programm hergestellt haben"
      → der Widerruf verspricht nichts, was er nicht hält: „Wirkt nur bei
        Ihnen. Ein Widerruf ohne Verteilweg erreicht niemanden sonst"
- [x] **Gelernt: Tests prüften nur, dass die Sperre hält — nie, dass sie
      aufgeht**
      → dahinter versteckte sich ein Bildschirm, der sich überhaupt nicht
        bedienen ließ: Ein `$effect`, der die Bestätigung „beim Stapelwechsel"
        zurücksetzen sollte, lief bei **jeder** Änderung und löschte sie
        sofort wieder. Der Knopf blieb dauerhaft gesperrt
      → alle Tests waren grün, weil jeder von ihnen `disabled === true`
        erwartete. Eine Sperre, die sich nie öffnet, besteht jeden dieser
        Tests. **Zu jeder Sperre gehört die Gegenprobe**
      → Konsequenz im Entwurf: kein Rücksetzen mehr per Effekt. Statt
        `gesehen: boolean` steht dort jetzt `bestaetigtFuer: string | null`
        und die Zugehörigkeit als Vergleich. Damit kann nirgends ein
        Rücksetzen vergessen werden — es gibt keins mehr
- [x] **Bildschirm „Identität"** — seine wichtigste Eigenschaft ist, was er
      nicht kann
      → es gibt keinen Knopf, der den privaten Schlüssel zeigt, exportiert
        oder kopiert, und der Typ `Identitaet` hat **gar kein Feld dafür**.
        Was nicht existiert, kann nicht versehentlich angezeigt werden
      → die Bezeichnung ist **nur lokal**. Wer die Austausch-Nutzlast
        aufnimmt, vergibt den Namen selbst. Wer das nicht weiß, hält den
        Namen für etwas Zugesichertes
      → „Es gibt keine Wiederherstellung" — nicht „schwierig", nicht „nur
        mit Aufwand". Ein Test verbietet die Weichmacher ausdrücklich
      → das Fehlen eines Signierschlüssels ist **grau, nicht gelb**:
        dieselbe Regel wie bei `unsigniert`
- [x] **Bildschirm „Erste Einrichtung"** — mit der unbequemsten Entscheidung
      des ganzen Entwurfs: **keine Passwort-Stärkeanzeige**
      → ein Balken von Rot nach Grün ist an dieser Stelle die bekannteste
        Lüge der Softwaregestaltung. `Sommer2024!` erfüllt jede Regel über
        Länge, Zeichenarten und Sonderzeichen und steht trotzdem in jeder
        Angriffsliste. Der Balken zeigt Grün und meint Rot
      → das ist genau der Fall, für den es den vierten Zustand gibt: Grau,
        keine Aussage — und daneben ein Rat, der etwas taugt
      → angezeigt wird nur, was das Programm **wirklich weiß**: Länge und
        Übereinstimmung. Beides in Cyan, denn es sind gelesene Werte
      → der Satz, der sonst nie fällt: **Die Passwortableitung verteuert
        jeden Rateversuch, sie macht ein erratbares Passwort nicht sicher.**
        Ohne ihn verkauft man Argon2 als Ersatz für ein gutes Passwort
      → die KDF-Stufen nennen ihren **Preis**, nicht nur ihren Nutzen: 1 GiB
        ist auch beim eigenen Entsperren langsam
- [x] **Bildschirm „Werkzeuge"** — sicheres Löschen und Außenansicht
      → `bestEffort` ist der **Normalfall**, nicht die Ausnahme. Der Satz
        dazu nennt den Grund (SSD, Copy-on-Write), damit man es nicht als
        Fehler des Programms liest
      → mehr Überschreibdurchgänge werden **nicht verboten, aber auch nicht
        beschwiegen**: Ab zwei erscheint der Hinweis, dass Gutmanns 35
        Durchgänge sich auf MFM/RLL der frühen 1990er beziehen. v1 hatte
        drei voreingestellt und suggerierte einen Nutzen, den es nicht gibt
      → „Kopien können nicht ausgeschlossen werden" erscheint fast immer —
        ehrlicher als eine Anbieterliste, die nie vollständig wird
      → die Außenansicht zeigt beide Fassungen nebeneinander: Bei v1 steht
        der Dateiname im Klartext, bei v2 nur die Kapselzahl. Und darunter
        der Satz, den Verschlüsselungswerkzeuge gern weglassen:
        **verborgen wird der Inhalt, nicht der Vorgang**
- [x] **Bildschirm „Kontakt aufnehmen"** — der Moment, in dem ein fremder
      Schlüssel in den Speicher kommt
      → **die Prüfsumme ist keine Sicherheitsprüfung.** Sie stellt fest,
        dass die Zeichenfolge unterwegs nicht zerrissen wurde. Deshalb
        erscheint ihr *Gelingen* nirgends als Erfolgsmeldung — „Prüfsumme
        stimmt" läse sich wie „Absender bestätigt" und wäre das Gegenteil.
        Nur ihr Ausbleiben ist ein Fehler, und der heißt
        **Übertragungsfehler**, nicht Angriff
      → der Fingerprint wird aus den Schlüsseln **neu berechnet**; dem
        mitgelieferten Wert zu vertrauen verbietet `spec/trust-store.md`
        §5.1 ausdrücklich. Dass das ein Unterschied ist, steht dabei
      → **der Name ist Ihrer.** Die Nutzlast trägt keinen — was eingetippt
        wird, ist eine Notiz an sich selbst
      → **kein Weg, gleich als verifiziert aufzunehmen.** Abgesichert nicht
        in der Anzeige, sondern im Speicher: `aufnehmen()` setzt `gesehen`,
        und eine andere Methode gibt es nicht
      → vier Nutzlastfälle: vollständig, ohne PQ-Schlüssel, ohne
        Signierschlüssel, bekannter Kontakt mit **anderem** Schlüssel
- [x] **Der Vergleich, der schiefgeht** — hatte bis dahin keinen Bildschirm
      → nennt den **häufigsten** Grund zuerst (Zahlendreher beim Vorlesen),
        nicht den schlimmsten. Erst danach: „Bleibt es dabei, sitzt jemand
        zwischen Ihnen"
      → setzt den Kontakt **zurück**, statt ihn zu widerrufen. Widerrufen
        hieße „dieser Schlüssel ist kompromittiert" — das weiß niemand.
        Bekannt ist nur, dass die Prüfung fehlgeschlagen ist
- [x] **Gemeinsamer Kontaktspeicher** (`kern/speicher.svelte.ts`)
      → vorher las jeder Bildschirm die Beispieldaten für sich. Ein
        aufgenommener Kontakt wäre beim Senden nicht aufgetaucht, und ein
        Prototyp, dessen Teile einander widersprechen, taugt nicht zum
        Beurteilen — beurteilen ist der ganze Zweck von Phase 3
      → die Methoden sind bereits so geschnitten, wie die Brücke sie
        brauchen wird: eine Änderung, ein Aufruf
- [x] **Löschen — und der Unterschied, den niemand von selbst sieht**
      → **Kontakt löschen ist nicht Kontakt widerrufen**, und die
        Verwechslung ist gefährlich: Widerrufen heißt „dieser Schlüssel ist
        kompromittiert" — der Eintrag bleibt und warnt. Löschen heißt „ich
        kenne diese Person nicht" — der Eintrag verschwindet, **und mit ihm
        die Warnung**. Wer einen verdächtigen Schlüssel löscht, sieht ihn
        beim nächsten Mal als unbekannten Absender wieder und nimmt ihn
        arglos neu auf. Der Bildschirm sagt das, und bei einem bereits
        widerrufenen Schlüssel wird daraus eine Warnung
      → **Identität löschen verlangt Abtippen, kein Häkchen.** Ein Häkchen
        erzieht zum Wegklicken, und dies ist der eine Vorgang, bei dem
        Wegklicken nicht passieren darf. Genannt wird auch die Folge für die
        Gegenseite: Die verschlüsselt weiter an einen Schlüssel, den es nicht
        mehr gibt
      → dazu der Hinweis auf den Weg, der meist gemeint ist: **eine zweite
        Identität anlegen und die alte stehen lassen**
      → leere Zustände für beides — ein Verzeichnis ohne Einträge ist kein
        Fehlerfall, sondern der Anfangszustand
- [x] **Was nach dem Verschlüsseln dasteht**
      → der Knopf tat nichts, also ließ sich nicht beurteilen, was er
        auslöst. Für einen Prototyp, dessen Zweck das Beurteilen ist, war
        das die schwerere Lücke
      → **die Ausgangsdateien liegen unverschlüsselt weiter da.** Der Satz,
        den Verschlüsselungswerkzeuge gern weglassen: Verschlüsseln legt eine
        zweite Datei daneben, es ersetzt die erste nicht
- [x] **Quelltextprüfungen** (`quelltext.test.ts`)
      → derselbe Anführungszeichenfehler war mir dreimal unterlaufen: „…“
        mit geradem `"` geschlossen, was den JavaScript-String beendet. Ein
        Fehler, der sich wiederholt, ist keine Unaufmerksamkeit mehr,
        sondern eine fehlende Prüfung
      → dazu zwei Prüfungen zur Architekturregel: Der Brückenvertrag führt
        kein Feld, dessen Name auf ein Geheimnis deutet, und kein Bildschirm
        enthält Wortlaut, der einen privaten Schlüssel anzeigen würde

**Phase 3 ist damit abgeschlossen.** Sechs Bildschirme, 102 Tests, kein Rust.

**Hier entsteht das, was das Produkt „ansprechend" macht.** Nicht in der Krypto.

### Nachtrag: der Kern konnte mehr als die Oberfläche zeigte

Bei der Frage, ob `cabrik-metadata` für feinere Entscheidungen erweitert
werden müsste, kam heraus: **Es musste gar nichts erweitert werden.** Der
Kern bot vier formatabhängige Entscheidungen an, die die Oberfläche
sämtlich verschwieg — `--revision`, `--keep-history`, `--remove-comments`,
`--accept-changes`. Dazu `metadata revisions`, das frühere PDF-Fassungen
samt dem daraus entfernten Text auflistet.

- [x] **Frühere PDF-Fassungen** im Befund — die klassische Schwärzungspanne
      → gezeigt wird nicht, wie eine Fassung aussah, sondern **was nur dort
        steht**: der Text, den jemand herausgenommen hat und der trotzdem
        mitfährt
- [x] **Die vier Schalter als Zielkonflikte**, nicht als Optionen
      → keiner, der den Inhalt verändert, ist voreingestellt
      → jeder wird nur angeboten, wo er etwas bewirkt
      → die Folge steht sofort daneben: „Historie behalten“ nennt die Zahl
        der Zeilen, die dadurch mitgehen
- [x] gegen den Ausbau des Kerns entschieden, und zwar begründet: Die
      Sicherheit der Bereinigung ruht auf „nichts verschieben, worauf etwas
      zeigt“. Selektives Entfernen einzelner Funde hieße, **Strukturen neu
      zu schreiben** statt bekannte Träger zu neutralisieren — leicht bei
      PNG und ID3, gefährlich bei EXIF und PDF, und die interessanten Fälle
      liegen bei EXIF und PDF
      → falls es später doch kommt: **nach Art, nicht nach Einzelfund.**
        Niemand will „GPS in Foto 3 behalten, in Foto 7 nicht“ — man will
        „Farbprofile behalten“. Eine Richtlinie über `FindingKind` ist ein
        weit kleinerer Eingriff

### Offen aus Phase 3

- Ein **Passwortgenerator** wäre die konsequente Fortsetzung des
  Stärkeanzeigen-Verzichts: Wenn das Programm die Güte eines fremden
  Passworts nicht beurteilen kann, kann es doch eines erzeugen, dessen
  Entropie es exakt kennt. Bewusst **nicht** im Frontend gebaut — er gehört
  in den Kern, mit einer ordentlichen Wortliste und dem RNG des Systems.
- **QR-Code-Darstellung** der Austausch-Nutzlast ist als Knopf vorhanden,
  aber ohne Erzeuger. Der Kern kann die Nutzlast bereits ausgeben
  (`spec/trust-store.md` §5.1, rund 2050 Zeichen, QR-Version 29).
- Eine **stabile Dateikennung** statt des Namens im Sendestapel. Zwei
  gleichnamige Dateien aus verschiedenen Ordnern kollidieren derzeit.

---

## Phase 3a — Der Brückenvertrag

*Vorbereitung von Phase 4, ohne Tauri.*

- [x] **Befund: Es gab nichts, wogegen man hätte prüfen können.** Der Kern
      trug **keine einzige** `Serialize`-Ableitung; `serde_json` kam nur in
      Tests und im v1-Leser vor. Es existierten drei unabhängige
      Auffassungen desselben Sachverhalts — die Typen in `cabrik-core` und
      `cabrik-metadata`, die von Hand gebauten `json!`-Blöcke der CLI, und
      `kern/typen.ts`. Keine zwei waren aneinander gehalten
- [x] **`crates/cabrik-bruecke`** — der Vertrag als eigene Schicht
      → bewusst **nicht** im Kern: `cabrik-core` bleibt frei von serde und
        von jeder Annahme darüber, wer die Daten anzeigt
      → und es ist zugleich die Sperre gegen Schlüsselmaterial: Was gar
        nicht erst in einen serialisierbaren Typ gerät, kann nicht
        versehentlich über die Brücke gehen
- [x] **Prüfmuster statt Vermutung** (`tests/vertragsmuster.rs`)
      → der Test **vergleicht** mit den eingecheckten Mustern, statt sie zu
        überschreiben. Ändert sich der Vertrag, schlägt er fehl — genau
        dann, wenn das Frontend nachziehen muss
      → jede Variante kommt vor, auch die unbequemen: `Unbekannt` **ohne**
        Formathinweis, ein verifizierter Absender **ohne** vermerkten Weg,
        ein Kontakt ohne Post-Quantum-Schlüssel
- [x] **`vertrag.test.ts`** hält die TypeScript-Typen dagegen
      → die Prüfung läuft zur **Laufzeit**, nicht im Übersetzer: TypeScript
        verbreitert JSON-Zeichenketten zu `string`, damit passt nichts auf
        eine Literal-Union, und ein `as` verdeckte beides ohne zu prüfen
      → die Listen, gegen die geprüft wird, sind selbst gegen `typen.ts`
        deklariert. Ändert sich dort eine Union, übersetzt der Test nicht
        mehr

### Was die Prüfung sofort fand

- `rename_all = "camelCase"` benennt bei Aufzählungen nur die **Varianten**,
  nicht die Felder. Der Vertrag lieferte `verifiziert_am`, das Frontend
  suchte `verifiziertAm` — behoben mit `rename_all_fields`
- `Authenticity::SignedVerified` trug **keinen Verifikationsweg**, obwohl
  `spec/trust-store.md` §5 ihn verlangt und die Oberfläche ihn seit heute
  anzeigt. Der Wert lag im Kontakt bereits vor und wurde nur nicht
  mitgenommen — ein Einzeiler
- `SignedVerified::verified_at` ist **`Option`**, der TS-Typ verlangte eine
  Zahl
- `SignedChanged` führt `previous_fingerprint`, im TS-Vertrag fehlte es ganz
- `FindingKind::FileName` heißt nicht `OriginalFilename`
- **`FindingKind` ist `#[non_exhaustive]`**, die TS-Union war geschlossen.
  Käme im Kern eine Fundart hinzu, zeigte die Oberfläche stumm nichts an.
  Jetzt gibt es `Fundart::Unbekannt` auf beiden Seiten — derselbe Gedanke
  wie beim vierten Anzeigezustand

### Offen

- Die CLI baut ihr JSON weiterhin von Hand. Sie sollte perspektivisch
  dieselbe Schicht benutzen, sonst bleiben zwei Übersetzungen bestehen
- Noch nicht im Vertrag: `Geoeffnet`, `Aussenansicht`, `Identitaet`,
  `Loeschbefund`. Sie hängen an Typen, die der Kern teils noch nicht in
  dieser Form führt

---

## Phase 4 — Tauri-Integration

### 4.1 Die Naht — erledigt, ohne Tauri

**Leitprinzip 2 gab die Reihenfolge vor:** Tauri einzuführen *und*
gleichzeitig sechs Bildschirme von Beispieldaten auf echte umzuhängen, wären
zwei Unbekannte — und wenn dann etwas nicht geht, weiß niemand, an welchem
von beiden es liegt.

- [x] **`kern/bruecke.ts`** — die Schnittstelle, hinter der der Kern einzieht
      → **alles asynchron**, obwohl heute nichts wartet. Das ist der
        strukturelle Unterschied zwischen Beispieldaten und einem echten
        Kern, und er ist nicht nachträglich einzuziehen
      → bewusst schmal: Jede Methode entspricht einer Handlung, die ein
        Mensch auslöst. Kein allgemeines „lies mir dieses Feld“ — der Kern
        entscheidet, was er herausgibt
      → die Regel „ein aufgenommener Kontakt ist **gesehen**“ steht jetzt in
        der Schnittstelle, nicht in der Anzeige: Es gibt keinen Parameter,
        mit dem sie sich umgehen ließe
- [x] **Der Speicher wird zum Zwischenhalter** — er hält, was der Kern
      zuletzt geantwortet hat, und fängt Fehler, statt sie zu werfen
      → ein Bildschirm, der beim Laden abstürzt, sagt dem Nutzer nichts
- [x] die Bildschirme warten die Antwort ab
- [x] 227 Tests, davon neun für die Naht selbst

**Was der Umbau kostete und was er zeigte:** Sieben Tests fielen sofort um —
jeder, der Synchronität annahm. Genau dafür war er gedacht. Ein vermuteter
Fehler beim Löschen des ersten Kontakts erwies sich in der Gegenprobe als
keiner: Der Rückfall im `$derived` fing ihn ab. Das `await` steht trotzdem
dort, weil der Code sonst etwas anderes sagt, als er meint — aber es hat
nichts behoben, und das steht so im Test.

### 4.1b Der Vertrag ist vollständig

- [x] **`Geoeffnet`, `Aussenansicht`, `Loeschbeurteilung`, `Loeschergebnis`**
      in `cabrik-bruecke` — mit Prüfmustern und Gegenprüfung im Frontend
- [x] `Geoeffnet` trägt **keinen Klartext einer Datei.** `Opened::plaintext`
      ist ein `Zeroizing<Vec<u8>>` und bleibt in Rust; die Oberfläche
      bekommt Name und Größe. Die einzige Ausnahme ist die Textnachricht,
      wo der Text zugleich das ist, was angezeigt werden soll — und sie
      steht als Test da, damit sie eine bleibt

**Drei weitere Erfindungen von mir, die der Vertrag widerlegt hat:**

- `Aussenansicht` führte feste Felder für Dateiname und Klartextgröße. Der
  Kern gibt stattdessen eine **freie Liste von Sätzen** aus. Feste Felder
  wären beim nächsten Format schon zu eng — was eines preisgibt, hängt am
  Format
- `Loeschbefund` mischte eine **Vorab-Beurteilung** mit einem **Ergebnis**.
  Der Kern trennt beides (`Assessment` gegen `ShredOutcome`), und die
  Oberfläche tut es jetzt auch
- dasselbe Feld führte eine `grundlage` mit Sätzen wie „NTFS auf
  rotierender Platte, keine Schattenkopien“. **Die gibt es im Kern nicht.**
  Ich hatte der Anzeige eine Gewissheit gegeben, die niemand geprüft hat —
  der Absatz ist ersatzlos entfallen

**Und ein Fund in die andere Richtung:** Mein Auffangzweig für neue
`Warning`-Varianten war unerreichbar. Anders als `FindingKind` ist `Warning`
nicht `non_exhaustive` — ein neuer Vorbehalt bricht dort die Übersetzung,
und das ist die bessere Nachricht: Ein stiller Auffangzweig verschlucke ihn,
und die Oberfläche zeigte eine Warnung an, die nicht die gemeinte ist.

### 4.1c Die Befehle — wieder ohne Tauri

Dieselbe Reihenfolge wie im Frontend, aus demselben Grund: Erst die
Befehle, geprüft und lauffähig, dann die Hülle darum. Ein
`#[tauri::command]` ist danach eine Zeile über einer Funktion, die bereits
tut, was sie soll.

- [x] **`crates/cabrik-app`** mit `Sitzung` — kennt Tauri nicht und läuft
      unter `cargo test`, ohne Fenster und ohne Ereignisschleife
- [x] die Gegenseite von `kern/bruecke.ts`: jede Methode entspricht genau
      einer dort, beide geben Typen aus `cabrik-bruecke` heraus
- [x] **`Sitzung` hat kein Feld für ein Passwort.** v1 hielt es dauerhaft
      im Klartext in seinem Zustand. Hier ist die Frage „wie lange halten
      wir es“ nicht beantwortet, sondern **weggefallen**
- [x] 10 Tests gegen einen echten `TrustStore`

**Ein Fund im Kern, den erst der erste externe Aufrufer aufdeckte:**
`Contact::new_seen` führt `PQ_PUB_LEN` im Typ, und die Konstante war
**privat**. Von außerhalb der Crate ließ sich damit kein Kontakt mit
Post-Quantum-Schlüssel anlegen — der gesamte Pfad war über die
Crate-Grenze hinweg unerreichbar. Es fiel nicht auf, weil die CLI ihn nie
gegangen ist: In ihren Tests steht überall `None`.

Beim Beheben kam heraus, dass dieselbe Länge an **drei** Stellen steht
(`trust`, `fingerprint`, `xwing`) und nichts sie aneinander band. Jetzt tun
es zwei `const _: () = assert!(…)` — der Übersetzer statt der Hoffnung.

### 4.2 Der Rest

- [x] **`crates/cabrik-fenster`** — die Fensterhülle, dünn mit Absicht
      → alles Entscheidende steht in `cabrik-app` und ist dort ohne Tauri
        geprüft. Geht am Ende etwas nicht, liegt es an der Hülle oder an
        Tauri — nicht an den Regeln darunter
      → **Regel: keine Regel in einem `#[tauri::command]`.** Sobald dort ein
        `if` über Vertrauen, Metadaten oder Schlüssel entscheidet, ist es
        nur noch mit laufender Webansicht prüfbar
      → `windows_subsystem = "windows"`: kein Konsolenfenster daneben. Bei
        einem Werkzeug für vertrauliche Dateien kein Schönheitsfehler —
        eine Konsole nimmt Ausgaben entgegen, die niemand sehen soll
      → kein `expect` in `main`: Eine Panik hinterließe unter Windows nur
        ein Fenster, das verschwindet
- [x] **`kern/tauri.ts`** — dieselbe Schnittstelle wie die Attrappe
      → `@tauri-apps/api` wird **erst beim Aufruf** geholt. Ein statischer
        Import risse im Browser und in den Tests alles mit
- [x] **Die Befehlsnamen als Prüfmuster** (`vertrag/befehle.json`)
      → die einzige Stelle im ganzen Aufbau, an der etwas stumm
        auseinanderlaufen kann: Rust-Funktionsnamen gegen TypeScript-
        Zeichenketten. Drei Rust-Tests und drei TS-Tests halten beide
        Richtungen fest; die Gegenprobe mit einer Umbenennung schlägt an

### Was Tauri an Abhängigkeiten mitbrachte

Beides geprüft, beides mit Begründung in `deny.toml` statt stillschweigend:

- **Fünf Crates unter MPL-2.0** (`cssparser`, `cssparser-macros`,
  `selectors`, `dtoa-short`, `option-ext`). MPL ist **dateiweises**
  Copyleft: Wer eine MPL-Datei ändert, veröffentlicht die geänderte Datei;
  das Einbinden in ein größeres, auch unfreies Werk erlaubt §3.3
  ausdrücklich. Keine davon wird hier verändert — kein `vendor`, kein
  `[patch]`. **Neu zu prüfen**, sobald eine davon eingebunden und angepasst
  wird
- **Sechzehn Meldungen vom Typ `unmaintained`, keine einzige
  Verwundbarkeit.** Zehn betreffen die GTK3-Bindungen, die Tauri nur unter
  **Linux** braucht — unter Windows und macOS wird der Code nicht einmal
  übersetzt. Die übrigen sechs sind reine Bauzeit-Abhängigkeiten
  (`proc-macro-error`, `unic-*`). **Neu zu prüfen**, sobald Linux ein Ziel
  wird

- [x] **Die Brücke ist umgeschaltet** — im Fenster der Kern, im Browser die
      Attrappe
      → die Unterscheidung steht an **genau einer Stelle**
        (`speicher.svelte.ts`). Kein Bildschirm fragt danach, keiner darf es
      → im Browser bleibt der Prototyp benutzbar, und das ist kein
        Übergangszustand: Die seltenen Zustände lassen sich dort ansehen,
        ohne sie im Kern herstellen zu müssen
      → Beispielkontakte im Fenster **nur mit `debug_assertions`**. Ein
        Werkzeug, das beim ersten Start fremde Namen im Verzeichnis zeigt,
        hätte sein Vertrauensmodell verspielt, bevor es benutzt wird
      → die Fußzeile sagt jetzt, was gerade gilt. „Prototyp mit
        Beispieldaten“ im Fenster anzuzeigen, wo es über den Kern geht, wäre
        die Sorte kleine Unwahrheit, die man später niemandem mehr erklärt
      → Fehler aus dem Kern stehen sichtbar in der Oberfläche, nicht in
        einer Konsole, die im Fenster gar nicht aufgeht
- [x] **Kontakte aufnehmen über den Kern** — die Schnittstelle reicht jetzt
      die **Austausch-Nutzlast** durch statt fertiger Felder
      → `nutzlastLesen` ist vom Aufnehmen **getrennt**: erst ansehen, was
        drinsteht, dann entscheiden. Ein Bildschirm, der beides in einem
        Aufruf erledigt, kann den Befund gar nicht zeigen, bevor er handelt
      → der Fingerprint entsteht im Kern aus den Schlüsseln. Ihn von der
        Oberfläche zu übergeben hieße, dem Absender zu glauben
- [x] **`QrFehler` im Kern** — zwei Fälle statt einer Sammelmeldung
      → vorher war jeder Fehlschlag `Error::Malformed` mit verschiedenen
        Texten. Wer sie unterscheiden wollte, musste auf **Zeichenketten**
        prüfen, und eine Umformulierung hätte die Anzeige stumm verändert
      → die CLI hatte es deshalb gar nicht erst versucht: Ihre Meldung
        zählte beide möglichen Ursachen auf. Genau das war der Hinweis
        darauf, dass die Unterscheidung fehlt
      → jetzt zwei Ratschläge: Wer etwas Falsches eingefügt hat, braucht
        die richtige Quelle. Wer die richtige eingefügt hat und sie kam
        verstümmelt an, braucht sie noch einmal — **und die Beruhigung,
        dass es kein Angriff ist**
- [x] **Die Sitzung mit Sperrzustand** (`spec/entsperrung.md`)
      → **die Sperre steht im Typ, nicht in einer Prüfung.** Die
        Kontaktbefehle liegen auf `Offen`, und an ein `&mut Offen` kommt man
        nur durch `Sitzung::offen` — das prüft die Frist selbst. Eine
        Prüfung am Anfang jeder Methode wäre beim nächsten Befehl zu
        vergessen; hier gibt es den Empfänger sonst gar nicht
      → `entsperren` nimmt ein `Zeroizing<String>` und **weiß nicht, woher
        es kommt** — heute die Webansicht, später ein natives Fenster (§5.2)
      → die Sitzung hat **kein Feld für ein Passwort**
      → `stand()` setzt die Messung **nicht** zurück: Sonst hielte allein
        das Anzeigen der Restzeit die Sitzung offen — genau das täte eine
        Oberfläche, die jede Sekunde nachfragt
      → 30 Tests, darunter die Frist auf die Sekunde, „Nachfragen ist keine
        Handlung“ und dass ein falsches Passwort nicht verrät, wie falsch
- [x] **`crates/cabrik-ablage`** — wo die Dateien liegen und wie sie
      geschrieben werden
      → dieselbe Überlegung wie beim Dateiformat: Die Pfadlogik stand in
        der CLI, und das Fenster hätte sie ein zweites Mal bekommen. Zwei
        Umsetzungen desselben Verzeichnisses laufen auseinander — dann
        schriebe die CLI woanders hin, als die Anwendung liest
      → **kein Krypto.** Diese Schicht liest und schreibt Bytes; was sie
        bedeuten, weiß der Kern
      → **eine fehlende Datei ist kein Fehler**, sondern der erste Start
      → **atomar geschrieben**: erst daneben, dann umbenennen. Und die
        Zwischendatei wird auch im Fehlerfall aufgeräumt — sonst bliebe
        eine `.tmp` mit einer älteren Fassung des Kontaktspeichers liegen,
        lesbar mit demselben Schlüssel
- [x] **Das Fenster lädt und sichert** — Schlüssel- und Kontaktdatei aus
      derselben Ablage wie die CLI
      → gesichert wird in **derselben Funktion**, die ändert. Den Aufrufer
        daran zu erinnern wäre schlechter: Wer es einmal vergisst, verliert
        stillschweigend eine Verifikation
- [x] **Der Sperrbildschirm** — ein eigener Bildschirm, kein Fenster darüber
      → gesperrt ist **keineAussage**, nicht `fehler`. Es ist der
        Normalzustand eines Programms, das seine Arbeit getan hat
      → **`Sitzung::taetigkeit` kam dabei dazu.** Ohne ihn liefe die Frist
        ab, während jemand eine lange Nachricht schreibt — in dieser Zeit
        wird kein anderer Befehl ausgelöst. Er prüft zuerst die Frist und
        weckt deshalb keine abgelaufene Sitzung auf
      → **Nachfragen ist keine Handlung.** Die Oberfläche fragt im
        Sekundentakt; würde das die Messung zurücksetzen, hielte die
        Sitzung sich durch das Anzeigen ihrer eigenen Restzeit offen
      → die Vorwarnung misst in **Anteilen** der eingestellten Zeit. Feste
        Minuten gingen bei einer Frist von einer Minute nicht auf
      → der Normalfall ist **Schweigen**. Der Test dafür prüft 800
        Restsekunden, wo nichts dastehen darf — ein Dauerzähler wäre in
        jedem Schwellentest grün gewesen
- [x] **Architekturregel:** Schlüsselmaterial bleibt in Rust. Das Frontend
      erhält ausschließlich Handles, Status und Fortschritt — nie Secrets.
      → geprüft in `quelltext.test.ts` gegen die Vertragsmuster
- [x] **Identität anlegen und löschen im Fenster** — der Weg vom leeren
      Rechner bis zur offenen Sitzung, ohne CLI
      → **angelegt heißt offen.** Wer gerade ein Passwort gesetzt hat, hat
        es eben getippt; ihn danach auf den Sperrbildschirm zu schicken
        verlangt dieselbe Eingabe zweimal und schützt vor nichts
      → drei Sperren gegen das Überschreiben einer bestehenden Identität.
        `cabrik_ablage::schreib_neu` ist die verlässliche — die im Fenster
        ist die höfliche, die in der Attrappe zeigt den Fall
      → Löschen verlangt eine **entsperrte** Sitzung. Das schützt die Datei
        nicht, aber es verhindert, dass das Programm selbst einen Knopf
        anbietet, mit dem jemand ohne Passwort alles vernichtet
      → `KdfStufe` samt Zahlen ist in den Kern gewandert. Vorher lag die
        Zuordnung in der CLI und die Zahlen noch einmal in der Anzeige —
        dasselbe Wort hätte zwei verschieden starke Dateien schreiben können
- [x] **Der verwaiste Kontaktspeicher** — gefunden auf einem echten Rechner
      → er ist an die alte Identität versiegelt und dauerhaft unlesbar.
        Bleibt er liegen, scheitert das Entsperren an ihm, mit dem
        **richtigen** Passwort, und die Identität ist unerreichbar
      → `identitaet_anlegen` schiebt ihn beiseite statt ihn zu löschen
      → `Befehlsfehler` trägt jetzt `betrifft`, damit der Aufrufer den Pfad
        ergänzen kann, den die Sitzungsschicht nicht kennt
- [x] **Dateien ansehen, bevor etwas geschieht** — `datei_pruefen`
      → der Befund **ist** der Vorgang: `strip` läuft wirklich, das
        Ergebnis wird weggeworfen. Eine zweite Einschätzung derselben Frage
        liefe beim nächsten Formatzusatz auseinander
      → über die Brücke geht der Befund, **nicht der Inhalt**
      → **der Pfad ist die Kennung, nicht der Name.** Zwei Ordner, dieselbe
        `Rechnung.pdf` — mit dem Namen als Schlüssel traf jede Ausnahme
        beide oder keine. Bei Namensgleichheit tritt der Ordner daneben
- [x] **Dateiauswahl im Fenster und Drag & Drop** — Dialog in Rust, damit
      die Webansicht keine Berechtigung bekommt, die sie sonst nicht hätte
- [x] **Verschlüsseln und Entschlüsseln über die Brücke** — Dateien und Text
      → die Prüfung der Empfänger steht **vor** dem ersten Byte
      → an einen widerrufenen Schlüssel wird nicht verschlüsselt; alles
        andere wird gesagt, nicht verhindert
      → der entschlüsselte Klartext geht **nicht** über die Brücke
- [x] **Armor** (`spec/envelope-v2.md` §14) — war spezifiziert und nie gebaut
      → bei Text ist Padding an: „ja" und ein langer Absatz ergeben gleich
        große Envelopes
- [x] **Die eigene Austausch-Nutzlast** — ohne sie war das Programm
      einseitig: Kontakte aufnehmen ging, sich mitteilen nicht
- [x] **QR-Code erzeugen** — als SVG-Pfad, nicht als Bild
      → **Befund:** Der Post-Quantum-Schlüssel treibt die Größe. Von rund
        2070 Zeichen einer Nutzlast sind 1946 der X-Wing-Schlüssel:
        gemessen 141 Module Kantenlänge mit ihm, 41 ohne
      → dunkel auf hell, auch im dunklen Modus. Kameras erwarten das; dem
        Farbschema zu folgen sähe stimmiger aus und wäre schlechter zu
        scannen
      → Byte-Modus. Der alphanumerische wäre sparsamer, kennt aber nur
        Großbuchstaben — die Nutzlast beginnt mit `cabrik:v2:`. Eine
        Änderung am Format gehört gesondert entschieden
- [ ] QR-Code abscannen — braucht eine Kamera. **Eigene Phase 4d**, weil es
      die erste Stelle wäre, an der Cabrik etwas ans Netz gibt
- [x] **Schlüsseldatei sichern und Passwort ändern**
      → das Ändern lässt die **Identität unberührt**: derselbe Fingerprint,
        dieselben Kontakte, alte Envelopes gehen weiter auf. Das ist die
        Erwartung, die am häufigsten danebenliegt, und sie steht dabei
      → eine **alte Sicherungskopie** öffnet weiter mit dem alten Passwort.
        Keine Fehlfunktion, sondern die Natur der Sache — und deshalb
        gesagt, bevor jemand tippt
      → das bisherige Passwort wird verlangt, obwohl entsperrt ist: „offen"
        heißt nicht, dass der Berechtigte davorsitzt
      → die Stärke der Ableitung bleibt. Sie dabei zu verschieben wäre eine
        zweite Entscheidung unter der Flagge der ersten
- [x] **Sicheres Löschen im Fenster**
      → die Beurteilung steht **vor** der Tat: Wer erst löscht und dann
        erfährt, dass Überschreiben auf einer SSD nichts ausrichtet, kann
        nichts mehr entscheiden
      → jeder Schritt einzeln gemeldet — überschrieben, umbenannt,
        entfernt. Ein pauschales „Gelöscht" wie in v1 wäre eine Behauptung
        über drei Dinge, von denen jedes einzeln scheitern kann
      → die Bestätigung hängt an der **Auswahl**: Kommt eine Datei dazu,
        ist sie von selbst weg
- [x] **Ein Befund über Empfangenes** — eigener Typ `Metadatenbefund`
      → `Bereinigung` beantwortete die falsche Frage: Beim Empfangen wird
        nichts bereinigt. `entfernt` und `geblieben` hätten einen Vorgang
        behauptet, den es nicht gab
      → **die Funde gehören dem Absender.** Ein Foto mit GPS-Angabe verrät,
        wo *er* stand. Diese Blickrichtung ist der eigentliche Nutzen und
        steht so auf dem Bildschirm
      → er kommt **ungefragt beim Öffnen**: Der einzige Zeitpunkt, an dem
        die Auskunft etwas ändert, liegt vor dem Speichern
      → „nichts gefunden" ist eine Aussage, `null` (Textnachricht) ist
        keine. Die beiden dürfen nie gleich aussehen
      → **Befund nebenbei:** Die Beispielfälle behaupteten alle eine
        Bereinigung, die beim Empfangen nie stattfand — der Begriffsfehler
        steckte auch in der Vorführung
- [x] **Fortschritt bei großen Stapeln** — alle fünf, über `Channel`
      → **Befund beim Bauen:** `dateien_pruefen` war als einziger ein
        `#[tauri::command]` **ohne** `(async)`. Der Makro-Quelltext von
        Tauri sagt, was das heißt: `ExecutionContext::Blocking` antwortet
        auf dem aufrufenden Faden, und das ist unter Windows der Faden,
        der das Fenster zeichnet. Vierzig Fotos froren die Anzeige ein —
        kein fehlender Balken, sondern ein hängendes Fenster
      → ein `Channel` statt eines globalen Ereignisses: Er gehört zu
        **einem** Aufruf und endet mit ihm. Kein hängender Zuhörer, kein
        Namenskonflikt, keine Berechtigung in `capabilities/` nötig
      → gemeldet wird **vor** der Datei: „arbeitet an X“, nicht „X ist
        fertig“. Bei einer langsamen Datei starrte man sonst auf den Namen
        der vorigen
      → die **Art** steckt im Fortschritt, nicht in einem zweiten Feld:
        Zwei Zustände, die zusammengehören, laufen auseinander — und dann
        stünde „Wird gelöscht“ über einem Prüflauf
      → ein Wächter liest `main.rs` und verlangt von jedem Befehl mit
        `pfade: Vec<String>` beides: `(async)` und einen Kanal
- [ ] Session-Entsperrung über OS-Keychain
      (v1 hielt das Passwort dauerhaft im Klartext in `STATE`)
- [ ] Fortschritt **innerhalb** einer großen Datei — der Balken zählt
      Dateien, nicht Bytes. Eine einzelne 3-GB-Datei steht bei „0 von 1“.
      Der Kern arbeitet schon in Blöcken (`stream::CHUNK_SIZE`), die
      Meldung fehlt. **Nach Phase 5.2 verschoben**
- [x] **`.cabrik`-Dateizuordnung** — Doppelklick im Explorer
      → **Befund vor dem Bauen:** Cabrik schrieb `.cab`, und das ist
        Microsoft Cabinet. Windows hat es fest vergeben
        (`HKLM\SOFTWARE\Classes\.cab` → `CABFolder` → `explorer.exe`).
        Eine Zuordnung darauf hieße, einen Systemdateityp zu kapern — ein
        dokumentiertes Verhalten von Schadsoftware. Dazu sortieren viele
        Firmen-Mailfilter `.cab`-Anhänge grundsätzlich aus. Die
        Magic-Bytes kollidierten nie (`CA 02` gegen `MSCF`); der **Name**
        war das Problem
      → Endung jetzt `.cabrik`, und sie steht in `cabrik-core` neben den
        Magic-Bytes: Fenster und CLI hatten je eine Abschrift, und beim
        Wechsel wäre eine stehengeblieben
      → `.cab` bleibt **lesbar** — im Dateidialog und über die Magic-Bytes
      → Einmaligkeitssperre dazu: Zwei Fenster über derselben
        Kontaktdatei schrieben beide, und der Letzte gewänne
      → **der Fall, um den es geht, ist der gesperrte.** Wer doppelklickt,
        hat das Fenster meist nicht offen. Der Pfad wartet, der
        Sperrbildschirm meldet **dass** etwas wartet — den Namen nennt er
        nicht: `Kuendigung_Meyer.pdf.cabrik` auf einem gesperrten
        Bildschirm verriete genau das, was er sonst zurückhält
      → ein Fehlschlag wiederholt sich nicht: Der Pfad wird **vor** dem
        Öffnen weggenommen, sonst entstünde eine Schleife aus
        Fehlermeldungen
- [x] **Fehlerbehandlung systematisch durchgegangen**
      → **Der Hauptfund:** Drei tödliche Pfade schrieben mit `eprintln!` auf
        eine Konsole, die es nicht gibt — das Fenster läuft mit
        `windows_subsystem = "windows"`. Wer Cabrik doppelklickte und dessen
        Schlüsseldatei beschädigt war, sah **gar nichts**. Kein Fenster,
        keine Meldung. v1 stürzte wenigstens sichtbar ab
      → jetzt ein `Startfehler`-Bildschirm mit **Pfad** und **Rat**. Kein
        Meldungsfenster: Das ist eine Sackgasse. Bei einer Schlüsseldatei
        ist die Auskunft, *welche* Datei im Weg liegt, der Unterschied
        zwischen einem Rätsel und einer Aufgabe
      → **er verdrängt die Einrichtung.** Der teure Fall: Eine beschädigte
        Schlüsseldatei sieht von außen aus wie gar keine — wer daraufhin
        eine neue Identität anlegt, hat alles verloren, was an die alte
        gerichtet war
      → **Zweiter Fund — stiller Datenverlust:** `lies` unterscheidet
        sauber zwischen „gibt es nicht" (`Ok(None)`) und „nicht lesbar"
        (`Err`); das Fenster warf beides mit `.ok().flatten()` zusammen.
        Eine unlesbare Kontaktdatei ergab damit ein **leeres Verzeichnis**,
        das Entsperren gelang, alle Verifikationen schienen fort — und die
        erste Änderung schrieb die Datei nieder. Danach waren sie es
      → **Dritter Fund:** `groesse_bytes` war `.unwrap_or(0)`. Eine Datei,
        die sich nicht ansehen lässt, ist keine leere Datei — und das stand
        auf dem Löschbildschirm. Jetzt `Option`, Anzeige „Größe unbekannt"
      → geht das Fenster selbst nicht auf (fehlende WebView2-Laufzeit), gibt
        es ein natives Meldungsfenster. Der einzige Fall, für den es das
        Richtige ist: Alles andere lässt sich *im* Fenster sagen
      → **Was geprüft und für gut befunden wurde:** Panics sind
        werkstattweit `deny` (`unwrap_used`, `expect_used`, `panic`,
        `indexing_slicing`, `arithmetic_side_effects`) — die v1-Klasse
        „Traceback" ist strukturell ausgeschlossen. Alle sechs Halter der
        Oberfläche zeigen ihren `fehler`. Die verbleibenden `.ok()` in
        `cabrik-metadata` sind Absicht: Ein kaputtes Segment wird
        übersprungen, und `understood: false` trägt die Ehrlichkeit

---

## Phase 4d — Das Handy als Scanner

*Verschoben, nicht verworfen. Entscheidung steht aus.*

Das Erzeugen des QR-Codes ist fertig; das **Abscannen** fehlt, und dafür
braucht es eine Kamera. Ein Entwicklungsrechner ohne Webcam ist kein
Sonderfall, sondern der Normalfall bei Standrechnern.

### Warum das eine eigene Phase ist

Weil es die **erste Stelle wäre, an der Cabrik einen Port öffnet**. Bisher
gibt das Programm nichts ans Netz — kein Server, keine Konten, keine Daten.
Das ist die beste denkbare Ausgangslage, und sie preiszugeben verlangt eine
eigene Entscheidung mit eigener Spezifikation, nicht einen Knopf nebenbei.

### Die drei Wege

| Weg | Was er verlangt | Warum er ausscheidet oder nicht |
|---|---|---|
| **Abtippen** — Handy scannt, Mensch überträgt | nichts | 2070 Zeichen. Unzumutbar, fällt aus |
| **Handy als Webcam über USB** (DroidCam, Iriun) | fremdes Programm auf beiden Seiten | Bild bleibt lokal über USB; für uns danach eine gewöhnliche Kamera. Sauber, aber wir hängen an fremder Software |
| **Richtung umdrehen: Handy sendet an PC** | lokaler Webserver im Fenster | Kein Programm auf dem Handy, keine Kamera am PC, nichts verlässt das Netz — aber ein offener Port |

Herstellergebundene Verfahren (Windows Phone Link, Apple Continuity Camera,
Intel Unison) scheiden aus: Sie verlangen dasselbe Konto oder denselben
Hersteller auf beiden Seiten, und der Bildstrom liefe über fremde
Infrastruktur. Für ein Programm, das Vertraulichkeit verspricht, ist das die
falsche Abhängigkeit.

### Der dritte Weg im Einzelnen

Der PC zeigt einen **kleinen** QR-Code mit einer lokalen Adresse
(`http://192.168.x.x:PORT/…`) plus einem Einmalgeheimnis. Das Handy scannt
den — winzig, sofort gelesen —, öffnet die Seite, scannt dort mit der
eigenen Kamera den echten Kontakt-Code und schickt die Nutzlast über das
lokale Netz zurück.

Der kleine Code ist der Trick: Das große Ding, das 141 Module braucht, muss
gar nicht vom PC gezeigt werden.

### Was vorher zu klären ist

- **TLS oder nicht.** Ohne TLS liest jeder im selben WLAN mit. Mit TLS
  braucht es ein Zertifikat für eine wechselnde lokale Adresse — und das
  Handy zeigt eine Warnung, die man wegklicken muss. Wer lernt, solche
  Warnungen wegzuklicken, hat mehr verloren als gewonnen
- **Bindung an das Einmalgeheimnis**, Zeitfenster, ein einziger Versuch
- **Nur an die lokale Schnittstelle binden**, nie an `0.0.0.0`
- **Was die Firewall meldet.** Windows fragt beim ersten Öffnen nach — bei
  einem Verschlüsselungsprogramm ist das ein Schreckmoment, der erklärt
  gehört, bevor er eintritt
- **Was überhaupt über die Leitung geht.** Nur die Austausch-Nutzlast, und
  die ist öffentlich. Aber die Regel „Schlüsselmaterial bleibt in Rust"
  bekäme eine Nachbarin, über die nachzudenken ist
- **Ob die Richtung des Vertrauens stimmt.** Die Nutzlast ist öffentlich,
  aber wer sie einschleust, bestimmt, mit wem man spricht. Ein zweites
  Gerät auf dem Weg ist ein zweiter Ort, an dem das schiefgehen kann

### Wann es sich lohnt

Erst, wenn jemand den Fall wirklich hat: zwei Menschen im selben Raum, die
sich verifizieren wollen. Dafür tut es heute die **Safety Number** — vorlesen
und vergleichen, ohne Netz und ohne Kamera. Der Datei- und der Textweg decken
alles andere ab.

---

## Phase 5 — Produktreife

*Der Übergang vom funktionierenden Programm zum ausliefer­baren Produkt.*

### Das Ordnungsprinzip

**Was unumkehrbar ist, kommt zuerst — solange es noch billig ist.**

Drei Dinge in dieser Phase lassen sich später nicht mehr zurücknehmen: Ein
veröffentlichtes Repository trägt seine Geschichte für immer. Ein Envelope,
der bei jemandem liegt, muss in zehn Jahren noch aufgehen. Und ein Name auf
einem signierten Programm ist teuer zu ändern.

Alles andere — CI, Installer, Website — lässt sich beliebig oft neu machen.
Deshalb steht es hinten, obwohl es nach mehr Arbeit aussieht.

---

### 5.0 Bevor irgendetwas nach außen geht

*Nichts davon ist Programmierarbeit. Alles davon ist danach teuer.*

- [x] **Markenrecherche „Cabrik"** — DPMA und EUIPO, Klasse 9 und 42.
      Ergebnis: **frei**. „Cabrik", „CabrikSecure" und das Zeichen sind
      EU-rechtlich nutzbar
      → **Recherche und Eintragung sind zweierlei**, und nur die erste
        gehört hierher. Die Recherche ist kostenlos und schützt vor dem
        teuren Fehler: eine Marke aufzubauen, die jemand anderem gehört.
        Die Eintragung kostet (DPMA 290 €, EUIPO ab 850 €, mit
        anwaltlicher Begleitung rund 1200 €) und schützt davor, dass
        jemand sie einem wegnimmt — das lohnt erst, wenn es etwas zu
        schützen gibt
      → Die **Eintragung** steht deshalb in 5.3a, nach dem Code Signing
- [x] **Lizenzen je Kiste aufgeteilt** — Apache-2.0 für `cabrik-core`,
      `-metadata`, `-shred`, `-ablage`; die übrigen fünf proprietär
      → **Die Falle war schon scharf:** `[workspace.package]` trug
        `license = "Apache-2.0"`, und das erbten **alle neun** — auch
        Fenster, Brücke, CLI und der v1-Leser. Eine Lizenzangabe ist eine
        Zusicherung an jeden, der den Quelltext bekommt, und sie ist nicht
        zurückzunehmen: Wer eine Fassung unter Apache-2.0 erhalten hat,
        darf sie weiter so nutzen
      → geprüft, dass die offenen einen **geschlossenen Teilgraphen**
        bilden: `-metadata` und `-shred` hängen an `-core`, `-ablage` an
        nichts, keine an einer proprietären. Sie lassen sich für sich
        bauen — die Bedingung dafür, dass die Sicherheitsaussagen
        überprüfbar werden.

        Es waren damals vier und ist seit `cabrik-speicher` (5.2) eine
        mehr. Genau daran fiel auf, dass eine **Handprüfung** hier der
        falsche Ort ist: Sie müsste bei jeder neuen Kiste wiederholt
        werden und wird es irgendwann nicht. Sie steht deshalb jetzt als
        Test in `crates/cabrik-speicher/tests/gleichlauf.rs`
      → die proprietären tragen `LicenseRef-Cabrik-Proprietary` und
        `publish = false`. SPDX kennt keinen Bezeichner für „proprietär";
        `LicenseRef-` ist die dafür vorgesehene Form
      → `[licenses.private] ignore = true` in `deny.toml` — greift **nur**
        bei `publish = false`. Gegengeprüft: Fehlt das Merkmal, schlägt
        `cargo deny` wieder fehl
      → der Lizenztext ist **nicht aus dem Gedächtnis** geschrieben,
        sondern aus dem Cargo-Registry übernommen und gegen 147
        übereinstimmende Kopien abgeglichen
- [x] **`SECURITY.md`** — Meldeweg, Fristen, und was ausdrücklich **keine**
      Lücke ist (Envelope-Größe, anonymer Versand, „keine Aussage" bei
      unbekanntem Format, unwiederbringliches Passwort)
      → **Meldeadresse ist ein Platzhalter.** Eine erfundene Adresse an
        einer fremden Domain machte die Meldung unmöglich — das ist eine
        Entscheidung des Betreibers. Solange der Platzhalter steht, ist
        das Repository nicht veröffentlichungsreif
- [x] **README für ein öffentliches Repository** — Stand, was offen ist und
      warum, die Spezifikationen, Bauanleitung
      → geprüft: jeder Verweis darin zeigt auf eine Datei, die es gibt
- [ ] **Repo-Hygiene.** *Geprüft am 16.08.2026: Die Historie enthält kein
      Schlüsselmaterial.* `probe/` (enthält echte Testschlüssel),
      `ich.contact` und die beiden `info_Cabrik_Secure*.txt` sind
      ungetrackt und müssen es bleiben; `.gitignore` deckt `*.key`,
      `*.pem`, `*.cabrik-key` ab. `legacy/python-v1` ist bewusst dabei —
      v1 ohne Schlüssel, als Beleg dafür, wovon dieses Projekt ausgeht
- [x] **Alle Spezifikationen von „Entwurf" auf verbindlich.** Wer
      `spec/envelope-v2.md` aufschlägt und „Entwurf" liest, weiß nicht,
      woran er ist — bei einem Dateiformat, das jahrelang lesbar bleiben
      muss, ist das die entscheidende Frage
- [x] **Formatfreeze Envelope v2 + Keyfile v2** — mit der Zusage
      schriftlich in beiden Dokumenten: **lesen immer, schreiben nur in
      der eingefrorenen Fassung**
      → **ein Wächter dazu:** `crates/cabrik-core/tests/formatfreeze.rs`
        nagelt fest, was eine fremde Umsetzung aus der Spezifikation
        abliest — Magic Bytes, Suite-Kennungen, Blockgröße, Rahmenzeilen,
        Empfängergrenze, das Präfix der Austausch-Nutzlast, die
        Byteanordnung des Prologs
      → die Vektortests prüfen **Verhalten** gegen Vorlagen; dieser prüft
        die **Zahlen selbst**. Ändert jemand `SUITE_HYBRID`, bricht
        womöglich kein Vektortest — aber jede Datei auf der Welt.
        Gegengeprüft: Eine heimlich geänderte Suite-Kennung lässt zwei
        Tests fehlschlagen
      → `dieselbe_eingabe_ergibt_denselben_envelope` trägt den Rest: Ohne
        Determinismus bei fester Quelle wäre jede Aussage über die
        Byteanordnung Zufall
      → **Falle beim Bauen:** Der Test braucht die deterministische
        Zufallsquelle hinter dem Merkmal `testing`, und `cargo test
        --workspace` hätte sie nicht gehabt. Gelöst über eine
        dev-Abhängigkeit der Kiste auf sich selbst — sonst wäre die CI an
        etwas gescheitert, dessen Grund niemand gesehen hätte

---

### 5.1 CI — macht alles Weitere erst überprüfbar

*Kommt vor allem anderen Technischen, weil ohne sie jede spätere Aussage
nur so gut ist wie die Sorgfalt des Tages.*

- [x] **`.github/workflows/pruefung.yml`** — drei Plattformen, vier
      Aufgaben: Rust (Windows/macOS/Linux), Oberfläche, Abhängigkeiten
      → **Erwartung beim ersten Lauf: Das findet etwas.** Dieser Quelltext
        wurde nie auf macOS oder Linux übersetzt. `cabrik-shred` fasst
        Dateisysteme an, `cabrik-ablage` Konfigurationspfade
      → vorab geprüft: `erkenne_faehigkeit` hat einen
        `cfg(not(target_os = "linux"))`-Zweig, macOS übersetzt also. Die
        Linux-Systempakete für WebKit und GTK3 stehen im Ablauf; GTK3 zieht
        `rfd` über `tauri-plugin-dialog` herein
      → `cargo build -p cabrik-fenster` **braucht das Frontend-Ergebnis
        nicht** — nachgeprüft mit weggenommenem `dist/` und leerem
        Zwischenspeicher. Eine Falle weniger im ersten Lauf
      → `npm run pruefung` und **nicht** `npx svelte-check`: Ohne
        `--tsconfig ./tsconfig.app.json` bleiben die Testdateien
        ungeprüft. Genau so sind hier schon Typfehler durchgerutscht
      → `permissions: contents: read`. Ein Arbeitsablauf mit
        Schreibrechten ist bei einem Verschlüsselungsprogramm ein
        Angriffsziel
- [x] **Werkzeugkette an je einer Stelle.** Die Rust-Fassung steht in
      `rust-toolchain.toml`, die Node-Fassung in `app/oberflaeche/.nvmrc`;
      die Abläufe nennen keine
      → `targets = ["x86_64-pc-windows-msvc"]` aus `rust-toolchain.toml`
        entfernt: auf Windows das Wirtsziel, auf den anderen zwei Läufern
        ein Download für nichts
      → `--locked` überall — der Lauf soll an `Cargo.lock` scheitern statt
        still eine andere Fassung zu ziehen
- [x] **`.github/workflows/fuzzing.yml`** — nachts, nicht bei jedem Push
      → Fuzzing tut zweierlei, und nur eines gehört in einen Lauf, auf den
        jemand wartet: **Neues finden** braucht Minuten bis Stunden und
        steht in der Nacht. **Gefundenes festhalten** kostet nichts — die
        Korpus-Tests unter `testvectors/fuzz/` laufen bei jedem
        `cargo test` mit
      → ein Fund ist der Zweck dieses Ablaufs, nicht sein Fehlschlag: Die
        Fundstücke werden als Artefakt gesichert
- [x] **`pruefung.ps1`** — dasselbe Tor lokal
      → weil die CI erst läuft, wenn dieses Repository einen Remote hat.
        Bis dahin wäre sie ein Versprechen ohne Deckung
      → `$ErrorActionPreference` steht auf `Continue`: Cargo schreibt
        seinen Fortschritt auf die Fehlerausgabe, und PowerShell 5.1 macht
        daraus einen `NativeCommandError`. Mit `Stop` bräche der Lauf beim
        ersten „Checking …" ab, obwohl nichts schiefging
      → UTF-8 **mit** BOM, sonst liest PowerShell 5.1 die Umlaute als ANSI
      → gegengeprüft: Ein eingebauter Typfehler wird gemeldet,
        Rückgabewert 1
- [x] **Linux durchgelaufen** — Ubuntu 24.04 unter WSL2, frischer Klon
      → **grün beim ersten Anlauf**: Clippy, alle Rust-Tests, 409
        Frontend-Tests. Auf einer Plattform, auf der dieser Quelltext nie
        übersetzt wurde
      → in einem **Klon im Linux-Dateisystem**, nicht unter `/mnt/c`: Ein
        `npm ci` dort legte Linux-Binärdateien in die `node_modules` der
        Windows-Seite, und `cargo` schriebe in dasselbe `target/debug`
      → die Paketliste aus `pruefung.yml` stimmt: WebKit 2.52, GTK 3.24
- [ ] **Erster CI-Lauf, sobald es einen Remote gibt.** macOS ist damit
      weiterhin ungeprüft — Linux ist es nicht mehr

---

### 5.1b Der Fund aus dem ersten Fuzzing-Lauf — behoben

**Ein präpariertes Envelope konnte den Rechner des Empfängers für
Jahrhunderte beschäftigen.**

Das nächtliche Ziel `envelope_open_passwort` lief in einen Zeitüberlauf.
Die auslösende Datei liegt als `testvectors/fuzz/envelope/timeout_t_cost.env`
im Korpus und trägt:

| Feld | Wert |
|---|---|
| `m_cost` | 299 775 KiB ≈ 293 MiB — **innerhalb der Grenzen** |
| `t_cost` | 4 294 901 763 Durchgänge |

**Die Lücke war in der Spezifikation, nicht nur im Code.**
`spec/keyfile-v2.md` §4 nannte eine Obergrenze allein für `m_cost` und
begründete sie mit dem *Speicher*überlauf. Dass `t_cost` ein `u32` ist und
die **Zeit** ebenso unbegrenzt war, hatte niemand gesehen — der Kommentar
im Quelltext wiederholte dieselbe blinde Stelle wörtlich.

**Warum es zählt:** Die Parameter stehen im Kopf des Envelopes, also in
einer Datei, die der **Absender** wählt — und den behandelt das Threat
Model ausdrücklich als nicht vertrauenswürdig. Ein Empfänger hätte ein
Programm vor sich, das nie zurückkehrt: kein Fehler, keine Meldung, nur
ein Fortschrittstext, der für immer stehen bleibt.

- [x] `T_COST_MAX = 16`. RFC 9106 empfiehlt einen Durchgang bei hohem
      Speicher und drei bei mittlerem; Cabrik schreibt drei. Sechzehn lässt
      mehr als das Fünffache Luft und begrenzt den ungünstigsten erlaubten
      Fall auf Minuten
- [x] `spec/keyfile-v2.md` §4.1 — **als Nachtrag an einem eingefrorenen
      Dokument gekennzeichnet**, mit der Begründung, warum er nichts
      bricht: Cabrik schreibt `t_cost = 3`, zurückgewiesen wird nur, was
      nie hätte entstehen dürfen
- [x] Die auslösende Datei liegt im Korpus — **vor** der Behebung, wie der
      Ablauf es vorschreibt. Der Korpus-Test ist damit der Regressionstest
- [x] Gegengeprüft: Ohne die Grenze bricht der Korpus-Test nach 90 s ab
      (Rückgabe 124, kein Ergebnis); mit ihr läuft er in 7 s durch
- [x] Dazu ein Test auf die **Regel** statt auf die Wirkung. Ohne ihn
      ließe sich der Korpus-Test durch Anheben der Grenze grün bekommen
- [x] `p_cost` bleibt ohne Obergrenze, und das ist Absicht: Es ist ein
      `u8` und verteilt die Arbeit, statt sie zu vermehren

---

### 5.1a Der Fund aus dem Linux-Lauf — behoben

**`cabrik_shred::assess` verspricht in virtuellen Maschinen zu viel.**

Der Linux-Zweig liest `/sys/dev/block/…/queue/rotational` und schließt bei
`1` auf eine rotierende Platte — dort wirkt Überschreiben tatsächlich, und
`ShredCapability::Overwrite` sagt das zu. Gemessen auf diesem Rechner:

| | |
|---|---|
| sysfs meldet | `rotational = 1` |
| `systemd-detect-virt` | `wsl` |
| tatsächliche Hardware | WD Blue **SSD**, Samsung 970 EVO Plus **NVMe** |

In der Maschine steckt keine einzige rotierende Platte. Der virtuelle
Datenträger meldet trotzdem `1`, weil der Hypervisor das Merkmal nicht
durchreicht — und Cabrik behauptet daraufhin eine Wirkung, die es nicht
gibt.

**Das ist der v1-Fehler in anderem Gewand.** Dort wurde eine unverstandene
Datei kopiert und als bereinigt gemeldet; hier wird eine unbekannte
Speicherschicht als rotierende Platte gemeldet. Beides ist eine Zusicherung
ohne Deckung, und beim Löschen wiegt sie schwerer: Wer sie glaubt, hält
Daten für vernichtet, die auf der SSD des Wirts weiterliegen.

**Der Umfang ist groß.** Es betrifft jede virtualisierte Linux-Umgebung —
WSL2, VirtualBox, VMware, Hyper-V, Proxmox und jeden Server in der Wolke.
Auf echter Hardware ist die Erkennung richtig.

**Behoben.** Auf derselben Maschine nachgewiesen, auf der der Fehler
gemessen wurde: vorher `Overwrite`, jetzt `BestEffort` samt dem Satz
„Dieses System läuft virtualisiert (Windows-Subsystem für Linux) — was
unter dem virtuellen Datenträger liegt, ist von hier aus nicht
feststellbar".

- [x] **Die Regel ist eine eigene reine Funktion.** `entscheide(rotierend,
      copy_on_write, virtualisiert)` — damit auf **jeder** Plattform
      prüfbar, auch unter Windows, wo sich der gemeldete Fall gar nicht
      herstellen ließe. Ein Test, der eine rotierende Platte in einer
      virtuellen Maschine braucht, liefe sonst nirgends, und die Regel
      bliebe genau in dem Fall ungeprüft, der schiefging
- [x] **Erkennung ohne fremde Programme** — `osrelease`, DMI-Herstellername,
      Prozessormerkmal. Der **Name** zuerst: „VirtualBox" ist für einen
      Menschen brauchbarer als „ein Hypervisor ist vorhanden".
      `systemd-detect-virt` scheidet aus, weil es vielerorts fehlt und ein
      Prozessaufruf mit geerbter Umgebung hier nichts zu suchen hat
- [x] **Ein eigener Vorbehalt**, keine stille Abstufung. Wer nur
      `BestEffort` sieht, hält es für die übliche SSD-Einschränkung und
      lernt nichts über seine Lage
- [x] **Container ausdrücklich ausgenommen.** Docker und LXC teilen sich
      den Kern mit dem Wirt und sehen dessen echte Datenträger — die
      Angabe stimmt dort. Sie gleich zu behandeln hieße, vor etwas zu
      warnen, das nicht vorliegt; und jede Warnung, die zu oft erscheint,
      entwertet die übrigen
- [x] `spec/shredding.md` §4.2a
- [x] Windows und macOS geprüft: Beide sagen ohnehin nie `Overwrite` zu
      und sind nicht betroffen — ein Vorbehalt dort wäre eine Warnung ohne
      Folge

---

### 5.2 Die Lücken, die vor einer Veröffentlichung nicht offen bleiben dürfen

- [x] **v1-Schlüssel im Fenster einlesen.** `cabrik-v1` hängt heute nur an
      der CLI. Wer die ausgelieferte v1.exe benutzt hat, kann seinen
      Schlüssel im Fenster **nicht** übernehmen — und käme an nichts mehr
      heran, was an ihn gerichtet wurde. Für bestehende Nutzer ist das ein
      Auslieferungshindernis, kein Komfortmangel
- [x] **`VirtualLock`/`mlock` — der festgenagelte Puffer.**
      `crates/cabrik-speicher`, die einzige Kiste mit `unsafe`. Beim
      Bauen fiel auf, dass `entsperrung.md` §5.3 zu weit ging: Festnageln
      hilft gegen Auslagerung, gegen den Ruhezustand **nicht** — dort ist
      es sogar das Gegenteil
- [x] **Das Passwort kommt als Bytes in die Befehlsschicht.** Die
      Entwurfsauflage aus §5.2, und bis dahin nur eine Absicht: Solange
      die Signaturen `&Zeroizing<String>` verlangten, hätte ein
      festgenagelter Puffer vorher eine ungeschützte Kopie anlegen müssen
- [x] **Sperren vor Bereitschaft und Ruhezustand** (`entsperrung.md`
      §3.4, dazugekommen aus der Berichtigung oben)
      → [x] Windows, über `PowerRegisterSuspendResumeNotification`. Der
        Weg über tao schied aus: `Event::Suspended` löst dort nie aus, und
        den Nachrichtenhaken belegt Tauri selbst
      → [x] Linux, über `PrepareForSleep` von logind. Der einzige Weg
        ganz ohne `unsafe` — die Zahl der Aufhebungen blieb bei fünfzehn.
        Mit Verzögerungssperre (`Inhibit`, Modus `delay`), sonst bliebe
        keine Zeit zum Überschreiben; sie wird erst losgelassen, wenn
        überschrieben ist
        → **aber nie im Betrieb gesehen.** Übersetzt wird der Zweig hier
          über `cargo clippy --target`, und die Anmeldung läuft in der
          Fortlaufprüfung gegen ein echtes logind. Dass die Meldung
          ankommt, müsste ein einschlafender Linux-Rechner zeigen — den
          gibt es nicht. Bleibt offen, bis es einen gibt
      → [x] macOS, über `IORegisterForSystemPower`. Der aufwendigste
        der drei — IOKit stellt über eine `CFRunLoop` zu, es braucht also
        einen eigenen Faden mit einer solchen Schleife. Dafür ist der
        Aufschub hier eingebaut statt zu erbitten: Das System wartet auf
        `IOAllowPowerChange`
        → Die Konstanten kommen aus Apples Kopfdateien, vorgelesen vom
          macOS-Läufer. Der Schritt in `pruefung.yml` ist inzwischen ein
          **Vergleich**: Verschiebt Apple eine, wird der Lauf rot — nicht
          irgendwann jemandes Mac beim Zuklappen
        → `kIOMessageCanSystemSleep` ist eine FRAGE, keine Ankündigung.
          Wer darauf sperrte, sperrte bei jeder Kaffeepause
        → **Nie im Betrieb gesehen**, wie unter Linux: Dass die Meldung
          ankommt, müsste ein einschlafender Mac zeigen
      → [x] die Anzeige, dass es auf diesem System **nicht** greift.
        Drei Fälle statt eines Wahrheitswerts: mit Aufschub, ohne
        Aufschub, gar nicht. Zwischen „das System wartet darauf" und
        „niemand steht für die Zeit gerade" liegt der Unterschied zwischen
        einer Zusage und einer Hoffnung
        → Sie schweigt im günstigen Fall. Ein Programm, das seine
          funktionierenden Schutzmaßnahmen aufzählt, erzieht dazu, den
          Kasten zu überlesen — und dann fällt auch der Fall nicht auf, in
          dem etwas fehlt
        → Gelb, nicht rot: Hier ist nichts gescheitert, es ist eine
          Eigenschaft des Rechners. Rot drängte zu einer Handlung, die es
          nicht gibt
- [ ] **Natives Passwortfenster.** `spec/entsperrung.md` §5.2 sagt es zu
      und §11 führt es als Ziel für Phase 5. Eine veröffentlichte
      Spezifikation, die etwas zusagt, was das Programm nicht tut, ist
      schlimmer als keine
      → **Umfang, nachgemessen:** Die Passwortfelder der Systeme selbst
        (`EDIT` mit `ES_PASSWORD`, `NSSecureTextField`) halten den Text in
        ihrem eigenen Puffer, den wir weder festnageln noch überschreiben
        können. Es braucht je System ein eigenes Steuerelement, das die
        Tasten selbst annimmt — deutlich mehr als beim Festnageln
- [x] **Fortschritt innerhalb einer großen Datei.** Der Balken zählt
      Dateien, nicht Bytes: Eine einzelne 3-GB-Datei steht bei „0 von 1"
      und rührt sich minutenlang nicht
      → [x] Im Kern: `stream::seal_into_gemeldet` meldet nach jedem Block
        die erledigten **Klartextbytes**. Ein Test hält fest, dass die
        Meldung am Ergebnis nichts ändert — ein Formatbruch für eine
        Anzeige wäre absurd, und der Envelope ist eingefroren
      → [x] Denselben Weg für `stream::open`. Mit einem Unterschied:
        Gemeldet wird erst, wenn der Block **beglaubigt** ist. Ein Balken,
        der bis kurz vors Ende läuft und dann „gefälscht" meldet, hätte
        die ganze Zeit etwas angezeigt, das nie galt
      → [x] **Zuerst die Phase, dann die Bytes** — nach einer Abschätzung
        umgestellt. ChaCha20-Poly1305 läuft in der Größenordnung mehrerer
        hundert MB/s; Lesen und Schreiben liegen bei einer 3-GB-Datei in
        derselben Größenordnung oder darüber. Der Kryptoschritt ist also
        einer von vier vergleichbar teuren, nicht der dominante — Bytes
        nur dort ließen den Balken dreimal stehenbleiben. Die Phase
        erklärt dagegen **jeden** Stillstand
      → [x] Durchreichen der Bytes: Envelope → `cabrik-app` → Fenster →
        Brücke. Hier stand, der Rückruf gehöre in `SealOptions` — beim
        Bauen erwies sich das als falsch: Ein Rückruf ist keine **Option**
        des Envelopes, sondern ein Ausgabekanal, und er bräuchte eine
        veränderliche Referenz. `seal` müsste die Optionen als `&mut`
        nehmen, also die Erlaubnis bekommen, **alle** zu verändern, um
        eine Zahl herauszugeben. Ein achter Wert mit klarem Typ ist
        dagegen das kleinere Übel
      → [x] **Lesen** in Blöcken, mit gedrosselter Byte-Meldung. Ohne
        Drosselung kämen bei 3 GB rund 48 000 Nachrichten über die Brücke
        — ein Balken, der die Anzeige lahmlegt, ist schlechter als keiner.
        Die letzte Meldung geht immer durch, sonst bliebe er kurz vor dem
        Ende stehen
      → [x] **Schreiben** in Blöcken, über `cabrik-ablage`. Die
        Drosselung sitzt im Fenster und nicht dort: Jene Schicht weiß
        nicht, wohin die Meldung geht, und zwänge sonst jedem Aufrufer
        denselben Abstand auf
        → Erst leeren, **dann** als fertig melden. Andersherum stünde der
          Balken auf voll, während die Puffer noch auf die Platte gehen —
          und bei einem Fehler dabei hätte er eine Vollendung gemeldet,
          die es nie gab
      → [ ] **Und die Phase, nicht nur die Bytes.** Hier stand einmal, im
        Kern „fehle nur die Meldung". Das war zu optimistisch: Pro Datei
        laufen VIER langsame Schritte — Lesen, Bereinigen, Verschlüsseln,
        Schreiben. Nur im dritten zu zählen ließe den Balken beim Lesen
        einer 3-GB-Datei weiter stillstehen, also genau dort, wo die
        Klage herkommt
      → [ ] **Getrennt davon, aber hier aufgefallen:** Die ganze Datei
        liegt im Arbeitsspeicher — `fs::read`, dann die bereinigte
        Fassung, dann der Envelope. Bei 3 GB sind das mehrere Gigabyte
        gleichzeitig. Ein Fortschrittsbalken behebt das nicht; echtes
        Strömen von der Platte wäre ein eigener Punkt
- [ ] **Die `.cabrik`-Zuordnung am echten Installer prüfen.** Sie steht
      heute in `tauri.conf.json` und ist nie gegen ein gebautes MSI/NSIS
      gelaufen. Eine Zuordnung, die nur in einer Konfigurationsdatei
      existiert, ist keine

---

### 5.3 Auslieferbarkeit

- [ ] **Code Signing.** Azure Trusted Signing, ~10 $/Monat
      → **Vorlauf einplanen:** Die Identitätsprüfung dauert Tage bis
        Wochen. Sie gehört angestoßen, sobald die Namensfrage entschieden
        ist
      → ohne Signatur blockt SmartScreen den Installer, und bei einem
        Verschlüsselungsprogramm installiert das niemand. Das ist der eine
        Posten, der Auslieferung schlicht verhindert
- [ ] **Installer bauen und auf einem frischen Windows prüfen** — ohne
      WebView2, ohne Rust, ohne Node. Der Startfehler-Bildschirm und das
      Meldungsfenster bei fehlender WebView2-Laufzeit sind dort das erste
      Mal echt
      → **und das Symbol.** Im `target\debug` bleibt es unscharf: Der Pfad
        wird bei jedem Bau überschrieben, und Windows' Symbolspeicher
        kommt damit nicht mit. Nachgewiesen ist, dass alle fünfzehn
        Stufen im Programm stecken und die Datei jede angefragte Größe als
        echte Stufe liefert — ob es beim Nutzer scharf ist, entscheidet
        sich erst am installierten Programm an festem Pfad
- [ ] **Nachvollziehbare Builds — und zwar unter dem richtigen Namen.**
      Bit-genau reproduzierbare Builds über Rust *und* Node *und* Tauri
      hinweg sind Forschungsstand, nicht Handwerk; das zuzusagen wäre
      genau die Sorte Versprechen, die dieses Projekt sonst vermeidet.
      Erreichbar und ehrlich ist:
      → festgenagelte Werkzeugkette, `Cargo.lock` und `package-lock.json`
        eingecheckt, `--locked` überall
      → eine dokumentierte Bauumgebung (Container), sodass **dieselbe**
        Umgebung dasselbe Ergebnis liefert
      → veröffentlichte Prüfsummen der CI-Artefakte, damit jeder abgleichen
        kann, dass sein Download dem entspricht, was die CI gebaut hat
      → **kein** Versprechen, dass ein fremder Rechner dieselben Bytes
        erzeugt
- [ ] macOS: Notarisierung (99 $/Jahr) — erst wenn macOS wirklich beliefert
      wird
- [ ] Store-Vertrieb: Verschlüsselungs-Deklaration
      (`ITSAppUsesNonExemptEncryption`), US-Selbstklassifizierung als
      Mass-Market (ECCN 5D992), separate Erklärung für Frankreich

---

### 5.3a Markeneintragung — erst wenn es etwas auszuliefern gibt

Die Recherche ist erledigt und der Name frei (5.0). Die **Eintragung**
steht hier und nicht dort, aus zwei Gründen:

- **Der Benutzungszwang.** Eine deutsche oder EU-Marke muss innerhalb von
  fünf Jahren ernsthaft benutzt werden, sonst wird sie wegen Nichtbenutzung
  angreifbar. Jetzt einzutragen hieße, die Uhr zu starten, während das
  Produkt noch nicht ausgeliefert wird.
- **Sie schützt gegen ein Problem, das es noch nicht gibt.** Solange
  niemand Cabrik kennt, will auch niemand den Namen.

Sinnvoll wird sie mit dem Code Signing: Ab dann steht der Name im
Zertifikat, im Installer, in der Dateizuordnung und in jeder
heruntergeladenen Kopie.

- [ ] DPMA (290 € für drei Klassen, elektronisch) oder EUIPO (ab 850 €)
- [ ] Klasse 9 (Software) und Klasse 42 (IT-Dienstleistungen)
- [ ] Vor der Anmeldung einem Fachanwalt vorlegen — an dieser Stelle ist
      das Geld gut angelegt, bei der bloßen Recherche war es das nicht

---

### 5.4 Updater — eine Entscheidung, kein Häkchen

**Der Signierschlüssel des Updaters wird der wertvollste Schlüssel des
ganzen Projekts.** Wer ihn hat, kann jedem Nutzer eines
Verschlüsselungsprogramms beliebigen Code schicken — an allem vorbei, was
dieses Programm sonst richtig macht. Ein Schlüssel auf einem
Entwicklungsrechner wäre dann die schwächste Stelle des Systems.

Drei Wege, und sie sind nicht gleichwertig:

| Weg | Was er kostet | Was er einbringt |
|---|---|---|
| **Kein Updater.** Das Programm sagt, dass es eine neue Fassung gibt, und verlinkt sie | Nutzer müssen selbst handeln | Kein Schlüssel, keine Angriffsfläche. Für ein Offline-Werkzeug vertretbar |
| **Signierter Updater, Schlüssel offline** (Hardware-Token, getrennt vom Baurechner) | Umständlich bei jeder Veröffentlichung | Bequemlichkeit ohne den offensichtlichen Angriff |
| **Signierter Updater, Schlüssel in der CI** | nichts | Wer die CI übernimmt, übernimmt alle Installationen |

Der dritte Weg fällt aus. Zwischen den ersten beiden ist zu entscheiden —
**bevor** gebaut wird, denn ein nachträglich eingeführter Updater braucht
eine ausgelieferte Fassung, die ihn schon kennt.

- [ ] Entscheidung treffen und in `spec/threat-model.md` aufnehmen
- [ ] Erst danach umsetzen

---

### 5.5 Dokumentation und Website

- [ ] Website: statisch, **ohne Krypto im Browser**, mit den Prüfsummen aus
      5.3. Eine Verschlüsselungsseite, die selbst verschlüsselt, wirft die
      Frage auf, warum es dann das Programm braucht
- [ ] Handbuch: die vier Wege (senden, empfangen, Kontakt aufnehmen,
      verifizieren) und **was Cabrik nicht kann** — dieselbe Ehrlichkeit
      wie auf den Bildschirmen
- [ ] Die Spezifikationen mitveröffentlichen. Sie sind der Grund, warum
      jemand diesem Programm glauben sollte
- [ ] Keine Telemetrie, keine Absturzberichte, keine Konten — als
      **festgehaltene Entscheidung**, nicht als Auslassung

---

### 5.6 Audit — zuletzt, wenn überhaupt

- [ ] Kleiner Umfang: 5.000–15.000 €
- [ ] Sinnvoll erst, wenn Format eingefroren, Kern quelloffen und
      Threat Model verbindlich ist. Vorher prüft ein Auditor einen
      Zustand, den es nächste Woche nicht mehr gibt

---

### Reihenfolge in einem Satz

**5.0 entscheiden → 5.1 CI → 5.2 Lücken → 5.3 signieren und ausliefern →
5.4 Updater → 5.5 dokumentieren → 5.6 prüfen lassen.**

Die einzige Verschränkung: Das Code Signing aus 5.3 hat Vorlauf und wird
angestoßen, sobald die Namensfrage aus 5.0 entschieden ist.

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

- [x] **Lizenzmodell** — entschieden und umgesetzt in 5.0. Apache-2.0 für
      `cabrik-core`, `-metadata`, `-shred`, `-ablage`; die übrigen fünf
      proprietär. Die CLI blieb entgegen dieser frühen Empfehlung
      geschlossen: Sie ist Bedienoberfläche, nicht Sicherheitsaussage.
- [x] **Markenrecherche** — erledigt in 5.0, Ergebnis frei. Die
      **Eintragung** steht in 5.3a; sie ist etwas anderes und hat Zeit.
- [ ] **Transport-Layer ja/nein.** Nicht jetzt, aber die Spec darf ihn nicht
      unmöglich machen.

---

## Nächster Schritt

Phase 0: Git-Repository anlegen, Repo aufräumen, Python v1 nach `legacy/` sichern.
