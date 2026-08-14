/**
 * Die einzige Stelle, an der etwas stumm auseinanderlaufen kann.
 *
 * Überall sonst passt ein Übersetzer auf: Die Typen sind beidseitig
 * festgenagelt, die Prüfmuster vergleichen Rust gegen TypeScript, und die
 * Schnittstelle `Bruecke` erzwingt, dass beide Umsetzungen dasselbe können.
 *
 * Die **Befehlsnamen** sind davon ausgenommen. Sie stehen als Zeichenketten
 * in `tauri.ts` und als Funktionsnamen in `cabrik-fenster/src/main.rs`.
 * Benennt jemand dort eine Funktion um, merkt es niemand — bis zur Laufzeit
 * im Fenster, wo die Meldung „command not found“ lautet und nichts darüber
 * sagt, welche Seite recht hat.
 *
 * Deshalb liest dieser Test die Rust-Datei.
 *
 * # Warum das nicht immer stimmte
 *
 * Dieser Satz stand hier, während der Test in Wahrheit eine JSON-Datei las,
 * die von Hand gepflegt war. Damit prüfte er, ob zwei Abschriften desselben
 * Gedankens zueinander passen — und ließ genau den Fall durch, vor dem er
 * warnt: einen Befehl, der in der Liste steht und im Fenster fehlt.
 *
 * Ein Wächter, der seine eigene Beschreibung nicht einhält, ist schlimmer
 * als keiner: Man verlässt sich auf ihn.
 */

import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { TauriBruecke, imFenster } from "./tauri";

import befehle from "./vertrag/befehle.json";

/**
 * Die Namen, die im Fenster tatsächlich angemeldet sind.
 *
 * Aus `generate_handler!` in `main.rs` gelesen — das ist die Stelle, die
 * zur Laufzeit zählt. Eine Funktion mit `#[tauri::command]`, die dort nicht
 * steht, gibt es für die Oberfläche nicht.
 */
function angemeldeteBefehle(): string[] {
  // Vitest läuft in `app/oberflaeche`.
  const rs = readFileSync("../../crates/cabrik-fenster/src/main.rs", "utf8");
  const block = /generate_handler!\[([^\]]*)\]/.exec(rs);
  if (!block) throw new Error("generate_handler! in main.rs nicht gefunden");
  return [...block[1]!.matchAll(/([a-z_]+)\s*,/g)].map((m) => m[1]!);
}

/** Die Namen, die `tauri.ts` ruft — aus der Datei selbst gelesen. */
function gerufeneBefehle(): string[] {
  const ts = Object.values(
    import.meta.glob("/src/lib/kern/tauri.ts", {
      query: "?raw",
      import: "default",
      eager: true,
    }) as Record<string, string>,
  )[0]!;
  return [...ts.matchAll(/\)\("([a-z_]+)"/g)].map((m) => m[1]!);
}

describe("die Befehlsnamen stimmen auf beiden Seiten", () => {
  it("die Liste aus dem Fenster ist da und nicht leer", () => {
    // Sonst prüfte alles Weitere nichts und wäre trotzdem grün.
    expect(angemeldeteBefehle().length).toBeGreaterThan(3);
  });

  it("jeder Befehl, den die Brücke ruft, ist im Fenster angemeldet", () => {
    // Der Fall, der im Fenster „command not found“ ergibt — und nichts
    // darüber sagt, welche Seite recht hat.
    const gerufen = gerufeneBefehle();
    const angemeldet = angemeldeteBefehle();
    expect(gerufen.length).toBeGreaterThan(3);

    for (const name of gerufen) {
      expect(angemeldet, `„${name}“ ist im Fenster nicht angemeldet`).toContain(
        name,
      );
    }
  });

  it("und jeder angemeldete wird auch gerufen", () => {
    // Die andere Richtung: Ein Befehl, den niemand ruft, ist tote Fläche —
    // und meistens ein Hinweis darauf, dass die Brücke etwas vergessen hat.
    const gerufen = gerufeneBefehle();
    for (const name of angemeldeteBefehle()) {
      expect(gerufen, `„${name}“ ruft niemand`).toContain(name);
    }
  });

  it("die abgelegte Liste ist nicht veraltet", () => {
    // `befehle.json` ist eine Abschrift für andere Prüfungen. Läuft sie
    // dem Fenster davon, prüfen die gegen etwas, das es nicht gibt.
    expect([...befehle].sort()).toEqual([...angemeldeteBefehle()].sort());
  });
});

describe("die Brücke außerhalb des Fensters", () => {
  it("erkennt, dass sie nicht im Fenster läuft", () => {
    // Im Test gibt es kein Tauri. Die Anwendung muss das feststellen
    // können, statt beim ersten Aufruf zu scheitern.
    expect(imFenster()).toBe(false);
  });

  it("lässt sich trotzdem laden", () => {
    // Ein statischer Import von `@tauri-apps/api` an der Dateispitze risse
    // im Browser alles mit. Deshalb wird er erst beim Aufruf geholt.
    expect(() => new TauriBruecke()).not.toThrow();
  });
});
