# Cabrik Secure — Keyfile-Format v2

**Status:** Entwurf · Phase 1, Dokument 4 von 7
**Setzt voraus:** `threat-model.md`, `envelope-v2.md`

---

## 1. Was sich gegenüber v1 ändert

| | v1 | v2 |
|---|---|---|
| Public Keys | **im Klartext** | verschlüsselt, aus den privaten Schlüsseln abgeleitet |
| Argon2id-Parameter | fest im Code (`OPSLIMIT_MODERATE`) | im Keyfile, versioniert, mit Untergrenzen |
| Verschlüsselung | zwei getrennte AEADs, je ein privater Schlüssel | ein AEAD über den gesamten Geheimnisblock |
| Format | JSON | binär, TLV wie `envelope-v2.md` |
| Erstellungszeitpunkt | Klartext | verschlüsselt |

### Warum die Public Keys nicht mehr im Klartext stehen

v1 legte `enc_pub` und `sig_pub` unverschlüsselt ab. Wer ein Keyfile findet —
gestohlener Laptop, Backup, beschlagnahmter Datenträger (Angreifermodell A4/A5)
— konnte damit belegen, **welche Identität** dem Gerät gehört, ohne das
Passwort zu kennen.

Beide Public Keys lassen sich aus den zugehörigen privaten Schlüsseln
berechnen. Sie im Klartext zu speichern war reiner Komfort und kostete
Schutzwirkung. In v2 enthält der unverschlüsselte Teil nur noch, was zum
Ableiten des Schlüssels aus dem Passwort nötig ist.

## 2. Aufbau

```
┌─ Klartext ─────────────────────────────────────┐
│  magic, version, Argon2id-Parameter, salt      │
├─ Verschlüsselter Geheimnisblock ───────────────┤
│  enc_sk, sig_sk, Erstellungszeit, Bezeichnung  │
└────────────────────────────────────────────────┘
```

| Offset | Größe | Feld | Wert |
|---|---|---|---|
| 0 | 2 | `magic` | `0xCA 0x4B` |
| 2 | 1 | `version` | `0x02` |
| 3 | 4 | `m_cost` | u32 BE, KiB |
| 7 | 4 | `t_cost` | u32 BE |
| 11 | 1 | `p_cost` | u8 |
| 12 | 16 | `salt` | Zufall |
| 28 | 4 | `secret_len` | u32 BE |
| 32 | … | `secret_ct` | AEAD-Ciphertext |

```
KEK       = Argon2id(password, salt, m_cost, t_cost, p_cost, out_len = 32)
secret_ct = ChaCha20Poly1305(key = KEK, nonce = 0^12,
                             aad = Bytes 0..28 des Keyfiles,
                             pt  = secret_plain)
```

`aad` umfasst den gesamten Klartextkopf. Manipulierte Argon2id-Parameter — etwa
das Herabsetzen auf `m_cost = 8` für einen billigen Rateangriff — führen damit
zu `KEYFILE_AUTH_FAILED` statt zu einer schwächeren Ableitung.

Nonce `0^12` ist zulässig, weil `KEK` durch das pro Keyfile zufällige `salt`
eindeutig ist.

## 3. Geheimnisblock (TLV)

Gleiche Regeln wie `envelope-v2.md` §7.2: aufsteigende `type`-Reihenfolge,
jeder Typ höchstens einmal, **unbekannter Typ ⇒ `MALFORMED`**.

| `type` | Feld | Typ | Pflicht |
|---|---|---|---|
| `0x01` | `enc_sk` | 32 Bytes X25519 | ja |
| `0x02` | `sig_sk` | 32 Bytes Ed25519-Seed | nein |
| `0x03` | `created` | u64 BE, Unix-Sekunden | ja |
| `0x04` | `label` | UTF-8, ≤ 64 Bytes | nein |
| `0x05` | `pq_seed` | 32 Bytes X-Wing-Seed | **ja** (siehe §3.1) |

Alle öffentlichen Schlüssel — `enc_pk`, `sig_vk` und der X-Wing-Public-Key —
werden nach dem Entschlüsseln berechnet und nie gespeichert.

Fehlt `sig_sk`, ist es ein **Anonymitäts-Keyfile**: die Identität kann
empfangen, aber nie dauerhaft signieren. Das entspricht `--no-signing` aus v1.

### 3.1 Warum der ML-KEM-Schlüssel Pflicht ist

Auch wenn Envelopes zunächst mit Suite `0x0001` geschrieben werden
(`envelope-v2.md` §4.2), **MUSS** jede in v2 erzeugte Identität ein
ML-KEM-768-Schlüsselpaar enthalten.

Der Grund ist der Umstellungszeitpunkt. Wird der Schlüssel erst später
eingeführt, müssen **alle** Nutzer neue Identitäten erzeugen, neu verteilen und
neu verifizieren — die teuerste denkbare Migration, und eine, bei der
erfahrungsgemäß ein großer Teil auf dem alten Verfahren stehenbleibt.

