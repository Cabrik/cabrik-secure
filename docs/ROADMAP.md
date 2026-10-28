# Cabrik Secure 2.0 — Fahrplan

**Stand:** 28.10.2026 · **Phase:** 5, Produktreife — Vorbereitung der Alpha
**Ziel:** Professionell gebautes, quelloffenes Krypto-Werkzeug. Desktop
zuerst, Mobile später. Kein Live-Termin — Qualität vor Geschwindigkeit.

> **Wie dieses Dokument gelesen wird.** Hier steht, **was** offen ist und
> in welcher Reihenfolge — kurz. **Warum**, was gemessen wurde, welche Wege
> verworfen sind: im [Verlauf](verlauf.md), sortiert nach denselben
> Phasennummern. Ist etwas erledigt, verschwindet es hier und bekommt dort
> einen Eintrag mit Datum.
>
> Bis zum 26.10.2026 war beides ein Dokument mit über 2 000 Zeilen. Der
> Stand war darin nicht mehr zu sehen.

---

## Der Rahmen (festgehalten am 26.10.2026)

- **Privat bis zum Ende einer Alpha** mit Kollegen und Freunden — mit dem
  Ziel, es danach marktreif zu verkaufen.
- **Name und Logo sind frei, aber nicht angemeldet.** Marke und Code
  Signing hängen an der Namens- und Gewerbefrage und rücken deshalb
  **hinter** die Alpha (5.3, 5.3a). Für Tester genügt ein unsigniertes
  Bündel mit Prüfsumme und einer Anleitung, wie man es trotzdem startet
  ([`testen.md`](testen.md)).
- **Das Repository ist öffentlich** — wegen der freien Actions-Minuten
  (Verlauf, 5.1). Privat zu stellen ginge, kostete aber Minuten, unter
  macOS das Zehnfache.
- **Zielrechner haben bis zu 32 GB Arbeitsspeicher.** Die Speichergrenze
  der Passwortableitung richtet sich nach dem, was gerade frei ist
  (`spec/keyfile-v2.md` §4.2).

## Leitprinzipien

1. **Die Spezifikation ist das Produkt.** Spec vor Implementierung.
2. **Nie zwei Unbekannte gleichzeitig.**
3. **Keine eigene Krypto-Konstruktion.**
4. **Geheimnisse verlassen Rust nicht.**
5. **Keine Telemetrie. Nie.**

Die Begründungen stehen im Verlauf, ganz oben.

---

## Stand auf einen Blick

| Phase | Stand |
|---|---|
| 0 — Fundament | ✓ |
| 1 — Spezifikation | ✓ eingefroren seit 18.08.2026; Nachträge sind als solche gekennzeichnet |
| 2 — Rust-Kern | ✓ |
| 3 — Oberfläche | ✓ |
| 3a — Brückenvertrag | ✓ |
| 4 — Tauri | ✓ bis auf QR-Scan (4d) und Schlüsselbund |
| **5 — Produktreife** | **in Arbeit** — siehe unten |
| 6 — Mobile | offen, erst nach dem Desktop |

**Geprüft wird** auf Linux, Windows und macOS bei jedem Push
(`pruefung.yml`); nachts Fuzzing, wöchentlich fremde Werkzeuge
(`gegenprobe.yml`) und Abhängigkeiten (`abhaengigkeiten.yml`). Ein roter
geplanter Lauf wird zum Issue (`meldung.yml`).

**Aber:** Nach 60 Tagen ohne Commit schaltet GitHub die geplanten Läufe
ab — still, und auch für Pull Requests (Verlauf, 5.7). Bleiben Läufe aus:
`gh workflow list --all`.

---

## Als Nächstes — die Alpha

In dieser Reihenfolge, weil jeder Schritt den nächsten erst sinnvoll macht:

1. **Die Fassung an Tester geben: 0.1.2.** Der Durchlauf in der
   Windows-Sandbox am 28.10. trug, mit [`testen.md`](testen.md) in der
   Hand, vom Herunterladen über den Doppelklick auf eine `.cabrik`-Datei
   bis zum Deinstallieren samt Schlüsseln (Verlauf, 5.7). 0.1.2 behebt
   zusätzlich die Kontakte nach der Sperre und bringt „Sicherung
   einspielen“; ihre Prüfsummen stehen auf der Seite. Auch das trug in
   der Sandbox: sichern, löschen, neu installieren, einspielen, samt
   Kontakten. Die Dateien aus dem Lauf verfallen am 11.11.
2. **Die Bündel auf echten Rechnern starten.** Windows: am 27.10. auf dem
   Entwicklungsrechner. Offen: macOS (`.dmg`, seit dem 27.10. ad hoc
   versiegelt) und Linux (`.deb`, `.AppImage`) — anlegen, sperren,
   entsperren, senden, empfangen. Am ehesten durch die ersten Tester mit
   Mac oder Linux; die Seite sagt ihnen, dass sie die Ersten sind.
