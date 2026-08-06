# Cabrik Secure — Envelope-Format v2

**Status:** Entwurf · Phase 1, Dokument 3 von 7
**Gilt für:** v2.0
**Setzt voraus:** `threat-model.md`, `test-vectors.md`

Normatives Format. Schlüsselwörter **MUSS**, **DARF NICHT**, **SOLLTE**,
**KANN** im Sinne von RFC 2119.

---

## 1. Aufbau in der Übersicht

```
┌─ Prolog (Klartext) ────────────────────────────────────┐
│  Magic, Version, Suite, Empfängerkapseln               │
├─ Verschlüsselter Header ───────────────────────────────┤
│  Dateiname, Größe, Zeitstempel, Absenderschlüssel      │
├─ Chunk-Stream ─────────────────────────────────────────┤
│  Nutzdaten, 64-KiB-Chunks, jeder einzeln authentisiert │
├─ Verschlüsselter Trailer (nur bei Signatur) ───────────┤
│  Ed25519-Signatur über das gesamte Transkript          │
└────────────────────────────────────────────────────────┘
```

Zwei Schichten:

1. **Schlüsselverpackung.** Ein zufälliger Content Encryption Key (CEK) wird
   pro Empfänger verpackt — per HPKE oder per Passwort.
2. **Nutzdaten.** Aus dem CEK werden Header-, Stream- und Trailer-Schlüssel
   abgeleitet. Die Nutzdaten werden **einmal** verschlüsselt, unabhängig von
   der Empfängerzahl.

Diese Trennung ist der Grund, warum Mehrfachempfänger ohne Mehrfachaufwand
möglich sind.

## 2. Warum diese Konstruktion

| Entscheidung | Begründung |
|---|---|
| HPKE statt eigener Ableitung | v1 nutzte rohes X25519-Ergebnis als HKDF-Eingabe ohne Bindung der beteiligten Public Keys. HPKE (RFC 9180) leistet das normgerecht und hat auditierte Implementierungen in Rust, Swift und Kotlin |
| HPKE **Base**-Modus, Signatur separat | HPKE-Auth bindet die statische Absenderidentität in die KEM-Kapsel — bei mehreren Empfängern unhandlich und ohne Nichtabstreitbarkeit. Eine Ed25519-Signatur im verschlüsselten Teil leistet beides und bleibt vor Außenstehenden verborgen |
| CEK-Verpackung statt N-facher Nutzdatenverschlüsselung | Größe unabhängig von der Empfängerzahl |
| Signatur im **Trailer**, nicht im Header | Nur so kann sie das vollständige Transkript abdecken. Siehe §8.4 für die Konsequenz |
| Binär statt Base64-über-JSON | v1 hatte 78,1 % Overhead (empirisch bestätigt). v2 hat < 0,1 % bei Dateien |
| Strikte Ablehnung unbekannter Felder | Ein Krypto-Format, das Unbekanntes überliest, ist angreifbar. Neue Felder erfordern eine neue Version |

## 3. Prolog (Klartext)

Der Prolog enthält **ausschließlich**, was zum Entschlüsseln zwingend nötig
ist. Alles Weitere liegt verschlüsselt (Threat Model §6.1).

| Offset | Größe | Feld | Wert |
|---|---|---|---|
| 0 | 2 | `magic` | `0xCA 0x02` |
| 2 | 2 | `suite_id` | u16 BE, siehe §4 |
| 4 | 1 | `stanza_count` | u8, 1–255 |
| 5 | … | `stanzas` | `stanza_count` Kapseln, siehe §5 |

**`prologue` bezeichnet im Folgenden alle Bytes von Offset 0 bis zum Ende der
letzten Kapsel.** `PH = SHA-256(prologue)` bindet den gesamten Empfängersatz
in jede weitere Ableitung — Hinzufügen, Entfernen oder Verändern einer Kapsel
macht den Envelope unlesbar.

### 3.1 Was der Prolog bewusst nicht enthält

- **Kein Empfänger-Identifikator.** v1 hatte `recipient_fp` und machte damit
  alle Envelopes an denselben Empfänger verknüpfbar. v2 nutzt Trial
  Decryption (§7.1).
- **Kein Produktname.** v1 hatte `"branding": "Cabrik Secure"` im Klartext.
- **Keine Algorithmusliste im Klartext.** Nur die kompakte `suite_id`.
- **Kein Zeitstempel, keine Dateigröße, kein Dateiname.**

**Ehrliche Einordnung:** Das Format bleibt *als Format* erkennbar — zwei feste
Magic-Bytes genügen jedem, der es kennt. Vermieden wird die Klartext-Nennung
des Produktnamens. Vollständige Unerkennbarkeit ist mit einem parsbaren Format
nicht erreichbar und wird nicht behauptet.

## 4. Ciphersuites

