/**
 * Wie weit es **innerhalb** einer Datei ist.
 *
 * # Warum das ein eigener Test ist
 *
 * Weil die Attrappe keine Bytes kennt — sie liest keine Dateien. Im
 * Prototyp kommt dieser Fall also nie vor, und was nie vorkommt, fällt
 * auch nicht auf, wenn es verschwindet.
 *
 * # Und was hier am leichtesten falsch wird
 *
 * `null` statt `0`. Beim Bereinigen lässt sich der Fortschritt nicht in
 * Bytes messen; eine Null wäre dort die Behauptung, es sei noch nichts
 * geschehen — und der Balken zeigte „0 von 0“, was aussieht wie ein
 * hängendes Programm.
 */

import { describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Fortschrittsbalken from "./Fortschrittsbalken.svelte";
import type { Stapelstand } from "../kern/typen";

const GRUND: Stapelstand = {
  erledigt: 0,
  gesamt: 1,
  laeuft: "urlaub.mp4",
  schritt: "lesen",
  bytesErledigt: null,
  bytesGesamt: null,
  art: "pruefen",
};

function zeigen(stand: Stapelstand) {
  const ziel = document.createElement("div");
  document.body.append(ziel);
  const b = mount(Fortschrittsbalken, {
    target: ziel,
    props: { fortschritt: stand },
  });
  flushSync();
  return {
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    aufraeumen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

describe("der Fortschritt innerhalb einer Datei", () => {
  it("zeigt die Bytes, wenn der Kern sie kennt", () => {
    const s = zeigen({
      ...GRUND,
      bytesErledigt: 1_500_000_000,
      bytesGesamt: 3_000_000_000,
    });
    const text = s.text();

    expect(text).toContain("Lese urlaub.mp4");
    // Nicht die rohe Bytezahl: „1500000000“ liest niemand.
    //
    // Und GiB, nicht GB: 1 500 000 000 Bytes sind 1,4 GiB. Wer hier
    // „1,4 GB“ schriebe, teilte durch 1024 und beschriftete es dezimal --
    // die verbreitetste Ungenauigkeit der Branche und trotzdem eine.
    expect(text).toContain("1.4 GiB");
    expect(text).toContain("2.8 GiB");
    expect(text).toContain("von");

    s.aufraeumen();
  });

  it("schweigt, wenn er sie nicht kennt", () => {
    // Der Fall „Bereinigen“: Es dauert, aber nicht in Bytes messbar.
    const s = zeigen({ ...GRUND, schritt: "bereinigen" });
    const text = s.text();

    expect(text).toContain("urlaub.mp4");
    // KEIN „0 von 0“ -- das saehe aus wie ein haengendes Programm.
    expect(text).not.toContain("0 Bytes von");
    expect(text).not.toMatch(/\bvon 0\b/);

    s.aufraeumen();
  });

  it("schweigt auch bei einer Gesamtzahl von null", () => {
    // Eine leere Datei. „0 von 0“ waere formal richtig und trotzdem
    // nutzlos -- und ein Balken daraus waere eine Division durch null.
    const s = zeigen({ ...GRUND, bytesErledigt: 0, bytesGesamt: 0 });
    expect(s.text()).not.toContain("von 0 Bytes");
    s.aufraeumen();
  });
});
