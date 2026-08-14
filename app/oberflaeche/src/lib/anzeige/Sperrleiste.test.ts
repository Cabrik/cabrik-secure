/**
 * Die Warnstaffel als Anzeige.
 *
 * Die Schwellen selbst sind in `zustand.test.ts` geprüft. Hier geht es um
 * das, was die reine Funktion nicht abdeckt: **dass der Normalfall
 * schweigt.**
 *
 * Das ist die Eigenschaft, die am leichtesten verlorengeht. Ein Zähler, der
 * die Restzeit dauerhaft anzeigt, wäre in jedem einzelnen Schwellentest
 * ebenfalls grün — er zeigt ja bei 150 Sekunden das Richtige an. Falsch ist
 * er bei 800, wo nichts dastehen darf.
 */

import { beforeEach, describe, expect, it } from "vitest";
import { mount, unmount } from "svelte";
import Sperrleiste from "./Sperrleiste.svelte";
import { sitzungsspeicher } from "../kern/speicher.svelte";
import { MockBruecke } from "../kern/bruecke";
import { KONTAKTE } from "../kern/mock";
import { abgewickelt } from "../kern/pruefstand.svelte";
import type { Sitzungsstand } from "../kern/typen";

/**
 * Eine Brücke mit anhaltbarer Uhr.
 *
 * Die Attrappe rechnet mit `Date.now()` — richtig für den Prototyp, aber
 * für einen Test hieße das, vierzehn Minuten zu warten.
 */
class StehendeUhr extends MockBruecke {
  constructor(private festerStand: Sitzungsstand) {
    super(KONTAKTE);
  }

  override async sitzungsstand(): Promise<Sitzungsstand> {
    return this.festerStand;
  }
}

async function darstellen(stand: Sitzungsstand) {
  sitzungsspeicher.verbinde(new StehendeUhr(stand));
  await sitzungsspeicher.laden();

  const ziel = document.createElement("div");
  document.body.append(ziel);
  const b = mount(Sperrleiste, { target: ziel });
  await abgewickelt();
  return {
    ziel,
    text: () => ziel.textContent ?? "",
    knopf: (teil: string) =>
      [...ziel.querySelectorAll("button")].find((k) =>
        k.textContent?.includes(teil),
      ) as HTMLButtonElement | undefined,
    abbauen: () => void unmount(b),
  };
}

const OFFEN = (rest: number | null): Sitzungsstand => ({
  gesperrt: false,
  frist: "fuenfzehnMinuten",
  restsekunden: rest,
});

beforeEach(() => {
  sitzungsspeicher.verbinde(new MockBruecke(KONTAKTE));
});

describe("Sperrleiste", () => {
  it("schweigt, solange noch Zeit ist", async () => {
    // Der Fall, der einen dauerhaften Zähler auffliegen lässt.
    const s = await darstellen(OFFEN(800));

    expect(s.text()).not.toMatch(/Sperrt|noch \d/);
    s.abbauen();
  });

  it("zeigt keine Restzeit in Ziffern, solange nur leise gewarnt wird", async () => {
    // „Sperrt bald“ genügt hier. Eine Zahl fordert dazu auf, sie im Auge zu
    // behalten — und genau das soll zehn Minuten vor Ablauf niemand tun.
    const s = await darstellen(OFFEN(250));

    expect(s.text()).toContain("Sperrt bald");
    expect(s.text()).not.toMatch(/noch \d/);
    s.abbauen();
  });

  it("nennt die Restzeit im Klartext, wenn es knapp wird", async () => {
    const s = await darstellen(OFFEN(120));

    expect(s.text()).toContain("noch 2 Minuten");
    s.abbauen();
  });

  it("zählt die letzten Sekunden herunter", async () => {
    const s = await darstellen(OFFEN(12));

    expect(s.text()).toContain("noch 12 Sekunden");
    s.abbauen();
  });

  it("warnt gelb und nie rot", async () => {
    // Eine bevorstehende Sperre ist kein Vorfall. Rot hier verbraucht die
    // Farbe, die beim nächsten echten Fehler etwas heißen soll.
    const s = await darstellen(OFFEN(12));
    const warnung = s.ziel.querySelector('[role="alert"]')!;

    expect(warnung.className).toContain("warnung");
    expect(warnung.className).not.toContain("fehler");
    s.abbauen();
  });

  it("zeigt die eingestellte Frist als Sollwert", async () => {
    // Magenta, nicht Cyan: Das hat der Nutzer verlangt, nicht das Programm
    // gelesen (`spec/anzeige.md` §3a).
    const s = await darstellen(OFFEN(800));
    const marke = [...s.ziel.querySelectorAll("p")].find((p) =>
      p.textContent?.includes("Nach 15 Minuten"),
    )!;

    expect(marke.className).toContain("sollwert");
    s.abbauen();
  });

  it("sagt bei „ohne Frist“ dazu, was das bedeutet", async () => {
    // Die Wahl klingt harmlos. Ohne den Satz wählt sie jemand, weil sie
    // bequem ist, und erfährt nie, was er damit aufgibt.
    const s = await darstellen(OFFEN(800));
    s.knopf("Frist")!.click();
    await abgewickelt();

    expect(s.text()).toContain("Bis das Fenster geschlossen wird");
    expect(s.text()).toContain("auch wenn Sie den Raum verlassen");
    s.abbauen();
  });

  it("bietet keine Frist über einer Stunde an", async () => {
    // Zwei oder vier Stunden wären keine eigene Entscheidung, sondern
    // dieselbe wie „ohne Frist“ — nur als Vorsicht verkleidet.
    const s = await darstellen(OFFEN(800));
    s.knopf("Frist")!.click();
    await abgewickelt();

    expect(s.text()).not.toMatch(/[2-9] Stunden|Nach 2 Stunden/);
    s.abbauen();
  });

  it("verschwindet, wenn gesperrt ist", async () => {
    // Sonst stünde neben dem Passwortfeld ein Knopf „Jetzt sperren“.
    const s = await darstellen({
      gesperrt: true,
      frist: "fuenfzehnMinuten",
      restsekunden: null,
    });

    expect(s.text().trim()).toBe("");
    s.abbauen();
  });

  it("sperrt auf Knopfdruck", async () => {
    sitzungsspeicher.verbinde(new MockBruecke(KONTAKTE));
    await sitzungsspeicher.laden();
    const ziel = document.createElement("div");
    document.body.append(ziel);
    const b = mount(Sperrleiste, { target: ziel });
    await abgewickelt();

    [...ziel.querySelectorAll("button")]
      .find((k) => k.textContent?.includes("Jetzt sperren"))!
      .click();
    await abgewickelt();

    expect(sitzungsspeicher.stand?.gesperrt).toBe(true);
    unmount(b);
  });
});