| `suite_id` | KEM | KDF | AEAD | Status |
|---|---|---|---|---|
| `0x0001` | DHKEM(X25519, HKDF-SHA256) | HKDF-SHA256 | ChaCha20-Poly1305 | verbindlich, **Voreinstellung** |
| `0x0002` | X-Wing (X25519 + ML-KEM-768) | HKDF-SHA256 | ChaCha20-Poly1305 | verbindlich |
| übrige | — | — | — | **MUSS** abgelehnt werden (`UNSUPPORTED_SUITE`) |

Eine Implementierung **MUSS** beide Suites unterstützen und jede unbekannte
`suite_id` ablehnen — auch dann, wenn der Rest des Envelopes lesbar erscheint.

### 4.1 Suite `0x0002` — Post-Quantum-Hybrid

Wehrt Angreifermodell A10 ab: „heute mitschneiden, später entschlüsseln".

**X-Wing** (CFRG-Entwurf, Connolly et al.) kombiniert X25519 und ML-KEM-768 zu
einem einzigen KEM, das als HPKE-KEM einsetzbar ist. Es wird ihm gegenüber
einer selbstgebauten Kombination der Vorzug gegeben, weil es einen
Sicherheitsbeweis mitbringt und die Kombinationsfunktion normativ festlegt —
genau die Stelle, an der hybride Eigenkonstruktionen typischerweise scheitern.

Die Konstruktion ist **mindestens so sicher wie ihr stärkerer Bestandteil**:
Sie bricht erst, wenn X25519 *und* ML-KEM-768 brechen.

**Größen:**

| | `0x0001` | `0x0002` |
|---|---|---|
| Public Key | 32 Bytes | 1 216 Bytes |
| Kapsel (`enc`) | 32 Bytes | 1 120 Bytes |
| Stanza gesamt | 80 Bytes | 1 168 Bytes |
| Privater Schlüssel im Keyfile | 32 Bytes | 32 Bytes (Seed, `keyfile-v2.md` §3.2) |

### 4.2 Warum die Voreinstellung vorerst klassisch bleibt

Nicht aus Zweifel an ML-KEM, sondern wegen der **Schlüsselgröße im
Bedienablauf**: Ein X-Wing-Public-Key ergibt rund 1 620 Base64-Zeichen. Der
Austausch per Copy-Paste, wie ihn v1 vorsah, ist damit praktisch beendet.

Die Voreinstellung kippt auf `0x0002`, sobald der Schlüsselaustausch über
QR-Code und Kontaktdateien läuft (`trust-store.md` §5) statt über die
Zwischenablage.

**Entscheidend ist, dass dieser Wechsel dann nichts kostet:** Jede in v2
erzeugte Identität enthält von Beginn an ein ML-KEM-Schlüsselpaar
(`keyfile-v2.md` §3). Niemand muss neue Schlüssel erzeugen oder neu verteilen —
es ändert sich nur, welche Suite beim Verschlüsseln gewählt wird.

Ein Absender **KANN** `0x0002` jederzeit wählen, sofern er den X-Wing-Public-Key
des Empfängers besitzt.

## 5. Empfängerkapseln

```
type    : u8
length  : u16 BE
body    : length Bytes
```

| `type` | Bedeutung |
|---|---|
| `0x01` | HPKE an einen Empfänger-Public-Key |
| `0x02` | Passwort (Argon2id) |
| `0xFF` | Attrappe, siehe §5.3 |
| übrige | **MUSS** abgelehnt werden (`MALFORMED`) |

**Längenbegrenzung.** Ein Leser **MUSS** jede Kapsel mit `length > 4096`
ablehnen (`MALFORMED`). Ohne diese Grenze könnte eine präparierte Datei
255 × 65 535 Bytes ≈ 16 MiB Speicher anfordern, bevor irgendetwas geprüft wurde.
Die größte legitime Kapsel misst 1 168 Bytes.

**Reihenfolge.** Kapseln werden vor dem Schreiben **lexikographisch nach ihren
`body`-Bytes sortiert**. Da diese Bytes praktisch zufällig sind, verrät die
Reihenfolge nichts über die Reihenfolge, in der die Empfänger angegeben wurden.
Eine Zufallsmischung wäre gleichwertig, würde aber zusätzlichen Zufall
verbrauchen und die Testvektoren verkomplizieren — die Sortierung ist
deterministisch und kostenlos.

### 5.1 HPKE-Kapsel (`0x01`)

`body = enc ‖ wrapped_cek`

| Suite | `enc` | `wrapped_cek` | gesamt |
|---|---|---|---|
| `0x0001` | 32 | 48 | **80** |
| `0x0002` | 1 120 | 48 | **1 168** |

```
(enc, ctx) = HPKE.SetupBaseS(recipient_pk, info)
wrapped_cek = ctx.Seal(aad = "", pt = CEK)

info = "cabrik-envelope-v2" ‖ suite_id(2 Bytes BE)
```

Das KEM ergibt sich aus `suite_id` im Prolog — es steht **nicht** in der Kapsel.
Alle Kapseln eines Envelopes verwenden dieselbe Suite.

