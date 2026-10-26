/**
 * Was vor dem Passwortfeld kommen kann (`spec/entsperrung.md` §4.4).
 *
 * # Was hier auf dem Spiel steht
 *
 * Zwei Dinge lassen sich am Klartextkopf der Schlüsseldatei ablesen, ohne
 * Passwort: ob ihre Werte ungewöhnlich sind, und ob die Ableitung in den
 * Speicher passt. Beide entscheiden, ob es überhaupt etwas einzutippen gibt.
 *
 * Ein Feld, das trotzdem dasteht, lädt zum Tippen ein — und erst danach
 * käme die Absage. Im eigenen Passwortfenster läge das Passwort dann in
 * einem Puffer, der nur noch überschrieben werden kann.
 *
 * # Und die andere Richtung
 *
 * Im Normalfall steht **nichts** davon da. Eine Rückfrage bei jeder
 * Schlüsseldatei wird weggeklickt, ohne gelesen zu werden.
 */

import { afterEach, describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Sperrbildschirm from "./Sperrbildschirm.svelte";
import { MockBruecke } from "../kern/bruecke";
import { KONTAKTE } from "../kern/mock";
import { sitzungsspeicher } from "../kern/speicher.svelte";
import type { Entsperrbedarf, Passwortweg } from "../kern/typen";

const MIB = 1024 * 1024;
const GIB = 1024 * MIB;

const UNGEWOEHNLICH: Omit<Entsperrbedarf, "zugelassen"> = {
  speicherBytes: 3 * GIB,
  durchgaenge: 8,
  ungewoehnlich: true,
  grenzeBytes: 15 * GIB,
  passt: true,
};

const ZU_WENIG: Omit<Entsperrbedarf, "zugelassen"> = {
  speicherBytes: GIB,
  durchgaenge: 4,
  ungewoehnlich: false,
  grenzeBytes: 700 * MIB,
  passt: false,
};

/** Zeigt den Sperrbildschirm über einer Attrappe mit diesem Bedarf. */
async function zeigen(
  vorgabe: Omit<Entsperrbedarf, "zugelassen"> | null,
  weg: Passwortweg = { art: "webansicht", grund: "Vorschau" },
) {
  const bruecke = new MockBruecke(KONTAKTE);
  if (vorgabe) bruecke.bedarfVorgabe = { ...vorgabe };
  sitzungsspeicher.verbinde(bruecke);
  await bruecke.sperren();
  await sitzungsspeicher.laden();
  sitzungsspeicher.passwortweg = weg;

  const ziel = document.createElement("div");
  document.body.append(ziel);
  const b = mount(Sperrbildschirm, { target: ziel, props: {} });
  // Der Bildschirm fragt beim Erscheinen; hier wird abgewartet, statt zu
  // hoffen, dass die Antwort schon da ist.
  await sitzungsspeicher.bedarfLaden();
  flushSync();

  return {
    bruecke,
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    feld: () => ziel.querySelector('input[type="password"]'),
    knopf: (teil: string) =>
      [...ziel.querySelectorAll("button")].find((k) => k.textContent?.includes(teil)),
    pruef: (name: string) => ziel.querySelector(`[data-pruef="${name}"]`),
    aufraeumen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

afterEach(() => {
  sitzungsspeicher.bedarf = null;
  sitzungsspeicher.fehler = null;
  sitzungsspeicher.verbinde(new MockBruecke(KONTAKTE));
});

describe("vor dem Passwortfeld", () => {
  it("steht im Normalfall nichts davon da", async () => {
    const s = await zeigen(null);
    expect(s.pruef("rueckfrage")).toBeNull();
    expect(s.pruef("zu-wenig-speicher")).toBeNull();
    expect(s.feld()).not.toBeNull();
    s.aufraeumen();
  });

  it("fragt bei einer ungewöhnlichen Datei zuerst — mit ihren Zahlen", async () => {
    const s = await zeigen(UNGEWOEHNLICH);

    expect(s.pruef("rueckfrage")).not.toBeNull();
    const text = s.text();
    expect(text).toContain("3.0 GiB");
    expect(text).toContain("8 Durchgänge");
    // Die Folge, nicht nur die Regel: Warum man fortfahren kann, und was
    // es heißt, wenn die Datei nicht von einem selbst stammt.
    expect(text).toContain("verändert worden");

    expect(s.feld(), "das Feld steht vor der Antwort da").toBeNull();
    s.aufraeumen();
  });

  it("bietet auch das eigene Fenster erst nach der Antwort an", async () => {
    const s = await zeigen(UNGEWOEHNLICH, { art: "eigenesFenster" });
    expect(s.knopf("Passwort eingeben")).toBeUndefined();
    s.aufraeumen();
  });

  it("zeigt nach dem Zulassen das Feld — und fragt nicht noch einmal", async () => {
    const s = await zeigen(UNGEWOEHNLICH);

    s.knopf("Trotzdem fortfahren")!.click();
    // Die Zulassung geht zum Kern, und der Bedarf wird neu geholt: Ob es
    // gegriffen hat, sagt der Kern, nicht dieser Bildschirm.
    await sitzungsspeicher.ungewoehnlicheZulassen();
    flushSync();

    expect(s.pruef("rueckfrage")).toBeNull();
    expect(s.feld()).not.toBeNull();
    expect((await s.bruecke.entsperrbedarf())!.zugelassen).toBe(true);
    s.aufraeumen();
  });

  it("nennt bei zu wenig Speicher beide Zahlen statt eines Feldes", async () => {
    const s = await zeigen(ZU_WENIG);

    expect(s.pruef("zu-wenig-speicher")).not.toBeNull();
    const text = s.text();
    // Bedarf aufgerundet, Grenze abgerundet — zwei verschiedene Zahlen.
    expect(text).toContain("1.0 GiB");
    expect(text).toContain("700.0 MiB");
    // Wer das liest, soll nicht an seinem Passwort zweifeln.
    expect(text).toContain("Am Passwort liegt es nicht");

    expect(s.feld()).toBeNull();
    s.aufraeumen();
  });

  it("prüft auf Wunsch erneut — wer Programme schließt, kommt weiter", async () => {
    const s = await zeigen(ZU_WENIG);

    s.bruecke.bedarfVorgabe = { ...ZU_WENIG, grenzeBytes: 4 * GIB, passt: true };
    s.knopf("Erneut prüfen")!.click();
    await sitzungsspeicher.bedarfLaden();
    flushSync();

    expect(s.pruef("zu-wenig-speicher")).toBeNull();
    expect(s.feld()).not.toBeNull();
    s.aufraeumen();
  });
});

describe("die Attrappe hält die Regel wie der Kern", () => {
  it("entsperrt ohne Zulassung nicht", async () => {
    const b = new MockBruecke(KONTAKTE);
    b.bedarfVorgabe = { ...UNGEWOEHNLICH };
    await b.sperren();

    await expect(b.entsperren("vier zufaellige woerter hier")).rejects.toThrow(
      "bestätigen",
    );
    await b.ungewoehnlicheAbleitungZulassen();
    await expect(b.entsperren("vier zufaellige woerter hier")).resolves.toBeUndefined();
  });

  it("sagt bei zu wenig Speicher, woran es liegt", async () => {
    const b = new MockBruecke(KONTAKTE);
    b.bedarfVorgabe = { ...ZU_WENIG };
    await b.sperren();

    await expect(b.entsperren("vier zufaellige woerter hier")).rejects.toThrow(
      "Arbeitsspeicher",
    );
  });
});
