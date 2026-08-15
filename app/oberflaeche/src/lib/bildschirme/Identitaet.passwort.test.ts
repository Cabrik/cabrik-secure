/**
 * Sichern und Passwort ändern.
 *
 * # Die Zusicherung, die zählt
 *
 * **Die Felder sind nach dem Versuch leer — auch bei Fehlschlag.** Was
 * dort stehen bleibt, ist ein Passwort im Speicher der Webansicht, und
 * zwar unbegrenzt lange: Niemand tippt es weg, wenn es funktioniert hat.
 * Dieselbe Regel wie auf dem Sperrbildschirm.
 *
 * # Und die zweite
 *
 * **Der Satz über die alten Sicherungskopien steht da, bevor jemand
 * tippt.** Wer wechselt, weil das bisherige Passwort verbrannt ist, hat
 * mit einer alten Kopie nichts gewonnen — sie öffnet sich weiter damit.
 */

import { describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Identitaet from "./Identitaet.svelte";
import { IDENTITAET } from "../kern/mock";

function zeigen(gelingt = true) {
  const gerufen: { alt: string; neu: string }[] = [];
  const ziel = document.createElement("div");
  document.body.append(ziel);
  const b = mount(Identitaet, {
    target: ziel,
    props: {
      identitaet: IDENTITAET,
      passwortAendern: async (alt: string, neu: string) => {
        gerufen.push({ alt, neu });
        return gelingt;
      },
      sichern: () => {},
    },
  });
  const felder = () => [
    ...ziel.querySelectorAll<HTMLInputElement>('input[type="password"]'),
  ];
  return {
    ziel,
    gerufen,
    felder,
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    knopf: (teil: string) =>
      [...ziel.querySelectorAll("button")].find((k) =>
        k.textContent?.includes(teil),
      ),
    wechselKnopf: () =>
      ziel.querySelector<HTMLButtonElement>(
        '[data-pruefstelle="passwort-aendern"]',
      ),
    oeffnen: () => {
      [...ziel.querySelectorAll("button")]
        .find((k) => k.textContent?.trim() === "Passwort ändern")!
        .click();
      flushSync();
    },
    tippen: (nr: number, wert: string) => {
      const f = felder()[nr]!;
      f.value = wert;
      f.dispatchEvent(new Event("input", { bubbles: true }));
      flushSync();
    },
    abbauen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

describe("Passwort ändern", () => {
  it("ist zugeklappt, bis jemand es aufmacht", () => {
    // Es ist der seltenere Vorgang. Drei Felder dauerhaft hinzustellen
    // hieße, den Bildschirm nach dem Ausnahmefall zu ordnen.
    const s = zeigen();

    expect(s.felder()).toHaveLength(0);
    s.oeffnen();
    expect(s.felder()).toHaveLength(3);
    s.abbauen();
  });

  it("sagt vorher, dass es keinen neuen Schlüssel gibt", () => {
    // Die Erwartung, die am häufigsten danebenliegt. Wer sie mitbringt,
    // hält sich für geschützt, wo er es nicht ist.
    const s = zeigen();
    s.oeffnen();

    expect(s.text()).toContain("schützt nicht davor, dass jemand Ihren Schlüssel schon hat");
    s.abbauen();
  });

  it("sagt vorher, dass alte Sicherungskopien weiter aufgehen", () => {
    const s = zeigen();
    s.oeffnen();

    expect(s.text()).toContain("weiter mit dem bisherigen Passwort");
    s.abbauen();
  });

  it("verlangt beide Passwörter und eine passende Wiederholung", () => {
    const s = zeigen();
    s.oeffnen();
    expect(s.wechselKnopf()!.disabled).toBe(true);

    s.tippen(0, "altes passwort");
    s.tippen(1, "neues passwort");
    expect(s.wechselKnopf()!.disabled, "ohne Wiederholung nicht").toBe(true);

    s.tippen(2, "neues passwor");
    expect(s.wechselKnopf()!.disabled, "bei Tippfehler nicht").toBe(true);

    s.tippen(2, "neues passwort");
    expect(s.wechselKnopf()!.disabled, "mit allem schon").toBe(false);
    s.abbauen();
  });

  it("reicht beide Passwörter weiter", () => {
    const s = zeigen();
    s.oeffnen();
    s.tippen(0, "altes passwort");
    s.tippen(1, "neues passwort");
    s.tippen(2, "neues passwort");

    s.wechselKnopf()!.click();
    flushSync();

    expect(s.gerufen).toEqual([{ alt: "altes passwort", neu: "neues passwort" }]);
    s.abbauen();
  });

  it("leert die Felder danach", () => {
    const s = zeigen();
    s.oeffnen();
    s.tippen(0, "altes passwort");
    s.tippen(1, "neues passwort");
    s.tippen(2, "neues passwort");

    s.wechselKnopf()!.click();
    flushSync();

    for (const f of s.felder()) expect(f.value).toBe("");
    s.abbauen();
  });

  it("leert sie auch, wenn es schiefgeht", () => {
    // Der eigentliche Fall. Nach dem Gelingen sieht man ohnehin weg; nach
    // einem Fehlschlag bleibt der Bildschirm stehen — und mit ihm die
    // Felder, wenn sie niemand leert.
    const s = zeigen(false);
    s.oeffnen();
    s.tippen(0, "falsches altes");
    s.tippen(1, "neues passwort");
    s.tippen(2, "neues passwort");

    s.wechselKnopf()!.click();
    flushSync();

    for (const f of s.felder()) expect(f.value).toBe("");
    s.abbauen();
  });

  it("bleibt gesperrt, wenn gar nicht gewechselt werden kann", () => {
    // Im Browser gibt es keine Schlüsseldatei.
    const ziel = document.createElement("div");
    document.body.append(ziel);
    const b = mount(Identitaet, {
      target: ziel,
      props: { identitaet: IDENTITAET },
    });

    const knopf = [...ziel.querySelectorAll("button")].find(
      (k) => k.textContent?.trim() === "Passwort ändern",
    );
    expect(knopf!.disabled).toBe(true);
    unmount(b);
    ziel.remove();
  });
});
