/**
 * Der Hinweis, dass §3.4 auf diesem Rechner nicht voll greift.
 *
 * # Warum das ein eigener Test ist
 *
 * Weil die Attrappe den **günstigen** Fall nachstellt — auf Windows und
 * macOS gilt er immer, auf den meisten Linux-Systemen auch. Die beiden
 * unbequemen Fälle kommen im Prototyp also nie vor, und was nie vorkommt,
 * fällt auch nicht auf, wenn es verschwindet.
 *
 * # Was auf dem Spiel steht
 *
 * `spec/entsperrung.md` §3.4 sagt zu, dass vor Bereitschaft und
 * Ruhezustand gesperrt wird. Auf einem Rechner, wo das nicht gilt, ist
 * diese Zusage eine Unwahrheit — es sei denn, das Programm sagt es.
 *
 * Und der schweigende Fall ist der gefährlichere: Wer „15 Minuten“ liest
 * und annimmt, der Deckel zähle auch, klappt den Laptop zu und glaubt
 * sich geschützt.
 */

import { beforeEach, describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Sperrleiste from "./Sperrleiste.svelte";
import { sitzungsspeicher } from "../kern/speicher.svelte";
import type { Ruheschutz, Sitzungsstand } from "../kern/typen";

const OFFEN: Sitzungsstand = {
  gesperrt: false,
  frist: "fuenfzehnMinuten",
  restsekunden: 900,
};

function zeigen(schutz: Ruheschutz | null) {
  sitzungsspeicher.stand = OFFEN;
  sitzungsspeicher.ruheschutz = schutz;

  const ziel = document.createElement("div");
  document.body.append(ziel);
  const b = mount(Sperrleiste, { target: ziel, props: {} });
  flushSync();
  return {
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    hinweis: () => ziel.querySelector('[data-pruef="ruheschutz"]'),
    aufraeumen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

beforeEach(() => {
  sitzungsspeicher.ruheschutz = null;
});

describe("der Hinweis zum Ruhezustand", () => {
  it("schweigt, solange noch nicht gefragt wurde", () => {
    // `null` heisst „noch nicht gefragt“, nicht „nein“. Ein Hinweis, der
    // im ersten Augenblick aufblitzt und dann verschwindet, ist schlimmer
    // als keiner: Er lehrt, ihn zu uebersehen.
    const s = zeigen(null);
    expect(s.hinweis()).toBeNull();
    s.aufraeumen();
  });

  it("schweigt auch im guenstigen Fall", () => {
    // Ein Programm, das seine funktionierenden Schutzmassnahmen aufzaehlt,
    // erzieht dazu, den Kasten zu ueberlesen -- und dann faellt auch der
    // Fall nicht auf, in dem etwas fehlt.
    const s = zeigen({ art: "mitAufschub" });
    expect(s.hinweis()).toBeNull();
    s.aufraeumen();
  });

  it("sagt es, wenn das System keine Zeit zusagt", () => {
    const s = zeigen({ art: "ohneAufschub" });
    const text = s.text();

    expect(s.hinweis()).not.toBeNull();
    // Beide Hälften: dass gesperrt wird UND was dabei offenbleibt.
    expect(text).toContain("wird gesperrt");
    expect(text).toContain("keine Zeit zu");

    s.aufraeumen();
  });

  it("sagt es, wenn gar nicht gesperrt wird — samt Grund", () => {
    const s = zeigen({
      art: "nicht",
      grund: "Der Systembus ist nicht erreichbar.",
    });
    const text = s.text();

    expect(text).toContain("nicht gesperrt");
    // Der Grund geht mit. Ohne ihn bliebe „geht nicht“ stehen, und
    // niemand wuesste, ob er etwas dagegen tun kann.
    expect(text).toContain("Systembus");
    // Und der Verweis auf das, was stattdessen gilt.
    expect(text).toContain("Frist");

    s.aufraeumen();
  });

  it("erschreckt nicht mit Rot", () => {
    // Hier ist nichts gescheitert. Es ist eine Eigenschaft dieses
    // Rechners, und der Nutzer kann sie meist nicht aendern -- ihn mit Rot
    // zu einer Handlung zu draengen, die es nicht gibt, waere die falsche
    // Farbe (`spec/anzeige.md`).
    const s = zeigen({ art: "nicht", grund: "Kein logind gefunden." });
    const klassen = s.hinweis()?.className ?? "";

    expect(klassen).toContain("text-warnung");
    expect(klassen).not.toContain("text-fehler");

    s.aufraeumen();
  });
});
