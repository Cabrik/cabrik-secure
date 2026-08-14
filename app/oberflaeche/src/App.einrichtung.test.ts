/**
 * Der Weg vom leeren Rechner bis zur sichtbaren Identität.
 *
 * # Warum das an der ganzen Anwendung geprüft wird
 *
 * Weil der gemeldete Fehler genau dazwischen lag. `Einrichtung.test.ts`
 * prüft, dass der Onboarding-Bildschirm etwas anlegt — und das tat er.
 * `Kontakte.test.ts` prüft die Verzeichnisse. Was keiner von beiden sieht:
 * ob die Hülle zwischen zwei Bildschirmen dasselbe hält.
 *
 * Ein Bildschirm für sich kann vollständig richtig sein, während die
 * Anwendung falsch ist. Das ist die Lücke, die dieser Test schließt.
 */
// @vitest-environment happy-dom

import { beforeEach, expect, it } from "vitest";
import { mount, unmount } from "svelte";
import App from "./App.svelte";
import { MockBruecke } from "./lib/kern/bruecke";
import { KONTAKTE } from "./lib/kern/mock";
import {
  identitaetsspeicher,
  kontaktspeicher,
  sitzungsspeicher,
} from "./lib/kern/speicher.svelte";
import { abgewickelt } from "./lib/kern/pruefstand.svelte";

/** Ein Rechner ohne Identität — die Lage beim allerersten Start. */
async function leererRechner() {
  const b = new MockBruecke(KONTAKTE);
  await b.identitaetLoeschen();
  for (const s of [sitzungsspeicher, kontaktspeicher, identitaetsspeicher]) {
    s.verbinde(b);
  }
  await sitzungsspeicher.laden();
  return b;
}

function anhaengen() {
  document.body.innerHTML = '<div id="app"></div>';
  const ziel = document.getElementById("app")!;
  const a = mount(App, { target: ziel });
  const knopf = (teil: string) =>
    [...ziel.querySelectorAll("button")].find((k) =>
      k.textContent?.includes(teil),
    );
  return {
    ziel,
    knopf,
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    klick: async (el: HTMLElement | undefined) => {
      el!.click();
      await abgewickelt();
    },
    tippen: async (feld: HTMLInputElement, wert: string) => {
      feld.value = wert;
      feld.dispatchEvent(new Event("input", { bubbles: true }));
      await abgewickelt();
    },
    abbauen: () => unmount(a),
  };
}

beforeEach(async () => {
  await leererRechner();
});

it("die angelegte Identität steht danach unter „Identität“", async () => {
  const s = anhaengen();
  await abgewickelt();

  // --- Einrichtung ---------------------------------------------------------
  await s.klick(s.knopf("Einrichtung"));
  const feld = () => s.ziel.querySelector<HTMLInputElement>("input");
  await s.tippen(feld()!, "Cabrik");
  await s.klick(s.knopf("Weiter"));

  const pw = () => [
    ...s.ziel.querySelectorAll<HTMLInputElement>('input[type="password"]'),
  ];
  await s.tippen(pw()[0]!, "vierwortpasswortmitlaenge");
  await s.tippen(pw()[1]!, "vierwortpasswortmitlaenge");
  await s.klick(s.ziel.querySelector<HTMLInputElement>('input[type="checkbox"]')!);
  await s.klick(
    s.ziel.querySelector<HTMLButtonElement>('button[data-pruefstelle="weiter"]')!,
  );
  await s.klick(
    s.ziel.querySelector<HTMLButtonElement>('button[data-pruefstelle="weiter"]')!,
  );

  expect(s.text(), "die Einrichtung selbst muss durchgelaufen sein").toContain(
    "Ihr Fingerprint",
  );

  // --- Und jetzt die Stelle, an der es scheiterte -------------------------
  await s.klick(s.knopf("Zur Identität"));

  expect(s.text()).not.toContain("Keine Identität vorhanden");
  expect(s.text()).toContain("Cabrik");
  expect(identitaetsspeicher.liste).toHaveLength(1);

  s.abbauen();
});

it("sie überlebt einen Wechsel in einen anderen Bereich und zurück", async () => {
  // Der Verdacht, der zuerst naheliegt: Der Takt der Sitzung lädt im
  // Hintergrund nach und überschreibt die Liste mit dem, was der Kern
  // gerade hergibt. Wenn dieser Aufruf fehlschlägt, ist sie danach leer —
  // und zwar erst eine Sekunde später, also lange nach jedem Klick.
  const s = anhaengen();
  await abgewickelt();

  await identitaetsspeicher.anlegen("Cabrik", "vierwortpasswortmitlaenge", "empfohlen", true);
  await abgewickelt();

  // Was der Takt tut, hier von Hand: nachladen.
  await sitzungsspeicher.laden();
  await identitaetsspeicher.laden();
  await abgewickelt();

  await s.klick(s.knopf("Identität"));

  expect(identitaetsspeicher.liste).toHaveLength(1);
  expect(s.text()).not.toContain("Keine Identität vorhanden");

  s.abbauen();
});
