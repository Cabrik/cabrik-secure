/**
 * Welchen Weg der Sperrbildschirm anbietet.
 *
 * # Was hier auf dem Spiel steht
 *
 * `spec/entsperrung.md` §5.1 zählt auf, was beim Tippen in der Webansicht
 * entsteht: eine JavaScript-Zeichenkette und ein Übergabepuffer, **beide
 * nicht überschreibbar**. Ein eigenes Fenster lässt sie ersatzlos
 * entfallen.
 *
 * Der Unterschied ist **nicht zu sehen**. Beide Wege zeigen Punkte, beide
 * entsperren. Wenn der Bildschirm ihn nicht benennt, benennt ihn niemand
 * — und der Nutzer hält eine Verbesserung für gegeben, die auf seinem
 * Rechner gerade nicht gilt.
 *
 * # Und die andere Richtung
 *
 * Auf einem System ohne eigenes Fenster darf **kein** Hinweis stehen. Wer
 * keine Wahl hat, wird durch die Aufzählung dessen, was er verpasst, nur
 * beunruhigt — ändern kann er nichts.
 */

import { beforeEach, describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Sperrbildschirm from "./Sperrbildschirm.svelte";
import { sitzungsspeicher } from "../kern/speicher.svelte";
import type { Passwortweg, Sitzungsstand } from "../kern/typen";

const GESPERRT: Sitzungsstand = {
  gesperrt: true,
  frist: "fuenfzehnMinuten",
  restsekunden: null,
};

function zeigen(weg: Passwortweg | null) {
  sitzungsspeicher.stand = GESPERRT;
  sitzungsspeicher.passwortweg = weg;

  const ziel = document.createElement("div");
  document.body.append(ziel);
  const b = mount(Sperrbildschirm, { target: ziel, props: {} });
  flushSync();
  return {
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    knopf: (teil: string) =>
      [...ziel.querySelectorAll("button")].find((k) =>
        k.textContent?.includes(teil),
      ),
    feld: () => ziel.querySelector('input[type="password"]'),
    hinweis: () => ziel.querySelector('[data-pruef="webansicht-hinweis"]'),
    klick: (el: HTMLElement | undefined) => {
      el!.click();
      flushSync();
    },
    aufraeumen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

beforeEach(() => {
  sitzungsspeicher.passwortweg = null;
});

describe("der Weg, auf dem das Passwort hereinkommt", () => {
  it("zeigt vor der Antwort das Feld wie bisher", () => {
    // `null` heisst „noch nicht gefragt“. Ein Bildschirm, der im ersten
    // Augenblick umspringt, ist schlechter als einer, der einen
    // Wimpernschlag spaeter richtig steht.
    const s = zeigen(null);
    expect(s.feld()).not.toBeNull();
    expect(s.knopf("Passwort eingeben")).toBeUndefined();
    s.aufraeumen();
  });

  it("bietet das eigene Fenster an und klappt das Feld zu", () => {
    const s = zeigen({ art: "eigenesFenster" });

    expect(s.knopf("Passwort eingeben")).toBeTruthy();
    expect(s.feld(), "das Feld steht offen daneben").toBeNull();
    // Kein Hinweis, solange niemand den schlechteren Weg gewaehlt hat.
    expect(s.hinweis()).toBeNull();

    s.aufraeumen();
  });

  it("nennt beim Aufklappen, was der andere Weg kostet", () => {
    const s = zeigen({ art: "eigenesFenster" });
    s.klick(s.knopf("Stattdessen hier eingeben"));

    expect(s.feld(), "das Feld ist nicht aufgegangen").not.toBeNull();
    const text = s.text();
    // Die FOLGE, nicht die Regel: „zwei Kopien, die sich nicht
    // ueberschreiben lassen“ sagt, warum es zaehlt.
    expect(text).toContain("zwei Kopien");
    expect(text).toContain("nicht überschreiben");

    s.aufraeumen();
  });

  it("schweigt, wo es keine Wahl gibt", () => {
    // DIE ANDERE RICHTUNG. Wer keine Wahl hat, wird durch die Aufzaehlung
    // dessen, was er verpasst, nur beunruhigt -- aendern kann er nichts.
    const s = zeigen({
      art: "webansicht",
      grund: "Auf diesem Betriebssystem gibt es noch kein eigenes Passwortfeld.",
    });

    expect(s.feld(), "ohne Feld käme niemand herein").not.toBeNull();
    expect(s.knopf("Passwort eingeben")).toBeUndefined();
    expect(s.hinweis()).toBeNull();

    s.aufraeumen();
  });
});