Die Bindung an den Empfängersatz erfolgt **nicht** hier — sie geschieht über
`PH` im AAD des verschlüsselten Headers (§6). Eine Bindung an dieser Stelle
wäre zirkulär, weil die Kapseln selbst Teil des Prologs sind.

### 5.2 Passwortkapsel (`0x02`)

`body = salt(16) ‖ m_cost(u32 BE) ‖ t_cost(u32 BE) ‖ p_cost(u8) ‖ wrapped_cek(48)`,
Länge 73 Bytes.

```
KEK = Argon2id(password, salt, m_cost, t_cost, p_cost, out_len = 32)
wrapped_cek = ChaCha20Poly1305(key = KEK, nonce = 0^12,
                               aad = "cabrik-v2 pwrap", pt = CEK)
```

Parameter werden **im Envelope mitgeführt**, damit sie später erhöht werden
können. Ein Leser **MUSS** Untergrenzen erzwingen (§5.4).

Nonce `0^12` ist zulässig, weil `KEK` durch das zufällige `salt` pro Envelope
eindeutig ist.

### 5.3 Attrappenkapsel (`0xFF`)

`body` = Zufallsbytes in der Länge einer echten Kapsel der verwendeten Suite.
Verschleiert die tatsächliche Empfängerzahl. Optional, per Voreinstellung aus.

Ein Leser behandelt sie wie eine fehlschlagende Kapsel. Da echte Kapseln beim
Trial Decryption ebenfalls fehlschlagen können, sind Attrappen für Dritte nicht
unterscheidbar.

**Obergrenzen:**

| | Wert |
|---|---|
| Echte Empfänger je Envelope | **32** |
| Auffüllung mit Attrappen | auf die nächste Zweierpotenz, gedeckelt bei **16** |
| `stanza_count` gesamt | ≤ 32 (Format erlaubt 255) |

Die Auffüllung bildet Anonymitätsgruppen: 1 echter Empfänger ergibt 2 Kapseln,
3 ergeben 4, 5 bis 8 ergeben 8, 9 bis 16 ergeben 16. Ab 17 echten Empfängern
entfällt die Auffüllung — die Zahl ist dann ohnehin wenig aussagekräftig.

Bei Suite `0x0002` kostet die Auffüllung spürbar: 16 Kapseln sind 18 688 Bytes
(18,25 KiB) Prolog. Deshalb bleibt sie abschaltbar.

Ein Leser **MUSS** `stanza_count` bis 255 verarbeiten können (Vorwärts­
kompatibilität), ein Schreiber **DARF NICHT** über 32 hinausgehen.

### 5.4 Untergrenzen für Argon2id

Ein Leser **MUSS** ablehnen: `m_cost < 65536` (64 MiB), `t_cost < 3`,
`p_cost < 1`. Ohne diese Prüfung könnte ein Angreifer einen Envelope mit
absichtlich schwachen Parametern konstruieren.

Empfohlene Werte beim Schreiben: `m_cost = 262144` (256 MiB), `t_cost = 3`,
`p_cost = 4`.

## 6. Schlüsselableitung

```
CEK          = Zufall(32)
PH           = SHA-256(prologue)

header_key   = HKDF-SHA256(ikm = CEK, salt = "",  info = "cabrik-v2 header",  L = 32)
stream_key   = HKDF-SHA256(ikm = CEK, salt = PH,  info = "cabrik-v2 stream",  L = 32)
trailer_key  = HKDF-SHA256(ikm = CEK, salt = PH,  info = "cabrik-v2 trailer", L = 32)
```

`PH` als Salt bindet Nutzdaten und Signatur an den exakten Empfängersatz.

## 7. Verschlüsselter Header

```
header_len : u32 BE          (Länge von header_ct)
header_ct  : ChaCha20Poly1305(key = header_key, nonce = 0^12,
                              aad = PH, pt = header_plain)
```

Nonce `0^12` ist zulässig, weil `header_key` über den pro Envelope zufälligen
`CEK` eindeutig ist und der Header genau einmal verschlüsselt wird.

`aad = PH` bewirkt: jede Veränderung am Prolog — auch das Entfernen einer
fremden Empfängerkapsel — lässt die Header-Entschlüsselung fehlschlagen.

### 7.1 Trial Decryption

Ein Leser versucht der Reihe nach jede Kapsel seines Typs zu öffnen. Gelingt
eine, entsteht ein CEK-Kandidat; damit wird der Header entschlüsselt. Gelingt
das ebenfalls, ist der Empfänger gefunden.

Schlägt **jede** Kapsel fehl: `NO_MATCHING_RECIPIENT`. Der Leser **DARF NICHT**
nach außen unterscheidbar machen, ob keine Kapsel passte oder der Header
fehlschlug (Threat Model §7, `test-vectors.md` §7).

Aufwand: bis zu 255 X25519-Operationen, in der Praxis unter einer Millisekunde.

### 7.2 Header-Inhalt (TLV)

```
type   : u8
length : u16 BE
value  : length Bytes
```

