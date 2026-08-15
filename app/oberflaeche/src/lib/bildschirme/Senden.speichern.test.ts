/**
 * Die bereinigte Fassung speichern, ohne zu verschlüsseln.
 *
 * # Die drei Zusicherungen
 *
 * 1. **Nur Dateien, für die es eine bereinigte Fassung gibt.** Bei einem
 *    nicht verstandenen Format weiß das Programm nicht, was es entfernen
 *    sollte. Eine Kopie „bereinigt“ zu nennen wäre die gefährlichste
 *    Falschaussage, die dieses Programm machen könnte.
 * 2. **Wer das Original verschicken will, will es nicht bereinigt
 *    gespeichert haben.** Das war ja gerade die Entscheidung.
 * 3. **Der Knopf verlangt keine Empfänger.** Es geht nichts hinaus — eine
 *    Bedingung, die nichts mit dem Vorgang zu tun hat, wäre Schikane.
 */

import { describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Senden from "./Senden.svelte";
import type { Sendedatei } from "../kern/typen";

function datei(
  name: string,
  fall: Sendedatei["befund"]["fall"],
): Sendedatei {
  const befund: Sendedatei["befund"] =
    fall === "vollstaendig"
      ? { fall, format: "JPEG", entfernt: [] }
      : fall === "teilweise"
        ? {
            fall,
            format: "PDF",
            entfernt: [],
            geblieben: [],
            grund: "Frühere Fassungen bleiben.",
          }
        : fall === "unbekannt"
          ? { fall, formathinweis: null }
          : { fall, grund: "nicht lesbar" };
  return {
    pfad: `C:\\Fotos\\${name}`,
    name,
    groesseBytes: 1000,
    fassungen: [],
    befund,
  };
}

function zeigen(dateien: Sendedatei[]) {
  const gerufen: string[][] = [];
  const ziel = document.createElement("div");
  document.body.append(ziel);
  const b = mount(Senden, {
    target: ziel,
    props: {
      dateien,
      kennung: "auswahl",
      bereinigtSpeichern: (pfade: string[]) => gerufen.push(pfade),
    },
  });
  return {
    ziel,
    gerufen,
    knopf: () =>
      ziel.querySelector<HTMLButtonElement>(
        '[data-pruefstelle="bereinigt-speichern"]',
      ),
    kaestchen: () => [
      ...ziel.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'),
    ],
    klick: (el: HTMLElement | undefined) => {
      el!.click();
      flushSync();
    },
    abbauen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

describe("bereinigt speichern", () => {
  it("steht neben dem Verschlüsseln bereit", () => {
    const s = zeigen([datei("Foto.jpg", "vollstaendig")]);

    expect(s.knopf()).toBeDefined();
    expect(s.knopf()!.disabled).toBe(false);
    s.abbauen();
  });

  it("verlangt keine Empfänger — es geht ja nichts hinaus", () => {
    // Der Verschlüsseln-Knopf ist ohne Empfänger gesperrt. Dieser nicht:
    // Eine Bedingung, die nichts mit dem Vorgang zu tun hat, wäre Schikane.
    const s = zeigen([datei("Foto.jpg", "vollstaendig")]);

    s.klick(s.knopf()!);

    expect(s.gerufen).toHaveLength(1);
    expect(s.gerufen[0]).toEqual(["C:\\Fotos\\Foto.jpg"]);
    s.abbauen();
  });

  it("nimmt nur Dateien mit, für die es eine bereinigte Fassung gibt", () => {
    // Die wichtigste Zusicherung. Bei einem nicht verstandenen Format
    // wüsste das Programm nicht, was es entfernen soll — eine Kopie
    // „bereinigt“ zu nennen wäre eine Falschaussage.
    const s = zeigen([
      datei("Gut.jpg", "vollstaendig"),
      datei("Teils.pdf", "teilweise"),
      datei("Fremd.xyz", "unbekannt"),
      datei("Kaputt.jpg", "fehler"),
    ]);

    s.klick(s.knopf()!);

    expect(s.gerufen[0]).toEqual(["C:\\Fotos\\Gut.jpg", "C:\\Fotos\\Teils.pdf"]);
    s.abbauen();
  });

  it("ist gesperrt, wenn keine einzige zu bereinigen ist", () => {
    // Ein Knopf, der nichts tun kann, darf nicht klickbar sein — sonst
    // passiert nichts, und das ist von einem Fehler nicht zu unterscheiden.
    const s = zeigen([datei("Fremd.xyz", "unbekannt")]);

    expect(s.knopf()!.disabled).toBe(true);
    s.abbauen();
  });

  it("nennt die Zahl, wenn nicht alle mitkommen", () => {
    // Sonst klickt jemand bei vier Dateien und bekommt zwei, ohne dass
    // vorher etwas darauf hingedeutet hätte.
    const s = zeigen([
      datei("Gut.jpg", "vollstaendig"),
      datei("Fremd.xyz", "unbekannt"),
    ]);

    expect(s.knopf()!.textContent).toContain("(1)");
    s.abbauen();
  });

  it("lässt ausgenommene Dateien aus", () => {
    // Wer eine Datei nicht mitsenden will, will sie auch nicht nebenbei
    // bereinigt auf der Platte liegen haben.
    const s = zeigen([
      datei("Eins.jpg", "vollstaendig"),
      datei("Zwei.jpg", "vollstaendig"),
    ]);
    s.klick(s.kaestchen()[0]!);

    s.klick(s.knopf()!);

    expect(s.gerufen[0]).toHaveLength(1);
    s.abbauen();
  });

  it("erscheint gar nicht bei den Beispielstapeln", () => {
    // Sie liegen nicht auf der Platte. Ein Knopf, der dort nichts täte,
    // wäre schlimmer als keiner.
    const ziel = document.createElement("div");
    document.body.append(ziel);
    const b = mount(Senden, {
      target: ziel,
      props: { dateien: [datei("Foto.jpg", "vollstaendig")], kennung: "beispiel" },
    });

    expect(
      ziel.querySelector('[data-pruefstelle="bereinigt-speichern"]'),
    ).toBeNull();
    unmount(b);
    ziel.remove();
  });
});

// ---------------------------------------------------------------------------
// Verschlüsseln
// ---------------------------------------------------------------------------

describe("verschlüsseln", () => {
  function mitVersand(dateien: Sendedatei[]) {
    const gerufen: {
      pfade: string[];
      empfaenger: string[];
      signieren: boolean;
      original: string[];
    }[] = [];
    const ziel = document.createElement("div");
    document.body.append(ziel);
    const b = mount(Senden, {
      target: ziel,
      props: {
        dateien,
        kennung: "auswahl",
        verschluesselnEcht: (
          pfade: string[],
          empfaenger: string[],
          signieren: boolean,
          original: string[],
        ) => gerufen.push({ pfade, empfaenger, signieren, original }),
      },
    });
    return {
      ziel,
      gerufen,
      knopf: () =>
        ziel.querySelector<HTMLButtonElement>('[data-pruefstelle="senden"]'),
      klickText: (teil: string) => {
        // Empfänger stehen als `<label>` mit Kästchen da, nicht als Knöpfe.
        const feld = [...ziel.querySelectorAll("label")]
          .find((l) => l.textContent?.includes(teil))
          ?.querySelector("input");
        feld?.click();
        flushSync();
      },
      abbauen: () => {
        unmount(b);
        ziel.remove();
      },
    };
  }

  it("verlangt einen Empfänger, bevor es losgeht", () => {
    // Ohne Empfänger ließe sich der Envelope von niemandem öffnen — auch
    // vom Absender nicht.
    const s = mitVersand([datei("Foto.jpg", "vollstaendig")]);

    expect(s.knopf()!.disabled).toBe(true);
    s.abbauen();
  });

  it("reicht Pfade und Empfänger weiter, sobald einer gewählt ist", () => {
    const s = mitVersand([datei("Foto.jpg", "vollstaendig")]);
    s.klickText("Dr. Anna Beispiel");

    s.knopf()!.click();
    flushSync();

    expect(s.gerufen).toHaveLength(1);
    expect(s.gerufen[0]!.pfade).toEqual(["C:\\Fotos\\Foto.jpg"]);
    expect(s.gerufen[0]!.empfaenger).toHaveLength(1);
    s.abbauen();
  });

  it("schickt ausgenommene Dateien nicht mit", () => {
    // Die Zusicherung, an der der ganze Bildschirm hängt: Was abgewählt
    // ist, geht nicht hinaus.
    const s = mitVersand([
      datei("Eins.jpg", "vollstaendig"),
      datei("Zwei.jpg", "vollstaendig"),
    ]);
    s.klickText("Dr. Anna Beispiel");
    const kaestchen = [
      ...s.ziel.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'),
    ].filter((k) => k.getAttribute("aria-label")?.includes("mitsenden"));
    kaestchen[0]!.click();
    flushSync();

    s.knopf()!.click();
    flushSync();

    expect(s.gerufen[0]!.pfade).toHaveLength(1);
    s.abbauen();
  });
});
