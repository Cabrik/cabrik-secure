/**
 * Der Weg von einer Datei auf der Platte bis auf den Bildschirm.
 *
 * # Warum das an der ganzen Anwendung geprüft wird
 *
 * Weil der gemeldete Fehler zwischen zwei richtigen Teilen lag.
 * `sendespeicher.test.ts` prüft, dass hineingezogene Dateien ankommen —
 * und sie kamen an. `Senden.test.ts` prüft den Bildschirm — und der war
 * richtig. Falsch war die Hülle: Sie öffnet auf **Empfangen**, und dort
 * gibt es weder einen Auswahlknopf noch irgendein Zeichen, dass eine Datei
 * angenommen wurde.
 *
 * Von außen sah beides aus wie „das Fenster nimmt nichts an“.
 */
// @vitest-environment happy-dom

import { beforeEach, expect, it } from "vitest";
import { mount, unmount } from "svelte";
import App from "./App.svelte";
import { MockBruecke } from "./lib/kern/bruecke";
import { KONTAKTE, STAPEL } from "./lib/kern/mock";
import {
  identitaetsspeicher,
  kontaktspeicher,
  sendespeicher,
  sitzungsspeicher,
} from "./lib/kern/speicher.svelte";
import { abgewickelt } from "./lib/kern/pruefstand.svelte";
import type { Ziehereignis } from "./lib/kern/typen";

/** Die Pfade des ersten Beispielstapels — die kennt die Attrappe. */
const PFADE = STAPEL[0]!.dateien.map((d) => d.pfad);

/** Eine Brücke, deren Ziehen-und-Fallenlassen der Test auslöst. */
class MitZiehen extends MockBruecke {
  private melde: ((e: Ziehereignis) => void) | null = null;

  override async aufDateienGezogen(
    melde: (e: Ziehereignis) => void,
  ): Promise<() => void> {
    this.melde = melde;
    return () => {
      this.melde = null;
    };
  }

  override async dateienWaehlen(): Promise<string[]> {
    return PFADE;
  }

  /** Stellt nach, was das Fenster meldet. */
  ereignis(e: Ziehereignis) {
    this.melde?.(e);
  }
}

let bruecke: MitZiehen;

beforeEach(async () => {
  bruecke = new MitZiehen(KONTAKTE);
  for (const s of [
    sitzungsspeicher,
    kontaktspeicher,
    identitaetsspeicher,
    sendespeicher,
  ]) {
    s.verbinde(bruecke);
  }
  await sitzungsspeicher.laden();
});

function anhaengen() {
  document.body.innerHTML = '<div id="app"></div>';
  const ziel = document.getElementById("app")!;
  const a = mount(App, { target: ziel });
  return {
    ziel,
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    knopf: (teil: string) =>
      [...ziel.querySelectorAll("button")].find((k) =>
        k.textContent?.includes(teil),
      ),
    abbauen: () => unmount(a),
  };
}

it("führt fallengelassene Dateien auf den Sendebildschirm", async () => {
  // Der gemeldete Fehler. Die Anwendung öffnet auf „Empfangen“; ohne
  // diesen Wechsel verschwinden die Dateien in einen Halter, den gerade
  // niemand ansieht.
  const s = anhaengen();
  await abgewickelt();
  expect(s.text(), "die Anwendung öffnet auf Empfangen").not.toContain(
    "Weitere hinzufügen",
  );

  bruecke.ereignis({ art: "fallen", pfade: PFADE });
  await abgewickelt();
  await abgewickelt();

  expect(sendespeicher.dateien.length).toBeGreaterThan(0);
  expect(s.text(), "der Sendebildschirm muss jetzt vorn sein").toContain(
    "Weitere hinzufügen",
  );
  s.abbauen();
});

it("zeigt schon beim Ziehen, dass es annimmt", async () => {
  // Ein Fenster, das erst beim Loslassen reagiert, sieht bis dahin aus
  // wie eines, das nichts annimmt — und dann lässt niemand los.
  const s = anhaengen();
  await abgewickelt();

  bruecke.ereignis({ art: "drueber" });
  await abgewickelt();

  expect(s.text()).toContain("Loslassen");
  s.abbauen();
});

it("nimmt den Hinweis zurück, wenn das Ziehen das Fenster verlässt", async () => {
  const s = anhaengen();
  await abgewickelt();
  bruecke.ereignis({ art: "drueber" });
  await abgewickelt();

  bruecke.ereignis({ art: "weg" });
  await abgewickelt();

  expect(s.text()).not.toContain("Loslassen");
  s.abbauen();
});

it("bietet auf dem Sendebildschirm einen Weg, Dateien auszuwählen", async () => {
  // Der zweite Teil des Berichts: kein Knopf zum Aussuchen. Wer nicht
  // ziehen will oder kann, braucht ihn.
  const s = anhaengen();
  await abgewickelt();

  s.knopf("Senden")!.click();
  await abgewickelt();

  expect(s.text()).toContain("Noch nichts ausgewählt");
  expect(s.knopf("Dateien auswählen")).toBeDefined();
  s.abbauen();
});

it("und dieser Weg führt tatsächlich zu Dateien", async () => {
  // Die Gegenprobe: Ein Knopf, der dasteht und nichts tut, bestünde die
  // Prüfung oben.
  const s = anhaengen();
  await abgewickelt();
  s.knopf("Senden")!.click();
  await abgewickelt();

  s.knopf("Dateien auswählen")!.click();
  await abgewickelt();
  await abgewickelt();

  expect(sendespeicher.dateien).toHaveLength(PFADE.length);
  expect(s.text()).not.toContain("Noch nichts ausgewählt");
  s.abbauen();
});

it("ein Fehlschlag beim Anmelden wird gezeigt, nicht verschluckt", async () => {
  // Sonst sieht ein Ziehen-und-Fallenlassen, das sich gar nicht anmelden
  // ließ, genauso aus wie eines, bei dem der Nutzer danebengezielt hat.
  class Kaputt extends MockBruecke {
    override async aufDateienGezogen(): Promise<() => void> {
      throw new Error("Die Webansicht meldet kein Ziehen.");
    }
  }
  sendespeicher.verbinde(new Kaputt(KONTAKTE));

  const s = anhaengen();
  await abgewickelt();
  await abgewickelt();

  expect(s.text()).toContain("Die Webansicht meldet kein Ziehen.");
  s.abbauen();
});
