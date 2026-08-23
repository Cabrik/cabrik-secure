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
  // `[a-z0-9_]`, nicht `[a-z_]`. Der erste Befehl mit einer Ziffer im
  // Namen -- `v1_uebernehmen` -- wurde sonst als `_uebernehmen` gelesen,
  // und der Vergleich unten schlug mit einer Meldung an, die den
  // eigentlichen Fehler verschwieg: Nicht die Listen liefen auseinander,
  // sondern dieser Ausdruck konnte den Namen nicht lesen.
  return [...block[1]!.matchAll(/([a-z0-9_]+)\s*,/g)].map((m) => m[1]!);
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
  return [...ts.matchAll(/\)\("([a-z0-9_]+)"/g)].map((m) => m[1]!);
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

// ---------------------------------------------------------------------------
// Stapelbefehle
// ---------------------------------------------------------------------------

/**
 * Die Befehle, die eine Liste von Pfaden abarbeiten — samt ihrer Anmeldung
 * und ihrer Argumentliste.
 *
 * Aus `main.rs` gelesen, weil dort steht, was gilt. Ein Stapelbefehl ist
 * daran zu erkennen, dass er `pfade: Vec<String>` entgegennimmt.
 */
function stapelbefehle(): { name: string; attribut: string; args: string }[] {
  const rs = readFileSync("../../crates/cabrik-fenster/src/main.rs", "utf8");
  const muster =
    /#\[tauri::command(\(async\))?\]\s*fn\s+([a-z_]+)\s*\(([^)]*)\)/g;
  return [...rs.matchAll(muster)]
    .map((m) => ({ attribut: m[1] ?? "", name: m[2]!, args: m[3]! }))
    .filter((b) => /pfade\s*:\s*Vec<String>/.test(b.args));
}

describe("kein Stapel ohne Fortschritt", () => {
  /*
   * Zwei Fehler auf einmal, beide hier festgehalten.
   *
   * DER ERSTE: `dateien_pruefen` war als einziger der fünf ein
   * `#[tauri::command]` ohne `(async)`. Der Makro-Quelltext von Tauri sagt,
   * was das heißt — `ExecutionContext::Blocking` antwortet auf dem
   * aufrufenden Faden, und das ist unter Windows der Faden, der das Fenster
   * zeichnet. Vierzig Fotos zu untersuchen fror die Anzeige also ein, und
   * ein Fortschrittsbericht käme gar nicht erst durch: Er würde zugestellt,
   * wenn schon alles fertig ist.
   *
   * DER ZWEITE: Ohne Bericht ist ein arbeitendes Fenster von einem
   * hängenden nicht zu unterscheiden. Beim Löschen ist das mehr als
   * unangenehm — wer es für hängend hält, greift zum Task-Manager, mitten
   * im Überschreiben.
   *
   * Der Test liest die Rust-Datei, damit ein SECHSTER Stapelbefehl nicht
   * still ohne beides hinzukommt.
   */
  it("die Liste ist da und vollzählig", () => {
    // Sonst prüfte alles Weitere nichts und wäre trotzdem grün.
    const namen = stapelbefehle().map((b) => b.name);
    expect(namen).toEqual(
      expect.arrayContaining([
        "dateien_pruefen",
        "bereinigt_speichern",
        "verschluesseln",
        "loeschen_beurteilen",
        "loeschen_ausfuehren",
      ]),
    );
  });

  it("jeder läuft neben dem Hauptfaden", () => {
    for (const b of stapelbefehle()) {
      expect(
        b.attribut,
        `„${b.name}“ braucht #[tauri::command(async)] — sonst friert das ` +
          `Fenster ein, solange er läuft`,
      ).toBe("(async)");
    }
  });

  it("jeder nimmt einen Kanal für den Fortschritt", () => {
    for (const b of stapelbefehle()) {
      expect(
        b.args,
        `„${b.name}“ arbeitet einen Stapel ab, meldet aber nicht, wo er steht`,
      ).toMatch(/fortschritt\s*:\s*Channel<Fortschritt>/);
    }
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