Mit dem Schlüssel ab Tag 1 ist der Wechsel auf Post-Quantum eine reine
Absenderentscheidung: Wer den X-Wing-Public-Key des Empfängers hat, wählt
Suite `0x0002` — ohne dass der Empfänger irgendetwas tun muss.

### 3.2 Gespeichert wird der Seed, nicht der Schlüssel

Der ML-KEM-768-Decapsulation-Key ist 2400 Bytes groß. Er wird **nicht**
gespeichert.

ML-KEM.KeyGen ist nach FIPS 203 deterministisch aus 64 Bytes Zufall
(`d ‖ z`). X-Wing geht einen Schritt weiter und leitet aus einem **einzigen
32-Byte-Seed** per SHAKE-256 sowohl `(d, z)` für ML-KEM als auch den
X25519-Anteil ab.

Gespeichert werden daher 32 Bytes; das Schlüsselpaar wird beim Entsperren
neu berechnet.

| | gespeichert |
|---|---|
| Expandierter Schlüssel | 2 432 Bytes |
| **Seed** | **32 Bytes** |

Damit kostet die Post-Quantum-Bereitschaft praktisch nichts — die anfangs
veranschlagten 2,4 KB je Keyfile entfallen.

**Abhängigkeit vom Entwurfsstand.** Die Expansionsfunktion ist Teil des
X-Wing-Entwurfs und noch nicht final. Ändert sie sich, ergäbe derselbe Seed
ein anderes Schlüsselpaar. Abgesichert wird das über das Feld `version` im
Klartextkopf: Keyfile-Version `0x02` ist an die im Anhang benannte
Entwurfsfassung gebunden; eine spätere Änderung erhält Version `0x03` und
kann alte Keyfiles weiterhin korrekt ableiten.

**Migrierte v1-Identitäten** (§5) erhalten dabei ein **neu erzeugtes**
ML-KEM-Paar. Die X25519- und Ed25519-Schlüssel bleiben unverändert, damit
bestehende Kontaktbeziehungen gültig bleiben — der Fingerprint ändert sich
dadurch allerdings, siehe `trust-store.md` §2.4.

## 4. Argon2id-Parameter

| | Wert |
|---|---|
| Empfohlen beim Schreiben | `m_cost = 262144` (256 MiB), `t_cost = 3`, `p_cost = 4` |
| Untergrenze beim Lesen | `m_cost ≥ 65536` (64 MiB), `t_cost ≥ 3`, `p_cost ≥ 1` |
| Obergrenze beim Lesen | `m_cost ≤ 4194304` (4 GiB) |

Die Untergrenze schützt vor untergeschobenen Schwachparametern. Die Obergrenze
schützt vor einem Keyfile, dessen Öffnen den Rechner in den Speicherüberlauf
treibt (Denial of Service durch eine präparierte Datei).

Ein Leser **MUSS** beide Grenzen erzwingen. Werte außerhalb ⇒ `MALFORMED`.

Zum Vergleich: v1 nutzte `OPSLIMIT_MODERATE`/`MEMLIMIT_MODERATE` von libsodium
— das entspricht etwa 256 MiB und 3 Durchgängen. Der Wert war angemessen, aber
nirgends festgehalten, und damit ohne Formatbruch nicht erhöhbar.

## 5. Migration von v1

v2 **MUSS** v1-Keyfiles lesen und **DARF NICHT** v1 schreiben.

**Erkennung:** v1-Keyfiles sind JSON und beginnen mit `{`.

**Ablauf:**

1. v1-Keyfile mit dem Passwort öffnen (Argon2id über `salt`, zwei getrennte
   AEADs mit `aad = "cabrik-keyfile|v1|Cabrik Secure"`)
2. `enc_sk` und, falls vorhanden, `sig_sk` entnehmen
3. Als v2 mit **frischem Salt** und aktuellen Parametern neu schreiben
4. Das v1-Keyfile **nicht** automatisch löschen

Punkt 4 ist bewusst: ein fehlgeschlagener Schreibvorgang darf nicht die einzige
Kopie der Identität vernichten. Das Löschen bietet die Oberfläche an, nachdem
das neue Keyfile nachweislich lesbar ist.

**Die Identität bleibt dieselbe.** Es entstehen keine neuen Schlüssel, nur eine
neue Verpackung — Public Keys, Fingerprints und bestehende Kontaktbeziehungen
bleiben gültig. Das ist der Grund, warum migriert und nicht neu erzeugt wird.

## 6. Umgang im Speicher

- Das Passwort **DARF NICHT** über die Entsperrung hinaus gehalten werden.
  v1 legte es dauerhaft in `STATE["keyfile"] = (path, pwd, ident)` ab, obwohl
  es nach dem Laden nie wieder gebraucht wurde.
- `KEK` und alle privaten Schlüssel **MÜSSEN** nach Gebrauch zeroisiert werden
  (`zeroize`), einschließlich der Zwischenpuffer beim Parsen.
