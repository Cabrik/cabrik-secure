/**
 * Vertrauen als Anzeige, Vertrauen als Handlung.
 *
 * Der heikelste Satz des ganzen Entwurfs steht in spec/anzeige.md §3a:
 * Derselbe Sachverhalt — ein Kontakt, dessen Safety Number nie verglichen
 * wurde — ist im Verzeichnis **grau** und beim Empfang einer Nachricht
 * **gelb**.
 *
 * Das ist kein Widerspruch, sondern der Unterschied zwischen „so fängt jeder
 * Kontakt an" und „auf diesen Namen sollen Sie sich jetzt verlassen". Es ist
 * aber genau die Art Unterscheidung, die beim Umbauen still verschwindet —
 * jemand vereinheitlicht die Farbe, und niemandem fällt auf, was verloren
 * ging. Deshalb steht sie hier als Test.
 */

import { describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Kontakte from "./Kontakte.svelte";
import { KONTAKTE } from "../kern/mock";
import { markeFuerAbsender, markeFuerKontakt } from "../anzeige/zustand";
import type { Kontakt } from "../kern/typen";

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
