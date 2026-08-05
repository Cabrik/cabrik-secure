# Cabrik Secure — Testvektoren und Konformität

**Status:** Entwurf · Phase 1, Dokument 2 von 7
**Gilt für:** v2.0

Legt fest, wie Implementierungen auf Übereinstimmung geprüft werden. Steht
**vor** dem Formatdokument, weil Bit-Genauigkeit eine Anforderung an die
Architektur ist, nicht an die Tests. Wird sie hier nicht festgeschrieben, ist
sie später nicht mehr nachrüstbar.

---

## 1. Das Problem

Verschlüsselung ist randomisiert. Bei jedem Aufruf entstehen ein neuer
ephemerer Schlüssel, neue Nonces, neue Salts. Zwei **korrekte**
Implementierungen erzeugen aus derselben Eingabe niemals dieselben Bytes.

Die naheliegende Erwartung „Desktop, iOS und Android erzeugen bitgenau
dasselbe" ist damit in dieser Form nicht erfüllbar — und wäre auch kein
sinnvolles Ziel, denn deterministische Verschlüsselung im Produktivbetrieb
wäre ein schwerer Fehler.

Erreichbar und sinnvoll ist stattdessen: **jede Implementierung entschlüsselt
dieselben Envelopes zu denselben Bytes, und im Testmodus mit fixierter
Zufallsquelle erzeugt sie auch dieselben Envelopes.**

## 2. Die drei Konformitätsebenen

| Ebene | Deterministisch? | Prüft |
|---|---|---|
| **K1 — Entschlüsselung** | von Natur aus ja | Fester Envelope + fester Schlüssel → feste Bytes |
| **K2 — Verschlüsselung** | nur mit fixiertem RNG | Feste Eingabe + fester Zufall → fester Envelope |
| **K3 — Kreuzmatrix** | nein | Impl A verschlüsselt, Impl B entschlüsselt, alle Paare |

K1 ist der primäre Konformitätstest. K2 findet Abweichungen, die K1 übersieht —
etwa eine falsche Reihenfolge im AAD, die beim eigenen Entschlüsseln
konsistent, aber formatwidrig ist. K3 ist die Absicherung gegen Fehler in den
Vektoren selbst.

## 3. Architekturanforderung: injizierbare Zufallsquelle

**Verbindlich für jede Implementierung.**

Jede Operation, die Zufall verbraucht, bezieht ihn über eine austauschbare
Quelle — nicht über einen direkten Aufruf des Betriebssystem-RNG.

```
trait Randomness {
    fn fill(&mut self, dest: &mut [u8]);
}
```

- **Produktiv:** ausschließlich der RNG des Betriebssystems.
- **Test:** ein deterministischer Generator, aus einem Seed der Vektor-Datei
  gespeist.

Vorbild ist RFC 9180 selbst: die offiziellen HPKE-Testvektoren fixieren `ikmE`,
das Eingangsmaterial des ephemeren Schlüssels, und werden dadurch reproduzierbar.

### 3.1 Absicherung gegen Missbrauch

Der deterministische Modus ist ein Sicherheitsrisiko, wenn er versehentlich im
Produktivbetrieb landet. Verbindliche Gegenmaßnahmen:

1. Der deterministische RNG liegt hinter einem Cargo-Feature, das im
   Release-Build nicht aktiviert ist.
2. Er lebt in einem Modul mit eindeutigem Namen (`testing::DeterministicRng`),
   nie in der öffentlichen API des Kerns.
3. Ein Test prüft, dass ein Build ohne das Feature den Typ nicht enthält.

### 3.2 Reihenfolge des Zufallsverbrauchs ist normativ

Damit K2 überhaupt funktionieren kann, muss die **Reihenfolge**, in der eine
Implementierung Zufall anfordert, Teil der Spezifikation sein. Sie wird in
`envelope-v2.md` je Operation festgeschrieben. Eine Implementierung, die
dieselben Bytes in anderer Reihenfolge verbraucht, ist nicht konform.

## 4. Verzeichnisaufbau

```
testvectors/
├── README.md
├── schema/
│   └── vector.schema.json        # JSON Schema aller Vektordateien
├── hpke/
│   └── rfc9180-x25519-chacha.json   # Auszug der offiziellen RFC-Vektoren
├── keyfile/
│   ├── v1-migration.json
│   └── v2-argon2id.json
├── envelope/
│   ├── single-recipient.json
│   ├── multi-recipient.json
│   ├── password-mode.json
│   ├── streaming-multichunk.json
│   ├── anonymous.json
│   └── v1-compat.json
├── fingerprint/
│   └── safety-numbers.json
└── negative/
    ├── tampered-header.json
    ├── truncated-stream.json
    ├── wrong-recipient.json
    ├── unknown-version.json
    └── unknown-ciphersuite.json
```

## 5. Dateiformat

JSON, UTF-8, LF-Zeilenenden. Binärwerte als **Base64 ohne Padding-Toleranz**
(kanonisch, damit Vektordateien selbst bit-vergleichbar bleiben).

