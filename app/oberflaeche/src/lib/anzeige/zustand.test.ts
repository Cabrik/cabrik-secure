/**
 * Der Anzeigevertrag, ausführbar.
 *
 * `spec/anzeige.md` legt fest, welcher Kernzustand welchen Anzeigezustand
 * bekommt. Diese Tests halten es fest. Sie prüfen keine Gestaltung — sie
 * prüfen Aussagen.
 *
 * Die drei wichtigsten stehen ganz oben, weil sie die Fälle sind, in denen
 * eine bequeme Vereinfachung genau die Ehrlichkeit kostet, um die es geht.
 */

import { describe, expect, it } from "vitest";
import { markeFuerAbsender, markeFuerBereinigung, nachSchwere } from "./zustand";
import type { Fund } from "../kern/typen";

// ---------------------------------------------------------------------------
// Die drei Fälle, auf die es ankommt
// ---------------------------------------------------------------------------

describe("die Flagge am Instrument", () => {
  it("ein unverstandenes Format ist weder grün noch ein Fehler", () => {
    const m = markeFuerBereinigung({
      fall: "unbekannt",
      formathinweis: "Photoshop-Dokument (PSD)",
    });

    expect(m.zustand).toBe("keineAussage");
    expect(m.zustand).not.toBe("bestaetigt");
    expect(m.zustand).not.toBe("fehler");
  });

  it("und sagt nichts, was nach Erfolg klingt", () => {
    const m = markeFuerBereinigung({ fall: "unbekannt", formathinweis: null });
    const text = `${m.wort} ${m.satz}`.toLowerCase();

    for (const verboten of ["bereinigt", "entfernt", "sauber", "sicher"]) {
      expect(text).not.toContain(verboten);
    }
  });
});

describe("Grün heißt nicht sicher", () => {
  it("nennt bei vollständiger Bereinigung das Format", () => {
    const m = markeFuerBereinigung({ fall: "vollstaendig", entfernt: [], format: "JPEG" });

    expect(m.zustand).toBe("bestaetigt");
    expect(m.satz).toContain("JPEG");
    // "alle bekannten" -- nicht "alle".
    expect(m.satz).toContain("bekannten");
  });

  it("behauptet nirgends Sicherheit oder Metadatenfreiheit", () => {
    const faelle = [
      markeFuerBereinigung({ fall: "vollstaendig", entfernt: [], format: "PNG" }),
      markeFuerBereinigung({
        fall: "teilweise",
        entfernt: [],
        geblieben: [],
        grund: "Kapitelnamen sind Inhalt",
        format: "Matroska",
      }),
    ];

    for (const m of faelle) {
      const text = `${m.wort} ${m.satz}`.toLowerCase();
      expect(text).not.toContain("sicher");
      expect(text).not.toContain("garantiert");
      expect(text).not.toContain("metadatenfrei");
    }
  });
});

describe("anonymer Versand ist ein legitimer Modus", () => {
  it("eine unsignierte Nachricht wird nicht gewarnt", () => {
    const m = markeFuerAbsender({ fall: "unsigniert" });

    expect(m.zustand).toBe("keineAussage");
    expect(m.zustand).not.toBe("warnung");
    expect(m.zustand).not.toBe("fehler");
  });

  it("es sei denn, der Nutzer hat eine Signatur verlangt", () => {
    const m = markeFuerAbsender({ fall: "unsigniert" }, true);

    expect(m.zustand).toBe("fehler");
    expect(m.satz).toContain("verlangt");
  });
});

// ---------------------------------------------------------------------------
// Die übrigen Absenderfälle
// ---------------------------------------------------------------------------

describe("Absender", () => {
  it("nur ein verifizierter Kontakt bekommt Grün", () => {
    const alle = [
      markeFuerAbsender({ fall: "unsigniert" }),
      markeFuerAbsender({ fall: "unbekannt", signierschluessel: "AB12" }),
      markeFuerAbsender({ fall: "bekannt", fingerprint: "F1", name: "Anna" }),
      markeFuerAbsender({
        fall: "verifiziert",
        fingerprint: "F1",
        name: "Anna",
        verifiziertAm: 1_700_000_000,
      }),
      markeFuerAbsender({
        fall: "gewechselt",
        fingerprint: "F1",
        name: "Anna",
        vorherVerifiziert: false,
      }),
      markeFuerAbsender({ fall: "widerrufen", fingerprint: "F1", name: "Anna" }),
    ];

    const gruen = alle.filter((m) => m.zustand === "bestaetigt");
    expect(gruen).toHaveLength(1);
    expect(gruen[0]!.wort).toBe("Anna");
  });

  it("ein widerrufener Schlüssel ist der einzige Fehler", () => {
    const alle = [
      markeFuerAbsender({ fall: "unsigniert" }),
      markeFuerAbsender({ fall: "unbekannt", signierschluessel: "AB12" }),
      markeFuerAbsender({ fall: "bekannt", fingerprint: "F1", name: "Anna" }),
      markeFuerAbsender({
        fall: "gewechselt",
        fingerprint: "F1",
        name: "Anna",
        vorherVerifiziert: true,
      }),
      markeFuerAbsender({ fall: "widerrufen", fingerprint: "F1", name: "Anna" }),
    ];

    expect(alle.filter((m) => m.zustand === "fehler")).toHaveLength(1);
  });

  it("ein bekannter Kontakt wird nicht als geprüft ausgegeben", () => {
    const m = markeFuerAbsender({ fall: "bekannt", fingerprint: "F1", name: "Anna" });

    expect(m.zustand).toBe("warnung");
    expect(m.wort).toContain("nicht verifiziert");
    expect(m.satz).toContain("Kontaktspeicher");
  });

  it("ein Schlüsselwechsel wiegt schwerer, wenn vorher verifiziert war", () => {
    const ohne = markeFuerAbsender({
      fall: "gewechselt",
      fingerprint: "F1",
      name: "Anna",
      vorherVerifiziert: false,
    });
    const mit = markeFuerAbsender({
      fall: "gewechselt",
      fingerprint: "F1",
      name: "Anna",
      vorherVerifiziert: true,
    });

    expect(mit.satz.length).toBeGreaterThan(ohne.satz.length);
    expect(mit.satz).toContain("verifiziert");
  });
});

// ---------------------------------------------------------------------------
// Funde
// ---------------------------------------------------------------------------

describe("Funde", () => {
  const fund = (art: Fund["art"], schwere: Fund["schwere"], ort: string): Fund => ({
    art,
    ort,
    wert: null,
    schwere,
  });

  it("Schwerwiegendes steht oben", () => {
    const sortiert = nachSchwere([
      fund("farbprofil", "gering", "b"),
      fund("ortsangabe", "kritisch", "c"),
      fund("software", "beachtlich", "a"),
    ]);

    expect(sortiert.map((f) => f.schwere)).toEqual(["kritisch", "beachtlich", "gering"]);
  });

  it("bei gleicher Schwere entscheidet die Fundstelle -- die Reihenfolge ist stabil", () => {
    const sortiert = nachSchwere([
      fund("software", "beachtlich", "z"),
      fund("kommentar", "beachtlich", "a"),
    ]);

    expect(sortiert.map((f) => f.ort)).toEqual(["a", "z"]);
  });

  it("verändert die übergebene Liste nicht", () => {
    const eingabe = [fund("farbprofil", "gering", "b"), fund("ortsangabe", "kritisch", "a")];
    const vorher = [...eingabe];
    nachSchwere(eingabe);

    expect(eingabe).toEqual(vorher);
  });
});
