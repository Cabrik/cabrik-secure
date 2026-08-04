# Spezifikation

Entstanden in **Phase 1**, geschrieben *vor* jeder Zeile Rust — das Format muss
über Desktop, iOS und Android identisch sein und jahrelang halten.

**Status: Entwurf.** Noch nicht eingefroren. Die offenen Punkte am Ende jedes
Dokuments werden vor dem Einfrieren geklärt.

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
| Empfänger je Envelope | 1 | bis 255 |
| Klartext-Metadaten im Envelope | 7 Felder | keine |

Der v1-Overhead ist mit `legacy/python-v1/smoke_test.py` empirisch bestätigt.

## Nächster Schritt

Offene Punkte klären, dann einfrieren. Ab dem Einfrieren gilt: Änderungen nur
über eine neue Formatversion mit Migrationspfad — und die Testvektoren
(`testvectors/`) bleiben unverändert bestehen, damit Abwärtskompatibilität
prüfbar ist.
