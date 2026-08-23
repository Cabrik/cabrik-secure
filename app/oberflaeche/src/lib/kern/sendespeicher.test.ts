/**
 * Die Auswahl der Dateien.
 *
 * # Was hier zählt
 *
 * Nicht, dass Dateien ankommen — das ist eine Zeile. Sondern die drei
 * Fälle, in denen ein naiver Halter das Falsche tut:
 *
 * 1. **Ein Abbruch verwirft nichts.** Wer vierzig Dateien gewählt hat und
 *    den Dialog danach versehentlich öffnet und schließt, darf sie nicht
 *    verlieren.
 * 2. **Dieselbe Datei zweimal ist einmal.** Sonst steht sie doppelt im
 *    Stapel und wird doppelt verschickt.
 * 3. **Gleiche Namen sind verschiedene Dateien.** Der Pfad entscheidet,
 *    nicht der Name.
 */

import { beforeEach, describe, expect, it } from "vitest";
import { MockBruecke } from "./bruecke";
import { KONTAKTE, STAPEL } from "./mock";
import { sendespeicher } from "./speicher.svelte";
import type { Fortschritt, Fortschrittsmelder, Sendedatei } from "./typen";

/** Die Pfade des ersten Beispielstapels — die kennt die Attrappe. */
const PFADE = STAPEL[0]!.dateien.map((d) => d.pfad);

/** Eine Brücke, deren Dialog liefert, was der Test vorgibt. */
class MitDialog extends MockBruecke {
  constructor(private ergebnis: string[]) {
    super(KONTAKTE);
  }

  override async dateienWaehlen(): Promise<string[]> {
    return this.ergebnis;
  }
}

beforeEach(() => {
  sendespeicher.verbinde(new MockBruecke(KONTAKTE));
});

describe("Sendespeicher", () => {
  it("beginnt leer", () => {
    expect(sendespeicher.dateien).toHaveLength(0);
  });

  it("nimmt auf, was ausgewählt wurde — samt Befund", async () => {
    sendespeicher.verbinde(new MitDialog(PFADE));

    await sendespeicher.waehlen();

    expect(sendespeicher.dateien).toHaveLength(PFADE.length);
    for (const d of sendespeicher.dateien) {
      expect(d.befund, `${d.name} ohne Befund`).toBeDefined();
    }
  });

  it("ein Abbruch verwirft die bisherige Auswahl nicht", async () => {
    // Der Dialog gibt eine leere Liste zurück, wenn jemand ihn schließt.
    // Das als „nichts mehr ausgewählt“ zu lesen, wäre die teuerste
    // Fehldeutung des ganzen Bildschirms.
    sendespeicher.verbinde(new MitDialog(PFADE));
    await sendespeicher.waehlen();
    const vorher = sendespeicher.dateien.length;
    expect(vorher).toBeGreaterThan(0);

    sendespeicher.verbinde(new MitDialog([]));
    // `verbinde` leert -- also erst wieder füllen, dann abbrechen.
    await sendespeicher.hinzufuegen(PFADE);
    await sendespeicher.waehlen();

    expect(sendespeicher.dateien).toHaveLength(vorher);
  });

  it("nimmt dieselbe Datei nicht zweimal auf", async () => {
    await sendespeicher.hinzufuegen(PFADE);
    const vorher = sendespeicher.dateien.length;

    await sendespeicher.hinzufuegen(PFADE);

    expect(sendespeicher.dateien).toHaveLength(vorher);
  });

  it("hält gleichnamige Dateien aus verschiedenen Ordnern auseinander", async () => {
    // Der Grund, warum der Pfad die Kennung ist. Über den Namen wäre die
    // zweite hier stillschweigend verschwunden.
    await sendespeicher.hinzufuegen([
      "C:\\Arbeit\\Rechnung.pdf",
      "C:\\Privat\\Rechnung.pdf",
    ]);

    expect(sendespeicher.dateien).toHaveLength(2);
    expect(new Set(sendespeicher.dateien.map((d) => d.name)).size).toBe(1);
  });

  it("legt nach, ohne das Vorhandene neu zu prüfen", async () => {
    await sendespeicher.hinzufuegen([PFADE[0]!]);
    await sendespeicher.hinzufuegen(PFADE);

    expect(sendespeicher.dateien).toHaveLength(PFADE.length);
    const pfade = sendespeicher.dateien.map((d: Sendedatei) => d.pfad);
    expect(new Set(pfade).size).toBe(pfade.length);
  });

  it("leeren nimmt alles zurück", async () => {
    await sendespeicher.hinzufuegen(PFADE);

    sendespeicher.leeren();

    expect(sendespeicher.dateien).toHaveLength(0);
  });

  it("eine unlesbare Datei steht im Stapel statt zu fehlen", async () => {
    // Sie stillschweigend wegzulassen wäre das Schlimmste: Der Nutzer
    // zählt zehn Dateien ab und bekommt neun verschickt.
    await sendespeicher.hinzufuegen(["Z:\\gibt-es-nicht\\weg.pdf"]);

    expect(sendespeicher.dateien).toHaveLength(1);
    expect(sendespeicher.dateien[0]!.befund.fall).toBe("fehler");
  });

  it("meldet sich nicht ab, bevor es sich angemeldet hat", async () => {
    // Der Abbau kann kommen, bevor die Anmeldung zurück ist. Ohne das
    // bliebe ein Empfänger für ein Fenster stehen, das es nicht mehr gibt.
    const weg = sendespeicher.beobachten();
    expect(() => weg()).not.toThrow();
  });
});

// ---------------------------------------------------------------------------
// Fortschritt
// ---------------------------------------------------------------------------

