# Cabrik Secure — Trust Store

**Status:** Verbindlich · Phase 1, Dokument 5 von 7
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
fingerprint = SHA-256( "cabrik-fp-v2"
                     ‖ enc_pub(32)
                     ‖ has_sig(1)   ‖ sig_pub(32)
                     ‖ has_pq(1)    ‖ xwing_pub(1216) )
```

Volle **256 Bit**. `has_sig` und `has_pq` sind `0x01`, wenn der jeweilige
Schlüssel vorhanden ist, sonst `0x00`; im Fall `0x00` stehen an seiner Stelle
Nullbytes in voller Länge.

**Korrektur gegenüber Stand 2.** Dort stand `mlkem_pub(1184)`. Das war falsch:
Ein X-Wing-Public-Key ist 1216 Bytes lang und besteht aus dem ML-KEM-Schlüssel
(1184) **plus einem eigenen X25519-Anteil** (32) — und der ist ein *anderer*
als `enc_pub`. Wer nur die 1184 Bytes führt, kann den X-Wing-Schlüssel nicht
rekonstruieren und an diesen Kontakt niemals mit Suite `0x0002` verschlüsseln.
Der Fehler kam beim Verdrahten des Trust Stores heraus.

Intern wird stets der volle Wert verglichen. Gekürzt wird nur für die Anzeige.

Der Post-Quantum-Public-Key gehört **zwingend** in die Ableitung: Ohne ihn hätten zwei
Identitäten mit gleichen klassischen, aber verschiedenen Post-Quantum-Schlüsseln
denselben Fingerprint. Ein Angreifer könnte dann einen eigenen ML-KEM-Schlüssel
unterschieben, ohne dass die Verifikation es bemerkt — und damit genau den
Schutz aushebeln, für den Suite `0x0002` gebaut wurde.

### 2.1 Warum Präsenz-Bytes und nicht nur Nullbytes

Ein früherer Entwurf ersetzte fehlende Schlüssel schlicht durch Nullbytes.
Damit wäre „kein Schlüssel vorhanden" nicht unterscheidbar von „Schlüssel
besteht aus lauter Nullen" gewesen.

Bei Ed25519 wäre das folgenlos: Zu einem Null-Public-Key ist kein passender
privater Schlüssel bekannt, es ließe sich also nicht damit signieren.

**Beim Post-Quantum-Schlüssel ist es ein Angriff.** Ein Encapsulation Key aus lauter Nullen ist
syntaktisch gültig. Ein Angreifer könnte eine Identität mit genau diesem
Schlüssel anlegen; ihr Fingerprint stimmte dann mit dem eines aus v1
migrierten Kontakts überein, der **gar keinen** PQ-Schlüssel besitzt. Wer
diesen Kontakt verifiziert hat und ihm anschließend mit Suite `0x0002`
schreibt, verschlüsselte an den Schlüssel des Angreifers.

Die Präsenz-Bytes schließen das, kosten zwei Bytes im Hash-Eingang und keine
Laufzeit. Der Fall wurde beim Implementieren von `Fingerprint::compute`
entdeckt — die Eigenschaft „`None` und ein Null-Schlüssel dürfen nicht
kollidieren" war als Test formuliert und schlug fehl.

### 2.2 Darstellung

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

### 2.3 Warum 32 Zeichen

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

### 2.4 Migrierte v1-Identitäten bekommen einen neuen Fingerprint

Eine aus v1 migrierte Identität behält ihre X25519- und Ed25519-Schlüssel,
erhält aber ein neu erzeugtes ML-KEM-Paar (`keyfile-v2.md` §3.1). Da dieses in
die Ableitung eingeht, **ändert sich der Fingerprint**.

Das ist unvermeidlich und wird nicht versteckt. Konsequenzen:

- Bestehende Kontakte sehen den Zustand `Geändert` (§4.2) — korrekt, denn es
  *ist* neues Schlüsselmaterial.
- Die Oberfläche **MUSS** bei der Migration darauf hinweisen und empfehlen,
  bestehende Gegenüber einmalig neu zu verifizieren.
- Empfangen bleibt uneingeschränkt möglich: Alte Envelopes an den
  X25519-Schlüssel werden weiterhin entschlüsselt.

Die Alternative — den Fingerprint nur aus den klassischen Schlüsseln zu bilden —
wurde wegen des in §2 beschriebenen Unterschiebungsangriffs verworfen.

## 3. Safety Number

Zum gegenseitigen Abgleich vergleichen beide Seiten **eine** Zeichenfolge statt
zweier Fingerprints — nach dem Vorbild der Signal Safety Numbers.

```
(a, b)   = Fingerprints, lexikografisch sortiert (jeweils 32 Bytes)
base     = SHA-256( "cabrik-sn-v2" ‖ a ‖ b )
material = HKDF-SHA256(ikm = base, salt = "", info = "cabrik-sn-digits", L = 96)

