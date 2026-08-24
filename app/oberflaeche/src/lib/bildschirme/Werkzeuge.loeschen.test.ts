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

/**
 * Die Beispielfälle — und warum sie sich zu erkennen geben müssen.
 *
 * # Die Lücke, die das hier schließt
 *
 * Jeder Test oben übergibt Dateien. Damit ist `echt` immer wahr, und der
 * Beispielzweig lief ausschließlich im fertigen Fenster — dort, wo ihn
 * kein Test ansieht.
 *
 * Und der Test darunter mountet ohne `loeschen`, trifft also den
 * Prototyp. Die eine Lage, auf die es ankommt, blieb übrig: Fenster
 * (`loeschen` vorhanden) **und** noch nichts ausgewählt. Genau so startet
 * das Programm.
 */
describe("solange nur Beispiele dastehen", () => {
  /** Wie das Fenster startet: Kern angeschlossen, Auswahl leer. */
  function imFenster() {
    const ziel = document.createElement("div");
    document.body.append(ziel);
    const gerufen: number[] = [];
    const props = reaktiv({
      kandidaten: [] as Loeschkandidat[],
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
        )!,
      haken: () =>
        [
          ...ziel.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'),
        ].at(-1)!,
      abbauen: () => {
        unmount(b);
        ziel.remove();
      },
    };
  }

  it("sagt das Fenster, dass es Beispiele sind", () => {
    // DER KERN DER SACHE. Ohne diesen Satz stehen drei erfundene Pfade
    // samt Löschbefund auf dem Bildschirm, bevor jemand eine Datei
    // ausgewählt hat -- und ein Befund ist eine Aussage über eine
    // wirkliche Datei. `C:\Users\name\Desktop\Notizen.txt` sieht echt
    // genug aus, um für einen eigenen gehalten zu werden.
    const s = imFenster();
    expect(
      s.ziel.querySelector('[data-pruefstelle="beispielhinweis"]'),
      "der Beispielhinweis fehlt",
    ).not.toBeNull();
    expect(s.text()).toContain("keine Dateien auf diesem Rechner");
    s.abbauen();
  });

  it("bleibt der Löschknopf stumpf, auch mit gesetztem Häkchen", () => {
    // Dieser Test hat nie etwas repariert -- er hielt schon vorher, weil
    // `bestaetigt` selbst `kandidaten.length > 0` verlangt. Beim Suchen
    // nach dem Beispielfehler stand die Vermutung im Raum, der Knopf sei
    // hier bedienbar; die Gegenprobe hat sie widerlegt.
    //
    // Er bleibt trotzdem stehen: Die Sperre hängt an einer Bedingung, die
    // wie eine Aussage über das Häkchen aussieht und nebenbei die über die
    // Auswahl trifft. Wer sie einmal aufteilt, nimmt sie leicht mit weg --
    // auf dem einen Bildschirm ohne Rückgängig.
    const s = imFenster();
    s.haken().click();
    flushSync();

    expect(s.knopf().disabled, "der Knopf ist bedienbar ohne Auswahl").toBe(
      true,
    );

    // Und er tut auch nichts, wenn man ihn doch trifft.
    s.knopf().click();
    flushSync();
    expect(s.gerufen).toEqual([]);
    s.abbauen();
  });

  it("verschwindet der Hinweis, sobald Dateien da sind", () => {
    // Sonst stünde er über echten Dateien und erklärte sie zu Beispielen
    // -- derselbe Fehler in die andere Richtung.
    const s = imFenster();
    s.props.kandidaten = [kandidat("Echt.jpg")];
    flushSync();

    expect(
      s.ziel.querySelector('[data-pruefstelle="beispielhinweis"]'),
      "der Hinweis steht über echten Dateien",
    ).toBeNull();
    expect(s.text()).toContain("Echt.jpg");
    s.abbauen();
  });
});
