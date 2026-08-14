# Spezifikation

Entstanden in **Phase 1**, geschrieben *vor* jeder Zeile Rust — das Format muss
über Desktop, iOS und Android identisch sein und jahrelang halten.

**Status: Entwurf, Stand 2 — offene Punkte entschieden.** Noch nicht
eingefroren. Jedes Dokument führt seine getroffenen Entscheidungen und die
verbliebenen offenen Punkte gesondert auf.

## Dokumente

Die Reihenfolge ist verbindlich, jedes Dokument setzt die vorherigen voraus.

| # | Dokument | Inhalt |
|---|---|---|
| 1 | [threat-model.md](threat-model.md) | Schutzgüter, neun Angreifermodelle, Vertrauensannahmen, Ausschlüsse |
| 2 | [test-vectors.md](test-vectors.md) | Injizierbare Zufallsquelle, drei Konformitätsebenen, Pflichtabdeckung |
| 3 | [envelope-v2.md](envelope-v2.md) | HPKE, Binärformat, Chunk-Streaming, Mehrfachempfänger, Passwort-Modus |
| 4 | [keyfile-v2.md](keyfile-v2.md) | Argon2id versioniert, verschlüsselte Public Keys, Migration von v1 |
| 5 | [trust-store.md](trust-store.md) | Kontakte, Fingerprints, Safety Numbers, sechs Vertrauenszustände |
| 6 | [metadata.md](metadata.md) | Fähigkeitsmodell, Formatabdeckung, Inspektion |
| 7 | [shredding.md](shredding.md) | Ehrliche Löschgarantien, Crypto-Shredding |
| 8 | [anzeige.md](anzeige.md) | Vier Anzeigezustände, zweite Farbachse, Zuordnung |
| 9 | [entsperrung.md](entsperrung.md) | Sitzung, Sperre nach Untätigkeit, der Weg des Passworts |

Dokumente 8 und 9 sind später entstanden — 8 in Phase 3, als die Oberfläche
begann, 9 in Phase 4 vor der Entsperrung. Beide folgen derselben Regel:
**geschrieben, bevor eine Zeile Code entsteht.**

Dokument 2 steht bewusst **vor** dem Formatdokument: Bit-Genauigkeit über
mehrere Implementierungen ist eine Anforderung an die Architektur, nicht an die
Tests. Wird sie nicht vorher festgeschrieben, ist sie später nicht nachrüstbar.

## Die vier Befunde, die v2 begründen

| Befund | Fundort in v1 | Behandelt in |
|---|---|---|
| **Der Header verrät alles.** Dateiname, Klartextgröße, Empfänger-Fingerprint, Zeitstempel, Produktname — und bei signierten Nachrichten die **dauerhafte Absenderkennung**. Authentizität und Anonymität schlossen sich damit aus | `crypto_core.py:176-199` | envelope-v2 §3, §9 |
| **`signature_valid` beweist nichts.** Der Prüfschlüssel stammt aus derselben Nachricht — die Prüfung ist zirkulär | `crypto_core.py:225` | trust-store, threat-model §8 |
| **Klartext landet auf der Platte.** Mehrere Anhänge wurden als unverschlüsseltes ZIP nach `%TEMP%` geschrieben, bevor verschlüsselt wurde | `gui/app.py:365-376` | shredding §3, envelope-v2 §7.4 |
| **Unbekannte Formate galten als bereinigt.** `shutil.copy2` kopierte stillschweigend durch — und erhielt sogar die Zeitstempel | `crypto_core.py:367` | metadata §3 |

Die ersten beiden sind Konstruktionsfehler des Formats und lassen sich nur mit
einem Formatwechsel beheben. Das ist der eigentliche Grund für v2.

## Was sich messbar ändert

| | v1 | v2 |
|---|---|---|
| Envelope-Overhead | +78,1 % | +0,03 % binär |
| Speicherbedarf | ~4–5× Dateigröße | konstant ~256 KiB |
| Fingerprint | 32 Bit (16 Bit Kollisionsschutz) | 160 Bit angezeigt |
| Empfänger je Envelope | 1 | bis 32 |
| Klartext-Metadaten im Envelope | 7 Felder | keine |
| Schutz gegen künftige Quantencomputer | keiner | Suite `0x0002` |

Der v1-Overhead ist mit `legacy/python-v1/smoke_test.py` empirisch bestätigt.

## Die Post-Quantum-Entscheidung

Zwei Ciphersuites: `0x0001` klassisch (X25519), `0x0002` hybrid
(X-Wing = X25519 + ML-KEM-768). Beide sind verbindlich zu implementieren, die
Voreinstellung bleibt zunächst klassisch — nicht aus Zweifel an ML-KEM, sondern
weil ein X-Wing-Public-Key rund 1 620 Base64-Zeichen ergibt und damit den
Austausch per Zwischenablage beendet.

**Entscheidend ist, dass der spätere Wechsel nichts kostet:** Jede in v2
erzeugte Identität trägt ab Tag 1 einen ML-KEM-Schlüssel. Der Umstieg ist dann
eine reine Absenderentscheidung — niemand muss neue Schlüssel erzeugen oder neu
verteilen. Wäre der Schlüssel erst später eingeführt worden, hätte es die
teuerste denkbare Migration erfordert.

## Nächster Schritt

Die verbliebenen offenen Punkte klären, dann einfrieren. Ab dem Einfrieren
gilt: Änderungen nur über eine neue Formatversion mit Migrationspfad — und die
Testvektoren (`testvectors/`) bleiben unverändert bestehen, damit
Abwärtskompatibilität prüfbar ist.