| `type` | Feld | Typ | Pflicht |
|---|---|---|---|
| `0x00` | `header_padding` | Nullbytes, Inhalt wird ignoriert | siehe §7.5 |
| `0x01` | `content_type` | u8: `0` Text, `1` Datei, `2` Archiv | ja |
| `0x02` | `plaintext_size` | u64 BE, echte Länge ohne Padding | ja |
| `0x03` | `padding_len` | u64 BE | ja |
| `0x04` | `signed` | u8: `0` oder `1` | ja |
| `0x05` | `sender_sig_pub` | 32 Bytes Ed25519 | nur bei `signed = 1` |
| `0x06` | `filename` | UTF-8, NFC-normalisiert, ≤ 255 Bytes | nur bei `content_type ≠ 0` |
| `0x07` | `timestamp` | u64 BE, Unix-Sekunden | nein |
| `0x08` | `archive_index` | siehe §7.4 | nur bei `content_type = 2` |
| `0x09` | — | **reserviert** für eine in-band-Widerrufserklärung, siehe `trust-store.md` §4.3. In 2.0 nicht geschrieben und **MUSS** abgelehnt werden | — |

**Regeln:**

- Felder **MÜSSEN** in aufsteigender `type`-Reihenfolge stehen.
- Jeder `type` **DARF** höchstens einmal vorkommen.
- Ein unbekannter `type` **MUSS** zu `MALFORMED` führen. Es gibt kein
  Überlesen — neue Felder erfordern eine neue Formatversion.
- Fehlende Pflichtfelder **MÜSSEN** zu `MALFORMED` führen.
- `filename` **MUSS** vor der Verwendung bereinigt werden: keine Pfadtrenner,
  kein `..`, keine reservierten Windows-Namen (`CON`, `NUL`, `LPT1` …), keine
  Steuerzeichen, keine Bidi-Override-Zeichen (`U+202E` und Verwandte, sonst
  lässt sich `harmlos‮fdp.exe` als `harmlos exe.pdf` darstellen).

### 7.3 Warum `timestamp` optional ist

v1 schrieb den Zeitstempel unverschlüsselt in den Header. In v2 liegt er
verschlüsselt und **KANN** ganz entfallen — für Nutzer, denen auch der
Empfänger nicht den Sendezeitpunkt kennen soll. Voreinstellung: vorhanden.

### 7.4 Mehrere Dateien

`content_type = 2` bedeutet: die Nutzdaten sind ein Archiv, dessen Verzeichnis
in `archive_index` liegt.

```
entry_count : u32 BE
für jeden Eintrag:
    name_len : u16 BE
    name     : UTF-8, NFC, bereinigt wie §7.2
    size     : u64 BE
```

Die Dateien folgen im Stream lückenlos in Verzeichnisreihenfolge.

**Grenzen (Speicherschutz beim Parsen):**

- `entry_count` ≤ **4096**, sonst `MALFORMED`
- `name_len` ≤ 255, sonst `MALFORMED`
- Die Summe aller `size`-Felder **MUSS** exakt `plaintext_size` ergeben,
  sonst `MALFORMED`

Entscheidend ist weniger die Obergrenze als die Umsetzungsregel: Ein Leser
**DARF NICHT** anhand von `entry_count` vorab Speicher reservieren, sondern
**MUSS** Einträge einzeln lesen und dabei gegen die verbleibende Headerlänge
prüfen. Ein `entry_count` von 4096 in einem 200-Byte-Header ist ein Angriff,
keine gültige Datei.

**Dies ersetzt das ZIP aus v1 vollständig.** v1 schrieb ein
**unverschlüsseltes** ZIP nach `tempfile.mkdtemp()` und verschlüsselte erst
danach — der Klartext lag also vollständig auf dem Datenträger (Threat Model
§7.1). In v2 entsteht kein Zwischenprodukt: die Dateien werden direkt in den
verschlüsselten Stream geschrieben.

Kompression findet **nicht** statt. Sie würde die Länge inhaltsabhängig machen
und damit Rückschlüsse erlauben.

### 7.5 Der Header muss selbst gepolstert werden

`header_len` steht im Klartext. Der Header enthält den Dateinamen und, bei
Archiven, sämtliche Einträge — er ist also **variabel lang und inhaltsabhängig**.
Ohne Gegenmaßnahme verrät `header_len` die Länge des Dateinamens und die Zahl
der Dateien, obwohl beides verschlüsselt ist.

**Regel:** `header_plain` **MUSS** mit einem `0x00`-TLV auf das nächste
Vielfache von **256 Bytes** aufgefüllt werden.

- Der Wert des Padding-TLV besteht aus Nullbytes.
- Er steht als **erstes** Feld (Typ `0x00` ist der kleinste, die aufsteigende
  Reihenfolge ergibt sich von selbst).
- Ein Leser **MUSS** prüfen, dass der Inhalt tatsächlich Nullbytes sind, und
  ihn danach verwerfen.