für i in 0..12:
    g          = u64_be( material[i*8 .. i*8+8] )
    digits[i]  = g mod 100000        # als 5 Ziffern, links mit Nullen aufgefüllt
```

Angezeigt als **60 Dezimalziffern** in 12 Gruppen zu 5 — vorlesbar am Telefon,
sprachunabhängig:

```
41827  09365  72104  88519  36047  21968
50713  84226  19570  63841  27395  70612
```

Die Sortierung sorgt dafür, dass beide Seiten dieselbe Zahl sehen, unabhängig
davon, wer fragt.

### 3.1 Warum 8 Bytes je Gruppe

`mod 100000` erzeugt eine Verzerrung, weil der Wertebereich kein Vielfaches von
100000 ist — kleine Ergebnisse treten geringfügig häufiger auf.

| Bytes je Gruppe | Verzerrung |
|---|---|
| 5 (Signals Verfahren) | ≈ 2,5 · 10⁻⁸ |
| **8** | **≈ 2,8 · 10⁻¹⁵** |

Der Unterschied ist praktisch bedeutungslos — 5 Bytes wären völlig ausreichend.
8 Bytes kosten aber nichts außer 96 statt 60 Bytes Ableitungsmaterial, und
ersparen die Diskussion.

**Rejection Sampling wurde ausdrücklich verworfen.** Es würde die Verzerrung
vollständig beseitigen, macht die Ableitung aber datenabhängig in der Zahl der
Schritte. Für eine Funktion, die in Testvektoren bit-genau reproduzierbar sein
muss (`test-vectors.md` §3), ist das der falsche Tausch: exakte Gleichverteilung
gegen deterministische Nachvollziehbarkeit — und die Gleichverteilung wird hier
nicht gebraucht.

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

### 4.3 Widerruf: in 2.0 nur lokal

Ein Widerruf ohne Verteilweg erreicht niemanden außer demjenigen, der ihn
einträgt. Cabrik Secure hat keinen Transportkanal, und einen zu bauen, um
Widerrufe zu verteilen, würde den Rahmen des Projekts sprengen.

**In 2.0 umgesetzt:** `Widerrufen` ist eine rein **lokale Markierung**. Der
Nutzer trägt ein, dass er einem Schlüssel nicht mehr traut; die Anwendung warnt
künftig bei Nachrichten von diesem Schlüssel. Mehr nicht — und die Oberfläche
**MUSS** klarstellen, dass diese Markierung niemanden sonst erreicht.

**Reserviert, nicht implementiert:** Eine in-band-Widerrufserklärung. Die Idee:
Alice legt eine signierte Erklärung „Schlüssel F ist ab Zeitpunkt T widerrufen"
in eine *spätere* Nachricht; Empfänger übernehmen sie beim Lesen. Der Widerruf
verbreitet sich damit mit der Geschwindigkeit der Kommunikation, ohne jede
Infrastruktur.

Zwei Regeln wären dafür zwingend:

1. **Monoton.** Einmal widerrufen wird **niemals** automatisch zurückgenommen.
   Sonst könnte ein Angreifer, der den Schlüssel bereits besitzt, den Widerruf
   mit einer eigenen Erklärung aufheben.
2. Ein Angreifer im Besitz des Schlüssels kann damit einen Widerruf
   *auslösen* — das ist eine Dienstverweigerung, aber kein Bruch der
   Vertraulichkeit, und Alice erzeugt schlicht eine neue Identität.

TLV-Typ `0x09` im verschlüsselten Header ist dafür reserviert
(`envelope-v2.md` §7.2). In 2.0 wird er nicht geschrieben und **MUSS** beim
Lesen abgelehnt werden. Die Nummer jetzt festzulegen kostet nichts; sie später
nachzuschieben würde eine neue Formatversion erfordern.

## 5. Verifikationswege

| Weg | Aufwand | Sicherheit |
|---|---|---|
| **QR-Code** | gering | hoch, erfordert physische Nähe |
| **Safety Number vorlesen** | mittel | hoch, wenn die Stimme erkannt wird |
| **Fingerprint tippen** | hoch | hoch |
| Fingerprint per Messenger senden | gering | **gering** — derselbe Kanal, denselben Angreifer |

Die letzte Zeile **MUSS** in der Oberfläche benannt werden. Ein Fingerprint,
der über denselben Kanal kommt wie die Nachricht, beweist nichts.

### 5.1 Austausch-Nutzlast

```
cabrik:v2:<Base32 enc_pub>:<Base32 sig_pub>:<Base32 xwing_pub>:<Base32 fingerprint[0..8]>
```

Der Fingerprint-Anfang dient nur als Prüfsumme gegen Übertragungsfehler. Der
Leser **MUSS** den Fingerprint aus den Schlüsseln neu berechnen und **DARF
NICHT** dem übertragenen Wert vertrauen.

Fehlen `sig_pub` oder `xwing_pub`, steht dort ein leeres Feld. Die Prüfsumme
wird über **genau den Schlüsselsatz** gebildet, der übertragen wird — bei
fehlendem Feld also mit `None` nach §2, nicht mit Nullbytes.

**Korrektur gegenüber Stand 3.** Dort führte die Nutzlast nur `enc_pub` und
`sig_pub`. Das war aus zwei Gründen falsch:

1. §2 nimmt `xwing_pub` zwingend in den Fingerprint. Wer die Nutzlast ohne
   ihn einliest, legt einen Kontakt mit `xwing_pub = None` an und berechnet
   damit einen **anderen** Fingerprint als den, den die Gegenseite anzeigt.
   Zwei ehrliche Beteiligte hätten sich nie verifizieren können — die
   Verifikation wäre genau in dem Fall fehlgeschlagen, für den sie gebaut ist.
2. Ohne den Schlüssel ist Suite `0x0002` für jeden so angelegten Kontakt
   unerreichbar. Der gesamte Post-Quantum-Pfad wäre totes Gewicht gewesen.

Der Fehler fiel beim Verdrahten der CLI auf. Er blieb vorher unentdeckt, weil
sämtliche Tests der QR-Funktionen `xwing_pub = None` übergaben und damit
dieselbe blinde Stelle abbildeten wie der Code.

**Größe.** Die Nutzlast wird dadurch rund 2050 Zeichen lang — als QR-Code
etwa Version 29, dicht aber lesbar. Wo ein QR-Code unpraktisch ist (CLI,
E-Mail-Anhang), wird dieselbe Zeichenfolge als Datei ausgetauscht. Es gibt
bewusst **nur ein** Austauschformat: zwei Formate hießen zwei Prüfsummenregeln
und damit die Wiederkehr genau dieses Fehlers.

## 6. Speicherformat

Eigene Datei neben dem Keyfile, verschlüsselt mit einem Schlüssel, der aus dem
Keyfile-Geheimnis abgeleitet wird:

```
contacts_key = HKDF-SHA256(ikm = enc_sk, salt = "", info = "cabrik-v2 contacts", L = 32)
```

Damit ist der Kontaktspeicher nur bei entsperrter Identität lesbar. Ein
Angreifer mit dem Dateisystemzugriff sieht nicht, **mit wem** kommuniziert wird
— eine der aussagekräftigsten Metadaten überhaupt.

Aufbau wie `keyfile-v2.md` §2, mit **einem entscheidenden Unterschied**:

```
magic(2) = 0xCA 0x43
version(1) = 0x02
nonce(12)                       ← zufällig, bei jedem Schreiben neu
ciphertext(...)                 ← AEAD, AAD = die 15 Bytes des Kopfes
```

*Korrektur gegenüber Stand 3.* Dort stand `salt` statt `nonce`, in Anlehnung
an das Keyfile. Das wäre ein Bruch gewesen: Das Keyfile darf einen Null-Nonce
führen, weil ein frisches Salz bei jedem Schreiben einen **neuen** Schlüssel
erzeugt. Hier gibt es kein Salz — der Schlüssel kommt aus `HKDF(enc_sk)` und
ist bei jedem Schreiben **derselbe**. Ein fester Nonce hieße also
Nonce-Wiederverwendung über alle Fassungen der Datei hinweg. Bei
ChaCha20-Poly1305 gibt das den XOR-Unterschied zweier Fassungen preis und
erlaubt darüber hinaus, den Authentisierungsschlüssel zu berechnen und
Fälschungen zu bauen.

Der Nonce **MUSS** deshalb bei jedem Schreiben neu gezogen werden. Der Fall
fiel auf, als die CLI den Speicher zum ersten Mal wirklich anlegte.

Kontakteinträge als TLV:

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
| `0x09` | `previous_keys` | Schlüsselhistorie, siehe unten |
| `0x0A` | `xwing_pub` | 1216 Bytes X-Wing, optional |
| `0x0B` | `revoked_at` | u64 BE, optional |
| `0x0C` | `revocation_note` | UTF-8, ≤ 256 Bytes, optional |

`xwing_pub` ist optional, weil aus v1 migrierte Kontakte ihn zunächst nicht
haben. Fehlt er, kann an diesen Kontakt nur mit Suite `0x0001` verschlüsselt
werden — die Oberfläche **SOLLTE** das anzeigen.

**Schlüsselhistorie.** `previous_keys` führt je Eintrag den vollständigen
früheren Schlüsselsatz mit Zeitpunkt und dem damaligen Vertrauenszustand:

```
entry_count : u16 BE
je Eintrag:
    fingerprint  : 32 Bytes
    replaced_at  : u64 BE
    was_verified : u8
