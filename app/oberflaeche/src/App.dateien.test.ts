/**
 * Der Weg von einer Datei auf der Platte bis auf den Bildschirm.
 *
 * # Warum das an der ganzen Anwendung geprüft wird
 *
 * Weil der gemeldete Fehler zwischen zwei richtigen Teilen lag.
 * `sendespeicher.test.ts` prüft, dass hineingezogene Dateien ankommen —
 * und sie kamen an. `Senden.test.ts` prüft den Bildschirm — und der war
 * richtig. Falsch war die Hülle: Sie öffnet auf **Empfangen**, und dort
 * gibt es weder einen Auswahlknopf noch irgendein Zeichen, dass eine Datei
 * angenommen wurde.
 *
 * Von außen sah beides aus wie „das Fenster nimmt nichts an“.
 */
// @vitest-environment happy-dom

import { beforeEach, expect, it } from "vitest";
import { mount, unmount } from "svelte";
import App from "./App.svelte";
import { MockBruecke } from "./lib/kern/bruecke";
import { KONTAKTE, STAPEL } from "./lib/kern/mock";
import {
  identitaetsspeicher,
  kontaktspeicher,
  sendespeicher,
  sitzungsspeicher,
} from "./lib/kern/speicher.svelte";
import { abgewickelt } from "./lib/kern/pruefstand.svelte";
import type { Ziehereignis } from "./lib/kern/typen";

/** Die Pfade des ersten Beispielstapels — die kennt die Attrappe. */
const PFADE = STAPEL[0]!.dateien.map((d) => d.pfad);

/** Zwei verschiedene Dateien. Der erste Stapel hat nur eine. */
const ERSTER = STAPEL[0]!.dateien[0]!.pfad;
const ZWEITER = STAPEL[1]!.dateien[0]!.pfad;

/** Eine Brücke, deren Ziehen-und-Fallenlassen der Test auslöst. */
class MitZiehen extends MockBruecke {
  private melde: ((e: Ziehereignis) => void) | null = null;

  override async aufDateienGezogen(
    melde: (e: Ziehereignis) => void,
  ): Promise<() => void> {
    this.melde = melde;
    return () => {
      this.melde = null;
    };
  }

  override async dateienWaehlen(): Promise<string[]> {
    return PFADE;
  }

  /** Stellt nach, was das Fenster meldet. */
  ereignis(e: Ziehereignis) {
    this.melde?.(e);
  }
}

let bruecke: MitZiehen;

beforeEach(async () => {
  bruecke = new MitZiehen(KONTAKTE);
  for (const s of [
    sitzungsspeicher,
    kontaktspeicher,
    identitaetsspeicher,
    sendespeicher,
  ]) {
    s.verbinde(bruecke);
  }
  await sitzungsspeicher.laden();
});

function anhaengen() {
  document.body.innerHTML = '<div id="app"></div>';
  const ziel = document.getElementById("app")!;
  const a = mount(App, { target: ziel });
  return {
    ziel,
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    knopf: (teil: string) =>
      [...ziel.querySelectorAll("button")].find((k) =>
        k.textContent?.includes(teil),
      ),
    abbauen: () => unmount(a),
  };
}

it("führt fallengelassene Dateien auf den Sendebildschirm", async () => {
  // Der gemeldete Fehler. Die Anwendung öffnet auf „Empfangen“; ohne
  // diesen Wechsel verschwinden die Dateien in einen Halter, den gerade
  // niemand ansieht.
  const s = anhaengen();
  await abgewickelt();
  expect(s.text(), "die Anwendung öffnet auf Empfangen").not.toContain(
    "Weitere hinzufügen",
  );

  bruecke.ereignis({ art: "fallen", pfade: PFADE });
  await abgewickelt();
  await abgewickelt();

  expect(sendespeicher.dateien.length).toBeGreaterThan(0);
  expect(s.text(), "der Sendebildschirm muss jetzt vorn sein").toContain(
    "Weitere hinzufügen",
  );
  s.abbauen();
});

it("zeigt schon beim Ziehen, dass es annimmt", async () => {
  // Ein Fenster, das erst beim Loslassen reagiert, sieht bis dahin aus
  // wie eines, das nichts annimmt — und dann lässt niemand los.
  const s = anhaengen();
  await abgewickelt();

  bruecke.ereignis({ art: "drueber" });
  await abgewickelt();

  expect(s.text()).toContain("Loslassen");
  s.abbauen();
});

it("nimmt den Hinweis zurück, wenn das Ziehen das Fenster verlässt", async () => {
  const s = anhaengen();
  await abgewickelt();
  bruecke.ereignis({ art: "drueber" });
  await abgewickelt();

  bruecke.ereignis({ art: "weg" });
  await abgewickelt();

  expect(s.text()).not.toContain("Loslassen");
  s.abbauen();
});

it("bietet auf dem Sendebildschirm einen Weg, Dateien auszuwählen", async () => {
  // Der zweite Teil des Berichts: kein Knopf zum Aussuchen. Wer nicht
  // ziehen will oder kann, braucht ihn.
  const s = anhaengen();
  await abgewickelt();

  s.knopf("Senden")!.click();
  await abgewickelt();

  expect(s.text()).toContain("Noch nichts ausgewählt");
  expect(s.knopf("Dateien auswählen")).toBeDefined();
  s.abbauen();
});

