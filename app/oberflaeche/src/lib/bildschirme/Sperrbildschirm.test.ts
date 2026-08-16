/**
 * Was der Sperrbildschirm leisten und was er lassen muss.
 *
 * # Der Test, um den es hier eigentlich geht
 *
 * `das_passwortfeld_ist_danach_leer`. Alles andere auf diesem Bildschirm
 * ist Gestaltung; das ist eine Zusicherung. Ein stehengebliebenes
 * Eingabefeld ist ein Passwort im Speicher der Webansicht — und zwar
 * unbegrenzt lange, denn niemand tippt es weg, wenn es funktioniert hat.
 *
 * Es ist zugleich die Sorte Eigenschaft, die beim Umbauen still
 * verschwindet: Wer das Feld später an einen Zustand bindet, um „den
 * letzten Versuch zu behalten“, hat sie aufgehoben, ohne es zu merken.
 *
 * # Was hier ebenfalls geprüft wird
 *
 * Dass der Bildschirm **nichts verrät**: kein Name, kein Fingerprint, kein
 * Versuchszähler, kein Hinweis darauf, wie falsch das Passwort war. Diese
 * Tests suchen nach Abwesenheit, was für sich genommen schwach ist —
 * deshalb suchen sie nach benannten Dingen aus den Beispieldaten, die
 * tatsächlich dastehen könnten.
 */

import { beforeEach, describe, expect, it } from "vitest";
import { mount, unmount } from "svelte";
import Sperrbildschirm from "./Sperrbildschirm.svelte";
import { empfangsspeicher, sitzungsspeicher } from "../kern/speicher.svelte";
import { MockBruecke } from "../kern/bruecke";
import { KONTAKTE } from "../kern/mock";
import { abgewickelt } from "../kern/pruefstand.svelte";

beforeEach(async () => {
  sitzungsspeicher.verbinde(new MockBruecke(KONTAKTE));
  await sitzungsspeicher.sperren();
});

function darstellen() {
  const ziel = document.createElement("div");
  document.body.append(ziel);
  const b = mount(Sperrbildschirm, { target: ziel });
  const feld = () =>
    ziel.querySelector<HTMLInputElement>('input[aria-label="Passwort"]')!;
  const knopf = (teil: string) =>
    [...ziel.querySelectorAll("button")].find((k) =>
      k.textContent?.includes(teil),
    ) as HTMLButtonElement | undefined;
  return {
    ziel,
    feld,
    knopf,
    /** Tippt und schickt ab — wie ein Mensch, über das Formular. */
    async versuchen(passwort: string) {
      const f = feld();
      f.value = passwort;
      f.dispatchEvent(new Event("input", { bubbles: true }));
      await abgewickelt();
      ziel.querySelector("form")!.dispatchEvent(
        new Event("submit", { bubbles: true, cancelable: true }),
      );
      await abgewickelt();
    },
    abbauen: () => void unmount(b),
  };
}