- Reicht der Platz für den 3-Byte-TLV-Kopf nicht, wird auf das übernächste
  Vielfache aufgefüllt.

Damit sind alle Dateinamen bis 250 Bytes und alle Archive bis etwa 8 Einträge
ununterscheidbar.

## 8. Chunk-Stream

STREAM-Konstruktion nach Hoang–Reyhanitabar–Rogaway–Vizár, wie in `age`.

- Chunk-Größe: **65536 Bytes** Klartext. Nur der letzte Chunk **DARF** kürzer
  sein und **DARF** 0 Bytes lang sein.
- Ein leerer Klartext ergibt **genau einen** Chunk der Länge 0 mit gesetztem
  Abschlussflag.

**Gechunkt wird nach dem Padding.** „Länge" bezeichnet in diesem Abschnitt
durchgängig `plaintext_size + padding_len`, nicht `plaintext_size`. Ein leerer
Klartext ergibt daher nur dann einen 0-Byte-Chunk, wenn Padding abgeschaltet
ist; bei aktivem Padding sind es 256 Bytes in einem Chunk.
- Jeder Chunk: `ChaCha20Poly1305(key = stream_key, nonce = N_i, aad = "")`
  → 65536 + 16 Bytes Ciphertext.

```
N_i = counter(11 Bytes BE) ‖ final_flag(1 Byte)
```

`counter` beginnt bei 0 und wird je Chunk um 1 erhöht. `final_flag` ist `0x00`,
im letzten Chunk `0x01`. Ein Überlauf des Zählers **MUSS** zum Abbruch führen.

### 8.1 Woran der Leser den letzten Chunk erkennt

Die Chunk-Längen stehen **nicht** im Envelope. Der Leser berechnet ihre Anzahl
aus den Pflichtfeldern des verschlüsselten Headers:

```
gesamt      = plaintext_size + padding_len
chunk_count = max(1, ceil(gesamt / 65536))
```

`max(1, …)` deckt den leeren Klartext ab: Er ergibt einen Chunk der Länge 0.

Damit steht **vor** dem Lesen des ersten Chunks fest, welcher der letzte ist.
Es wird nicht vorausgeschaut und nichts geraten.

**Korrektur gegenüber Stand 1.** Dort stand: „Der Leser erkennt den letzten
Chunk daran, dass keine weiteren Bytes folgen." Das war falsch — bei
signierten Nachrichten folgt der Trailer (§9). Die Regel war zudem unnötig,
weil der Header die Länge ohnehin führt.

Das Abschlussflag im Nonce bleibt als **zweite, unabhängige** Absicherung
bestehen: Selbst wenn ein Angreifer die Längenangabe im Header verändern
könnte — was er nicht kann, weil sie AEAD-geschützt ist — passte das Flag
nicht.

### 8.2 Folge: Die Klartextlänge muss vorab bekannt sein

`plaintext_size` steht im Header, also **vor** den Chunks. Einpassiges
Verschlüsseln einer Eingabe unbekannter Länge ist damit nicht möglich.

**Der Umfang der Einschränkung ist eng.** Nicht betroffen sind:

- große Dateien ohne Speicherlast zu verarbeiten — dafür ist die Chunk-Schicht
  da, es genügt ein `stat()` für die Länge;
- das Entschlüsseln als Strom — dort steht die Länge im Header.

Betroffen ist ausschließlich das Verschlüsseln einer Eingabe, deren Länge sich
vorher nicht ermitteln lässt. Für ein Dateiwerkzeug ist das folgenlos; der
verbleibende Fall sind erzeugte Datenströme (`pg_dump`, Protokolle).

**Verhalten der Anwendung bei Pipe-Eingaben:**

- Sie **puffert im Arbeitsspeicher**, niemals auf dem Datenträger. Eine
  Zwischendatei wäre genau das Klartext-Leck aus `shredding.md` §3.
- Voreingestellte Obergrenze **256 MiB**, per Schalter änderbar. Darüber wird
  mit klarer Meldung abgelehnt, statt stillschweigend Speicher zu belegen.
- Das braucht **keine Formatänderung** — es ist reine Anwendungslogik.

**Umkehrbarkeit.** Sollte echtes Streaming ohne Längenvorwissen später
erforderlich werden, geschieht das über eine **neue Formatversion**, nicht
über einen Bruch: Alte Leser lehnen sie mit `UNSUPPORTED_VERSION` sauber ab.
Der Preis wäre die Mehrdeutigkeit, die §8.1 beseitigt — deshalb erst dann,
wenn ein Produktziel es verlangt.

Die Alternative — ein Sentinel für „Länge unbekannt" mit Rückfall auf
Flag-Erkennung — wurde verworfen. Sie bringt genau die Mehrdeutigkeit zurück,
die §8.1 beseitigt, und der Leser müsste dann das Ende des Chunk-Bereichs
gegen den Trailer abgrenzen, ohne dessen Vorhandensein sicher zu kennen.

