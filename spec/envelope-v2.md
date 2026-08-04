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
| Signatur im **Trailer**, nicht im Header | Nur so kann sie das vollständige Transkript abdecken. Siehe §8.2 für die Konsequenz |
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
| `0x0001` | DHKEM(X25519, HKDF-SHA256) | HKDF-SHA256 | ChaCha20-Poly1305 | verbindlich |
| `0x0002` | reserviert für Hybrid X25519 + ML-KEM-768 | | | nicht in 2.0 |
| übrige | — | — | — | **MUSS** abgelehnt werden (`UNSUPPORTED_SUITE`) |

Eine Implementierung **MUSS** `0x0001` unterstützen und jede unbekannte
`suite_id` ablehnen — auch dann, wenn der Rest des Envelopes lesbar erscheint.

**Post-Quantum:** X25519 schützt nicht gegen „heute mitschneiden, später
entschlüsseln". Eine Hybrid-Suite ist für 2.0 nicht vorgesehen, aber die
Längenrahmung der Kapseln (§5) trägt bereits größere Encapsulations, sodass
`0x0002` ohne Formatbruch nachrüstbar ist.

## 5. Empfängerkapseln

```
type    : u8
length  : u16 BE
body    : length Bytes
```

| `type` | Bedeutung |
|---|---|
| `0x01` | HPKE an einen X25519-Public-Key |
| `0x02` | Passwort (Argon2id) |
| `0xFF` | Attrappe, siehe §5.3 |
| übrige | **MUSS** abgelehnt werden (`MALFORMED`) |

### 5.1 HPKE-Kapsel (`0x01`)

`body = enc ‖ wrapped_cek`, Länge 80 Bytes (32 + 48).

```
(enc, ctx) = HPKE.SetupBaseS(recipient_pk, info)
wrapped_cek = ctx.Seal(aad = "", pt = CEK)

info = "cabrik-envelope-v2" ‖ suite_id(2 Bytes BE)
```

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

`body` = Zufallsbytes in der Länge einer echten Kapsel des gleichen Typs.
Verschleiert die tatsächliche Empfängerzahl. Optional, per Voreinstellung aus.

Ein Leser behandelt sie wie eine fehlschlagende Kapsel. Da echte Kapseln beim
Trial Decryption ebenfalls fehlschlagen können, sind Attrappen für Dritte nicht
unterscheidbar.

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
| `0x01` | `content_type` | u8: `0` Text, `1` Datei, `2` Archiv | ja |
| `0x02` | `plaintext_size` | u64 BE, echte Länge ohne Padding | ja |
| `0x03` | `padding_len` | u64 BE | ja |
| `0x04` | `signed` | u8: `0` oder `1` | ja |
| `0x05` | `sender_sig_pub` | 32 Bytes Ed25519 | nur bei `signed = 1` |
| `0x06` | `filename` | UTF-8, NFC-normalisiert, ≤ 255 Bytes | nur bei `content_type ≠ 0` |
| `0x07` | `timestamp` | u64 BE, Unix-Sekunden | nein |
| `0x08` | `archive_index` | siehe §7.4 | nur bei `content_type = 2` |

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

**Dies ersetzt das ZIP aus v1 vollständig.** v1 schrieb ein
**unverschlüsseltes** ZIP nach `tempfile.mkdtemp()` und verschlüsselte erst
danach — der Klartext lag also vollständig auf dem Datenträger (Threat Model
§7.1). In v2 entsteht kein Zwischenprodukt: die Dateien werden direkt in den
verschlüsselten Stream geschrieben.

Kompression findet **nicht** statt. Sie würde die Länge inhaltsabhängig machen
und damit Rückschlüsse erlauben.

## 8. Chunk-Stream

STREAM-Konstruktion nach Hoang–Reyhanitabar–Rogaway–Vizár, wie in `age`.

- Chunk-Größe: **65536 Bytes** Klartext. Nur der letzte Chunk **DARF** kürzer
  sein und **DARF** 0 Bytes lang sein.
- Ein leerer Klartext ergibt **genau einen** Chunk der Länge 0 mit gesetztem
  Abschlussflag.