```json
{
  "spec_version": "2.0",
  "kind": "envelope-encrypt",
  "description": "Ein Empfaenger, signiert, Textnachricht",
  "vectors": [
    {
      "id": "enc-single-signed-001",
      "rng_seed": "AAECAwQFBgcICQoLDA0ODw==",
      "input": {
        "plaintext": "SGFsbG8gV2VsdA==",
        "recipient_enc_pub": "…",
        "sender_sig_priv": "…",
        "filename": "notiz.txt",
        "padding": "text-class"
      },
      "expected": {
        "envelope": "…",
        "envelope_sha256": "…"
      }
    }
  ]
}
```

### Feldbedeutung

| Feld | Pflicht | Bedeutung |
|---|---|---|
| `spec_version` | ja | Formatversion, gegen die geprüft wird |
| `kind` | ja | `envelope-encrypt`, `envelope-decrypt`, `keyfile`, `fingerprint`, `negative` |
| `rng_seed` | bei `*-encrypt` | Seed des deterministischen RNG |
| `input` | ja | Eingaben der Operation |
| `expected` | bei positiven Fällen | Erwartetes Ergebnis |
| `expect_error` | bei `negative` | Erwarteter Fehlerkode, siehe §7 |

`envelope_sha256` ist redundant zu `envelope`, aber praktisch: bei einer
Abweichung sieht man sofort, ob es an einem Byte oder an der Struktur liegt.

## 6. Pflichtabdeckung

Eine Implementierung gilt erst als konform, wenn sie alle folgenden Fälle
besteht.

### 6.1 Fremde Vektoren

- [ ] **RFC 9180**, Ciphersuite `DHKEM(X25519, HKDF-SHA256)` +
      `HKDF-SHA256` + `ChaCha20-Poly1305`, Modi Base und Auth.
      Prüft die HPKE-Anbindung, bevor eigene Vektoren etwas beweisen können.
- [ ] **FIPS 203** ML-KEM-768-Vektoren (Key Generation, Encapsulation,
      Decapsulation)
- [ ] **X-Wing**-Vektoren aus dem CFRG-Entwurf
- [ ] **RFC 9106** Argon2id-Vektoren
- [ ] **RFC 8032** Ed25519-Vektoren

Diese Ebene ist entscheidend: Ohne sie testet man nur die eigene
Implementierung gegen sich selbst.

**Umfang der RFC-9180-Vektoren.** Die offizielle Datei enthält alle
Kombinationen aus KEM, KDF und AEAD und ist entsprechend groß. Aufgenommen wird
**vollständig, aber nur für die eine Suite, die wir implementieren** — alle
übrigen lehnt der Leser ohnehin mit `UNSUPPORTED_SUITE` ab, ihre Vektoren
könnten also gar nichts prüfen.

Gefiltert bleiben rund 150 KB. Damit erübrigt sich jede Sonderbehandlung großer
Vektordateien: Sie werden vollständig geladen, kein Streaming, keine
Teilauswertung. Der Filterschritt wird als Skript abgelegt, damit
nachvollziehbar bleibt, was weggelassen wurde.

### 6.2 Eigene positive Vektoren

- [ ] Ein Empfänger, signiert — Suite `0x0001` **und** `0x0002`
- [ ] Ein Empfänger, anonym
- [ ] Mehrere Empfänger (3), jeder entschlüsselt erfolgreich
- [ ] Mehrere Empfänger mit Attrappen-Auffüllung
- [ ] Kapselsortierung: dieselben Empfänger in anderer Eingabereihenfolge
      ergeben einen **bitgleichen** Envelope
- [ ] `PADME` für die Beispielwerte aus `envelope-v2.md` §10.2
- [ ] Header-Padding: Dateinamen unterschiedlicher Länge ergeben identische
      `header_len`
- [ ] Safety Number für ein festes Fingerprint-Paar, in beiden Reihenfolgen
- [ ] Passwort-Modus
- [ ] Streaming über **mindestens drei Chunks**, inklusive eines letzten
      Chunks, der kleiner als die Chunk-Größe ist
- [ ] Leerer Klartext (0 Bytes)
- [ ] Klartext exakt auf der Chunk-Grenze
- [ ] Padding aktiv und inaktiv
- [ ] Dateiname mit Umlauten, Emoji und einem Zeichen aus einer
      Rechts-nach-links-Schrift
- [ ] v1-Envelope wird korrekt gelesen
- [ ] v1-Keyfile wird korrekt migriert

### 6.3 Negativvektoren

Jeder muss **zuverlässig fehlschlagen** — und mit dem richtigen Fehler.
Ein Test, der nur „irgendein Fehler" prüft, übersieht, dass eine
Implementierung aus dem falschen Grund scheitert.