### 8.3 Was das abwehrt

| Angriff | Wirkung |
|---|---|
| Abschneiden | Der letzte gelesene Chunk trägt `final_flag = 0` → `TRUNCATED` |
| Chunks vertauschen | Zähler im Nonce stimmt nicht → `AUTH_FAILED` |
| Chunk wiederholen | dito |
| Chunk aus fremdem Envelope einsetzen | `stream_key` hängt über `PH` am Prolog → `AUTH_FAILED` |
| Anhängen weiterer Chunks | Der echte letzte Chunk trägt `final_flag = 1`; danach dürfen keine Bytes folgen → `MALFORMED` |

### 8.4 Signaturprüfung erfolgt erst am Ende

Die Signatur deckt das gesamte Transkript und kann daher erst nach dem letzten
Chunk geprüft werden. Beim Streaming entsteht Klartext also, **bevor** die
Absenderauthentizität feststeht.

**Verbindliche Regel für jeden Aufrufer:** Gestreamter Klartext **DARF NICHT**
verwendet, angezeigt oder unter seinem endgültigen Namen gespeichert werden,
bevor der Trailer geprüft wurde.

Die API **MUSS** das erzwingen, statt es zu dokumentieren:

- Beim Entschlüsseln in eine Datei wird in eine temporäre Datei geschrieben
  und erst nach erfolgreicher Prüfung umbenannt.
- Beim Entschlüsseln in den Speicher wird das Ergebnis erst nach der Prüfung
  herausgegeben.
- Schlägt die Prüfung fehl, wird die temporäre Datei gelöscht und der Fehler
  weitergereicht — **kein** Teilergebnis.

## 9. Trailer

Vorhanden genau dann, wenn `signed = 1`.

```
transcript = SHA-256( "cabrik-transcript-v2"
                    ‖ PH
                    ‖ SHA-256(header_ct)
                    ‖ SHA-256(alle Chunk-Ciphertexte in Reihenfolge) )

signature  = Ed25519.Sign(sig_sk, transcript)

trailer_ct = ChaCha20Poly1305(key = trailer_key, nonce = 0^12,
                              aad = transcript, pt = signature)
```

`trailer_ct` ist 80 Bytes (64 + 16) und bildet den Abschluss des Envelopes.

### 9.1 Was die Signatur leistet

Sie deckt Prolog (und damit den Empfängersatz), Header und sämtliche
Nutzdaten ab. Daraus folgt:

- Ein **Mitempfänger** kennt den CEK und könnte Nutzdaten neu verschlüsseln —
  aber keine gültige Signatur erzeugen. Ohne Signatur fällt die Nachricht in
  den Zustand „nicht signiert" (Threat Model §8).
- Eine Signatur **KANN NICHT** auf einen anderen Empfängersatz übertragen
  werden, weil `PH` im Transkript steckt.
- Der Absenderschlüssel ist **verschlüsselt**. Ein Außenstehender sieht nicht
  einmal, *ob* signiert wurde. Damit sind Authentizität und Anonymität
  gegenüber Dritten erstmals gleichzeitig erreichbar — in v1 schlossen sie
  sich aus.

### 9.2 Was sie nicht leistet

Eine gültige Signatur belegt nur, dass der Inhaber eines bestimmten
Ed25519-Schlüssels die Nachricht erzeugt hat. **Wer** das ist, entscheidet
ausschließlich der Trust Store. Die Bibliothek **DARF NICHT** einen
Wahrheitswert wie `signature_valid` zurückgeben, sondern **MUSS** den
dreiwertigen Zustand aus Threat Model §8 liefern.

## 10. Padding

### 10.1 Wozu

AEAD verbirgt den Inhalt, aber nicht die **Länge**. Ein Ciphertext ist so lang
wie sein Klartext plus konstanter Overhead. Wer den Envelope sieht, kennt die
Nachrichtenlänge damit auf wenige Bytes genau.

Bei Dateien ist das meist hinnehmbar. Bei kurzen Texten ist es die Nachricht
selbst: Wer den Kontext kennt, unterscheidet `Ja` (2 Bytes) von `Nein`
(4 Bytes) ohne jeden Schlüssel.

Padding hängt Füllbytes an, damit verschiedene Klartexte auf dieselbe Länge
fallen. Die echte Länge steht in `plaintext_size`, die Fülllänge in
`padding_len`; der Empfänger schneidet ab. Füllbytes sind `0x00`.

### 10.2 Padmé

Feste Größenklassen (256 / 1024 / 4096 / …) funktionieren, sind aber willkürlich
gewählt und verschwenden bei großen Dateien viel. **Padmé** (Nikitin et al.,
PURB, PETS 2019) rundet stattdessen auf Zahlen mit wenigen signifikanten Bits.
Dadurch gibt es bei kleinen Längen viele Klassen (wenig Verschnitt) und bei
großen wenige (starke Verschleierung) — mit beschränktem relativem Verschnitt
über den gesamten Bereich. Eine Formel für alles, kein Sonderfall für kleine
und große Dateien.