- Jeder Chunk: `ChaCha20Poly1305(key = stream_key, nonce = N_i, aad = "")`
  → 65536 + 16 Bytes Ciphertext.

```
N_i = counter(11 Bytes BE) ‖ final_flag(1 Byte)
```

`counter` beginnt bei 0 und wird je Chunk um 1 erhöht. `final_flag` ist `0x00`,
im letzten Chunk `0x01`. Ein Überlauf des Zählers **MUSS** zum Abbruch führen.

Die Chunk-Längen stehen **nicht** im Envelope — sie ergeben sich aus der
Gesamtlänge. Der Leser erkennt den letzten Chunk daran, dass keine weiteren
Bytes folgen, und prüft ihn mit gesetztem Flag.

### 8.1 Was das abwehrt

| Angriff | Wirkung |
|---|---|
| Abschneiden | Der letzte gelesene Chunk trägt `final_flag = 0` → `TRUNCATED` |
| Chunks vertauschen | Zähler im Nonce stimmt nicht → `AUTH_FAILED` |
| Chunk wiederholen | dito |
| Chunk aus fremdem Envelope einsetzen | `stream_key` hängt über `PH` am Prolog → `AUTH_FAILED` |
| Anhängen weiterer Chunks | Der echte letzte Chunk trägt `final_flag = 1`; danach dürfen keine Bytes folgen → `MALFORMED` |

### 8.2 Signaturprüfung erfolgt erst am Ende

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

Padding wird an den Klartext angehängt, bevor gechunkt wird. Die echte Länge
steht in `plaintext_size`, die Fülllänge in `padding_len`. Füllbytes sind
`0x00`.

**Padmé** (Nikitin et al., PURB): begrenzt den Overhead auf ≤ 12 % und die
Zahl unterscheidbarer Längenklassen auf O(log log n).

| Betriebsart | Voreinstellung |
|---|---|
| Text (`content_type = 0`) | **an**, Mindestlänge 256 Bytes |
| Datei / Archiv | **aus**, zuschaltbar |

Bei Dateien ist Padding voreingestellt aus, weil der Nutzen bei großen Dateien
gering und der Preis spürbar ist. Bei kurzen Textnachrichten ist er hoch: ohne
Padding lässt sich aus der Länge oft auf den Inhalt schließen.

## 11. Reihenfolge des Zufallsverbrauchs (normativ)

Damit bit-genaue Verschlüsselungsvektoren möglich sind (`test-vectors.md` §3),
ist die Reihenfolge Teil der Spezifikation. Eine Implementierung, die dieselben
Bytes in anderer Reihenfolge anfordert, ist **nicht konform**.

1. `CEK` — 32 Bytes
2. Für jede Kapsel in Schreibreihenfolge:
   - Typ `0x01`: 32 Bytes HPKE-`ikmE`
   - Typ `0x02`: 16 Bytes `salt`
   - Typ `0xFF`: so viele Bytes, wie der `body` lang ist
3. Nichts weiter. Nonces sind abgeleitet, Padding besteht aus Nullen.

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
   mitführen, Klartext **zurückhalten** (§8.2)
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

Bei einer 10-MiB-Datei, ein Empfänger, signiert:

| | v1 | v2 binär | v2 Armor |
|---|---|---|---|
| Envelope | 18,6 MiB | 10,0 MiB | 13,4 MiB |
| Overhead | **+78,1 %** | **+0,03 %** | +33 % |
| Speicherbedarf | ~4–5× Dateigröße | konstant ~256 KiB | konstant |

Der v1-Wert ist mit `legacy/python-v1/smoke_test.py` empirisch bestätigt.

## 16. Offene Punkte

- Genaue Padmé-Formel als Pseudocode ergänzen
- Obergrenze für `stanza_count` bei Attrappen festlegen
- Verhalten bei `plaintext_size`, das nicht zur tatsächlichen Streamlänge passt:
  vermutlich `MALFORMED`, muss aber gegen Padding-Randfälle geprüft werden
- Ob `archive_index` eine Obergrenze für `entry_count` braucht (Speicherschutz
  beim Parsen)
