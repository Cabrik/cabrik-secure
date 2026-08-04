# Testvektoren

Sprachunabhängige Prüffälle: feste Eingaben → feste Envelopes, als JSON.

Zweck: nachweisen, dass Desktop-, iOS- und Android-Implementierung bitgenau
dasselbe tun. Ohne diese Vektoren ist die Portierung nach Swift und Kotlin
in Phase 6 nicht verifizierbar.

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