- Private Schlüssel **DÜRFEN NICHT** an das Frontend gereicht werden
  (Threat Model §7.6).
- Der entsperrte Zustand **SOLLTE** nach konfigurierbarer Untätigkeit
  verfallen.

## 7. Sicherung und Wiederherstellung

Ein verlorenes Keyfile bedeutet: alle an diese Identität gerichteten Envelopes
sind dauerhaft unlesbar. Es gibt keine Hintertür und keinen Wiederherstellungs­
dienst.

Die Oberfläche **MUSS** beim Erzeugen einer Identität deutlich darauf hinweisen
und zur Sicherung auffordern.

**Nicht in 2.0, aber jetzt greifbar:** Wiederherstellungscodes nach Art einer
Seed-Phrase.

Durch §3.2 besteht die Identität bereits fast vollständig aus kurzen Seeds:
32 Bytes X25519, 32 Bytes Ed25519, 32 Bytes X-Wing. Ein einziger Master-Seed,
aus dem alle drei per HKDF abgeleitet werden, würde die gesamte Identität auf
**24 Wörter** einer BIP-39-artigen Liste reduzieren.

Das war vor der Seed-Entscheidung nicht praktikabel — mit einem 2400-Byte-
Schlüssel gibt es keine vorlesbare Darstellung. Jetzt ist es ein überschaubarer
Zusatzentwurf. Für 2.0 dennoch zurückgestellt: Ein zweiter, gleichwertiger
Zugangsweg zur Identität ist ein zweiter Angriffspunkt und braucht eigene
Sorgfalt bei Anzeige, Eingabe und Prüfsumme.

## 8. Mehrere Identitäten

**Eine Datei je Identität.** Die Anwendung verwaltet beliebig viele.

Ein Format mit mehreren Identitäten in einer Datei wurde verworfen: Die
Komplexität läge nicht im Format, sondern im gleichzeitigen Schreiben — sobald
zwei Vorgänge dieselbe Datei ändern wollen, braucht es Sperren, und ein
abgebrochener Schreibvorgang gefährdet **alle** Identitäten statt einer.

Bei einer Datei je Identität ist jede Operation atomar durch Umbenennen
umsetzbar, und der Verlust einer Datei kostet eine Identität, nicht alle.

`label` bleibt erhalten: Die Alternative — Anzeigenamen in der
Anwendungskonfiguration — verlagert Komplexität nach außen und bricht, sobald
der Nutzer die Datei umbenennt oder auf ein anderes Gerät kopiert. Ein
selbstbeschreibendes Keyfile ist die einfachere Lösung.

## 9. Anbindung an den OS-Schlüsselspeicher

Vorgesehen für Phase 4, **standardmäßig aus**.

**Ehrliche Einordnung des Gewinns:**

| Angreifer | Wirkung |
|---|---|
| A5 — Datenträger in fremder Hand | **echter Schutz.** Windows DPAPI bindet an die Anmeldedaten; ohne Anmeldung ist der Eintrag wertlos |
| A6 — laufendes, entsperrtes Gerät | **kein Schutz.** Jeder Prozess des Benutzers kann DPAPI-Daten entschlüsseln. A6 ist ohnehin außerhalb des Schutzbereichs |

Der Speicher bringt also Bequemlichkeit und Schutz gegen genau das Szenario,
gegen das das Keyfile-Passwort ohnehin schützt — er verschlechtert nichts,
solange die folgenden Regeln gelten.

**Verbindliche Bedingungen:**

1. Gespeichert wird **ausschließlich ein zeitlich begrenztes Sitzungstoken**,
   niemals die Passphrase und niemals ein privater Schlüssel.
2. Das Token verfällt nach konfigurierbarer Zeit und beim Abmelden.
3. Opt-in, mit klarer Erklärung, was es bewirkt und was nicht.
4. Auf macOS: `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`, Bindung an die
   Codesignatur der Anwendung, **niemals** Synchronisation in die
   iCloud-Keychain.
5. Auf Linux ist die Absicherung des Secret Service je nach Desktop sehr
   unterschiedlich — dort **SOLLTE** die Funktion mit einem entsprechenden
   Hinweis versehen werden.

## 10. Offene Punkte

- Master-Seed für Wiederherstellungscodes (§7): technisch geklärt und
  praktikabel, aber für 2.0 zurückgestellt. Zu entscheiden ist, ob die
  Ableitung `X25519 ‖ Ed25519 ‖ X-Wing = HKDF(master_seed)` schon jetzt
  festgelegt wird — dann ließen sich bestehende Identitäten später nachrüsten,
  statt neue erzeugen zu müssen. Dieselbe Überlegung wie beim ML-KEM-Schlüssel
  in §3.1.
- Welche Entwurfsfassung von X-Wing für Keyfile-Version `0x02` verbindlich
  ist (§3.2) — festzulegen, sobald mit der Implementierung begonnen wird
- Ob das Keyfile eine Kennung tragen sollte, die es einem Gerät zuordnet,
  um Mehrfachnutzung derselben Identität erkennbar zu machen
