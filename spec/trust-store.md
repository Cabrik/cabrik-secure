# Cabrik Secure — Trust Store

**Status:** Entwurf · Phase 1, Dokument 5 von 7
**Setzt voraus:** `threat-model.md`, `keyfile-v2.md`, `envelope-v2.md`

Der wichtigste konzeptionelle Fix gegenüber v1.

---

## 1. Das Problem

In v1 stammt der Signaturprüfschlüssel aus dem Header **derselben Nachricht**.
Der Empfänger prüft damit eine Signatur gegen einen Schlüssel, den der Absender
mitgeliefert hat.

Das ist zirkulär. Ein Angreifer erzeugt einfach ein eigenes Ed25519-Paar,
signiert damit und legt den passenden Public Key bei — v1 meldet
`signature_valid: true`. Die Prüfung belegt ausschließlich, dass Signatur und
mitgelieferter Schlüssel zusammenpassen. Über die **Person** sagt sie nichts.

Im Anonymitätsmodus, der in v1 voreingestellt **an** war, wird der Signier­
schlüssel sogar pro Nachricht neu erzeugt. `signature_valid: true` erschien
also selbst dann, wenn keinerlei Identitätsaussage vorlag.

**Kryptographie kann dieses Problem nicht lösen.** Die Zuordnung Schlüssel ↔
Mensch entsteht ausschließlich durch einen Vorgang außerhalb des Kanals. Genau
den bildet der Trust Store ab.

## 2. Fingerprint

```
fingerprint = SHA-256( "cabrik-fp-v2" ‖ enc_pub(32) ‖ sig_pub(32) )
```

Volle **256 Bit**. Fehlt `sig_pub` (Anonymitätsidentität), werden 32 Nullbytes
eingesetzt.

Intern wird stets der volle Wert verglichen. Gekürzt wird nur für die Anzeige.

### 2.1 Darstellung

**Crockford-Base32** — kein `I`, `L`, `O`, `U`, dadurch keine Verwechslung von
`0`/`O` und `1`/`I`/`l`, und beim Eintippen tolerant gegenüber Groß- und
Kleinschreibung.

Gruppiert zu vier Zeichen:

```
K7QF-3MXB-9TWH-2RND-5PJC-8VGA-4YSE-6ZKQ
```

| | Wert |
|---|---|
| Voll | 52 Zeichen = 256 Bit |
| **Mindestanzeige** | **32 Zeichen = 160 Bit** |
| Kurzform (nur Listen) | 8 Zeichen, **nie** zur Verifikation |

### 2.2 Warum 32 Zeichen

Bei einem gekürzten Hash der Länge *n* Bit liegt die Sicherheit gegen
**Kollisionen** bei *n*/2 Bit (Geburtstagsparadoxon) — der Angreifer sucht
*zwei* Schlüssel mit gleichem Fingerprint, nicht einen zu einem vorgegebenen.

| Anzeige | Bit | Kollisionssicherheit | Bewertung |
|---|---|---|---|
| v1: 8 Hex | 32 | **16 Bit** | in Sekunden brechbar |
| 16 Zeichen Base32 | 80 | 40 Bit | zu wenig |
| **32 Zeichen Base32** | **160** | **80 Bit** | ausreichend |
| 52 Zeichen (voll) | 256 | 128 Bit | für Anzeige unnötig |

v1s 32-Bit-Fingerprint war nicht nur knapp, sondern praktisch wertlos: zwei
kollidierende Schlüssel findet man mit rund 65 000 Versuchen — Sekundenarbeit.

Die 8-Zeichen-Kurzform **DARF** ausschließlich zur optischen Unterscheidung in
Listen dienen und **DARF NIEMALS** als Verifikationsgrundlage angeboten werden.

## 3. Safety Number

Zum gegenseitigen Abgleich vergleichen beide Seiten **eine** Zeichenfolge statt
zweier Fingerprints — nach dem Vorbild der Signal Safety Numbers.

