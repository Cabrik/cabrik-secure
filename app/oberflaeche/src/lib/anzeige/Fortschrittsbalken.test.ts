/**
 * Der Balken für große Stapel.
 *
 * # Die Zusicherung, die zählt
 *
 * **Die Zahlen stehen da, nicht nur der Balken.** Ein Balken allein ist
 * Farbe allein, und die genügt in diesem Programm nirgends (`spec/anzeige.md`
 * §2.3). Wer ihn nicht sieht — zu blass, schlechter Bildschirm,
 * Bildschirmleser —, bekommt „3 von 40, Foto.jpg“ und weiß dasselbe.
 *
 * # Und die zweite
 *
 * **Es steht dabei, WAS läuft.** Beim Löschen ist das keine Kleinigkeit:
 * Der Vorgang ist unwiderruflich, und wer ihn mit dem Prüfen verwechselt,
 * wartet gelassen auf etwas anderes, als gerade geschieht.
 */

import { describe, expect, it } from "vitest";
import { mount, unmount } from "svelte";
import Fortschrittsbalken from "./Fortschrittsbalken.svelte";
import type { Stapelstand } from "../kern/typen";

function zeigen(stand: Stapelstand) {
  const ziel = document.createElement("div");
  document.body.append(ziel);
  const b = mount(Fortschrittsbalken, {
    target: ziel,
    props: { fortschritt: stand },
  });
  return {
    ziel,
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    balken: () => ziel.querySelector<HTMLElement>('[role="progressbar"]'),
    abbauen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

const STAND: Stapelstand = {
  erledigt: 3,
  gesamt: 40,
  laeuft: "Foto.jpg",
  art: "pruefen",
};

describe("Fortschrittsbalken", () => {
  it("schreibt die Zahlen hin, nicht nur den Balken", () => {
    const s = zeigen(STAND);

    expect(s.text()).toContain("3 von 40");
    s.abbauen();
  });

  it("nennt die Datei, an der es gerade steht", () => {
    // „3 von 40“ allein sagt nicht, ob es hakt oder läuft. Bleibt eine
    // Minute lang derselbe Name stehen, weiß man wenigstens, WELCHE Datei
    // aufhält — und dass es nicht das Programm ist.
    const s = zeigen(STAND);

    expect(s.text()).toContain("Foto.jpg");
    s.abbauen();
  });

  it("sagt, was gerade geschieht", () => {
    const s = zeigen(STAND);

    expect(s.text()).toContain("Wird geprüft");
    s.abbauen();
  });

  it("unterscheidet Löschen vom Prüfen", () => {
    // Der Unterschied, der hier am meisten wiegt: Löschen ist
    // unwiderruflich.
    const s = zeigen({ ...STAND, art: "loeschen" });

    expect(s.text()).toContain("Wird gelöscht");
    expect(s.text()).not.toContain("Wird geprüft");
    s.abbauen();
  });

  it("meldet den Stand an Bildschirmleser weiter", () => {
    const s = zeigen(STAND);
    const balken = s.balken();

    expect(balken?.getAttribute("aria-valuenow")).toBe("3");
    expect(balken?.getAttribute("aria-valuemax")).toBe("40");
    s.abbauen();
  });

  it("steht bei der ersten Datei auf null", () => {
    /*
     * `erledigt` zählt die FERTIGEN — die laufende ist noch nicht dabei.
     * Den Balken bei der ersten Datei schon ein Vierzigstel weit zu füllen
     * hieße, eine Datei als erledigt zu zeigen, die gerade erst anfängt.
     */
    const s = zeigen({ ...STAND, erledigt: 0 });

    expect(s.ziel.querySelector<HTMLElement>("[style]")?.style.width).toBe("0%");
    s.abbauen();
  });

  it("kommt mit einem leeren Stapel zurecht, statt durch null zu teilen", () => {
    const s = zeigen({ ...STAND, erledigt: 0, gesamt: 0, laeuft: "" });

    expect(s.text()).toContain("0 von 0");
    expect(s.ziel.querySelector<HTMLElement>("[style]")?.style.width).toBe("0%");
    s.abbauen();
  });
});
