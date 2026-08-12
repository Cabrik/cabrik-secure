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
import {
  markeFuerAbsender,
  markeFuerBereinigung,
  markeFuerKontakt,
  nachSchwere,
} from "./zustand";
import type { Absender, Fund, Kontakt, Verifikationsweg } from "../kern/typen";

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
    const m = markeFuerBereinigung({
      fall: "vollstaendig",
      entfernt: [],
      format: "JPEG",
    });

    expect(m.zustand).toBe("bestaetigt");
    expect(m.satz).toContain("JPEG");
    // "alle bekannten" -- nicht "alle".
    expect(m.satz).toContain("bekannten");
  });

  it("behauptet nirgends Sicherheit oder Metadatenfreiheit", () => {
    const faelle = [
      markeFuerBereinigung({
        fall: "vollstaendig",
        entfernt: [],
        format: "PNG",
      }),
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
        verifiziertUeber: "safetyNumber",
      }),
      markeFuerAbsender({
        fall: "gewechselt",
        fingerprint: "F1",
        name: "Anna",
        vorherVerifiziert: false,
      }),
      markeFuerAbsender({
        fall: "widerrufen",
        fingerprint: "F1",
        name: "Anna",
      }),
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
      markeFuerAbsender({
        fall: "widerrufen",
        fingerprint: "F1",
        name: "Anna",
      }),
    ];

    expect(alle.filter((m) => m.zustand === "fehler")).toHaveLength(1);
  });

  it("ein bekannter Kontakt wird nicht als geprüft ausgegeben", () => {
    const m = markeFuerAbsender({
      fall: "bekannt",
      fingerprint: "F1",
      name: "Anna",
    });

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
  const fund = (
    art: Fund["art"],
    schwere: Fund["schwere"],
    ort: string,
  ): Fund => ({
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

    expect(sortiert.map((f) => f.schwere)).toEqual([
      "kritisch",
      "beachtlich",
      "gering",
    ]);
  });

  it("bei gleicher Schwere entscheidet die Fundstelle -- die Reihenfolge ist stabil", () => {
    const sortiert = nachSchwere([
      fund("software", "beachtlich", "z"),
      fund("kommentar", "beachtlich", "a"),
    ]);

    expect(sortiert.map((f) => f.ort)).toEqual(["a", "z"]);
  });

  it("verändert die übergebene Liste nicht", () => {
    const eingabe = [
      fund("farbprofil", "gering", "b"),
      fund("ortsangabe", "kritisch", "a"),
    ];
    const vorher = [...eingabe];
    nachSchwere(eingabe);

    expect(eingabe).toEqual(vorher);
  });
});

// ---------------------------------------------------------------------------
// Die zweite Achse
// ---------------------------------------------------------------------------

describe("Cyan und Magenta sind keine Zustände", () => {
  it("es gibt genau vier Zustände", () => {
    // Ein Tippfehler-sicherer Beleg: Die Vereinigung aller Zustände, die
    // irgendeine Zuordnungsfunktion je zurückgeben kann.
    const alle = new Set([
      markeFuerBereinigung({
        fall: "vollstaendig",
        entfernt: [],
        format: "PNG",
      }).zustand,
      markeFuerBereinigung({
        fall: "teilweise",
        entfernt: [],
        geblieben: [],
        grund: "x",
        format: "PNG",
      }).zustand,
      markeFuerBereinigung({ fall: "unbekannt", formathinweis: null }).zustand,
      markeFuerBereinigung({ fall: "fehler", grund: "x" }).zustand,
      markeFuerAbsender({ fall: "unsigniert" }).zustand,
      markeFuerAbsender({ fall: "unbekannt", signierschluessel: "x" }).zustand,
      markeFuerAbsender({ fall: "bekannt", fingerprint: "x", name: "A" })
        .zustand,
      markeFuerAbsender({
        fall: "verifiziert",
        fingerprint: "x",
        name: "A",
        verifiziertAm: 1,
        verifiziertUeber: "safetyNumber",
      }).zustand,
      markeFuerAbsender({
        fall: "gewechselt",
        fingerprint: "x",
        name: "A",
        vorherVerifiziert: true,
      }).zustand,
      markeFuerAbsender({ fall: "widerrufen", fingerprint: "x", name: "A" })
        .zustand,
    ]);

    expect([...alle].sort()).toEqual([
      "bestaetigt",
      "fehler",
      "keineAussage",
      "warnung",
    ]);
  });
});

// ---------------------------------------------------------------------------
// Die Verifikationswege
// ---------------------------------------------------------------------------

/**
 * `spec/trust-store.md` §5: „Die letzte Zeile MUSS in der Oberfläche benannt
 * werden. Ein Fingerprint, der über denselben Kanal kommt wie die Nachricht,
 * beweist nichts."
 *
 * Das ist keine Empfehlung, sondern eine Auflage — und ohne Test genau die
 * Sorte Satz, die beim nächsten Umformulieren verlorengeht.
 */
describe("der Weg der Verifikation wird benannt", () => {
  const absender = (weg: Verifikationsweg | null): Absender => ({
    fall: "verifiziert",
    name: "Dr. Anna Beispiel",
    fingerprint: "8F3B 1C2A",
    verifiziertAm: 1_770_000_000,
    verifiziertUeber: weg,
  });

  it("nennt jeden Weg beim Namen statt eines Einheitssatzes", () => {
    expect(markeFuerAbsender(absender("qr")).satz).toContain("QR-Code");
    expect(markeFuerAbsender(absender("safetyNumber")).satz).toContain(
      "Safety Number",
    );
    expect(markeFuerAbsender(absender("fingerprint")).satz).toContain(
      "Fingerprint",
    );
  });

  it("benennt beim Fingerprint den Vorbehalt aus spec §5", () => {
    const satz = markeFuerAbsender(absender("fingerprint")).satz;
    expect(satz).toContain("derselbe Kanal, derselbe Angreifer");
  });

  it("behauptet keinen Weg, wenn keiner vermerkt ist", () => {
    const satz = markeFuerAbsender(absender(null)).satz;
    expect(satz).toContain("nicht vermerkt");
    for (const weg of ["QR-Code", "Safety Number", "Fingerprint"]) {
      expect(satz).not.toContain(weg);
    }
  });

  it("bleibt in allen Fällen grün — das Urteil bleibt beim Nutzer", () => {
    for (const weg of ["qr", "safetyNumber", "fingerprint", null] as const) {
      expect(markeFuerAbsender(absender(weg)).zustand).toBe("bestaetigt");
    }
  });

  it("gilt im Verzeichnis wortgleich wie bei der Nachricht", () => {
    // Einheitlich: derselbe Weg, derselbe Vorbehalt, egal wo er erscheint.
    const k: Kontakt = {
      name: "X",
      fingerprint: "AAAA",
      vertrauen: "verifiziert",
      seit: 0,
      verifiziertAm: 1_770_000_000,
      verifiziertUeber: "fingerprint",
      notiz: null,
      hatPostQuantum: true,
      safetyNumber: "00000",
    };
    expect(markeFuerKontakt(k).satz).toContain(
      "derselbe Kanal, derselbe Angreifer",
    );
  });
});