```

Sie ist die Grundlage für den Zustand `Geändert` (§4.2) und **DARF NICHT**
beim Schlüsselwechsel überschrieben werden. `was_verified` ist wichtig: Der
Wechsel eines *verifizierten* Schlüssels wiegt schwerer als der eines nie
verifizierten und **MUSS** deutlicher gewarnt werden.

Damit ist Schlüsselrotation bereits in 2.0 im Format abgebildet, auch wenn die
Bedienoberfläche zunächst nur den Warnfall zeigt.

## 7. Auflösung beim Entschlüsseln

`envelope-v2.md` §12 Schritt 10. Die Bibliothek liefert:

```
enum Authenticity {
    Unsigned,
    SignedUnknown  { sig_pub },
    SignedSeen     { fingerprint, name },
    SignedVerified { fingerprint, name, verified_at },
    SignedChanged  { fingerprint, name, previous_fingerprint },
    SignedRevoked  { fingerprint, name },
}
```

Die Bibliothek **DARF NICHT** zusätzlich ein `bool` anbieten, aus dem sich
dieser Zustand einebnen ließe. Genau diese Einebnung war der Fehler in v1.

### 7.1 Warum `SignedUnknown` keinen Fingerprint trägt

*Korrektur gegenüber Stand 2.* Dort stand `SignedUnknown { fingerprint }` —
das ist nicht berechenbar.

Eine Signatur liefert ausschließlich den **Ed25519-Signierschlüssel**. Der
Fingerprint entsteht aber aus `enc_pub ‖ sig_pub ‖ mlkem_pub` (§2). Bei einem
unbekannten Absender fehlen zwei der drei Bestandteile; es gibt schlicht
nichts, woraus sich ein Fingerprint bilden ließe.

`SignedUnknown` trägt daher den Signierschlüssel selbst. Die Oberfläche zeigt
dessen Crockford-Base32-Darstellung und beschriftet sie als
**Signierschlüssel**, nicht als Fingerprint.

Die Alternative — ein zweiter, eigener Hash allein über `sig_pub` — wurde
verworfen: Dann gäbe es zwei Größen, die beide „Fingerprint" heißen und sich
**nicht** miteinander vergleichen lassen. Genau diese Verwechslung untergräbt
ein Vertrauensmodell. Was man hat, wird benannt, wie es heißt.

### 7.1.1 Aus `SignedUnknown` entsteht **kein** Kontakt

Es liegt nahe, den unbekannten Absender gleich aufzunehmen — „Trust on First
Use". Eine Implementierung **DARF** das nicht tun, und der Grund ist derselbe
wie in §7.1, nur eine Stufe weitergedacht.

Ein Kontakt braucht `enc_pub`. Aus der Nachricht ist er nicht zu gewinnen: Der
Schlüsselaustausch ist ephemer, der dauerhafte Verschlüsselungsschlüssel des
Absenders steht nirgends im Envelope. Das ist eine **Stärke** des Formats —
genau dieses Feld schickte v1 offen mit und machte damit jeden Absender für
Mitleser erkennbar (`envelope-v2.md` §13).

Ein Eintrag mit leerem `enc_pub` hätte drei Folgen, jede für sich
disqualifizierend:

1. Sein Fingerprint entstünde über einen Nullschlüssel und stimmte mit
   **nichts** überein, was die Gegenseite anzeigt. Die Oberfläche lüde zu einer
   Verifikation ein, die niemals gelingen kann.
2. Ein Verschlüsselungsversuch liefe gegen einen unbrauchbaren Schlüssel
   (`test-vectors.md` §7.1).
3. `supports_post_quantum` wäre falsch, und die Begründung dafür wäre gelogen.

Die Oberfläche **MUSS** stattdessen den Signierschlüssel zeigen und den Weg
nennen: Wer den Absender wiedererkennen will, braucht dessen Austausch-Nutzlast
(§5.1) — die er ohnehin braucht, um zu antworten. Ab dann greift die Erkennung
von Schlüsselwechseln nach §7.2.

Der Fall kam beim Verdrahten der CLI heraus. Sie legte den Absender zunächst
automatisch an, und `contacts show` zeigte prompt einen Fingerprint, den die
Gegenseite nie zu Gesicht bekommen konnte.

### 7.2 Nachschlagen geschieht über `sig_pub`

Aus demselben Grund ist der Signierschlüssel der Suchschlüssel, nicht der
Fingerprint. Ein Kontakt ohne `sig_pub` (Anonymitätsidentität) ist über eine
Signatur grundsätzlich nicht auffindbar — er signiert ja nie.

Drei Ausgänge:

| Fund | Ergebnis |
|---|---|
| `sig_pub` ist der **aktuelle** Schlüssel eines Kontakts | Zustand des Kontakts |
| `sig_pub` steht in dessen **`previous_keys`** | `SignedChanged` |
| nirgends gefunden | `SignedUnknown` |

Der mittlere Fall verdient die Warnung genauso wie der ursprünglich gemeinte:
Entweder hat das Gegenüber den Schlüssel gewechselt und benutzt noch den
alten — oder jemand anderes verwendet einen ausgemusterten Schlüssel. Beides
soll auffallen.

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

## 10. Import und Export

Vorgesehen für **Phase 5**, ohne Einbußen bei der Sicherheit — sofern die
folgenden Regeln gelten.

- Die Exportdatei wird unter einem **eigenen, frisch abgeleiteten Schlüssel**
  verschlüsselt (Argon2id über eine vom Nutzer gewählte Passphrase), **nicht**
  unter dem Identitätsschlüssel. Sonst würde ein Export den Identitätsschlüssel
  faktisch mit übertragen.
- Die Verifikationszustände werden **mit exportiert**. Das ist zulässig, weil
  der Export authentifiziert ist und aus dem eigenen Bestand stammt — es ist
  eine Gerätesynchronisation, keine Vertrauensübertragung an Dritte.
- Beim Import wird **zusammengeführt, nie überschrieben**. Konflikte —
  derselbe Schlüssel unter anderem Namen, derselbe Name mit anderem Schlüssel —
  **MÜSSEN** dem Nutzer einzeln vorgelegt werden.
- Ein Import **DARF NICHT** einen lokalen Zustand `Widerrufen` aufheben
  (Monotonie, §4.3).

Das Format aus §6 trägt das bereits; es braucht dafür keine Erweiterung.

## 11. Entschiedene Punkte

| Frage | Entscheidung |
|---|---|
| Import/Export | Ja, Phase 5, unter eigener Passphrase, mit Zusammenführung statt Überschreiben (§10) |
| Widerruf | In 2.0 nur lokale Markierung; in-band-Erklärung als TLV `0x09` reserviert (§4.3) |
| Safety-Number-Ableitung | 8 Bytes je Gruppe über HKDF, kein Rejection Sampling (§3.1) |
| Schlüsselrotation | Historie ab 2.0 im Format, inklusive `was_verified` (§6) |
| PQ-Schlüssel im Fingerprint | Ja, zwingend, in voller X-Wing-Länge — sonst Unterschiebungsangriff (§2) |

## 12. Offene Punkte

- Ob der Wechsel eines verifizierten Schlüssels den Kontakt automatisch auf
  `Gesehen` zurückstufen sollte, oder ob `Geändert` als eigener Zustand
  bestehen bleibt, bis der Nutzer entscheidet
- Ob die Safety Number bei Kontakten ohne `xwing_pub` gesondert gekennzeichnet
  werden muss, damit nach deren Migration keine Verwirrung entsteht
