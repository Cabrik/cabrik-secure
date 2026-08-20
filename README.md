# Cabrik Secure

Ein Offline-Werkzeug, das Dateien und Textnachrichten verschlüsselt.
Kein Server, keine Konten, keine Telemetrie. Es transportiert nichts —
den Envelope bringen Sie selbst zum Empfänger, per Mail, Messenger,
USB-Stick oder Cloud-Ablage.

> **Kein Installer, keine unabhängige Prüfung.** Der Quelltext ist offen,
> das Programm aber nicht ausgeliefert: Es gibt keine signierte Fassung
> zum Herunterladen und kein Audit. Setzen Sie Cabrik Secure nicht für
> Daten ein, deren Offenlegung Ihnen ernsthaft schadet.
>
> Was fehlt und in welcher Reihenfolge, steht in
> [`docs/ROADMAP.md`](docs/ROADMAP.md) — Phase 5.

## Was es tut

- **Verschlüsselt** Dateien und Text an einen oder mehrere Empfänger —
  wahlweise mit Post-Quantum-Verfahren (X-Wing: X25519 + ML-KEM-768)
- **Entfernt Metadaten** vor dem Versand: GPS-Angaben, Kameramodelle,
  Klarnamen, eingebettete Vorschaubilder, frühere Fassungen in PDF und
  Office-Dokumenten
- **Sagt, was in Empfangenem steht** — was der Absender über sich
  preisgegeben hat, bevor Sie die Datei speichern oder weiterreichen
- **Löscht sicher** — und sagt dabei, was Überschreiben auf Ihrem
  Datenträger *nicht* erreicht

## Der Grundsatz

**Nichts behaupten, was nicht gedeckt ist.**

Das klingt selbstverständlich und ist es nicht. Version 1 dieses Programms
kopierte jede Datei, deren Format sie nicht verstand, und meldete Erfolg.
Der Nutzer bekam eine `.clean`-Datei und schloss daraus, sie sei bereinigt.

Deshalb kennt Cabrik Secure **keinen Wahrheitswert, sondern vier Zustände**
— und der wichtigste ist der vierte: *keine Aussage*. Er entspricht der
Flagge am künstlichen Horizont, die erscheint, wenn das Instrument seine
Eingangsdaten verliert. Er ist kein abgestuftes Gelb: Gelb heißt „ich weiß
etwas", Grau heißt „ich weiß es nicht".

Die Regeln dazu stehen in [`spec/anzeige.md`](spec/anzeige.md) und sind als
ausführbare Tests hinterlegt, nicht als Absichtserklärung.

## Was quelloffen ist — und warum nicht alles

| Kiste | Inhalt | Lizenz |
|---|---|---|
| `cabrik-core` | Envelope, Keyfile, Kontaktspeicher, Fingerprints | Apache-2.0 |
| `cabrik-metadata` | Metadaten erkennen und entfernen | Apache-2.0 |
| `cabrik-shred` | sicheres Löschen | Apache-2.0 |
| `cabrik-ablage` | Dateiablage | Apache-2.0 |
| `cabrik-app`, `cabrik-bruecke`, `cabrik-cli`, `cabrik-fenster`, `cabrik-v1` | Befehlsschicht, Brücke, Befehlszeile, Fensterhülle, v1-Leser | proprietär |

**Überprüfbar sein muss, was Sicherheit zusagt.** Das ist der Kern — nicht
die Fensterhülle. Wer die Sicherheitsaussagen dieses Programms nachprüfen
will, findet in den vier offenen Kisten alles, was dafür nötig ist: die
Verschlüsselung, die Schlüsselverwaltung, das Vertrauensmodell, die
Metadatenbehandlung.

Die vier hängen an nichts Proprietärem und lassen sich für sich bauen.

## Die Spezifikationen

Sie sind der Grund, warum jemand diesem Programm glauben sollte — und sie
entstanden **vor** der Umsetzung.

| Dokument | Worum es geht |
|---|---|
| [`threat-model.md`](spec/threat-model.md) | Wogegen geschützt wird **und wogegen ausdrücklich nicht** |
| [`envelope-v2.md`](spec/envelope-v2.md) | Das Dateiformat |
| [`keyfile-v2.md`](spec/keyfile-v2.md) | Wie der Schlüssel auf der Platte liegt |
| [`trust-store.md`](spec/trust-store.md) | Kontakte, Verifikation, Safety Numbers |
| [`metadata.md`](spec/metadata.md) | Was je Format gefunden und entfernt wird |
| [`shredding.md`](spec/shredding.md) | Was Löschen erreicht — und was nicht |
| [`anzeige.md`](spec/anzeige.md) | Die vier Zustände und ihre Zuordnung |
| [`entsperrung.md`](spec/entsperrung.md) | Sperre, Frist, Umgang mit dem Passwort |
| [`test-vectors.md`](spec/test-vectors.md) | Wie eine fremde Umsetzung sich gegenprüft |

Unter [`testvectors/`](testvectors/) liegen die Vektoren selbst. Sie sind
sprachunabhängig: Wer das Format nachbaut, kann sich damit Byte für Byte
gegen diese Umsetzung prüfen — und gegen eine unabhängige Python-Referenz,
die dieselben Vektoren erzeugt hat.

## Bauen

Voraussetzungen: [Rust](https://rustup.rs) (die Fassung steht in
`rust-toolchain.toml` und wird automatisch geholt) und
[Node](https://nodejs.org) (Fassung in `app/oberflaeche/.nvmrc`).

```bash
# Nur den quelloffenen Kern prüfen — braucht kein Node:
cargo test -p cabrik-core -p cabrik-metadata -p cabrik-shred -p cabrik-ablage

# Alles, wie es die CI tut:
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cd app/oberflaeche && npm ci && npm run pruefung
```

Unter Windows tut es dasselbe in einem Aufruf:

```powershell
.\pruefung.ps1
```

Unter Linux braucht die Fensterhülle Systempakete für WebKit und GTK; die
Liste steht in [`.github/workflows/pruefung.yml`](.github/workflows/pruefung.yml).

## Sicherheitslücken melden

Siehe [`SECURITY.md`](SECURITY.md). Bitte kein öffentliches Issue.

## Lizenz

Die vier oben genannten Kisten stehen unter der
[Apache-Lizenz 2.0](LICENSE). Die übrigen sind proprietär; jede trägt ihre
eigene `LICENSE`.

„Cabrik" ist keine freigegebene Marke. Die Apache-Lizenz erteilt in §6
ausdrücklich keine Markenrechte.