```
PADME(L):                        # L = Klartextlänge in Bytes
    if L <= PAD_MIN:
        return PAD_MIN
    E    = floor(log2(L))        # Größenordnung von L
    S    = floor(log2(E)) + 1    # Bits, um E darzustellen
    z    = E - S                 # so viele niederwertige Bits werden genullt
    mask = (1 << z) - 1
    return (L + mask) & ~mask    # aufrunden auf ein Vielfaches von 2^z
```

`PAD_MIN = 256`.

**Umsetzungshinweise, verbindlich:**

- `L` ist `u64`. `L + mask` **MUSS** auf Überlauf geprüft werden; bei Überlauf
  bricht die Operation ab (betrifft nur Längen nahe 2⁶⁴).
- `floor(log2(x))` wird als `63 - x.leading_zeros()` berechnet, **nicht** über
  Gleitkomma — `log2` in Gleitkomma liefert an Zweierpotenzen je nach Plattform
  unterschiedliche Ergebnisse und würde die Testvektoren brechen.
- Für `E = 1` wird `S = 1` und `z = 0`, die Funktion ist also die Identität.
  Das ist korrekt und durch `PAD_MIN` ohnehin unerreichbar.

**Beispiele** (gehören als Testvektoren nach `testvectors/`):

| L | E | S | z | `PADME(L)` | Verschnitt |
|---|---|---|---|---|---|
| 100 | — | — | — | 256 | *Untergrenze* |
| 1 000 | 9 | 4 | 5 | 1 024 | 2,4 % |
| 1 025 | 10 | 4 | 6 | 1 088 | 6,1 % |
| 10 000 | 13 | 4 | 9 | 10 240 | 2,4 % |
| 1 000 000 | 19 | 5 | 14 | 1 015 808 | 1,6 % |
| 10 000 000 | 23 | 5 | 18 | 10 223 616 | 2,2 % |

**Obere Schranke.** Der Verschnitt beträgt höchstens `2^-S`. Wegen
`PAD_MIN = 256` ist `E ≥ 8` und damit `S ≥ 4`, der Verschnitt also **≤ 6,25 %**
— erreicht bei Längen knapp oberhalb einer Zweierpotenz, etwa `L = 32769`.
Ab `E ≥ 16` (also ab 64 KiB) sinkt die Schranke auf 3,125 %.

Die in der PURB-Arbeit genannten ~12 % gelten für den ungebremsten Fall ohne
Untergrenze; mit `PAD_MIN = 256` ist die Schranke schärfer.

### 10.3 Voreinstellungen

| Betriebsart | Padding |
|---|---|
| Text (`content_type = 0`) | **an** |
| Datei / Archiv | **aus**, zuschaltbar |

Bei Dateien ist der Nutzen gering und der Preis spürbar; bei kurzen Texten ist
es umgekehrt.

### 10.4 Prüfung beim Lesen

Ein Leser **MUSS** nach dem letzten Chunk prüfen:

```
summe_der_chunk_klartexte == plaintext_size + padding_len
```

Trifft das nicht zu: `MALFORMED`. Zusätzlich **MUSS** geprüft werden, dass die
`padding_len` letzten Bytes tatsächlich `0x00` sind — andernfalls ebenfalls
`MALFORMED`.

Die zweite Prüfung kostet nichts und schließt einen verdeckten Kanal: Ohne sie
könnte ein Absender beliebige Daten im Padding unterbringen, die kein Empfänger
zu sehen bekommt und keine Implementierung bemerkt.

## 11. Reihenfolge des Zufallsverbrauchs (normativ)

Damit bit-genaue Verschlüsselungsvektoren möglich sind (`test-vectors.md` §3),
ist die Reihenfolge Teil der Spezifikation. Eine Implementierung, die dieselben
Bytes in anderer Reihenfolge anfordert, ist **nicht konform**.

1. `CEK` — 32 Bytes
2. Für jede Kapsel **in der Reihenfolge, in der die Empfänger angegeben
   wurden** — nicht in der späteren Schreibreihenfolge:
   - Typ `0x01`, Suite `0x0001`: 32 Bytes HPKE-`ikmE`
   - Typ `0x01`, Suite `0x0002`: 32 Bytes X25519-`ikmE`, danach 64 Bytes
     ML-KEM-`ikm` (in dieser Reihenfolge, wie im X-Wing-Entwurf festgelegt)
   - Typ `0x02`: 16 Bytes `salt`
   - Typ `0xFF`: so viele Bytes, wie der `body` lang ist
3. Nichts weiter.

Die Sortierung der Kapseln (§5) findet **nach** der Erzeugung statt und
verbraucht keinen Zufall. Nonces sind abgeleitet, Padding besteht aus Nullen,
und `PADME` ist eine reine Funktion — an keiner dieser Stellen wird Zufall
benötigt.

## 12. Lesevorgang