```
(a, b)       = Fingerprints, lexikografisch sortiert
safety_input = "cabrik-sn-v2" ‖ a ‖ b
safety_hash  = SHA-256(safety_input)
```

Angezeigt als **60 Dezimalziffern** in 12 Gruppen zu 5 — vorlesbar am Telefon,
sprachunabhängig:

```
41827  09365  72104  88519  36047  21968
50713  84226  19570  63841  27395  70612
```

Jede Gruppe entstammt 5 Bytes des Hashes, modulo 100000. Die Sortierung sorgt
dafür, dass beide Seiten dieselbe Zahl sehen, unabhängig davon, wer fragt.

## 4. Vertrauenszustände

| Zustand | Bedeutung |
|---|---|
| `Unbekannt` | Schlüssel nicht im Store |
| `Gesehen` | Im Store, aber nie verifiziert (Trust on First Use) |
| `Verifiziert` | Fingerprint oder Safety Number wurde außerhalb des Kanals abgeglichen |
| `Geändert` | **Warnzustand.** Ein bekannter Kontakt tritt mit anderem Schlüssel auf |
| `Widerrufen` | Schlüssel als kompromittiert markiert |

### 4.1 Trust on First Use

Beim ersten Empfang von einem unbekannten Schlüssel wird der Kontakt als
`Gesehen` angelegt — **nicht** als verifiziert. Das erlaubt es, wiederkehrende
Absender wiederzuerkennen, ohne Sicherheit vorzutäuschen.

### 4.2 `Geändert` ist der wichtigste Zustand

Tritt ein Kontakt mit einem anderen Schlüssel auf als zuvor, ist das entweder
ein Gerätewechsel — oder ein Angriff.

Die Oberfläche **MUSS** das deutlich anzeigen, den Inhalt **NICHT** stillschweigend
als authentisch darstellen und den alten Schlüssel **NICHT** automatisch
ersetzen. Die Bestätigung erfordert eine erneute Verifikation.

Dies ist der Punkt, an dem Messenger historisch am häufigsten scheitern:
ein stiller Schlüsselwechsel macht die gesamte Verifikationskette wertlos.

## 5. Verifikationswege

| Weg | Aufwand | Sicherheit |
|---|---|---|
| **QR-Code** | gering | hoch, erfordert physische Nähe |
| **Safety Number vorlesen** | mittel | hoch, wenn die Stimme erkannt wird |
| **Fingerprint tippen** | hoch | hoch |
| Fingerprint per Messenger senden | gering | **gering** — derselbe Kanal, denselben Angreifer |

Die letzte Zeile **MUSS** in der Oberfläche benannt werden. Ein Fingerprint,
der über denselben Kanal kommt wie die Nachricht, beweist nichts.

### 5.1 QR-Nutzlast

```
cabrik:v2:<Base32 enc_pub>:<Base32 sig_pub>:<Base32 fingerprint[0..8]>
```

Der Fingerprint-Anfang dient nur als Prüfsumme gegen Übertragungsfehler. Der
Leser **MUSS** den Fingerprint aus den Schlüsseln neu berechnen und **DARF
NICHT** dem übertragenen Wert vertrauen.

Fehlt `sig_pub`, steht dort ein leeres Feld.

## 6. Speicherformat

Eigene Datei neben dem Keyfile, verschlüsselt mit einem Schlüssel, der aus dem
Keyfile-Geheimnis abgeleitet wird:

```
contacts_key = HKDF-SHA256(ikm = enc_sk, salt = "", info = "cabrik-v2 contacts", L = 32)
```

Damit ist der Kontaktspeicher nur bei entsperrter Identität lesbar. Ein
Angreifer mit dem Dateisystemzugriff sieht nicht, **mit wem** kommuniziert wird
— eine der aussagekräftigsten Metadaten überhaupt.

Aufbau wie `keyfile-v2.md` §2: Klartextkopf mit `magic`, `version`, `salt`,
darunter ein AEAD-Block. Kontakteinträge als TLV:

