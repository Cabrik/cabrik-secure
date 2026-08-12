# Frontend

Stack: **Svelte 5 + TypeScript + Tailwind 4**. Der Prototyp liegt unter
[`oberflaeche/`](oberflaeche/).

```sh
cd app/oberflaeche
npm install
npm run dev        # Prototyp im Browser
npm run pruefung   # Typprüfung und Tests
```

## Was hier gebaut wird — und was nicht

Phase 3 baut **gegen Beispieldaten**, ohne jede Rust-Anbindung. Die
Integration folgt in Phase 4. Grund: nie an zwei Unbekannten gleichzeitig
arbeiten.

## Architekturregel

Schlüsselmaterial verlässt Rust nicht. Das Frontend erhält ausschließlich
Handles, Statuswerte und Fortschritt — niemals Secrets. Das ist der
eigentliche Grund für Tauri statt Electron oder einer Web-App.

v1 hielt das Passwort dauerhaft im Klartext in einer globalen Variablen.
Das ist der Fehler, den diese Regel verhindert.

## Der Anzeigevertrag

[`spec/anzeige.md`](../spec/anzeige.md) legt fest, welcher Zustand des Kerns
welche Anzeige bekommt und was jede behaupten darf. Er steht **vor** den
Bildschirmen, weil eine Oberfläche verkürzt: Aus einem Absatz wird ein
Häkchen, aus einer Einschränkung eine Fußnote, und aus „alle bekannten" wird
„alle".

Vier Zustände, nicht drei:

| | | |
|---|---|---|
| **Bestätigt** | grün ✓ | geprüft, traf zu |
| **Warnung** | gelb ! | geprüft, etwas ist zu beachten |
| **Fehler** | rot ✕ | gescheitert |
| **Keine Aussage** | grau ? | konnte nicht geprüft werden |

Der vierte entspricht der Flagge am künstlichen Horizont, die erscheint,
wenn das Instrument seine Eingangsdaten verliert. Er ist der wichtigste:
Genau dort lag v1s Fehler.

## Aufbau

```
oberflaeche/src/lib/
  kern/typen.ts        Die Typen des Rust-Kerns, eins zu eins nachgebildet
  kern/mock.ts         Acht Beispielfälle aus der wirklichen Arbeit
  anzeige/zustand.ts   Die Zuordnung Kernzustand -> Anzeigezustand
  anzeige/*.svelte     Die Bausteine, die den Vertrag durchsetzen
  bildschirme/         Die Bildschirme
```

**`kern/typen.ts` bildet die Rust-Aufzählungen genau nach**, statt sich
bequeme Typen auszudenken. Ein `status: "ok" | "warn"` wäre schneller
geschrieben und ließe die Oberfläche gegen eine Wirklichkeit entstehen, die
es nicht gibt. So ist die Datei zugleich der Entwurf des Brückenvertrags
für Phase 4.

**`anzeige/zustand.ts` hat Tests.** Ein Anzeigevertrag, der nur in einem
Dokument steht, wird beim dritten Bildschirm gebrochen — nicht aus
Nachlässigkeit, sondern weil niemand beim Schreiben eines Knopfes ein
Kapitel nachschlägt. Wer eine Nachricht anders einordnen will, muss einen
Test ändern, und das fällt in einer Durchsicht auf.

## Stand

- [x] **Empfangen** — zuerst gebaut, weil dort alle vier Zustände
      zusammentreffen und Absender und Metadaten unabhängig voneinander
      bewertet werden
- [ ] Senden
- [ ] Kontakte mit Verifikation
- [ ] Identität und Schlüssel
- [ ] Onboarding
- [ ] Werkzeuge