it("und dieser Weg führt tatsächlich zu Dateien", async () => {
  // Die Gegenprobe: Ein Knopf, der dasteht und nichts tut, bestünde die
  // Prüfung oben.
  const s = anhaengen();
  await abgewickelt();
  s.knopf("Senden")!.click();
  await abgewickelt();

  s.knopf("Dateien auswählen")!.click();
  await abgewickelt();
  await abgewickelt();

  expect(sendespeicher.dateien).toHaveLength(PFADE.length);
  expect(s.text()).not.toContain("Noch nichts ausgewählt");
  s.abbauen();
});

it("ein Fehlschlag beim Anmelden wird gezeigt, nicht verschluckt", async () => {
  // Sonst sieht ein Ziehen-und-Fallenlassen, das sich gar nicht anmelden
  // ließ, genauso aus wie eines, bei dem der Nutzer danebengezielt hat.
  class Kaputt extends MockBruecke {
    override async aufDateienGezogen(): Promise<() => void> {
      throw new Error("Die Webansicht meldet kein Ziehen.");
    }
  }
  sendespeicher.verbinde(new Kaputt(KONTAKTE));

  const s = anhaengen();
  await abgewickelt();
  await abgewickelt();

  expect(s.text()).toContain("Die Webansicht meldet kein Ziehen.");
  s.abbauen();
});

it("der Metadatenbericht ist auch aus der ganzen Anwendung heraus erreichbar", async () => {
  // Auf Bauteilebene geht es. Hier läuft zusätzlich der Sekundentakt der
  // Sitzung, und `stapel` ist ein abgeleitetes Objekt, das dabei neu
  // entsteht — genau die Art Unterschied, die einen Bildschirm für sich
  // richtig und in der Anwendung falsch macht.
  const s = anhaengen();
  await abgewickelt();
  s.knopf("Senden")!.click();
  await abgewickelt();
  s.knopf("Dateien auswählen")!.click();
  await abgewickelt();
  await abgewickelt();

  const bericht = s.knopf("Funde entfernt") ?? s.knopf("Bericht ansehen");
  expect(bericht, "es muss einen Weg zum Befund geben").toBeDefined();

  bericht!.click();
  await abgewickelt();

  expect(s.text()).toContain("Gefunden (");
  s.abbauen();
});

it("eine abgewählte Datei verschwindet auch in der ganzen Anwendung nicht", async () => {
  // Der zweite Bericht: „die erste ist verschwunden, es steht aber weiter
  // da, dass zwei ausgewählt sind“. Hier läuft der Sekundentakt der
  // Sitzung mit, der `stapel` bei jedem Schlag neu erzeugt.
  const s = anhaengen();
  await abgewickelt();
  s.knopf("Senden")!.click();
  await abgewickelt();
  s.knopf("Dateien auswählen")!.click();
  await abgewickelt();
  await abgewickelt();

  const namen = sendespeicher.dateien.map((d) => d.name);
  expect(namen.length).toBeGreaterThan(0);

  const kaestchen = [
    ...s.ziel.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'),
  ].filter((k) => k.getAttribute("aria-label")?.includes("mitsenden"));
  expect(kaestchen.length, "es muss Häkchen geben").toBeGreaterThan(0);

  kaestchen[0]!.click();
  await abgewickelt();
  // Und jetzt ein Schlag des Taktes -- er erzeugt `stapel` neu.
  await sitzungsspeicher.laden();
  await abgewickelt();

  const text = s.text();
  for (const name of namen) {
    expect(text, `${name} ist vom Bildschirm verschwunden`).toContain(name);
  }
  s.abbauen();
});

it("ein ZWEITES Bild erscheint sofort, ohne Bildschirmwechsel", async () => {
  // Der gemeldete Fehler: „wenn ich ein zweites Bild hinzufüge, muss ich
  // die Seite einmal wechseln, erst dann wird es angezeigt. Vorher bleibt
  // es auf 'wird geprüft' hängen.“
  //
  // Der erste Zugang klappte -- deshalb griff kein bisheriger Test.
  class ZweiRunden extends MockBruecke {
    private runde = 0;
    override async dateienWaehlen(): Promise<string[]> {
      this.runde += 1;
      return this.runde === 1 ? [ERSTER] : [ZWEITER];
    }
  }
  sendespeicher.verbinde(new ZweiRunden(KONTAKTE));

  const s = anhaengen();
  await abgewickelt();
  s.knopf("Senden")!.click();
  await abgewickelt();

  s.knopf("Dateien auswählen")!.click();
  await abgewickelt();
  await abgewickelt();
  expect(sendespeicher.dateien).toHaveLength(1);

  const nachlegen = s.knopf("Weitere hinzufügen");
  expect(nachlegen, "der Knopf zum Nachlegen muss da sein").toBeDefined();
  expect(nachlegen!.disabled, "und er darf nicht gesperrt sein").toBe(false);
  nachlegen!.click();
  await abgewickelt();
  await abgewickelt();
  await abgewickelt();

  expect(sendespeicher.dateien, "der Halter hat beide").toHaveLength(2);
  expect(s.text(), "und der Bildschirm zeigt beide").toContain(
    sendespeicher.dateien[1]!.name,
  );
  expect(s.text(), "und haengt nicht auf „wird geprueft“").not.toContain(
    "Wird geprüft",
  );
  s.abbauen();
});
