# Testvektoren

Sprachunabhängige Prüffälle: feste Eingaben → feste Envelopes, als JSON.

Zweck: nachweisen, dass Desktop-, iOS- und Android-Implementierung bitgenau
dasselbe tun. Ohne diese Vektoren ist die Portierung nach Swift und Kotlin
in Phase 6 nicht verifizierbar.

## Vorhanden

| Datei | Inhalt | Erzeugt von |
|---|---|---|
| `padme.json` | 16 Vektoren inkl. Randfälle (0, `PAD_MIN`, Zweierpotenzen, Chunk-Grenze, größter Verschnitt, 1 TiB) | `tools/gen_padme.py` |
| `fingerprint.json` | 7 Fingerprints und 3 Safety Numbers, einschließlich der Null-Schlüssel-Fälle aus `trust-store.md` §2.1 | `tools/gen_fingerprint.py` |

Beide Erzeuger sind **unabhängige Python-Referenzen** — sie greifen weder auf
den Rust-Code noch (bei HKDF) auf eine Bibliothek zurück. Das ist der Punkt:
Ein Vektor, der nur bestätigt, was der Code ohnehin tut, zementiert einen
Implementierungsfehler zur Norm (§8 der Spezifikation). Stimmen zwei
unabhängig geschriebene Implementierungen überein, ist das ein echter Nachweis.

Geprüft werden sie von `crates/cabrik-core/tests/vectors.rs`.

## Geplanter Umfang

- **v1-Kompatibilität** — mit `legacy/python-v1` erzeugte Envelopes und
  Keyfiles mit bekanntem Passwort, damit der v2-Leser sie nachweislich öffnet.
  Die Original-Testdateien aus der v1-Entwicklung liegen unter `_archive_v1/`,
  sind aber unbrauchbar, weil die zugehörigen Passwörter nicht bekannt sind.
- **v2-Format** — HPKE-Vektoren, Streaming über mehrere Chunks,
  Mehrfachempfänger, Passwort-Modus, Keyfile-Migration.
- **Negativfälle** — manipulierte Header, falsche Empfänger, abgeschnittene
  Envelopes, unbekannte Formatversionen. Jeder muss zuverlässig fehlschlagen.

Die Dateien hier sind von `.gitignore` bewusst ausgenommen und gehören ins
Repository. Sie enthalten ausschließlich Wegwerf-Schlüssel.