1. `magic` prüfen → sonst v1-Erkennung (§13), sonst `MALFORMED`
2. `suite_id` prüfen → sonst `UNSUPPORTED_SUITE`
3. Kapseln parsen, `PH` berechnen
4. Trial Decryption (§7.1) → `CEK`
5. `header_key` ableiten, Header entschlüsseln → sonst `NO_MATCHING_RECIPIENT`
6. Header-TLV strikt validieren
7. Verlangt der Aufrufer eine Signatur und ist `signed = 0` →
   `SIGNATURE_MISSING`, **vor** jeder Nutzdatenverarbeitung
8. `stream_key` ableiten, Chunks fortlaufend entschlüsseln, Transkript
   mitführen, Klartext **zurückhalten** (§8.4)
9. Bei `signed = 1`: Trailer entschlüsseln, Signatur gegen `transcript` prüfen
   → sonst `SIGNATURE_INVALID`
10. Absenderschlüssel gegen den Trust Store auflösen → dreiwertiger Zustand
11. Klartext freigeben

Schlägt ein Schritt fehl, wird abgebrochen und jedes Zwischenergebnis
verworfen.

## 13. Kompatibilität mit v1

**Erkennung:** Ein v1-Envelope ist Base64 über JSON und beginnt daher mit
`eyJ` (Base64 von `{"`). Beginnt die Eingabe damit, wird der v1-Pfad genommen.

Ein v2-Leser **MUSS** v1-Envelopes lesen können und **DARF NICHT** v1
schreiben.

Nach dem Lesen eines v1-Envelopes **MUSS** dem Nutzer angezeigt werden:

> Diese Nachricht liegt im alten Format v1 vor. Dateiname, Größe, Zeitstempel
> und — falls signiert — die dauerhafte Absenderkennung waren darin für jeden
> lesbar, der die Datei besaß.

Der Signaturzustand eines v1-Envelopes wird nach denselben Regeln aufgelöst wie
bei v2: eine gültige v1-Signatur eines unbekannten Schlüssels ergibt
„Unbekannt", nicht „Verifiziert".

## 14. Armor (Base64-Modus)

Für Kopieren und Einfügen in Textkanäle. Optional, Voreinstellung aus.

```
-----BEGIN CABRIK ENVELOPE-----
<Base64, Standardalphabet, Zeilen zu 64 Zeichen>
-----END CABRIK ENVELOPE-----
```

Overhead 33 % gegenüber < 0,1 % im Binärmodus.

**Zielkonflikt, bewusst so entschieden:** Die Rahmenzeilen nennen das Produkt
und stehen damit gegen Threat Model §6.3. Wer diesen Schutz braucht, nutzt den
Binärmodus. Wer Armor nutzt, fügt den Text ohnehin in einen Kanal ein, der den
Kontext bereits preisgibt.

## 15. Größenvergleich

Bei einer 10-MiB-Datei, ein Empfänger, signiert, ohne Padding:

| | v1 | v2 binär `0x0001` | v2 binär `0x0002` | v2 Armor |
|---|---|---|---|---|
| Envelope | 18,6 MiB | 10,0 MiB | 10,0 MiB | 13,4 MiB |
| Overhead | **+78,1 %** | **+0,03 %** | +0,04 % | +33 % |
| Speicherbedarf | ~4–5× Dateigröße | konstant ~256 KiB | konstant | konstant |

Der v1-Wert ist mit `legacy/python-v1/smoke_test.py` empirisch bestätigt.

Der Post-Quantum-Aufschlag beträgt 1 088 Bytes je Empfänger — bei Dateien
bedeutungslos. Spürbar wird er nur bei sehr kurzen Nachrichten mit vielen
Empfängern und aktivierten Attrappen.

## 16. Entschiedene Punkte

| Frage | Entscheidung |
|---|---|
| Padmé | §10.2, mit Ganzzahl-Logarithmus statt Gleitkomma |
| Attrappen-Obergrenze | 32 echte Empfänger, Auffüllung auf Zweierpotenz bis 16 (§5.3) |
| Kapsellänge | ≤ 4096 Bytes, sonst `MALFORMED` (§5) |
| `plaintext_size`-Abgleich | `MALFORMED` bei Abweichung, plus Prüfung der Füllbytes (§10.4) |
| `entry_count` | ≤ 4096, und keine Vorabreservierung von Speicher (§7.4) |
| Chunk-Größe | 64 KiB, fest — eine verhandelbare Größe wäre ein weiteres Feld und eine weitere Fehlerquelle |
| Post-Quantum | Suite `0x0002` verbindlich implementiert, Voreinstellung vorerst `0x0001` (§4.1, §4.2) |

## 17. Offene Punkte

- Ob der Header über 256 Bytes hinaus in gröberen Stufen gepolstert werden
  sollte, wenn Archive mit vielen Einträgen häufig vorkommen
- Genaue Formulierung der X-Wing-Anbindung, sobald der CFRG-Entwurf final ist —
  die KEM-Kennung kann sich bis dahin noch ändern
