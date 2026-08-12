# Fuzzing

Fünf Ziele, alle auf Eingaben gerichtet, die **von außen** kommen:

| Ziel | Was es beschießt |
|---|---|
| `envelope_open` | der Umschlag, mit einer Identität geöffnet |
| `envelope_open_passwort` | derselbe, mit einem Passwort — andere Kapselsuche |
| `metadata_inspect` | die Formaterkennung und alle siebzehn Leser |
| `metadata_strip` | das Bereinigen, **zweimal hintereinander** |
| `v1_open` | das Altformat, dessen Kopf im Klartext steht |

## Warum das nötig ist

Jeder andere Test in diesem Projekt füttert die Leser mit Dateien, die
dieses Projekt selbst oder ffmpeg erzeugt hat — also mit wohlgeformten. Sie
prüfen, ob der Leser zum eigenen Schreiber passt.

Ein Umschlag kommt aber immer von außen, und er wird verarbeitet, **bevor**
irgendetwas beglaubigt ist: Längen, Anzahlen und Versätze müssen gelesen
werden, um überhaupt an die Beglaubigung zu kommen. Ein Absturz dort ist der
Unterschied zwischen „die Datei wird abgelehnt" und „das Programm bricht ab".

## Aufruf

```sh
cargo +nightly fuzz run envelope_open
cargo +nightly fuzz run metadata_inspect -- -max_total_time=600
cargo +nightly fuzz list          # alle Ziele
```

Braucht `cargo install cargo-fuzz` und eine nightly-Toolchain. Die
Hauptwerkbank bleibt davon unberührt: `fuzz/` ist eine **eigene** Werkbank
(`exclude = ["fuzz"]` in der Wurzel), damit die Festlegung auf 1.97.1 nicht
aufgeweicht wird.

## Auf Windows

Die Ziele **bauen** hier einwandfrei — mit nightly, libFuzzer und
Abdeckungsinstrumentierung. Sie **starten** aber nicht:

```
STATUS_DLL_NOT_FOUND
```

Es fehlt `clang_rt.asan_dynamic-x86_64.dll`, die Laufzeit des
Adressprüfers. Sie ist ein **optionaler Bestandteil von Visual Studio**
(„C++ AddressSanitizer") und auf diesem Rechner nicht installiert.
`--sanitizer=none` hilft nicht: Dann fehlen dem Binder die
Abdeckungssymbole (`__stop___sancov_pcs`), die aus derselben Laufzeit
kommen.

Das ist keine Notlage. Fuzzing gehört in die **CI auf Linux**, wo es ohne
Zutun läuft und Stunden statt Minuten laufen kann. Wer es lokal auf Windows
braucht, installiert den genannten Visual-Studio-Bestandteil nach.

**Auf diesem Rechner geprüft wird stattdessen** mit den
Robustheitstests in `crates/*/tests/robustheit.rs`: dieselbe Idee,
deterministische Verstümmelung mit festem Startwert, läuft auf stable in
jedem `cargo test`. 13 000 Umschläge und 3 100 Mediendateien je Lauf.

## Was gefunden wird, bleibt gefunden

Meldet der Fuzzer einen Absturz, legt er die auslösende Datei unter
`fuzz/artifacts/<ziel>/` ab. Sie gehört **vor der Fehlerbehebung** nach:

```
testvectors/fuzz/envelope/     bzw.
testvectors/fuzz/metadata/
```

Von dort holen die Tests `korpus_bleibt_beherrschbar` sie ab und prüfen sie
bei **jedem** `cargo test` erneut — auf stable, ohne Sonderwerkzeug. Fuzzing
findet, der Korpus hält fest.

Derselbe Ordner dient dem Fuzzer als Startkorpus. Ohne ihn verbrächte er
Stunden damit, überhaupt einen gültigen Kopf zu erraten.
