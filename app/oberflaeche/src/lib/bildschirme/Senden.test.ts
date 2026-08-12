/**
 * „Stör nur, wenn du wirklich etwas zu sagen hast" — ausführbar.
 *
 * Die Regel klingt einfach und ist beim Bauen leicht zu brechen: Ein
 * Bestätigungshäkchen, das immer da ist, kostet nichts und wirkt gründlich.
 * Es erzieht aber zum Wegklicken, und dann wirkt es auch dort nicht mehr,
 * wo es zählt.
 *
 * Diese Tests halten beide Hälften fest: dass **nicht** gestört wird, wenn
 * es nichts zu sagen gibt, und dass sich der Vorgang **nicht fortsetzen
 * lässt**, wenn es etwas gibt.
 */

import { describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Senden from "./Senden.svelte";
import { STAPEL } from "../kern/mock";
import { brauchtEntscheidung, fasseStapel } from "../anzeige/zustand";
import type { Sendedatei } from "../kern/typen";

function darstellen(kennung: string) {
  const stapel = STAPEL.find((s) => s.kennung === kennung)!;
  const ziel = document.createElement("div");
  document.body.append(ziel);
  const b = mount(Senden, { target: ziel, props: { stapel } });
  const auffaelligeKarten = [
    ...(ziel.querySelector('[data-pruefstelle="auffaellig"]')?.children ?? []),
  ];
  return {
    text: (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    /** Nur der Bereich, in dem einzeln aufgeführt wird. */
    auffaellig: (
      ziel.querySelector('[data-pruefstelle="auffaellig"]')?.textContent ?? ""
    ).replace(/\s+/g, " "),
    auffaellige: auffaelligeKarten,
    klappe: ziel.querySelector("details"),
    knopf: [...ziel.querySelectorAll("button")].find((k) =>
      k.textContent?.includes("Verschlüsseln"),
    ) as HTMLButtonElement | undefined,
    kaestchen: [
      ...ziel.querySelectorAll('input[type="checkbox"]'),
    ] as HTMLInputElement[],
    aufraeumen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

// ---------------------------------------------------------------------------
// Die Regel selbst
// ---------------------------------------------------------------------------

describe("stör nur, wenn du wirklich etwas zu sagen hast", () => {
  it("ohne Befund gibt es nichts zu bestätigen", () => {
    const s = darstellen("eine-saubere");

    expect(s.text).toContain("Es gibt nichts zu entscheiden");
    expect(s.text).not.toContain("Ich habe gesehen");
    expect(s.knopf?.disabled).toBe(false);

    s.aufraeumen();
  });

  it("mit Befund lässt sich nicht verschlüsseln, ohne ihn zu bestätigen", () => {
    const s = darstellen("eine-mit-rest");

    expect(s.text).toContain("Ich habe gesehen");
    expect(s.knopf?.disabled).toBe(true);
    expect(s.text).toContain("Bestätigen Sie oben");

    s.aufraeumen();
  });

  it("der Grund für das Verbleibende steht da, nicht nur die Zahl", () => {
    const s = darstellen("eine-mit-rest");

    expect(s.text).toContain("Neuberechnen des Tons");
    expect(s.text).toContain("Bleibt in der Datei");

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Der Stapel
// ---------------------------------------------------------------------------

describe("bei vielen Dateien wird zusammengefasst statt übersprungen", () => {
  it("das Unauffällige schrumpft auf eine Zeile, das Auffällige bleibt einzeln", () => {
    const s = darstellen("grosser-stapel");

    // 38 vollständig bereinigte: eine Zeile. Darunter fällt auch
    // Interview.wav, aus der ein Name und eine Gerätekennung entfernt
    // wurden — was weg ist, ist keine Entscheidung mehr.
    expect(s.text).toContain("38");
    expect(s.text).toContain("vollständig bereinigt");

    // Die drei, bei denen etwas offenbleibt: einzeln und mit Namen.
    expect(
      s.auffaellige.map((k) => k.querySelector("p")?.textContent?.trim()),
    ).toEqual(["Uebersicht.psd", "DSC_0042.NEF", "Notiz.txt.gpg"]);

    // Und die 38 gerade nicht einzeln.
    expect(s.auffaellig).not.toContain("Scan_001.jpg");
    expect(s.auffaellig).not.toContain("Interview.wav");

    s.aufraeumen();
  });

  it("das Zusammengefasste ist zugeklappt, aber auffindbar", () => {
    const s = darstellen("grosser-stapel");

    // Zugeklappt: es stört niemanden.
    expect(s.klappe?.open).toBe(false);
    // Auffindbar: jede der 38 steht drin, mit der Zahl ihrer Funde.
    expect(s.klappe?.textContent).toContain("Interview.wav");
    expect(s.klappe?.textContent).toContain("Scan_001.jpg");
    expect(s.klappe?.textContent).toContain("2 Funde entfernt");

    s.aufraeumen();
  });

  it("es gibt keinen Weg, die Vorschau zu überspringen", () => {
    const s = darstellen("grosser-stapel");

    for (const wort of [
      "überspringen",
      "Überspringen",
      "später",
      "ignorieren",
    ]) {
      expect(s.text).not.toContain(wort);
    }
    expect(s.knopf?.disabled).toBe(true);

    s.aufraeumen();
  });

  it("auch bei 41 Dateien hängt alles an einer einzigen Bestätigung", () => {
    const s = darstellen("grosser-stapel");

    // Ein Kästchen für den Befund, eines fürs Signieren, je eines je Kontakt.
    const befundKaestchen = s.kaestchen.filter((k) =>
      k.closest("label")?.textContent?.includes("Ich habe gesehen"),
    );
    expect(befundKaestchen).toHaveLength(1);

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Die Zusammenfassung als Funktion
// ---------------------------------------------------------------------------

describe("fasseStapel", () => {
  const datei = (name: string, befund: Sendedatei["befund"]): Sendedatei => ({
    name,
    groesseBytes: 1000,
    befund,
  });

  const sauber = (n: string) =>
    datei(n, { fall: "vollstaendig", format: "JPEG", entfernt: [] });

  it("zählt das Unauffällige und listet das Auffällige", () => {
    const b = fasseStapel([
      sauber("a.jpg"),
      sauber("b.jpg"),
      datei("c.psd", { fall: "unbekannt", formathinweis: "PSD" }),
    ]);

    expect(b.gesamt).toBe(3);
    expect(b.vollstaendig).toBe(2);
    expect(b.auffaellig.map((d) => d.name)).toEqual(["c.psd"]);
  });

  it("ein reiner Stapel braucht keine Entscheidung", () => {
    expect(brauchtEntscheidung(fasseStapel([sauber("a.jpg")]))).toBe(false);
  });

  it("eine einzige unverstandene Datei unter vierzig genügt", () => {
    const viele = [
      ...Array.from({ length: 40 }, (_, i) => sauber(`s${i}.jpg`)),
      datei("x.psd", { fall: "unbekannt", formathinweis: "PSD" }),
    ];

    expect(brauchtEntscheidung(fasseStapel(viele))).toBe(true);
  });

  it("ein Lesefehler zählt ebenfalls als auffällig", () => {
    const b = fasseStapel([
      sauber("a.jpg"),
      datei("b.bin", { fall: "fehler", grund: "kaputt" }),
    ]);
    expect(b.auffaellig).toHaveLength(1);
  });
});

// ---------------------------------------------------------------------------
// Der Empfänger ohne Post-Quantum-Schlüssel
// ---------------------------------------------------------------------------

describe("ein Empfänger aus Version 1 zieht die ganze Nachricht herunter", () => {
  it("die Suite wird genannt und der Grund dazu", () => {
    const stapel = STAPEL[0]!;
    const ziel = document.createElement("div");
    document.body.append(ziel);
    const b = mount(Senden, { target: ziel, props: { stapel } });

    // Voreingestellt ist der erste Kontakt, der Post-Quantum kann.
    expect(ziel.textContent).toContain("Post-Quantum-Hybrid");

    // Den v1-Kontakt hinzuwählen.
    const kaestchen = [...ziel.querySelectorAll('input[type="checkbox"]')].find(
      (k) => k.closest("label")?.textContent?.includes("Archiv"),
    ) as HTMLInputElement;
    // Svelte bündelt DOM-Änderungen. Ohne dieses Ausspülen läse der
    // Test den Stand von vor dem Klick.
    kaestchen.click();
    flushSync();

    const text = (ziel.textContent ?? "").replace(/\s+/g, " ");
    expect(text).toContain("Ohne Post-Quantum-Schutz");
    expect(text).toContain("klassisch");

    unmount(b);
    ziel.remove();
  });
});
