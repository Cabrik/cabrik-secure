/**
 * Der vollständige Befund — und die Wahl der Fassung.
 *
 * Die tragende Regel dieses Bildschirms ist eine, die man beim Bauen leicht
 * anders trifft: **Es wird alles gezeigt, nicht nur das Verbleibende.**
 *
 * Zeigt man nur den Rest, sieht eine sauber bereinigte Datei aus wie eine,
 * in der nie etwas stand. Der Nutzer erfährt nie, dass sein Name, die
 * Seriennummer seiner Kamera und der Aufnahmeort drin waren — und lernt
 * dadurch nie, dass er das mit sich herumträgt.
 */

import { describe, expect, it, vi } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Befund from "./Befund.svelte";
import { STAPEL } from "../kern/mock";
import { WAHL_VOREINSTELLUNG } from "../kern/typen";
import type { Bereinigungswahl, Sendedatei } from "../kern/typen";

function datei(stapel: string, name: string): Sendedatei {
  return STAPEL.find((s) => s.kennung === stapel)!.dateien.find(
    (d) => d.name === name,
  )!;
}

function darstellen(
  d: Sendedatei,
  original = false,
  wahl: Bereinigungswahl = WAHL_VOREINSTELLUNG,
) {
  const ziel = document.createElement("div");
  document.body.append(ziel);
  const waehle = vi.fn();
  const schliessen = vi.fn();
  const setzeWahl = vi.fn();
  const b = mount(Befund, {
    target: ziel,
    props: { datei: d, original, waehle, wahl, setzeWahl, schliessen },
  });
  return {
    ziel,
    waehle,
    setzeWahl,
    schliessen,
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    fassung: () =>
      (
        ziel.querySelector('[data-pruefstelle="fassung"]')?.textContent ?? ""
      ).replace(/\s+/g, " "),
    /** Nur die Fundliste — die Fassungsliste hat eigene Einträge. */
    eintraege: () => [
      ...(ziel.querySelectorAll('[data-pruefstelle="funde"] > li') ?? []),
    ],
    fassungen: () =>
      (
        ziel.querySelector('[data-pruefstelle="fassungen"]')?.textContent ?? ""
      ).replace(/\s+/g, " "),
    wahl: () =>
      (
        ziel.querySelector('[data-pruefstelle="wahl"]')?.textContent ?? ""
      ).replace(/\s+/g, " "),
    funkKnopf: (nr: number) =>
      [...ziel.querySelectorAll<HTMLInputElement>('input[type="radio"]')][nr],
    aufraeumen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

// ---------------------------------------------------------------------------
// Alles zeigen, nicht nur den Rest
// ---------------------------------------------------------------------------

describe("der Befund zeigt jeden Fund, auch den entfernten", () => {
  it("bei einer vollständig bereinigten Datei stehen die Funde trotzdem da", () => {
    // Genau der Fall, der sonst wie „da war nie etwas“ aussähe.
    const s = darstellen(datei("eine-saubere", "Protokoll.pdf"));

    expect(s.text()).toContain("Gefunden (2)");
    expect(s.text()).toContain("Dr. Anna Beispiel");
    expect(s.text()).toContain("PDF:DocInfo/Author");

    s.aufraeumen();
  });

  it("je Fund steht dabei, ob er die Datei verlässt", () => {
    const s = darstellen(datei("eine-saubere", "Protokoll.pdf"));

    for (const e of s.eintraege()) {
      expect(e.textContent).toContain("wird entfernt");
    }

    s.aufraeumen();
  });

  it("bei teilweiser Bereinigung stehen beide Sorten nebeneinander", () => {
    const s = darstellen(datei("eine-mit-rest", "Mitschnitt.mp3"));
    const text = s.text();

    // Das Entfernte …
    expect(text).toContain("Dr. Anna Beispiel");
    expect(text).toContain("wird entfernt");
    // … und das Bleibende, mit Grund.
    expect(text).toContain("bleibt");
    expect(text).toContain("Neuberechnen des Tons");

    s.aufraeumen();
  });

  it("die Reihenfolge richtet sich nach der Schwere", () => {
    const s = darstellen(datei("eine-saubere", "Protokoll.pdf"));
    const erster = s.eintraege()[0]!.textContent ?? "";

    expect(erster).toContain("kritisch");

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Was das Programm nicht weiß
// ---------------------------------------------------------------------------

describe("ohne verstandenes Format gibt es keinen Befund", () => {
  it("und das wird gesagt, statt Leere als Sauberkeit auszugeben", () => {
    const s = darstellen(datei("grosser-stapel", "Uebersicht.psd"));
    const text = s.text();

    expect(text).toContain("Kein Befund möglich");
    expect(text).toContain("auch nicht, dass nichts drin ist");

    s.aufraeumen();
  });

  it("und es gibt dann auch keine Wahl zwischen zwei Fassungen", () => {
    const s = darstellen(datei("grosser-stapel", "Uebersicht.psd"));

    expect(s.funkKnopf(0)).toBeUndefined();
    expect(s.fassung()).toContain("Es gibt nur eine");

    s.aufraeumen();
  });

  it("eine unlesbare Datei meldet den Grund", () => {
    const s = darstellen(datei("grosser-stapel", "Notiz.txt.gpg"));

    expect(s.text()).toContain("Nicht lesbar");
    expect(s.text()).toContain("ließ sich nicht lesen");

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Die Fassungswahl
// ---------------------------------------------------------------------------

describe("welche Fassung hinausgeht", () => {
  it("beide Fassungen nennen, was sie bedeuten", () => {
    const s = darstellen(datei("eine-mit-rest", "Mitschnitt.mp3"));
    const text = s.fassung();

    expect(text).toContain("1 Angabe wird entfernt");
    expect(text).toContain("1 bleibt in der Datei");
    expect(text).toContain("gehen mit hinaus");

    s.aufraeumen();
  });

  it("voreingestellt ist die bereinigte Fassung", () => {
    const s = darstellen(datei("eine-saubere", "Protokoll.pdf"));

    expect(s.funkKnopf(0)?.checked).toBe(true);
    expect(s.funkKnopf(1)?.checked).toBe(false);

    s.aufraeumen();
  });

  it("die Wahl des Originals wird gemeldet", () => {
    const s = darstellen(datei("eine-saubere", "Protokoll.pdf"));

    s.funkKnopf(1)!.click();
    flushSync();

    expect(s.waehle).toHaveBeenCalledWith(true);

    s.aufraeumen();
  });

  it("und ist magenta, nicht rot — eine Einstellung, kein Fehler", () => {
    const s = darstellen(datei("eine-saubere", "Protokoll.pdf"), true);

    expect(s.text()).toContain("Sie senden das Original");
    expect(s.text()).toContain("2 Angaben mehr als nötig");
    // Kein Vorwurf.
    expect(s.text()).not.toContain("Fehler");
    expect(s.text()).not.toContain("unsicher");

    s.aufraeumen();
  });

  it("nennt einen Grund, warum jemand das Original wollen könnte", () => {
    // Ohne diesen Satz wirkt die Wahl wie eine Falle. Sie ist aber
    // manchmal genau das Richtige — und wer sie nicht bekommt, umgeht das
    // Programm.
    const s = darstellen(datei("eine-saubere", "Protokoll.pdf"));

    expect(s.fassung()).toContain("wenn die Angaben der Zweck sind");
    expect(s.fassung()).toContain("Urheberangabe");

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Frühere PDF-Fassungen
// ---------------------------------------------------------------------------

/**
 * Die klassische Schwärzungspanne: ein Dokument, aus dem jemand Namen
 * entfernt hat — und die vorige Fassung steckt vollständig weiter darin.
 * Ein Leser zeigt sie nicht an. Ein Werkzeug schon.
 *
 * `cabrik-metadata` erkennt das seit Phase 2 (`metadata revisions`). Die
 * Oberfläche hat es bis eben verschwiegen.
 */
describe("frühere Fassungen sind kein Metadatum, sondern Inhalt", () => {
  const pdf = () => datei("eine-saubere", "Protokoll.pdf");

  it("werden gesondert gezeigt, nicht in der Fundliste", () => {
    const s = darstellen(pdf());

    expect(s.fassungen()).toContain("Frühere Fassungen (3)");
    // Und gerade NICHT unter den Funden.
    expect(s.eintraege()).toHaveLength(2);
  });

  it("nennen den entfernten Text wörtlich", () => {
    // Das ist die eigentliche Auskunft: nicht „wie sah die Fassung aus“,
    // sondern „was wurde herausgenommen und fährt trotzdem mit“.
    const s = darstellen(pdf());
    const text = s.fassungen();

    expect(text).toContain("Martin Kessler");
    expect(text).toContain("0170 4432190");
    expect(text).toContain("Nur hier — später entfernt");

    s.aufraeumen();
  });

  it("sagen, dass ein Leser sie nicht anzeigt", () => {
    const s = darstellen(pdf());
    const text = s.fassungen();

    expect(text).toContain("enthält alle 3 Fassungen");
    expect(text).toContain("angezeigt wird nur die letzte");
    expect(text).toContain("fahren trotzdem mit");

    s.aufraeumen();
  });

  it("markieren, welche angezeigt wird", () => {
    const s = darstellen(pdf());
    expect(s.fassungen()).toContain("Fassung 3 wird angezeigt");
    s.aufraeumen();
  });

  it("erscheinen nicht bei einer Datei mit nur einer Fassung", () => {
    // Eine einzelne Fassung ist der Normalfall und keine Nachricht.
    const s = darstellen(datei("eine-mit-rest", "Mitschnitt.mp3"));
    expect(s.fassungen()).toBe("");
    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Die vier Entscheidungen des Kerns
// ---------------------------------------------------------------------------

describe("welche Fassung eingeflacht wird", () => {
  const pdf = () => datei("eine-saubere", "Protokoll.pdf");

  it("voreingestellt ist die angezeigte, und die Historie verschwindet", () => {
    const s = darstellen(pdf());
    const text = s.wahl();

    expect(text).toContain("Die angezeigte Fassung");
    expect(text).toContain("die Historie verschwindet");

    s.aufraeumen();
  });

  it("jede frühere Fassung lässt sich einzeln wählen", () => {
    const s = darstellen(pdf());
    const text = s.wahl();

    expect(text).toContain("Fassung 1");
    expect(text).toContain("Fassung 2");
    // Die angezeigte steht oben als „Die angezeigte Fassung“, nicht doppelt.
    expect(text).not.toContain("Fassung 3");

    s.aufraeumen();
  });

  it("eine gewählte frühere Fassung wird als Sollwert gemeldet", () => {
    const s = darstellen(pdf(), false, {
      ...WAHL_VOREINSTELLUNG,
      fassung: 1,
    });

    expect(s.wahl()).toContain("Fassung 1 wird zur einzigen");

    s.aufraeumen();
  });

  it("„Historie behalten“ nennt sofort die Folge", () => {
    // Der Punkt, den der Nutzer selbst benannt hat: Manchmal braucht man
    // alle Fassungen, manchmal wäre es fatal. Beides muss dastehen.
    const s = darstellen(pdf(), false, {
      ...WAHL_VOREINSTELLUNG,
      historieBehalten: true,
    });
    const text = s.wahl();

    expect(text).toContain("bleiben wiederherstellbar");
    expect(text).toContain("3 Zeilen");
    expect(text).toContain("gehen mit hinaus");

    s.aufraeumen();
  });

  it("nennt den Zweck, für den man die Historie behält", () => {
    const s = darstellen(pdf());
    expect(s.wahl()).toContain("Beweismittel");
    s.aufraeumen();
  });
});

describe("die Office-Schalter", () => {
  const docx = () => datei("mit-verlauf", "Vertragsentwurf.docx");

  it("Anmerkungen entfernen lässt den Text unangetastet", () => {
    const s = darstellen(docx());
    const text = s.wahl();

    expect(text).toContain("Anmerkungen entfernen");
    expect(text).toContain("Zeichen für Zeichen erhalten");

    s.aufraeumen();
  });

  it("nachverfolgte Änderungen anzunehmen verändert den Inhalt", () => {
    const s = darstellen(docx(), false, {
      ...WAHL_VOREINSTELLUNG,
      aenderungenAnnehmen: true,
    });
    const text = s.wahl();

    expect(text).toContain("Das verändert den Inhalt");
    expect(text).toContain("ein anderes Dokument, als Sie hier geöffnet haben");

    s.aufraeumen();
  });

  it("keiner der beiden ist voreingestellt", () => {
    // Ein Schalter, der den Inhalt verändert, darf nie voreingestellt sein.
    const s = darstellen(docx());
    const kaesten = [
      ...s.ziel.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'),
    ];

    expect(kaesten.length).toBeGreaterThan(0);
    for (const k of kaesten) expect(k.checked).toBe(false);

    s.aufraeumen();
  });

  it("werden nur angeboten, wo es etwas zu schalten gibt", () => {
    // Ein Häkchen ohne Wirkung wäre eine Behauptung über die Datei.
    const s = darstellen(datei("eine-mit-rest", "Mitschnitt.mp3"));

    expect(s.wahl()).not.toContain("Anmerkungen entfernen");
    expect(s.wahl()).not.toContain("Nachverfolgte");

    s.aufraeumen();
  });

  it("eine Änderung wird nach oben gemeldet", () => {
    const s = darstellen(docx());
    const kasten = [
      ...s.ziel.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'),
    ].find((k) =>
      k.closest("label")?.textContent?.includes("Anmerkungen entfernen"),
    )!;

    kasten.click();
    flushSync();

    expect(s.setzeWahl).toHaveBeenCalledWith(
      expect.objectContaining({ kommentareEntfernen: true }),
    );

    s.aufraeumen();
  });
});

describe("beim Original entfallen die Zusatzentscheidungen", () => {
  it("denn dort wird nichts bereinigt", () => {
    // Sie anzubieten wäre widersprüchlich: „nichts entfernen“ und
    // gleichzeitig „so entfernen“.
    const s = darstellen(datei("eine-saubere", "Protokoll.pdf"), true);
    expect(s.wahl()).toBe("");
    s.aufraeumen();
  });

  it("die Fassungsliste bleibt aber stehen", () => {
    // Was in der Datei steckt, ändert sich durch die Wahl nicht — und beim
    // Original geht es sogar vollständig mit hinaus.
    const s = darstellen(datei("eine-saubere", "Protokoll.pdf"), true);
    expect(s.fassungen()).toContain("Martin Kessler");
    s.aufraeumen();
  });
});
