/**
 * Das Handbuch — geprüft wie ein Bildschirm.
 *
 * Ein Handbuch, das mehr verspricht als die Anzeige, macht deren
 * Ehrlichkeit wertlos: Wer „Ihre Dateien sind sicher“ liest, liest das
 * gelbe „Überschreiben ist hier nicht verlässlich“ danach nicht mehr. Die
 * Wörter aus `spec/anzeige.md` §5 gelten deshalb hier genauso.
 *
 * Und ein Verweis ins Leere sieht aus wie ein toter Knopf.
 */
import { describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import { KAPITEL, type Baustein, type Ziel } from "./handbuch";
import { BEISPIELGRUPPEN } from "./beispiele";
import { FAELLE, STAPEL } from "../kern/mock";
import Hilfe from "../bildschirme/Hilfe.svelte";

/** Jeder Text eines Bausteins, auch die Punkte einer Liste. */
function texte(b: Baustein): string[] {
  return b.art === "liste" || b.art === "schritte" ? b.punkte : [b.text];
}

const ALLE = KAPITEL.flatMap((k) => [k.titel, ...k.inhalt.flatMap(texte)]);

describe("die Wörter aus spec/anzeige.md §5", () => {
  it("nichts davon kommt vor", () => {
    const verboten = [
      /garantiert/i,
      /vollständig gelöscht/i,
      /anonym/i,
      /\bsauber\b/i,
      /metadatenfrei/i,
      /100\s*%/,
    ];
    const treffer = ALLE.flatMap((t) =>
      verboten.filter((m) => m.test(t)).map((m) => `${m} in: ${t.slice(0, 70)}`),
    );
    expect(treffer).toEqual([]);
  });

  it("„sicher“ nur als Name des Werkzeugs, nie als Zusage", () => {
    // „Sicher löschen“ heißt der Knopf, „sicher löschen“ der Haken beim
    // Senden. Jedes andere „sicher“ wäre eine Behauptung.
    const treffer = ALLE.map((t) => t.replace(/sicher löschen/gi, "")).filter((t) =>
      /\bsicher\b/i.test(t),
    );
    expect(treffer).toEqual([]);
  });

  it("die Auszeichnung ist geschlossen", () => {
    // Ein offenes `**` setzte den Rest des Absatzes fett -- oder, je nach
    // Stelle, den falschen Teil.
    const offen = ALLE.filter((t) => t.split("**").length % 2 === 0);
    expect(offen).toEqual([]);
  });
});

describe("was das Bedrohungsmodell verlangt", () => {
  it("das Kapitel über die Grenzen nennt Gerät, Datenverkehr und Zwang", () => {
    // `spec/threat-model.md` §9: Die Dokumentation muss ausdrücklich
    // benennen, dass CabrikSecure nicht gegen A6, A7 und A8 schützt.
    const grenzen = KAPITEL.find((k) => k.kennung === "grenzen")!;
    const text = grenzen.inhalt.flatMap(texte).join(" ");
    expect(text).toContain("fremde Software");
    expect(text).toContain("Datenverkehr");
    expect(text).toContain("rechtlichem Zwang");
  });

  it("das Kapitel über Reisen verspricht keinen Schutz vor Zwang", () => {
    // Es beschreibt einen Weg, den Schlüssel nicht mitzunehmen — und muss
    // dabei sagen, was das nicht leistet. Sonst liest es sich als die
    // Abstreitbarkeit, die A8 ausdrücklich nicht bietet.
    const reisen = KAPITEL.find((k) => k.kennung === "reisen")!;
    const text = reisen.inhalt.flatMap(texte).join(" ");
    expect(text).toContain("Davor schützt CabrikSecure nicht");
    expect(text).toContain("Recht des Landes");
    expect(text).toContain("Falsche Angaben");
  });

  it("die Grenzen führen zum Kapitel über Reisen", () => {
    const grenzen = KAPITEL.find((k) => k.kennung === "grenzen")!;
    expect(grenzen.inhalt).toContainEqual(
      expect.objectContaining({ art: "verweis", ziel: { art: "kapitel", kapitel: "reisen" } }),
    );
  });
});

describe("Verweise", () => {
  const verweise = KAPITEL.flatMap((k) =>
    k.inhalt.flatMap((b) => (b.art === "verweis" ? [b.ziel] : [])),
  );

  it("es gibt welche", () => {
    expect(verweise.length).toBeGreaterThan(3);
  });

  it("jedes Kapitel und jedes Beispiel, auf das verwiesen wird, gibt es", () => {
    const fehlt = verweise.filter((z) => {
      if (z.art === "kapitel") return !KAPITEL.some((k) => k.kennung === z.kapitel);
      if (z.art !== "beispiel") return false;
      const b = z.beispiel;
      if (b.art === "empfangen") return !FAELLE.some((f) => f.kennung === b.kennung);
      if (b.art === "senden") return !STAPEL.some((s) => s.kennung === b.kennung);
      return false;
    });
    expect(fehlt).toEqual([]);
  });

  it("die Kapitel haben verschiedene Kennungen", () => {
    const kennungen = KAPITEL.map((k) => k.kennung);
    expect(new Set(kennungen).size).toBe(kennungen.length);
  });
});

describe("der Bildschirm", () => {
  function zeigen(kennung: string) {
    const ziel = document.createElement("div");
    document.body.append(ziel);
    const gerufen: Ziel[] = [];
    const b = mount(Hilfe, { target: ziel, props: { kennung, oeffnen: (z: Ziel) => gerufen.push(z) } });
    return {
      ziel,
      gerufen,
      text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
      knopf: (teil: string) =>
        [...ziel.querySelectorAll("button")].find((k) => k.textContent?.includes(teil)),
      abbauen: () => {
        unmount(b);
        ziel.remove();
      },
    };
  }

  it.each(KAPITEL.map((k) => [k.kennung, k.titel]))("%s zeigt sich", (kennung, titel) => {
    const s = zeigen(kennung);
    expect(s.ziel.querySelector("h2")?.textContent).toBe(titel);
    // Nicht nur die Überschrift: Ein Kapitel ohne Text wäre eine Lücke,
    // die wie ein Kapitel aussieht.
    expect(s.text().length).toBeGreaterThan(titel.length + 80);
    s.abbauen();
  });

  it("setzt Fettes als Text, nicht als HTML", () => {
    // Was zwischen `**` steht, wird ein <strong> mit Textinhalt. Ein `<`
    // darin bliebe ein Zeichen.
    const s = zeigen("identitaet");
    const fett = [...s.ziel.querySelectorAll("strong")].map((e) => e.textContent);
    expect(fett).toContain("öffentlichen");
    expect(s.text()).not.toContain("**");
    s.abbauen();
  });

  it("jedes Beispiel steht im Kapitel „Beispiele“ und ruft sich selbst auf", () => {
    const s = zeigen("beispiele");
    for (const g of BEISPIELGRUPPEN) {
      for (const e of g.eintraege) {
        s.knopf(e.titel)!.click();
        flushSync();
        expect(s.gerufen.at(-1)).toEqual({ art: "beispiel", beispiel: e.wahl });
      }
    }
    expect(s.gerufen.length).toBe(FAELLE.length + STAPEL.length + 2);
    s.abbauen();
  });

  it("ein Verweis ruft sein Ziel auf", () => {
    const s = zeigen("start");
    s.knopf("Zur Einrichtung")!.click();
    flushSync();
    expect(s.gerufen).toEqual([{ art: "bereich", bereich: "onboarding" }]);
    s.abbauen();
  });
});
