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
| `keyfile.json` | 3 vollständige Keyfiles: mit Signierschlüssel, anonym, mit Bezeichnung | `tools/gen_keyfile.py` |
| `hpke/rfc9180-x25519-chacha.json` | Offizielle RFC-9180-Vektoren, gefiltert auf Mode Base und unsere Suite | `tools/filter_rfc9180.py` |
| `v1-compat.json` | 2 v1-Keyfiles und 5 v1-Envelopes mit bekanntem Passwort, samt kanonischer AAD | `tools/gen_v1_compat.py` |
| `xwing/draft10.json` | 3 X-Wing-Vektoren aus Anhang C des IETF-Entwurfs, Revision 10 | `tools/extract_xwing.py` |
| `metadata/` | 4 echte Bilddateien mit echtem EXIF, GPS und Vorschaubild | `tools/gen_metadata_fixtures.py` |

Alle Erzeuger sind **unabhängige Referenzen** — sie greifen nicht auf den
Rust-Code zurück. Das ist der Punkt: Ein Vektor, der nur bestätigt, was der
Code ohnehin tut, zementiert einen Implementierungsfehler zur Norm (§8 der
Spezifikation). Stimmen zwei unabhängig geschriebene Implementierungen
überein, ist das ein echter Nachweis.

Bei `keyfile.json` geht die Trennung besonders weit: Die Referenz nutzt
**libsodium** für Argon2id und ChaCha20-Poly1305, der Rust-Kern die
**RustCrypto**-Crates. Vier Implementierungen zweier Verfahren, byteweise
identisches Ergebnis.

Geprüft werden sie von `crates/cabrik-core/tests/vectors.rs`, und zwar in
**beide Richtungen**: Der Kern muss die Referenzdateien lesen *und* mit
festgelegtem Zufall bitgleiche Dateien erzeugen.

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

## Die Metadaten-Vorlagen sind ein Sonderfall

Sie werden **zweimal** geprüft. Der Rust-Test misst die Bytestruktur; danach
öffnet `tools/verify_metadata_stripped.py` die bereinigten Ergebnisse mit
Pillow und misst nach, was die Struktur nicht zeigt: ob das Bild noch gültig
ist, ob die Pixel unverändert sind, ob die Farbtabelle steht.

Der Grund ist der v1-Palette-Bug — er erzeugte eine **gültige Datei mit
falschen Farben**. So etwas fällt nur auf, wenn jemand das Bild wirklich
öffnet.

```
cargo test -p cabrik-metadata --test fixtures
python testvectors/tools/verify_metadata_stripped.py
```