/**
 * Hält jeden Stand fest, statt nur den letzten.
 *
 * Der Halter überschreibt `fortschritt` bei jeder Meldung — wer hinterher
 * nachsieht, findet immer nur den letzten. Diese Brücke reicht sie durch
 * **und** legt sie ab, damit der Test den ganzen Verlauf beurteilen kann.
 */
class MitMitschrift extends MockBruecke {
  readonly staende: Fortschritt[] = [];

  constructor() {
    super(KONTAKTE);
  }

  override async dateienPruefen(
    pfade: string[],
    melden: Fortschrittsmelder,
  ): Promise<Sendedatei[]> {
    return super.dateienPruefen(pfade, (f) => {
      this.staende.push({ ...f });
      melden(f);
    });
  }
}

describe("Fortschritt bei großen Stapeln", () => {
  it("meldet jede Datei — und dabei, was mit ihr geschieht", async () => {
    // Hier stand einmal `toHaveLength(PFADE.length)`: genau eine Meldung
    // je Datei. Seit es Schritte gibt, sind es mehrere -- und das ist der
    // Zweck, nicht ein Nebeneffekt. Geprüft wird deshalb, dass JEDE Datei
    // vorkommt, nicht dass es genau so viele Meldungen wie Dateien gibt.
    const b = new MitMitschrift();
    sendespeicher.verbinde(b);

    await sendespeicher.hinzufuegen(PFADE);

    expect(b.staende.length).toBeGreaterThanOrEqual(PFADE.length);
    const erledigte = new Set(b.staende.map((s) => s.erledigt));
    expect([...erledigte].sort()).toEqual(PFADE.map((_, i) => i));
  });

  it("zählt hoch und nennt dabei die laufende Datei", async () => {
    const b = new MitMitschrift();
    sendespeicher.verbinde(b);

    await sendespeicher.hinzufuegen(PFADE);

    // Nie rückwärts. Ein Balken, der zurückspringt, sieht aus wie ein
    // Fehler -- auch dann, wenn nur die Schritte durcheinandergeraten.
    let vorher = 0;
    for (const s of b.staende) {
      expect(s.erledigt).toBeGreaterThanOrEqual(vorher);
      vorher = s.erledigt;
      expect(s.gesamt).toBe(PFADE.length);
      expect(s.laeuft.length, "ohne Namen sagt der Balken zu wenig").toBeGreaterThan(0);
    }
  });

  it("sagt beim Prüfen, dass gelesen und untersucht wird", async () => {
    // Der eigentliche Gewinn: Der Name allein erklärt einen Stillstand
    // nicht. „Lese urlaub.mp4“ und „Entferne Metadaten aus urlaub.mp4“
    // sehen beide stillstehend aus -- aber nur das eine heisst, dass die
    // Platte langsam ist.
    const b = new MitMitschrift();
    sendespeicher.verbinde(b);

    await sendespeicher.hinzufuegen(PFADE);

    const schritte = new Set(b.staende.map((s) => s.schritt));
    expect(schritte.has("lesen")).toBe(true);
    expect(schritte.has("bereinigen")).toBe(true);
  });

  it("beginnt bei null erledigten, nicht bei einer", async () => {
    // `erledigt` zählt die FERTIGEN. Bei der ersten Datei ist noch nichts
    // fertig — sie schon mitzuzählen hieße, den Balken vorzudatieren.
    const b = new MitMitschrift();
    sendespeicher.verbinde(b);

    await sendespeicher.hinzufuegen(PFADE);

    expect(b.staende[0]!.erledigt).toBe(0);
  });

  it("der Halter trägt die Art mit, nicht nur die Zahlen", async () => {
    // Ohne sie sähen alle fünf Stapel gleich aus — und „Wird gelöscht“
    // stünde über einem Prüflauf.
    const gesehen: string[] = [];
    class Mitschreibend extends MockBruecke {
      override async dateienPruefen(
        pfade: string[],
        melden: Fortschrittsmelder,
      ): Promise<Sendedatei[]> {
        return super.dateienPruefen(pfade, (f) => {
          melden(f);
          gesehen.push(sendespeicher.fortschritt?.art ?? "keine");
        });
      }
    }
    sendespeicher.verbinde(new Mitschreibend(KONTAKTE));

    await sendespeicher.hinzufuegen(PFADE);

    expect(new Set(gesehen)).toEqual(new Set(["pruefen"]));
  });

  it("räumt den Stand am Ende weg", async () => {
    // Sonst stünde der Balken für immer bei „39 von 40“ — und behauptete
    // Arbeit, die längst getan ist.
    sendespeicher.verbinde(new MockBruecke(KONTAKTE));

    await sendespeicher.hinzufuegen(PFADE);

    expect(sendespeicher.fortschritt).toBeNull();
  });

  it("räumt ihn auch weg, wenn es schiefgeht", async () => {
    // Der eigentliche Fall. Nach einem Fehlschlag bleibt der Bildschirm
    // stehen — und mit ihm der Balken, wenn ihn niemand wegnimmt.
    class Scheiternd extends MockBruecke {
      override async dateienPruefen(
        pfade: string[],
        melden: Fortschrittsmelder,
      ): Promise<Sendedatei[]> {
        melden({
          erledigt: 0,
          gesamt: pfade.length,
          laeuft: "erste.jpg",
          schritt: "lesen",
          bytesErledigt: null,
          bytesGesamt: null,
        });
        throw new Error("Der Kern ist ausgestiegen.");
      }
    }
    sendespeicher.verbinde(new Scheiternd(KONTAKTE));

    await sendespeicher.hinzufuegen(PFADE);

    expect(sendespeicher.fehler).toContain("ausgestiegen");
    expect(sendespeicher.fortschritt).toBeNull();
  });
});
