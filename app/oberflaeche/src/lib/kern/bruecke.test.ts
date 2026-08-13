/**
 * Die Naht selbst.
 *
 * Sie ist die Stelle, an der in Phase 4 der Kern einzieht. Was hier als
 * Regel steht, muss dort genauso gelten — deshalb prüfen diese Tests die
 * **Schnittstelle**, nicht den Bildschirm darüber.
 */

import { beforeEach, describe, expect, it } from "vitest";
import { MockBruecke, type Bruecke } from "./bruecke";
import { KONTAKTE, NUTZLASTEN } from "./mock";

let b: Bruecke;
beforeEach(() => {
  b = new MockBruecke(KONTAKTE);
});

describe("aufnehmen", () => {
  it("legt immer als „gesehen“ an", async () => {
    // Die tragende Regel des Vertrauensmodells, und sie steht hier statt
    // in der Anzeige: Es gibt keinen Parameter, mit dem man sie umginge.
    const neu = await b.kontaktAufnehmen("Neu", NUTZLASTEN[0]!.text);

    expect(neu.vertrauen).toBe("gesehen");
    expect(neu.verifiziertAm).toBeNull();
    expect(neu.verifiziertUeber).toBeNull();
  });

  it("vergibt eine Safety Number in zwölf Fünfergruppen", async () => {
    const neu = await b.kontaktAufnehmen("Neu", NUTZLASTEN[0]!.text);
    const gruppen = neu.safetyNumber.split(" ");

    expect(gruppen).toHaveLength(12);
    for (const g of gruppen) expect(g).toMatch(/^\d{5}$/);
  });

  it("und der Kontakt taucht danach in der Liste auf", async () => {
    const vorher = (await b.kontakte()).length;
    await b.kontaktAufnehmen("Neu", NUTZLASTEN[0]!.text);

    expect(await b.kontakte()).toHaveLength(vorher + 1);
  });
});

describe("verifizieren und zurücksetzen", () => {
  const bert = async () =>
    (await b.kontakte()).find((k) => k.name === "Bert Muster")!;

  it("verifizieren hält den benutzten Weg fest", async () => {
    const k = await bert();
    const neu = await b.kontaktVerifizieren(k.fingerprint, "qr");

    expect(neu.vertrauen).toBe("verifiziert");
    expect(neu.verifiziertUeber).toBe("qr");
    expect(neu.verifiziertAm).toBeTypeOf("number");
  });

  it("zurücksetzen widerruft nicht", async () => {
    const k = await bert();
    await b.kontaktVerifizieren(k.fingerprint, "safetyNumber");
    const neu = await b.kontaktZuruecksetzen(k.fingerprint);

    expect(neu.vertrauen).toBe("gesehen");
    expect(neu.vertrauen).not.toBe("widerrufen");
    expect(neu.verifiziertUeber).toBeNull();
  });

  it("widerrufen lässt den Eintrag stehen", async () => {
    // Der Unterschied zum Löschen: Der Eintrag bleibt und warnt künftig.
    const k = await bert();
    const vorher = (await b.kontakte()).length;
    await b.kontaktWiderrufen(k.fingerprint);

    expect(await b.kontakte()).toHaveLength(vorher);
    expect((await bert()).vertrauen).toBe("widerrufen");
  });

  it("löschen entfernt ihn — und damit die Warnung", async () => {
    const k = await bert();
    const vorher = (await b.kontakte()).length;
    await b.kontaktWiderrufen(k.fingerprint);
    await b.kontaktLoeschen(k.fingerprint);

    expect(await b.kontakte()).toHaveLength(vorher - 1);
    expect(await bert()).toBeUndefined();
  });
});

describe("die Naht schweigt nicht", () => {
  it("ein Aufruf auf einen unbekannten Kontakt wirft", async () => {
    // Stilles Nichtstun wäre das Schlimmste: Der Bildschirm meldete
    // Erfolg, und nichts wäre geschehen. In Phase 4 antwortet an dieser
    // Stelle der Kern — und der schweigt auch nicht.
    await expect(b.kontaktVerifizieren("GIBT ES NICHT", "qr")).rejects.toThrow(
      /gibt es nicht/,
    );
  });
});

describe("die Brücke gibt Kopien heraus", () => {
  it("wer die Antwort verändert, ändert den Speicher nicht", async () => {
    // Sonst könnte ein Bildschirm den Stand des Kerns umschreiben, ohne
    // ihn je gefragt zu haben — in Phase 4 wäre das schlicht unmöglich,
    // und die Attrappe soll sich nicht großzügiger verhalten.
    const liste = await b.kontakte();
    liste[0]!.name = "Umbenannt";

    expect((await b.kontakte())[0]!.name).not.toBe("Umbenannt");
  });
});
