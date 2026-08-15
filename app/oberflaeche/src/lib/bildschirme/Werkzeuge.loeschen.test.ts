/**
 * Endgültig löschen.
 *
 * # Die Zusicherung, die zählt
 *
 * **Die Bestätigung gilt für eine bestimmte Auswahl.** Kommt eine Datei
 * dazu, ist sie von selbst weg. Ein Häkchen, das eine Änderung überlebt,
 * bestätigt etwas, das der Nutzer nie gesehen hat — und dies ist der eine
 * Vorgang, bei dem es kein Zurück gibt.
 *
 * # Und die zweite
 *
 * **Was nicht erreicht wurde, steht einzeln da.** Ein pauschales
 * „Gelöscht“ wäre eine Behauptung über drei verschiedene Dinge —
 * überschrieben, umbenannt, entfernt —, von denen jedes einzeln scheitern
 * kann. Version 1 sagte genau dieses eine Wort.
 */

import { describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Werkzeuge from "./Werkzeuge.svelte";
import type { Loeschergebnis, Loeschkandidat } from "../kern/typen";
import { reaktiv } from "../kern/pruefstand.svelte";

function kandidat(name: string): Loeschkandidat {
  return {
    pfad: `C:\\Fotos\\${name}`,
    name,
    groesseBytes: 1000,
    beurteilung: {
      faehigkeit: "bestEffort",
      vorbehalte: [{ art: "kopienMoeglich" }],
    },
  };
}

function zeigen(kandidaten: Loeschkandidat[], ergebnisse: Loeschergebnis[] = []) {
  const gerufen: number[] = [];
  const ziel = document.createElement("div");
  document.body.append(ziel);
  // `$state` gibt es nur in `.svelte`-Dateien -- `reaktiv` aus dem
  // Pruefstand baut veraenderliche Props fuer gewoehnliche Tests.
  const props = reaktiv({
    kandidaten,
    ergebnisse,
    loeschen: (d: number) => gerufen.push(d),
    waehlen: () => {},
  });
  const b = mount(Werkzeuge, { target: ziel, props });
  return {
    ziel,
    props,
    gerufen,
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    knopf: () =>
      ziel.querySelector<HTMLButtonElement>(
        '[data-pruefstelle="endgueltig-loeschen"]',
      ),
    /** Das Bestätigungshäkchen — das letzte auf dem Bildschirm. */
    haken: () =>
      [
        ...ziel.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'),
      ].at(-1)!,
    klick: (el: HTMLElement) => {
      el.click();
      flushSync();
    },
    abbauen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

describe("endgültig löschen", () => {
  it("ist gesperrt, solange nicht bestätigt ist", () => {
    const s = zeigen([kandidat("Alt.jpg")]);

    expect(s.knopf()!.disabled).toBe(true);
    s.abbauen();
  });

  it("und geht auf, wenn bestätigt wurde", () => {
    // Die Gegenprobe: Eine Sperre, die immer sperrt, wäre keine.
    const s = zeigen([kandidat("Alt.jpg")]);

    s.klick(s.haken());

    expect(s.knopf()!.disabled).toBe(false);
    s.abbauen();
  });

  it("die Bestätigung nennt, wie viele Dateien fort sind", () => {
    // „Ich bin einverstanden“ allein ist kein Einverständnis. Die Zahl
    // gehört in den Satz, den jemand ankreuzt.
    const s = zeigen([kandidat("Eins.jpg"), kandidat("Zwei.jpg")]);

    expect(s.text()).toContain("2 Dateien sind danach fort");
    s.abbauen();
  });

  it("eine geänderte Auswahl macht die Bestätigung ungültig", () => {
    // Die Zusicherung, die zählt. Ein Häkchen, das eine Änderung
    // überlebt, bestätigt etwas, das niemand gesehen hat — und hier gibt
    // es kein Zurück.
    const s = zeigen([kandidat("Eins.jpg")]);
    s.klick(s.haken());
    expect(s.knopf()!.disabled).toBe(false);

    s.props.kandidaten = [kandidat("Eins.jpg"), kandidat("Zwei.jpg")];
    flushSync();

    expect(
      s.knopf()!.disabled,
      "nach einer Änderung muss erneut bestätigt werden",
    ).toBe(true);
    s.abbauen();
  });

  it("reicht die Zahl der Durchgänge weiter", () => {
    const s = zeigen([kandidat("Alt.jpg")]);
    s.klick(s.haken());

    s.klick(s.knopf()!);

    expect(s.gerufen).toEqual([1]);
    s.abbauen();
  });

  it("meldet jeden Schritt einzeln, nicht ein pauschales „gelöscht“", () => {
    // Version 1 sagte genau dieses eine Wort — über drei verschiedene
    // Dinge, von denen jedes einzeln scheitern kann.
    const s = zeigen(
      [],
      [
        {
          pfad: "C:\\Fotos\\Alt.jpg",
          faehigkeit: "bestEffort",
          ueberschrieben: true,
          umbenannt: true,
          entfernt: true,
          vorbehalte: [],
          fehler: null,
        },
        {
          pfad: "\\\\server\\freigabe\\Liste.xlsx",
          faehigkeit: "nichtMoeglich",
          ueberschrieben: false,
          umbenannt: false,
          entfernt: false,
          vorbehalte: [],
          fehler: "Zugriff verweigert",
        },
      ],
    );

    const text = s.text();
    expect(text).toContain("1 von 2");
    expect(text).toContain("überschrieben");
    expect(text).toContain("nicht überschrieben");
    expect(text, "der Grund gehört dazu").toContain("Zugriff verweigert");
    s.abbauen();
  });

  it("ohne echte Dateien gibt es keinen Löschknopf, der etwas verspricht", () => {
    // Die Beispielfälle liegen nicht auf der Platte.
    const ziel = document.createElement("div");
    document.body.append(ziel);
    const b = mount(Werkzeuge, { target: ziel, props: {} });

    const knopf = ziel.querySelector<HTMLButtonElement>(
      '[data-pruefstelle="endgueltig-loeschen"]',
    );
    expect(knopf!.disabled).toBe(true);
    unmount(b);
    ziel.remove();
  });
});
