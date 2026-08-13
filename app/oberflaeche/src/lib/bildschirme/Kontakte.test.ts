/**
 * Vertrauen als Anzeige, Vertrauen als Handlung.
 *
 * Der heikelste Satz des ganzen Entwurfs steht in spec/anzeige.md §3a:
 * Derselbe Sachverhalt — ein Kontakt, dessen Safety Number nie verglichen
 * wurde — ist im Verzeichnis **grau** und beim Empfang einer Nachricht
 * **gelb**.
 *
 * Das ist kein Widerspruch, sondern der Unterschied zwischen „so fängt jeder
 * Kontakt an“ und „auf diesen Namen sollen Sie sich jetzt verlassen“. Es ist
 * aber genau die Art Unterscheidung, die beim Umbauen still verschwindet —
 * jemand vereinheitlicht die Farbe, und niemandem fällt auf, was verloren
 * ging. Deshalb steht sie hier als Test.
 */

import { beforeEach, describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Kontakte from "./Kontakte.svelte";
import { KONTAKTE } from "../kern/mock";
import { kontaktspeicher } from "../kern/speicher.svelte";
import { MockBruecke } from "../kern/bruecke";
import { abgewickelt } from "../kern/pruefstand.svelte";
import { markeFuerAbsender, markeFuerKontakt } from "../anzeige/zustand";
import type { Kontakt } from "../kern/typen";

const ANFANG = kontaktspeicher.liste.map((k) => ({ ...k }));

beforeEach(async () => {
  kontaktspeicher.verbinde(new MockBruecke(ANFANG));
  await kontaktspeicher.laden();
});

function darstellen() {
  const ziel = document.createElement("div");
  document.body.append(ziel);
  const b = mount(Kontakte, { target: ziel });
  const knopf = (teil: string) =>
    [...ziel.querySelectorAll("button")].find((k) =>
      k.textContent?.includes(teil),
    ) as HTMLButtonElement | undefined;
  return {
    ziel,
    knopf,
    waehlen: (name: string) => {
      knopf(name)!.click();
      flushSync();
    },
    /** Wie `waehlen`, wartet aber auf die Antwort der Bruecke. */
    handeln: async (name: string) => {
      knopf(name)!.click();
      await abgewickelt();
    },
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    aufraeumen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

// ---------------------------------------------------------------------------
// Die zwei Bewertungen desselben Sachverhalts
// ---------------------------------------------------------------------------

describe("derselbe Sachverhalt, zwei Bewertungen", () => {
  const bert = KONTAKTE.find((k) => k.vertrauen === "gesehen")!;

  it("im Verzeichnis ist „nicht verifiziert“ grau, nicht gelb", () => {
    // Als Eintrag im Verzeichnis ist es erwartbar: So fängt jeder Kontakt an.
    expect(markeFuerKontakt(bert).zustand).toBe("keineAussage");
  });

  it("beim Empfang einer Nachricht wird daraus eine Warnung", () => {
    // Hier soll man sich auf den Namen verlassen — jetzt ist es gelb.
    const marke = markeFuerAbsender({
      fall: "bekannt",
      name: bert.name,
      fingerprint: bert.fingerprint,
    });
    expect(marke.zustand).toBe("warnung");
    expect(marke.wort).toContain("nicht verifiziert");
  });

  it("verifiziert ist in beiden Zusammenhängen grün", () => {
    const anna = KONTAKTE.find((k) => k.vertrauen === "verifiziert")!;
    expect(markeFuerKontakt(anna).zustand).toBe("bestaetigt");
    expect(
      markeFuerAbsender({
        fall: "verifiziert",
        name: anna.name,
        fingerprint: anna.fingerprint,
        verifiziertAm: anna.verifiziertAm!,
        verifiziertUeber: "safetyNumber",
      }).zustand,
    ).toBe("bestaetigt");
  });
});

// ---------------------------------------------------------------------------
// markeFuerKontakt vollständig
// ---------------------------------------------------------------------------

describe("markeFuerKontakt", () => {
  const bau = (vertrauen: Kontakt["vertrauen"]): Kontakt => ({
    name: "X",
    fingerprint: "AAAA",
    vertrauen,
    seit: 0,
    verifiziertAm: null,
    verifiziertUeber: null,
    notiz: null,
    hatPostQuantum: true,
    safetyNumber: "00000",
  });

  it("bildet alle vier Vertrauenszustände ab", () => {
    expect(markeFuerKontakt(bau("verifiziert")).zustand).toBe("bestaetigt");
    expect(markeFuerKontakt(bau("gesehen")).zustand).toBe("keineAussage");
    expect(markeFuerKontakt(bau("gewechselt")).zustand).toBe("warnung");
    expect(markeFuerKontakt(bau("widerrufen")).zustand).toBe("fehler");
  });

  it("sagt nie, ein Kontakt sei sicher — nur, was geprüft wurde", () => {
    for (const v of [
      "verifiziert",
      "gesehen",
      "gewechselt",
      "widerrufen",
    ] as const) {
      const m = markeFuerKontakt(bau(v));
      for (const verboten of ["sicher", "Sicher", "garantiert", "unknackbar"]) {
        expect(`${m.wort} ${m.satz}`).not.toContain(verboten);
      }
    }
  });
});

// ---------------------------------------------------------------------------
// Die Safety Number
// ---------------------------------------------------------------------------

describe("die Safety Number", () => {
  it("steht in zwölf Gruppen zu fünf Ziffern da", () => {
    for (const k of KONTAKTE) {
      const gruppen = k.safetyNumber.trim().split(/\s+/);
      expect(gruppen).toHaveLength(12);
      for (const g of gruppen) expect(g).toMatch(/^\d{5}$/);
    }
  });

  it("ist bei jedem Kontakt eine andere", () => {
    const alle = new Set(KONTAKTE.map((k) => k.safetyNumber));
    expect(alle.size).toBe(KONTAKTE.length);
  });

  it("verlangt beim Vergleich einen Weg außerhalb des Programms", () => {
    const s = darstellen();
    s.waehlen("Bert Muster");
    s.waehlen("Jetzt vergleichen");

    const text = s.text();
    expect(text).toContain("Rufen Sie");
    expect(text).toContain(
      "den Sie nicht über dieses Programm hergestellt haben",
    );

    s.aufraeumen();
  });

  it("wird bei einem verifizierten Kontakt nicht erneut angeboten", () => {
    const s = darstellen();
    s.waehlen("Dr. Anna Beispiel");
    expect(s.knopf("Jetzt vergleichen")).toBeUndefined();
    s.aufraeumen();
  });

  it("ein begonnener Vergleich wirkt nicht auf den nächsten Kontakt", () => {
    const s = darstellen();
    s.waehlen("Bert Muster");
    s.waehlen("Jetzt vergleichen");
    expect(s.text()).toContain("Rufen Sie");

    // Umschalten: Der Vergleich gehört zu dem Kontakt, für den er begann.
    s.waehlen("Cora Steinbach");
    expect(s.text()).not.toContain("Rufen Sie");

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Was der Bildschirm nicht verschweigt
// ---------------------------------------------------------------------------

describe("die unbequemen Wahrheiten stehen da", () => {
  it("ein Schlüsselwechsel wird nicht als Kleinigkeit behandelt", () => {
    const s = darstellen();
    s.waehlen("Cora Steinbach");

    const text = s.text();
    expect(text).toContain("anderen Schlüssel");
    expect(text).toContain("vergleichen");

    s.aufraeumen();
  });

  it("ein Kontakt aus Version 1 nennt die fehlende Post-Quantum-Deckung", () => {
    const s = darstellen();
    s.waehlen("Archiv");

    const text = s.text();
    expect(text).toContain("Kein Post-Quantum-Schlüssel");
    expect(text).toContain("nicht geschützt");

    s.aufraeumen();
  });

  it("der Widerruf verspricht keine Wirkung, die er nicht hat", () => {
    const s = darstellen();
    s.waehlen("Bert Muster");

    // Der ehrliche Satz: Ein Widerruf ohne Verteilweg erreicht niemanden sonst.
    expect(s.text()).toContain("Wirkt nur bei Ihnen");

    s.aufraeumen();
  });

  it("ein bereits widerrufener Kontakt bietet den Widerruf nicht erneut an", () => {
    const s = darstellen();
    s.waehlen("Unbekannter Zuträger");
    expect(s.knopf("kompromittiert")).toBeUndefined();
    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Der Vergleich, der schiefgeht
// ---------------------------------------------------------------------------

/**
 * Der Fall, für den die Safety Number überhaupt gebaut ist — und der bis
 * eben keinen Bildschirm hatte. Zwei Knöpfe standen da und taten nichts.
 */
describe("wenn die Nummern nicht übereinstimmen", () => {
  it("wird nicht behauptet, jemand höre mit", async () => {
    const s = darstellen();
    s.waehlen("Bert Muster");
    s.waehlen("Jetzt vergleichen");
    await s.handeln("Sie stimmen nicht überein");

    const text = s.text();
    expect(text).toContain("stimmen nicht überein");
    // Der häufigste Grund zuerst, nicht der schlimmste.
    expect(text).toContain("Zahlendreher");
    expect(text).toContain("versuchen Sie es ruhig noch einmal");
    // Aber die ernste Möglichkeit steht auch da.
    expect(text).toContain("sitzt jemand zwischen Ihnen");

    s.aufraeumen();
  });

  it("und der Kontakt wird zurückgesetzt, nicht widerrufen", async () => {
    const s = darstellen();
    s.waehlen("Bert Muster");
    s.waehlen("Jetzt vergleichen");
    await s.handeln("Sie stimmen nicht überein");

    const bert = kontaktspeicher.liste.find((k) => k.name === "Bert Muster")!;
    expect(bert.vertrauen).toBe("gesehen");
    expect(bert.vertrauen).not.toBe("widerrufen");

    s.aufraeumen();
  });

  it("ein geglückter Vergleich macht daraus verifiziert", async () => {
    const s = darstellen();
    s.waehlen("Bert Muster");
    s.waehlen("Jetzt vergleichen");
    await s.handeln("Sie stimmen überein");

    const bert = kontaktspeicher.liste.find((k) => k.name === "Bert Muster")!;
    expect(bert.vertrauen).toBe("verifiziert");
    expect(s.text()).toContain("Safety Number verglichen");

    s.aufraeumen();
  });
});

describe("der Widerruf fragt nach", () => {
  it("ein Klick allein widerruft nicht", () => {
    const s = darstellen();
    s.waehlen("Bert Muster");
    s.waehlen("kompromittiert");

    expect(
      kontaktspeicher.liste.find((k) => k.name === "Bert Muster")!.vertrauen,
    ).not.toBe("widerrufen");
    // Stattdessen steht da, was der Widerruf bewirkt.
    expect(s.text()).toContain("wird künftig rot angezeigt");

    s.aufraeumen();
  });

  it("erst die Rückfrage tut es — und sagt vorher, was folgt", async () => {
    const s = darstellen();
    s.waehlen("Bert Muster");
    s.waehlen("kompromittiert");
    await s.handeln("Ja, widerrufen");

    expect(
      kontaktspeicher.liste.find((k) => k.name === "Bert Muster")!.vertrauen,
    ).toBe("widerrufen");

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Löschen ist nicht Widerrufen
// ---------------------------------------------------------------------------

/**
 * Die gefährlichste Verwechslung dieses Bildschirms.
 *
 * Widerrufen heißt „dieser Schlüssel ist kompromittiert“ — der Eintrag
 * bleibt und warnt künftig. Löschen heißt „ich kenne diese Person nicht“ —
 * der Eintrag verschwindet, **und mit ihm die Warnung**. Wer einen
 * verdächtigen Schlüssel löscht, sieht ihn beim nächsten Mal als
 * unbekannten Absender wieder und nimmt ihn arglos neu auf.
 */
describe("Löschen ist nicht Widerrufen", () => {
  it("wer misstraut, wird auf den Widerruf verwiesen", () => {
    const s = darstellen();
    s.waehlen("Bert Muster");
    s.waehlen("Kontakt löschen");

    const text = s.text();
    expect(text).toContain("misstrauen");
    expect(text).toContain(
      "entfernt den Eintrag und damit jede spätere Warnung",
    );

    s.aufraeumen();
  });

  it("bei einem widerrufenen Schlüssel wird es zur Warnung", () => {
    const s = darstellen();
    s.waehlen("Unbekannter Zuträger");
    s.waehlen("Kontakt löschen");

    const text = s.text();
    expect(text).toContain("Sie löschen gerade Ihre eigene Warnung");
    expect(text).toContain("Zum Vergessen löschen — zum Schützen behalten");

    s.aufraeumen();
  });

  it("ein Klick allein löscht nicht", () => {
    const s = darstellen();
    s.waehlen("Bert Muster");
    s.waehlen("Kontakt löschen");

    expect(kontaktspeicher.liste.some((k) => k.name === "Bert Muster")).toBe(
      true,
    );

    s.aufraeumen();
  });

  it("die Rückfrage tut es — die Gegenprobe", async () => {
    const s = darstellen();
    const vorher = kontaktspeicher.liste.length;
    s.waehlen("Bert Muster");
    s.waehlen("Kontakt löschen");
    await s.handeln("Ja, aus dem Verzeichnis entfernen");

    expect(kontaktspeicher.liste).toHaveLength(vorher - 1);
    expect(kontaktspeicher.liste.some((k) => k.name === "Bert Muster")).toBe(
      false,
    );

    s.aufraeumen();
  });

  it("beim Löschen eines verifizierten Kontakts steht der Verlust dabei", () => {
    const s = darstellen();
    s.waehlen("Dr. Anna Beispiel");
    s.waehlen("Kontakt löschen");

    expect(s.text()).toContain("Die Verifikation vom");
    expect(s.text()).toContain("noch einmal vergleichen");

    s.aufraeumen();
  });

  it("sagt, dass das Gegenüber nichts davon merkt", () => {
    const s = darstellen();
    s.waehlen("Bert Muster");
    s.waehlen("Kontakt löschen");

    expect(s.text()).toContain("beim Gegenüber ändert sich nichts");

    s.aufraeumen();
  });

  it("ein leeres Verzeichnis stürzt nicht ab, sondern erklärt sich", () => {
    kontaktspeicher.liste = [];
    const s = darstellen();

    expect(s.text()).toContain("Noch keine Kontakte");
    expect(s.text()).toContain("Empfangen können Sie trotzdem");

    s.aufraeumen();
  });
});

describe("die Auswahl nach einer Änderung", () => {
  /**
   * Beim Umbau auf die asynchrone Brücke sah es so aus, als läge hier ein
   * Fehler: Die Zeile nach dem Löschen liest den Speicher, und ohne
   * `await` läse sie den Stand von vorher — `gewaehlt` zeigte auf einen
   * Kontakt, den es nicht mehr gibt.
   *
   * **Die Gegenprobe hat das widerlegt.** Der Rückfall im `$derived`
   * (`?? KONTAKTE[0]`) fängt genau das ab, und die Anzeige ist mit und ohne
   * `await` dieselbe. Das `await` steht trotzdem dort, weil der Code sonst
   * etwas anderes sagt, als er meint — aber es hat keinen Fehler behoben,
   * und das gehört dazugesagt.
   *
   * Die beiden Tests bleiben: Sie halten fest, dass der Bildschirm das
   * Löschen des angezeigten und des letzten Kontakts übersteht — und das
   * hatte vorher niemand geprüft.
   */
  it("überlebt das Löschen des gerade angezeigten ersten Kontakts", async () => {
    const s = darstellen();
    const erster = kontaktspeicher.liste[0]!.name;

    s.waehlen(erster);
    s.waehlen("Kontakt löschen");
    await s.handeln("Ja, aus dem Verzeichnis entfernen");

    // Kein leeres Verzeichnis, kein Absturz — der nächste ist gewählt.
    expect(kontaktspeicher.liste.some((k) => k.name === erster)).toBe(false);
    expect(s.text()).not.toContain("Noch keine Kontakte");
    expect(s.text()).toContain(kontaktspeicher.liste[0]!.name);
  });

  it("und das Löschen des letzten verbliebenen", async () => {
    const s = darstellen();
    // Bis auf einen alle entfernen.
    while (kontaktspeicher.liste.length > 1) {
      await kontaktspeicher.loeschen(kontaktspeicher.liste[0]!.fingerprint);
    }
    await abgewickelt();

    s.waehlen(kontaktspeicher.liste[0]!.name);
    s.waehlen("Kontakt löschen");
    await s.handeln("Ja, aus dem Verzeichnis entfernen");

    expect(kontaktspeicher.liste).toHaveLength(0);
    expect(s.text()).toContain("Noch keine Kontakte");
  });
});
