/**
 * Prüfungen am Quelltext selbst.
 *
 * # Warum es das gibt
 *
 * Dieselbe Sorte Fehler ist mir dreimal unterlaufen: Ein deutsches
 * Anführungszeichen wird geöffnet („) und mit dem geraden ASCII-Zeichen
 * geschlossen. Innerhalb eines JavaScript-Strings beendet das den String —
 * mal gibt es einen Übersetzungsfehler, mal, schlimmer, lautet ein Text
 * stillschweigend anders als beabsichtigt.
 *
 * Ein Fehler, der sich wiederholt, ist keine Unaufmerksamkeit mehr, sondern
 * eine fehlende Prüfung.
 *
 * Die Dateien kommen über `import.meta.glob` statt über `node:fs`: So
 * braucht der App-Quelltext keine Node-Typen, und der Test läuft in
 * derselben Umgebung wie alles andere.
 */

import { describe, expect, it } from "vitest";

const QUELLEN = import.meta.glob("/src/**/*.{ts,svelte}", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

const ZU_PRUEFEN = Object.entries(QUELLEN).filter(
  ([pfad]) => !pfad.endsWith("quelltext.test.ts"),
);

describe("Anführungszeichen", () => {
  it("findet überhaupt Quelldateien", () => {
    // Ohne diese Prüfung wäre der Test unten grün, weil er nichts ansieht.
    expect(ZU_PRUEFEN.length).toBeGreaterThan(15);
  });

  it('jedes geöffnete „ wird mit “ geschlossen, nie mit "', () => {
    const fehler: string[] = [];

    for (const [pfad, roh] of ZU_PRUEFEN) {
      // Was in `${…}` steht, ist Code und nicht Text: `„${name || "x"}“` ist
      // richtig, obwohl dazwischen ein gerades Anführungszeichen steht.
      // Gleiche Länge einsetzen, damit die Zeilennummern stimmen.
      const inhalt = roh.replace(/\$\{[^{}]*\}/g, (m) => "·".repeat(m.length));

      // Über die ganze Datei, nicht je Zeile: Ein Anführungspaar darf einen
      // Zeilenumbruch überspannen, und genau so ist mir der Fehler zuletzt
      // durchgerutscht.
      let ab = inhalt.indexOf("„");
      while (ab !== -1) {
        const rest = inhalt.slice(ab + 1);
        const zu = rest.indexOf("“");
        const gerade = rest.indexOf('"');
        const naechstesAuf = rest.indexOf("„");

        // Ein gerades Anführungszeichen vor dem schließenden und vor dem
        // nächsten öffnenden ist der Fehler.
        const falsch =
          gerade !== -1 &&
          (zu === -1 || gerade < zu) &&
          (naechstesAuf === -1 || gerade < naechstesAuf);

        if (falsch) {
          const zeile = inhalt.slice(0, ab).split("\n").length;
          fehler.push(`${pfad}:${zeile} — …${inhalt.slice(ab, ab + 60)}…`);
        }
        ab = inhalt.indexOf("„", ab + 1);
      }
    }

    expect(fehler, `\n${fehler.join("\n")}\n`).toEqual([]);
  });
});

describe("was die Oberfläche nie zu sehen bekommt", () => {
  /**
   * `spec/anzeige.md` §6 und die Architekturregel für Phase 4:
   * Schlüsselmaterial bleibt in Rust. Das Frontend erhält Handles, Status
   * und Fortschritt — nie Secrets.
   *
   * Der Brückenvertrag (`kern/typen.ts`) ist die Stelle, an der das
   * einreißen würde: Ein Feld für den privaten Schlüssel wäre schnell
   * ergänzt und fiele in einer Durchsicht nicht auf.
   */
  it("der Brückenvertrag führt kein Feld für Geheimnisse", () => {
    const vertrag = QUELLEN["/src/lib/kern/typen.ts"];
    expect(vertrag, "der Brückenvertrag wurde nicht gefunden").toBeTypeOf(
      "string",
    );

    // Feldnamen, nicht Fließtext: nur Zeilen der Form `name: Typ`.
    const felder = [...vertrag!.matchAll(/^\s{2,}(\w+)\??:/gm)].map(
      (m) => m[1]!,
    );

    const verdaechtig = felder.filter((f) =>
      /passwor|passphrase|secret|geheim|privat|seed|privkey/i.test(f),
    );

    expect(verdaechtig).toEqual([]);
  });

  it("kein Bildschirm zeigt einen privaten Schlüssel an", () => {
    const treffer: string[] = [];

    for (const [pfad, inhalt] of ZU_PRUEFEN) {
      if (!pfad.includes("/bildschirme/")) continue;
      // Wortlaut, der nur in einer Anzeige stünde, die es nie geben darf.
      for (const wendung of [
        "privater Schlüssel anzeigen",
        "Privaten Schlüssel",
        "Geheimen Schlüssel",
        "secretKey",
        "privateKey",
      ]) {
        if (inhalt.includes(wendung)) treffer.push(`${pfad}: ${wendung}`);
      }
    }

    expect(treffer).toEqual([]);
  });
});
