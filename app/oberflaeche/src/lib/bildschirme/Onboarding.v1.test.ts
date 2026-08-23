/**
 * Der Weg für die, die schon Version 1 benutzt haben.
 *
 * # Warum das kein Komfortweg ist
 *
 * Ohne ihn kommt jemand, der die ausgelieferte v1 benutzt hat, an
 * **nichts** mehr heran, was an ihn gerichtet wurde — auch nicht an das,
 * was noch unterwegs ist. Ein verschlossenes Schloss, kein fehlender
 * Bedienknopf.
 *
 * # Was hier auf dem Spiel steht
 *
 * Drei Sätze, die leicht wieder verschwinden, weil ihr Fehlen niemandem
 * auffällt — und die dann jemanden ratlos zurücklassen:
 *
 * 1. **Der Fingerprint ändert sich.** Ohne diesen Hinweis hält der Nutzer
 *    ihn für einen Angriff, und er liegt damit gar nicht so falsch: Genau
 *    so sähe einer aus.
 * 2. **Nur einmal übernehmen.** Zweimal ergäbe zwei Identitäten, die er
 *    für eine hält.
 * 3. **Der Signierschlüssel ist keine Wahl**, sondern eine Auskunft aus
 *    der alten Datei.
 */

import { beforeEach, describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Onboarding from "./Onboarding.svelte";
import { identitaetsspeicher } from "../kern/speicher.svelte";
import { MockBruecke } from "../kern/bruecke";
import { KONTAKTE } from "../kern/mock";

function einhaengen() {
  const ziel = document.createElement("div");
  document.body.append(ziel);
  const b = mount(Onboarding, { target: ziel, props: {} });
  return {
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    knopf: (teil: string) =>
      [...ziel.querySelectorAll("button")].find((k) =>
        k.textContent?.includes(teil),
      ),
    feld: (typ: string, nr = 0) =>
      [...ziel.querySelectorAll<HTMLInputElement>(`input[type="${typ}"]`)][nr],
    kaesten: () =>
      [...ziel.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')],
    tippen: (feld: HTMLInputElement, wert: string) => {
      feld.value = wert;
      feld.dispatchEvent(new Event("input", { bubbles: true }));
      flushSync();
    },
    klick: (el: HTMLElement | undefined) => {
      el!.click();
      flushSync();
    },
    /**
     * Für Knöpfe, deren Rückruf `async` ist — die Dateiwahl etwa.
     *
     * `flushSync` allein genügt dort nicht: Es zieht die Anzeige nach,
     * wartet aber nicht auf das Versprechen dahinter. Ohne dieses `await`
     * bliebe „Noch keine gewählt“ stehen, und der Test prüfte den Zustand
     * vor der Wahl.
     */
    klickWarten: async (el: HTMLElement | undefined) => {
      el!.click();
      // Ein Makrotask statt einzelner `await Promise.resolve()`: Der Weg
      // vom Knopf zum Zustand geht ueber mehrere Versprechen -- Bildschirm,
      // Speicher, Bruecke --, und wie viele es genau sind, soll dieser Test
      // nicht wissen muessen. `setTimeout(0)` laeuft erst, wenn die
      // Mikrotask-Warteschlange leer ist.
      await new Promise((f) => setTimeout(f, 0));
      flushSync();
    },
    aufraeumen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

const NEUES = "vier zufaellige woerter hier";

beforeEach(() => {
  // Ohne Identität: die Lage beim allerersten Start — und die einzige, in
  // der eine Übernahme überhaupt zulässig ist.
  const leer = new MockBruecke(KONTAKTE);
  void leer.identitaetLoeschen();
  identitaetsspeicher.verbinde(leer);
});

describe("die Übernahme aus Version 1", () => {
  it("steht im ersten Schritt und nicht in einer Einstellung", () => {
    const s = einhaengen();
    expect(s.text()).toContain("Version 1");
    expect(s.knopf("Schlüssel aus Version 1 übernehmen")).toBeTruthy();
    s.aufraeumen();
  });

  it("sagt beim Umschalten, dass nur einmal übernommen werden darf", () => {
    const s = einhaengen();

    // Vorher steht der Satz NICHT da -- sonst prüfte der Test nichts.
    expect(s.text()).not.toContain("Nur einmal übernehmen");

    s.klick(s.knopf("Schlüssel aus Version 1 übernehmen"));
    const text = s.text();
    expect(text).toContain("Nur einmal übernehmen");
    expect(text).toContain("zwei Identitäten");

    s.aufraeumen();
  });

  it("fragt nach Datei und altem Passwort, bevor irgendetwas abgeleitet wird", () => {
    const s = einhaengen();
    s.klick(s.knopf("Schlüssel aus Version 1 übernehmen"));
    s.klick(s.knopf("Weiter"));

    expect(s.text()).toContain("Noch keine gewählt");
    expect(s.knopf("Datei wählen")).toBeTruthy();
    s.aufraeumen();
  });

  it("lässt nicht weiter, solange Datei oder altes Passwort fehlen", async () => {
    const s = einhaengen();
    s.klick(s.knopf("Schlüssel aus Version 1 übernehmen"));
    s.klick(s.knopf("Weiter"));

    // Das neue Passwort allein genügt nicht.
    s.tippen(s.feld("password", 1)!, NEUES);
    s.tippen(s.feld("password", 2)!, NEUES);
    s.klick(s.kaesten()[0]!);
    expect(s.knopf("Weiter")!.hasAttribute("disabled")).toBe(true);

    // Datei dazu -- immer noch nicht, das alte Passwort fehlt.
    await s.klickWarten(s.knopf("Datei wählen"));
    expect(s.knopf("Weiter")!.hasAttribute("disabled")).toBe(true);

    // Und jetzt.
    s.tippen(s.feld("password", 0)!, "das alte Wort");
    expect(s.knopf("Weiter")!.hasAttribute("disabled")).toBe(false);

    s.aufraeumen();
  });

  it("macht aus dem Signierschlüssel eine Auskunft statt einer Wahl", async () => {
    const s = einhaengen();
    s.klick(s.knopf("Schlüssel aus Version 1 übernehmen"));
    s.klick(s.knopf("Weiter"));
    await s.klickWarten(s.knopf("Datei wählen"));
    s.tippen(s.feld("password", 0)!, "das alte Wort");
    s.tippen(s.feld("password", 1)!, NEUES);
    s.tippen(s.feld("password", 2)!, NEUES);
    s.klick(s.kaesten()[0]!);
    s.klick(s.knopf("Weiter"));

    const text = s.text();
    expect(text).toContain("Wird aus dem alten Schlüssel übernommen");
    // Kein Kästchen, das eine Entscheidung vortäuscht, die es nicht gibt.
    expect(text).not.toContain("Signierschlüssel anlegen");

    s.aufraeumen();
  });

  it("nennt am Ende den geänderten Fingerprint und was bleibt", async () => {
    const s = einhaengen();
    s.klick(s.knopf("Schlüssel aus Version 1 übernehmen"));
    s.klick(s.knopf("Weiter"));
    await s.klickWarten(s.knopf("Datei wählen"));
    s.tippen(s.feld("password", 0)!, "das alte Wort");
    s.tippen(s.feld("password", 1)!, NEUES);
    s.tippen(s.feld("password", 2)!, NEUES);
    s.klick(s.kaesten()[0]!);
    s.klick(s.knopf("Weiter"));
    // Und der Knopf heisst hier „Schluessel uebernehmen“ und nicht
    // „Schluessel erzeugen“: Es entsteht kein neuer Schluessel.
    expect(s.knopf("Schlüssel erzeugen")).toBeUndefined();
    await s.klickWarten(s.knopf("Schlüssel übernehmen"));

    const text = s.text();
    expect(text).toContain("Fingerprint hat sich geändert");
    // Und der zweite Halbsatz, ohne den der erste Angst macht.
    expect(text).toContain("weiterhin öffnen");

    s.aufraeumen();
  });
});
