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

`enc_pk` und `sig_vk` werden nach dem Entschlüsseln berechnet, nie gespeichert.

Fehlt `sig_sk`, ist es ein **Anonymitäts-Keyfile**: die Identität kann
empfangen, aber nie dauerhaft signieren. Das entspricht `--no-signing` aus v1.

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

**Nicht in 2.0:** Wiederherstellungscodes nach Art einer Seed-Phrase. Technisch
möglich (der X25519-Seed ließe sich als BIP-39-artige Wortfolge darstellen),
aber es entsteht ein zweiter, gleichwertiger Angriffspunkt, der eigene
Sorgfalt braucht. Vermerkt für später.

## 8. Offene Punkte

- Ob mehrere Identitäten in einer Datei liegen können sollen, oder je eine
  Datei je Identität (aktuell: je eine Datei)
- Anbindung an OS-Schlüsselspeicher (Windows DPAPI, macOS Keychain,
  Linux Secret Service) für den entsperrten Sitzungszustand → Phase 4
- Ob `label` überhaupt nötig ist, wenn der Dateiname bereits benennt
