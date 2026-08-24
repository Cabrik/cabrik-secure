/**
 * Die beiden Symbole — und wo sie stehen dürfen.
 *
 * # Die Regel, die zweimal fast gebrochen wurde
 *
 * `spec/anzeige.md` §5: Die vier Zeichen gelten für **Befunde** — für
 * Aussagen über eine Datei, einen Absender, einen Kontakt. Nicht für
 * Zustände des Programms.
 *
 * Der Sperrbildschirm trug einmal ein `?` vor „Gesperrt“, weil grau in
 * diesem System „keine Aussage“ heißt. Gelesen wurde es als „etwas ist
 * schiefgegangen“. Der nächste Vorschlag war ein Warndreieck an
 * derselben Stelle — derselbe Fehler in Gelb.
 *
 * Und er hätte einen Preis über den einen Bildschirm hinaus: Wer das
 * Dreieck bei **jedem** Entsperren sieht, liest es nach zwei Wochen nicht
 * mehr. Wenn dann ein echtes auftaucht, trägt es nichts.
 *
 * # Warum kein Test auf das Aussehen
 *
 * Weil es dazu nichts zu prüfen gibt, was nicht schon das Auge sieht.
 * Geprüft wird, was sich still ändern kann: **wo** die Symbole stehen und
 * dass sie ihre Farbe vom Text erben.
 */

import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";

function quelle(pfad: string): string {
  return readFileSync(new URL(pfad, import.meta.url), "utf8");
}

describe("die Symbole folgen dem Erscheinungsbild", () => {
  it("tragen keine feste Farbe", () => {
    // Die Vorlagen kamen mit `#FFC400`. Im hellen Erscheinungsbild ist
    // die Warnfarbe aber ein dunkles Bernstein -- sonst waere sie auf
    // Weiss nicht lesbar. Eine feste Farbe stimmte in genau einem der
    // beiden Faelle.
    for (const datei of ["./Warnzeichen.svelte", "./Wartezeichen.svelte"]) {
      const text = quelle(datei);
      expect(text, `${datei} hat eine feste Farbe`).not.toMatch(/#[0-9A-Fa-f]{6}/);
      expect(text).toContain("currentColor");
    }
  });
});

describe("wo die Symbole stehen", () => {
  it("kein Warndreieck auf dem Sperrbildschirm", () => {
    // DER KERN DER SACHE. Gesperrt zu sein ist der erwuenschte
    // Normalzustand -- keine Beobachtung, die jemandem zu denken geben
    // soll.
    const text = quelle("../bildschirme/Sperrbildschirm.svelte");
    expect(text).not.toContain("Warnzeichen");
  });

  it("und auch kein Fragezeichen mehr", () => {
    // Dasselbe in Grau: `?` heisst „konnte nicht geprueft werden“.
    const text = quelle("../bildschirme/Sperrbildschirm.svelte");
    expect(text).not.toContain('aria-hidden="true">?');
  });

  it("die Sanduhr steht beim Ableiten, nicht bei der wartenden Datei", () => {
    const text = quelle("../bildschirme/Sperrbildschirm.svelte");
    // Sie steht da.
    expect(text).toContain("Wartezeichen");

    // Und zwar im Abschnitt, der waehrend der Ableitung erscheint. Der
    // Abstand zwischen beiden Stellen ist der Pruefstein: Stuende sie bei
    // „Eine Datei wartet“, laege sie weit davor.
    const beimAbleiten = text.indexOf("Das dauert einen Moment");
    const symbol = text.lastIndexOf("<Wartezeichen");
    expect(symbol).toBeGreaterThan(0);
    expect(
      Math.abs(beimAbleiten - symbol),
      "die Sanduhr steht nicht beim Ableiten",
    ).toBeLessThan(400);
  });

  it("das Warndreieck steht bei den echten Warnungen", () => {
    // Dort, wo vorher ein nacktes `!` stand: Vorbehalte beim Versand,
    // misslungene Dateien, die ablaufende Frist.
    for (const datei of ["../../App.svelte", "./Sperrleiste.svelte"]) {
      const text = quelle(datei);
      expect(text, `${datei} hat kein Warnzeichen`).toContain("Warnzeichen");
      // Und das alte ASCII-Zeichen ist weg.
      expect(text).not.toContain('aria-hidden="true">!</span>');
    }
  });
});