describe("Sperrbildschirm", () => {
  it("leert das Passwortfeld nach einem erfolgreichen Versuch", async () => {
    const s = darstellen();

    await s.versuchen("ein gutes langes passwort");

    expect(s.feld().value).toBe("");
    s.abbauen();
  });

  it("leert das Passwortfeld auch nach einem gescheiterten Versuch", async () => {
    // Der eigentliche Fall. Nach dem Gelingen verschwindet der Bildschirm
    // ohnehin; nach dem Fehlschlag bleibt er stehen — und mit ihm das Feld,
    // wenn es niemand leert.
    const s = darstellen();

    await s.versuchen("x");

    expect(s.feld().value).toBe("");
    expect(sitzungsspeicher.stand?.gesperrt).toBe(true);
    s.abbauen();
  });

  it("nennt genau eine Meldung und nicht, wie falsch das Passwort war", async () => {
    const s = darstellen();

    await s.versuchen("x");
    const text = s.ziel.textContent ?? "";

    expect(text).toContain("Das Passwort passt nicht");
    // Nichts, was den Abstand zum richtigen Passwort beziffert.
    expect(text).not.toMatch(/Zeichen|Länge|fast|beinahe|ähnlich/i);
    s.abbauen();
  });

  it("zählt Versuche nicht", async () => {
    const s = darstellen();

    await s.versuchen("x");
    await s.versuchen("y");
    await s.versuchen("z");

    const text = s.ziel.textContent ?? "";
    expect(text).not.toMatch(/Versuch|verbleibend|gesperrt für|erneut in/i);
    s.abbauen();
  });

  it("verrät nicht, wessen Rechner das ist", () => {
    // Die Beispieldaten führen Namen und Fingerprints. Steht einer davon
    // auf dem Sperrbildschirm, ist die Zusicherung aus §4.1 gebrochen.
    const s = darstellen();
    const text = s.ziel.textContent ?? "";

    for (const k of KONTAKTE) {
      expect(text).not.toContain(k.name);
      expect(text).not.toContain(k.fingerprint);
    }
    s.abbauen();
  });

  it("hält den Satz aus der Einrichtung wörtlich bereit", () => {
    // In dem Augenblick, in dem er zählt. Wer ihn bei der Einrichtung für
    // Beiwerk gehalten hat, liest ihn hier wieder.
    const s = darstellen();

    expect(s.ziel.textContent).toContain(
      "Wenn dieses Passwort weg ist, ist alles weg.",
    );
    s.abbauen();
  });

  it("schickt kein leeres Passwort ab", async () => {
    const s = darstellen();

    expect(s.knopf("Entsperren")!.disabled).toBe(true);
    s.abbauen();
  });

  it("versteckt das Passwort, bis jemand es sehen will", async () => {
    const s = darstellen();
    expect(s.feld().type).toBe("password");

    s.knopf("anzeigen")!.click();
    await abgewickelt();

    expect(s.feld().type).toBe("text");
    s.abbauen();
  });

  it("fällt nach dem Absenden auf verborgen zurück", async () => {
    // Sonst stünde das Passwort beim nächsten Versuch offen da — der
    // Zustand aus dem vorigen überlebte den Fehlschlag.
    const s = darstellen();
    s.knopf("anzeigen")!.click();
    await abgewickelt();

    await s.versuchen("x");

    expect(s.feld().type).toBe("password");
    s.abbauen();
  });

  it("entsperrt bei richtigem Passwort", async () => {
    // Die Gegenprobe zu allem oben: Ein Bildschirm, der niemanden
    // hereinlässt, bestünde jede der bisherigen Prüfungen.
    const s = darstellen();

    await s.versuchen("ein gutes langes passwort");

    expect(sitzungsspeicher.stand?.gesperrt).toBe(false);
    s.abbauen();
  });
});
// ---------------------------------------------------------------------------
// Der Doppelklick, der auf das Entsperren wartet
// ---------------------------------------------------------------------------

describe("eine wartende Datei", () => {
  it("wird gemeldet, damit der Klick nicht ins Leere geht", async () => {
    // Ohne das hält jemand das Programm für kaputt und klickt weiter.
    empfangsspeicher.wartendeDatei = "C:\Post\Kuendigung_Meyer.pdf.cabrik";
    const s = darstellen();

    expect(s.ziel.textContent).toContain("Eine Datei wartet");
    s.abbauen();
    empfangsspeicher.wartendeDatei = null;
  });

  it("aber ihr NAME steht nirgends", async () => {
    /*
     * Die Grundregel dieses Bildschirms: Wer auf einen fremden gesperrten
     * Bildschirm sieht, soll daraus nichts erfahren. Ein Dateiname verriete
     * genau das, was hier sonst überall zurückgehalten wird — und der
     * Mensch, der geklickt hat, muss nicht danebenstehen bleiben.
     *
     * Er weiß ohnehin, was er angeklickt hat. Der Fremde soll es nicht
     * erfahren.
     */
    empfangsspeicher.wartendeDatei = "C:\Post\Kuendigung_Meyer.pdf.cabrik";
    const s = darstellen();
    const text = s.ziel.textContent ?? "";

    expect(text).not.toContain("Kuendigung");
    expect(text).not.toContain("Meyer");
    expect(text).not.toContain("cabrik");
    expect(text).not.toContain("C:\\");
    s.abbauen();
    empfangsspeicher.wartendeDatei = null;
  });

  it("und ohne wartende Datei steht der Satz nicht da", async () => {
    // Die Gegenprobe: Ein Satz, der immer dasteht, sagt nichts.
    empfangsspeicher.wartendeDatei = null;
    const s = darstellen();

    expect(s.ziel.textContent).not.toContain("Eine Datei wartet");
    s.abbauen();
  });
});