- [ ] Manipuliertes Byte im Header
- [ ] Manipuliertes Byte im Ciphertext
- [ ] Abgeschnittener Stream nach dem zweiten Chunk
- [ ] Vertauschte Chunk-Reihenfolge
- [ ] Wiederholter Chunk
- [ ] Falscher Empfänger
- [ ] Unbekannte Formatversion
- [ ] Unbekannte Ciphersuite
- [ ] Signatur vorhanden, aber ungültig
- [ ] Signatur verlangt, aber nicht vorhanden
- [ ] Keyfile mit falschem Passwort
- [ ] Keyfile mit manipulierten Argon2id-Parametern
- [ ] Kapsel mit `length > 4096`
- [ ] `archive_index` mit `entry_count = 4096` in einem 200-Byte-Header
- [ ] `plaintext_size + padding_len` passt nicht zur Streamlänge
- [ ] Füllbytes sind nicht `0x00`
- [ ] Header-Padding enthält keine Nullbytes
- [ ] Unbekannter TLV-Typ im Header
- [ ] TLV-Typ `0x09` (reservierter Widerruf) vorhanden

### 6.4 Eigenschaftstests

Ergänzend, nicht als Vektordatei, sondern als Property-Test im Code:

- [ ] Für zufällige Klartexte beliebiger Länge gilt
      `decrypt(encrypt(x)) == x`
- [ ] Zwei Verschlüsselungen desselben Klartexts mit echtem RNG erzeugen
      **unterschiedliche** Envelopes (Nachweis, dass der deterministische
      Modus nicht versehentlich aktiv ist)
- [ ] Jede Ein-Byte-Änderung an einem gültigen Envelope führt zum Fehlschlag

## 7. Fehlerkodes

Negativvektoren benennen den erwarteten Fehler. Die Kodes sind Teil der
Spezifikation, damit alle Implementierungen dieselbe Unterscheidung treffen.

| Kode | Bedeutung |
|---|---|
| `UNSUPPORTED_VERSION` | Formatversion unbekannt |
| `UNSUPPORTED_SUITE` | Ciphersuite unbekannt oder nicht erlaubt |
| `MALFORMED` | Struktur nicht lesbar |
| `AUTH_FAILED` | AEAD-Prüfung fehlgeschlagen (Manipulation oder falscher Schlüssel) |
| `NO_MATCHING_RECIPIENT` | Keine Kapsel ließ sich mit diesem Schlüssel öffnen |
| `TRUNCATED` | Stream endet ohne Abschluss-Chunk |
| `CHUNK_ORDER` | Chunk-Position stimmt nicht |
| `SIGNATURE_INVALID` | Signatur vorhanden, Prüfung fehlgeschlagen |
| `SIGNATURE_MISSING` | Signatur gefordert, keine vorhanden |
| `KEYFILE_AUTH_FAILED` | Passwort falsch oder Keyfile manipuliert |

**Wichtig:** `AUTH_FAILED` und `NO_MATCHING_RECIPIENT` dürfen nach außen nicht
unterscheidbar gemacht werden, wo das einem Angreifer nützt. Die
Unterscheidung existiert für Tests und Diagnose, nicht für die Fehlermeldung
an den Nutzer.

## 8. Erzeugung der Vektoren

Die v1-Kompatibilitätsvektoren werden mit `legacy/python-v1` erzeugt, mit
**bekanntem, in der Vektordatei dokumentiertem Passwort**.

Die Originaldateien aus der v1-Entwicklung unter `_archive_v1/` sind dafür
unbrauchbar — ihre Passwörter sind nicht bekannt.

Alle v2-Vektoren werden von `cabrik-core` erzeugt und **manuell gegen die
Spezifikation geprüft**, bevor sie eingefroren werden. Ein Vektor, der nur
bestätigt, was der Code ohnehin tut, ist wertlos: Er zementiert dann einen
Implementierungsfehler zur Norm. Deshalb gilt:

> Ein Vektor wird erst eingefroren, wenn seine Struktur von Hand gegen das
> Formatdokument nachvollzogen wurde — Feld für Feld, mindestens einmal je
> Vektorklasse.

## 9. Verbindlichkeit

Eingefrorene Vektoren dürfen **nicht** geändert werden. Weicht eine
Implementierung ab, ist entweder die Implementierung falsch oder die
Spezifikation war es. Im zweiten Fall entsteht eine neue Formatversion mit
neuen Vektoren — die alten bleiben bestehen, damit die Abwärtskompatibilität
prüfbar bleibt.

Die Vektordateien gehören ins Repository (`.gitignore` nimmt `testvectors/`
ausdrücklich aus). Sie enthalten ausschließlich Wegwerf-Schlüssel.

## 10. Entschiedene Punkte

| Frage | Entscheidung |
|---|---|
| Reihenfolge des Zufallsverbrauchs | `envelope-v2.md` §11. Erzeugung in Eingabereihenfolge, Sortierung erst danach und ohne Zufall |
| Chunk-Größe | 64 KiB, fest |
| Padding | Padmé, `envelope-v2.md` §10.2 — reine Funktion, kein Zufall |
| RFC-9180-Vektoren | Vollständig, aber gefiltert auf die implementierte Suite (§6.1) |

## 11. Offene Punkte

- Ob die X-Wing-Vektoren stabil genug sind, um sie einzufrieren — der
  CFRG-Entwurf ist noch nicht final
- Ob Property-Tests (§6.4) in dieselbe Konformitätsaussage einfließen oder
  davon getrennt geführt werden
