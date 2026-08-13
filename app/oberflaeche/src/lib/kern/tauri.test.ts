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
 */

import { describe, expect, it } from "vitest";
import { TauriBruecke, imFenster } from "./tauri";

import befehle from "./vertrag/befehle.json";

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
  it("die Liste aus Rust ist da und nicht leer", () => {
    // Sonst prüfte alles Weitere nichts und wäre trotzdem grün.
    expect(befehle.length).toBeGreaterThan(3);
  });

  it("jeder Befehl, den die Brücke ruft, ist im Fenster angemeldet", () => {
    const gerufen = gerufeneBefehle();
    expect(gerufen.length).toBeGreaterThan(3);

    for (const name of gerufen) {
      expect(befehle, `„${name}“ ist im Fenster nicht angemeldet`).toContain(
        name,
      );
    }
  });

  it("und jeder angemeldete wird auch gerufen", () => {
    // Die andere Richtung: Ein Befehl, den niemand ruft, ist tote Fläche —
    // und meistens ein Hinweis darauf, dass die Brücke etwas vergessen hat.
    const gerufen = gerufeneBefehle();
    for (const name of befehle) {
      expect(gerufen, `„${name}“ ruft niemand`).toContain(name);
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

describe("was noch nicht durchgereicht ist, sagt es", () => {
  it("Aufnehmen scheitert mit einem Satz, statt still nichts zu tun", async () => {
    /*
     * Die Attrappe nimmt Name, Fingerprint und ein Merkmal entgegen. Der
     * Kern braucht die Austausch-Nutzlast — daraus entstehen die Schlüssel,
     * und der Fingerprint wird neu berechnet statt übernommen.
     *
     * Die Signatur hier stillschweigend anzupassen hieße, den Bildschirm
     * auf eine Form zu ziehen, die noch niemand geprüft hat.
     */
    const b = new TauriBruecke();
    await expect(b.kontaktAufnehmen()).rejects.toThrow(/Austausch-Nutzlast/);
  });
});