3. **Große Dateien strömen** statt sie ganz in den Speicher zu legen
   (5.2). Für eine Alpha mit Fotos und Dokumenten kein Hindernis, für ein
   Video schon — die Seite für Tester nennt es als bekannte Grenze.
4. **Natives Passwortfeld unter macOS und Linux** (5.2) — oder bewusst
   auf nach der Alpha verschieben; die Spezifikation sagt bereits, dass es
   dort fehlt.

---

## Offen, nach Phase

### 5.2 Lücken

- [ ] Große Dateien strömen: Heute liegen Datei, bereinigte Fassung und
      Envelope gleichzeitig im Speicher — bei 3 GB mehrere Gigabyte
- [ ] Natives Passwortfeld unter macOS und Linux

### 5.3 Auslieferbarkeit

- [ ] macOS- und Linux-Bündel auf echten Rechnern prüfen
- [ ] Code Signing — **nach** der Namens- und Gewerbeentscheidung
      (`docs/signatur.md`)
- [ ] macOS: Notarisierung — erst, wenn macOS wirklich beliefert wird
- [ ] Store-Vertrieb: Verschlüsselungsdeklarationen
- [ ] Dauerhaft: Repo-Hygiene — `probe/`, `ich.contact` und die
      `info_Cabrik_Secure*.txt` bleiben ungetrackt

### 5.3a Marke — nach der Alpha

- [ ] DPMA oder EUIPO, Klassen 9 und 42, vorher ein Fachanwalt

### 5.4 Updater — eine Entscheidung, kein Häkchen

- [ ] Entscheiden zwischen „kein Updater, nur ein Hinweis“ und „signierter
      Updater, Schlüssel offline“ — und in `spec/threat-model.md`
      festhalten. Für die Alpha genügt der Hinweis.

### 5.5 Dokumentation

- [ ] Website: statisch, ohne Krypto im Browser
- [ ] Die Spezifikationen mitveröffentlichen
- [ ] „Keine Telemetrie“ als festgehaltene Entscheidung, nicht als
      Auslassung

### 5.6 Audit — zuletzt

- [ ] Kleiner Umfang, 5 000–15 000 €, erst wenn alles davor steht

### Pflege

- [ ] Jede Fassung, die an Tester geht: die Nummer in
      `crates/cabrik-fenster/tauri.conf.json` erhöhen, die Auslieferung
      auf `main` anstoßen und in [`testen.md`](testen.md) Nummer,
      Dateinamen, Prüfsummen und die glibc aus dem Lauf nachtragen. Ohne
      neue Nummer hießen zwei verschiedene Dateien gleich. Die Ablage
      eines Laufs verfällt nach 14 Tagen — vorher herunterladen und
      aufheben
- [ ] Die nächste Tauri-Nebenfassung wie 2.12: Kiste, `tauri-build`,
      Plugins, `@tauri-apps/api` und `TAURI_CLI` in `auslieferung.yml`
      zusammen, und vorher die Bündel bauen. Dependabot bringt bei Tauri
      nur noch Patches
- [ ] Kisten, die nur über Tauri hereinkommen, hebt Dependabot nicht —
      sie bewegen sich nur, wenn jemand `cargo update` aufruft. Fällt
      eine Behebung an, meldet es `cargo deny` (seit dem 27.10. auch bei
      „unsound“); dann `cargo update -p <kiste>`
- [ ] Große Sprünge, die Dependabot einzeln anbietet: `quick-xml` 0.42,
      `lopdf` 0.46, TypeScript 7, vitest 5. Hebt einer davon eine
      Anforderung in der Wurzel an, muss `fuzz/Cargo.lock` mit nachgezogen
      werden — wie bei `argon2` 0.6 und `sha2` 0.11 (erledigt, siehe
      unten)

### Später

- [ ] 4d — QR-Code mit dem Handy abscannen
- [ ] Lösch-Passwort — nur, wenn es vollständig trägt: mit einem
      Schlüssel im Sicherheitschip (TPM, Secure Enclave), den das Löschen
      mitnimmt, und ohne die Sicherung zum Weg daran vorbei zu machen
      (`spec/threat-model.md`, A8)
- [ ] Entsperren über den Schlüsselbund des Betriebssystems
- [ ] 6 — Mobile: UniFFI, natives UI je Plattform
- [ ] Entscheidung: Transport-Layer ja oder nein — die Spezifikation darf
      ihn nicht unmöglich machen

---

## Zuletzt erledigt

**28.10.2026** — Einzelheiten im Verlauf, 5.7:

- **Der Installer auf einem frischen Windows**, in der Windows-Sandbox
  und nur mit der Seite für Tester: Prüfsumme, Setup, Einrichtung,
  Dateien und Text zwischen Sandbox und Entwicklungsrechner in beide
  Richtungen, eine `.cabrik`-Datei per Doppelklick geöffnet — damit ist
  auch die Zuordnung geprüft —, Deinstallieren samt Schlüsseln. Zwei Funde: WebView2 war
  in der Sandbox eingetragen, aber nicht benutzbar (die Seite sagt jetzt,
  wo es die richtige Laufzeit gibt), und in der Einrichtung war die Wahl
  zwischen „neu“ und „aus Version 1“ unsichtbar (behoben)