| `type` | Feld | Typ |
|---|---|---|
| `0x01` | `enc_pub` | 32 Bytes |
| `0x02` | `sig_pub` | 32 Bytes, optional |
| `0x03` | `name` | UTF-8, ≤ 128 Bytes |
| `0x04` | `state` | u8, siehe §4 |
| `0x05` | `first_seen` | u64 BE |
| `0x06` | `verified_at` | u64 BE, optional |
| `0x07` | `verified_via` | u8: QR / Safety Number / Fingerprint |
| `0x08` | `note` | UTF-8, ≤ 512 Bytes, optional |
| `0x09` | `previous_keys` | Liste früherer `sig_pub` mit Zeitpunkt |

`previous_keys` ist die Grundlage für den Zustand `Geändert` und **DARF NICHT**
beim Schlüsselwechsel überschrieben werden.

## 7. Auflösung beim Entschlüsseln

`envelope-v2.md` §12 Schritt 10. Die Bibliothek liefert:

```
enum Authenticity {
    Unsigned,
    SignedUnknown  { fingerprint },
    SignedSeen     { fingerprint, name },
    SignedVerified { fingerprint, name, verified_at },
    SignedChanged  { fingerprint, name, previous_fingerprint },
    SignedRevoked  { fingerprint, name },
}
```

Die Bibliothek **DARF NICHT** zusätzlich ein `bool` anbieten, aus dem sich
dieser Zustand einebnen ließe. Genau diese Einebnung war der Fehler in v1.

## 8. Darstellung

| Zustand | Anzeige | Farbe |
|---|---|---|
| `SignedVerified` | „Signiert von **Alice** ✓" | grün |
| `SignedSeen` | „Signiert von **Alice** — nicht verifiziert" | neutral |
| `SignedUnknown` | „Signiert von unbekanntem Schlüssel `K7QF-3MXB…`" | neutral |
| `SignedChanged` | „⚠ **Alice** verwendet einen neuen Schlüssel" | **Warnung** |
| `SignedRevoked` | „⚠ Schlüssel wurde als kompromittiert markiert" | **Warnung** |
| `Unsigned` | „Nicht signiert — Absender unbestimmt" | neutral |

**Verbindliche Regeln:**

1. Grün **nur** bei `SignedVerified`.
2. Ein Häkchen **nur** bei `SignedVerified`.
3. `SignedUnknown` **DARF NICHT** wie ein Fehler aussehen — anonymer Versand ist
   ein legitimer Modus, kein Mangel.
4. `SignedChanged` **MUSS** den Inhalt überlagern, nicht nur danebenstehen.
5. Nirgendwo erscheint das Wort „verifiziert" für einen nicht verifizierten
   Schlüssel.

## 9. Was der Trust Store nicht leistet

- **Kein Schutz bei kompromittiertem Gerät** (Threat Model A6). Wer den
  Prozess kontrolliert, kann Einträge ändern.
- **Keine Aussage über die Person hinter dem Schlüssel.** Verifiziert heißt:
  „dieser Schlüssel gehört zu dem Gegenüber, mit dem ich den Abgleich gemacht
  habe" — nicht, dass dieses Gegenüber die Wahrheit über seine Identität sagt.
- **Keine Übertragbarkeit.** Verifikationen gelten lokal. Es gibt kein Web of
  Trust und keine Zertifizierungsstelle. Bewusste Entscheidung: beides bringt
  erheblichen Aufwand und neue Angriffsflächen für eine Zielgruppe, die
  überwiegend mit einer überschaubaren Zahl bekannter Gegenüber arbeitet.

## 10. Offene Punkte

- Import und Export des Kontaktspeichers zwischen Geräten (verschlüsselt),
  vermutlich Phase 5
- Ob `Widerrufen` ohne Transportkanal sinnvoll durchsetzbar ist, oder nur
  lokale Markierung bleibt
- Genaue Ableitung der 60 Dezimalziffern aus dem Hash (Bytegrenzen, Modulo-Bias)
