/**
 * Der Doppelklick im Explorer.
 *
 * # Der Fall, um den es geht
 *
 * **Doppelgeklickt, während gesperrt ist.** Das ist nicht der Randfall,
 * sondern der Normalfall: Wer eine Datei im Explorer anklickt, hat das
 * Fenster gerade nicht offen — und wenn es offen war, ist die Frist
 * wahrscheinlich abgelaufen. Ohne Behandlung endete der Doppelklick
 * stillschweigend am Sperrbildschirm, und das Programm sähe kaputt aus.
 *
 * # Die zweite Zusicherung
 *
 * **Das Fach wird auch im gesperrten Zustand geleert.** Der Kern hält den
 * Pfad, bis jemand ihn abholt; wer nur bei entsperrter Sitzung abholte,
 * bekäme ihn bei jedem Takt erneut — und ein Doppelklick von gestern
 * öffnete sich morgen wieder.
 *
 * # Und die dritte
 *
 * **Ein Fehlschlag wiederholt sich nicht.** Der Pfad wird vor dem Öffnen
 * weggenommen. Andersherum bliebe er bei einer kaputten Datei stehen und
 * würde beim nächsten Takt erneut versucht — eine Schleife aus
 * Fehlermeldungen, die niemand abstellen kann.
 */

import { beforeEach, describe, expect, it } from "vitest";
import { MockBruecke } from "./bruecke";
import { KONTAKTE, FAELLE } from "./mock";
import { empfangsspeicher } from "./speicher.svelte";
import type { Geoeffnet } from "./typen";

/** Eine Brücke, die eine hereingereichte Datei vortäuscht. */
class MitDatei extends MockBruecke {
  /** Wie oft das Fach abgefragt wurde. */
  abgefragt = 0;
  /** Was tatsächlich geöffnet wurde. */
  readonly geoeffnet: string[] = [];

  constructor(
    private fach: string | null,
    private scheitern = false,
  ) {
    super(KONTAKTE);
  }

  override async dateiAbholen(): Promise<string | null> {
    this.abgefragt += 1;
    // Das Abholen LEERT das Fach -- wie im Kern.
    const p = this.fach;
    this.fach = null;
    return p;
  }

  override async envelopeOeffnen(pfad: string): Promise<Geoeffnet> {
    this.geoeffnet.push(pfad);
    if (this.scheitern) throw new Error("Diese Datei ließ sich nicht öffnen.");
    return FAELLE[0]!.daten;
  }
}

beforeEach(() => {
  empfangsspeicher.verbinde(new MockBruecke(KONTAKTE));
  empfangsspeicher.wartendeDatei = null;
});

describe("eine hereingereichte Datei", () => {
  it("wird bei entsperrter Sitzung sofort geöffnet", async () => {
    const b = new MitDatei("C:\\Post\\bericht.pdf.cabrik");
    empfangsspeicher.verbinde(b);

    await empfangsspeicher.hereingereichtePruefen(true);

    expect(b.geoeffnet).toEqual(["C:\\Post\\bericht.pdf.cabrik"]);
    expect(empfangsspeicher.geoeffnet).not.toBeNull();
  });

  it("wartet, solange gesperrt ist — statt zu scheitern", async () => {
    const b = new MitDatei("C:\\Post\\bericht.pdf.cabrik");
    empfangsspeicher.verbinde(b);

    await empfangsspeicher.hereingereichtePruefen(false);

    expect(b.geoeffnet, "gesperrt darf nichts geöffnet werden").toEqual([]);
    expect(empfangsspeicher.wartendeDatei).toBe("C:\\Post\\bericht.pdf.cabrik");
    expect(empfangsspeicher.fehler).toBeNull();
  });

  it("geht nach dem Entsperren von selbst auf", async () => {
    // Der ganze Sinn des Wartens.
    const b = new MitDatei("C:\\Post\\bericht.pdf.cabrik");
    empfangsspeicher.verbinde(b);
    await empfangsspeicher.hereingereichtePruefen(false);

    await empfangsspeicher.hereingereichtePruefen(true);

    expect(b.geoeffnet).toEqual(["C:\\Post\\bericht.pdf.cabrik"]);
    expect(empfangsspeicher.wartendeDatei).toBeNull();
  });

  it("leert das Fach im Kern auch im gesperrten Zustand", async () => {
    /*
     * Sonst bekäme die Oberfläche den Pfad bei jedem Takt erneut — und ein
     * Doppelklick von gestern öffnete sich morgen wieder.
     */
    const b = new MitDatei("C:\\Post\\bericht.pdf.cabrik");
    empfangsspeicher.verbinde(b);

    await empfangsspeicher.hereingereichtePruefen(false);
    await empfangsspeicher.hereingereichtePruefen(false);

    expect(b.abgefragt).toBe(2);
    // Beim zweiten Mal kam nichts mehr -- der Pfad von vorhin steht aber
    // weiter bereit.
    expect(empfangsspeicher.wartendeDatei).toBe("C:\\Post\\bericht.pdf.cabrik");
  });

  it("versucht eine kaputte Datei nicht endlos erneut", async () => {
    // Der Pfad wird VOR dem Öffnen weggenommen. Andersherum entstünde eine
    // Schleife aus Fehlermeldungen, die niemand abstellen kann.
    const b = new MitDatei("C:\\Post\\kaputt.cabrik", true);
    empfangsspeicher.verbinde(b);

    await empfangsspeicher.hereingereichtePruefen(true);
    expect(empfangsspeicher.fehler).toContain("nicht öffnen");
    expect(empfangsspeicher.wartendeDatei).toBeNull();

    await empfangsspeicher.hereingereichtePruefen(true);

    expect(b.geoeffnet, "einmal versucht, nicht zweimal").toHaveLength(1);
  });

  it("tut nichts, wenn nichts hereingereicht wurde", async () => {
    const b = new MitDatei(null);
    empfangsspeicher.verbinde(b);

    await empfangsspeicher.hereingereichtePruefen(true);

    expect(b.geoeffnet).toEqual([]);
    expect(empfangsspeicher.wartendeDatei).toBeNull();
    expect(empfangsspeicher.fehler).toBeNull();
  });

  it("im Browser passiert nichts und nichts geht kaputt", async () => {
    // Die Attrappe reicht nie etwas herein. Ein erfundener Pfad brächte den
    // Empfangsbildschirm dazu, eine Datei zu suchen, die es nicht gibt.
    empfangsspeicher.verbinde(new MockBruecke(KONTAKTE));

    await empfangsspeicher.hereingereichtePruefen(true);

    expect(empfangsspeicher.wartendeDatei).toBeNull();
    expect(empfangsspeicher.fehler).toBeNull();
  });

  it("das Abmelden geht auch ohne Ereignis", async () => {
    const loesen = await empfangsspeicher.aufHereingereichtHorchen(() => true);

    expect(() => loesen()).not.toThrow();
  });
});