- **Kontakte nach einer Sperre:** Das Entsperren öffnete den Kontaktstand
  vom Programmstart; was seither dazukam, fehlte und ging mit der nächsten
  Änderung verloren. Behoben — steckt aber in 0.1.1, die Seite für Tester
  nennt die Abhilfe
- **Sicherung einspielen** in der Einrichtung, mit den Kontakten, wenn sie
  mitgesichert wurden, und ein Handbuchkapitel „Auf Reisen und bei
  Kontrollen“. Kein zweites Passwort: A8 bleibt, wie es ist
- **0.1.2** gebaut, Prüfsummen nachgerechnet und auf der Seite für Tester.
  QR-Scan bewusst nicht davor: Er wäre der erste offene Port (Verlauf,
  Phase 4d) und wartet, bis Tester ihn vermissen
- **0.1.2 in der Sandbox:** sichern, löschen, neu installieren,
  einspielen — Identität und Kontakte kamen zurück
- **Kein Lösch-Passwort:** erwogen und verworfen (`spec/threat-model.md`,
  A8). Vorgemerkt unter „Später“, aber nur mit einem Schlüssel im
  Sicherheitschip des Rechners

**27.10.2026** — Einzelheiten im Verlauf, 5.7:

- `sha2` 0.11 und `hkdf` 0.13 — der Kern rechnet ganz über `digest`
  0.11, ohne eine Zeile Code und mit Byte für Byte denselben Vektoren
- **Tauri 2.12** — Kiste, `tauri-build`, Plugins und `@tauri-apps/api`
  gemeinsam, Bündel für alle drei Systeme gebaut; fünf Ausnahmen weniger
  in `deny.toml`
- Dabei gefunden: Eine **hoch** eingestufte Tauri-Meldung
  (GHSA-w28w-mhc8-qvjv) stand vom 26.09. bis 26.10. offen, und keine
  Prüfung schlug an. Behoben hat sie ein Dependabot-Sammel-PR, ohne dass
  es jemand wusste. Cabrik traf sie nicht
- **Abgleich gegen Tauris eigene Meldungen**, wöchentlich
  (`abhaengigkeiten.yml`). Gegen die Sperrdatei von damals meldet er
  beide unerkannt behobenen Lücken. Dazu: `cargo deny` sieht „unsound“
  jetzt auch bei fremden Kisten (`tokio` gehoben, `glib` begründet
  ausgenommen), die Dependabot-Warnungen sind an, und das Tauri-Werkzeug
  hat eine genaue Fassung
- **Hilfe im Programm:** ein Handbuch in elf Kapiteln — die vier Wege
  (senden, empfangen, Kontakt aufnehmen, verifizieren) und was Cabrik
  nicht kann —, die Beispiele als eigenes Kapitel, jedes mit dem Band
  „Beispiel“. Das Programm startet leer und echt; ohne Identität führt
  der erste Start zur Einrichtung
- **Deinstallieren darf Schlüssel und Kontakte löschen** (Windows-Setup):
  nur mit Häkchen und nach einer zweiten Rückfrage, „Nein“ vorgewählt,
  nie bei einem Update
- **Die Seite für Tester** ([`testen.md`](testen.md)): welche Datei zu
  welchem System gehört, die Prüfsumme mit einer Zeile zum Einfügen, das
  unsignierte Programm starten, deinstallieren. Damit sind die Prüfsummen
  veröffentlicht. Dafür ist das macOS-Bündel jetzt ad hoc versiegelt —
  ohne Siegel gilt es auf Rechnern mit Apple-Chip als beschädigt —, und
  das Linux-Bündel nennt die glibc, die es braucht

**26.10.2026** — Einzelheiten im Verlauf, 5.7:

- Alle geplanten Läufe wieder grün: Gegenprobe (LAME-Fassung), npm audit,
  cargo deny (`zip` 9, `ctutils`, `chacha20`, zbus-Ausnahmen begründet,
  zehn erledigte GTK3-Ausnahmen entfernt)
- Passwortableitung mit **Speichergrenze** und **Rückfrage**; ihr
  Arbeitsspeicher wird überschrieben; **höchstens eine Passwortkapsel**
  je Envelope
- CI: Actions an Commits geheftet, nur Leserechte, Dependabot, Issue bei
  rotem geplantem Lauf, Bündel für macOS und Linux
- Der Fahrplan wurde geteilt: dieser Plan und der [Verlauf](verlauf.md)
- Dependabot zugeschnitten: Fuzzing-Werkbank nur `libfuzzer-sys`,
  `@types/node` folgt `.nvmrc`, Tauri nur im Gleichschritt
- `argon2` 0.6 — ohne Vorgabemerkmale; der Arbeitsspeicher wird weiter
  vom Kern selbst angelegt und überschrieben, weil 0.6 das nicht tut
