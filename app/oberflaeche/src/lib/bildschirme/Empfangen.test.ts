/**
 * Jeder Beispielfall muss sich darstellen lassen — und das Richtige sagen.
 *
 * Die Typprüfung sagt nur, dass es sich übersetzt. Dieser Test rendert jeden
 * der acht Fälle serverseitig und liest nach, was tatsächlich dasteht. Damit
 * wird der Anzeigevertrag nicht nur in den Zuordnungsfunktionen geprüft,
 * sondern bis zur fertigen Ausgabe.
 */

import { describe, expect, it } from "vitest";
import { mount } from "svelte";
import Empfangen from "./Empfangen.svelte";
import { FAELLE } from "../kern/mock";

/**
 * Hängt den Bildschirm an ein echtes Dokument und liest, was dasteht.
 *
 * Bewusst nicht serverseitig gerendert: Das sieht den Start im Browser
 * nicht — und genau dort entstand einmal eine weiße Seite, während alle
 * Tests grün blieben.
 */
function darstellen(kennung: string): string {
  const ziel = anhaengen(kennung);
  const t = (ziel.textContent ?? "").replace(/\s+/g, " ").trim();
  ziel.remove();
  return t;
}

/**
 * Wie `darstellen`, gibt aber das Element zurück statt seines Textes.
 *
 * Für Zusicherungen, die am Text nicht zu erkennen sind — etwa ob ein
 * `<details>` aufgeklappt ist: Sein Inhalt steht auch zugeklappt im DOM.
 */
function anhaengen(kennung: string): HTMLElement {
  const fall = FAELLE.find((f) => f.kennung === kennung)!;
  const ziel = document.createElement("div");
  document.body.append(ziel);
  mount(Empfangen, { target: ziel, props: { fall } });
  return ziel;
}

describe("jeder Beispielfall stellt sich dar", () => {
  it.each(FAELLE.map((f) => [f.kennung, f.titel]))("%s", (kennung) => {
    const t = darstellen(kennung as string);
    expect(t.length).toBeGreaterThan(80);
  });
});

describe("die Aussagen kommen bis in die Ausgabe", () => {
  it("das unverstandene Format sagt nirgends, es sei bereinigt", () => {
    const t = darstellen("unbekanntes-format").toLowerCase();

    expect(t).toContain("keine aussage");
    expect(t).toContain("photoshop");
    expect(t).not.toContain("bereinigt");
  });

  it("beim Handyvideo steht der Aufnahmeort im Klartext da", () => {
    const t = darstellen("handyvideo");

    expect(t).toContain("+46.9481");
    expect(t).toContain("Live Photo");
  });

  it("sagt beim Empfangenen nirgends, es sei bereinigt worden", () => {
    // Der Begriffsfehler, den dieser Bildschirm lange trug: Beim Empfangen
    // wird NICHTS bereinigt. Die Datei gehoert jemand anderem, und was
    // darinsteht, steht weiter darin. „Entfernt“ waere schlicht gelogen.
    // Gesucht ist die BEHAUPTUNG, nicht das Wort: Der erklärende Satz sagt
    // „Cabrik entfernt sie nicht“, und der darf natürlich stehen bleiben.
    const behauptungen = [
      /alle bekannten metadaten entfernt/,
      /teilweise bereinigt/,
      /\bwurden? entfernt/,
      // Die alte Überschrift der Fundliste. Sie stand für einen Vorgang,
      // den es beim Empfangen nicht gibt.
      /entfernt \(\d/,
      /geblieben \(\d/,
    ];
    for (const kennung of ["handyvideo", "mp3-rest", "rohdatei"]) {
      const t = darstellen(kennung).toLowerCase();

      for (const b of behauptungen) {
        expect(t, `${kennung}: ${b.source}`).not.toMatch(b);
      }
    }
  });

  it("benennt die kritischen Funde, statt sie nur zu zaehlen", () => {
    // Wer „4 Funde“ liest, klappt die Liste vielleicht nicht auf. „Darunter
    // eine Ortsangabe“ liest er.
    const t = darstellen("rohdatei");

    expect(t).toContain("3 Funde");
    expect(t).toContain("Ortsangabe");
    expect(t).toContain("2 kritische Funde");
  });

  it("sagt, dass die Funde dem Absender gehoeren", () => {
    // Die eigentliche Neuigkeit dieses Bildschirms. Metadaten in einer
    // ankommenden Datei sind das, was der ABSENDER preisgegeben hat.
    const t = darstellen("handyvideo");

    expect(t).toContain("der Absender");
    expect(t).toContain("weitergeben");
  });

  it("die Fundliste steht offen, nicht zugeklappt", () => {
    /*
     * Beim Senden ist sie eine Quittung; hier ist sie das Einzige, was vor
     * dem Speichern noch etwas ändert.
     *
     * **Am `open`-Merkmal geprüft, nicht am Text.** Ein `<details>` hält
     * seinen Inhalt auch zugeklappt im DOM — ein Test, der nur nach
     * „TIFF:GPS-IFD“ sucht, bleibt grün, wenn die Liste zufällt. Genau so
     * war er zuerst geschrieben; die Gegenprobe hat es gezeigt.
     */
    const ziel = anhaengen("rohdatei");
    const liste = ziel.querySelector("details");

    expect(liste?.open, "die Fundliste muss aufgeklappt stehen").toBe(true);
    expect(ziel.textContent).toContain("TIFF:GPS-IFD");
    ziel.remove();
  });

  it("nichts gefunden ist eine Aussage und kein Schweigen", () => {
    // Der Unterschied zu einer Textnachricht, wo `metadaten` null ist:
    // Dort stellt sich die Frage nicht, hier wurde nachgesehen.
    const t = darstellen("alles-gut");

    expect(t).toContain("Nichts gefunden");
    // Und der Vorbehalt steht dabei -- „nichts gefunden“ ist nicht „leer“.
    expect(t).toContain("bekannten Metadatenträgern");
  });

  it("eine Textnachricht sagt, dass sich die Frage nicht stellt", () => {
    const t = darstellen("nicht-verifiziert");

    expect(t).toContain("keine Dateimetadaten");
    expect(t).not.toContain("Nichts gefunden");
  });

  it("ein widerrufener Schlüssel wird als Fehler gezeigt", () => {
    const t = darstellen("widerrufen");

    expect(t).toContain("widerrufen");
    expect(t).toContain("gerade nichts Gutes");
  });

  it("der Schlüsselwechsel sagt, dass der alte verifiziert war", () => {
    const t = darstellen("schluessel-gewechselt");

    expect(t).toContain("Schlüssel gewechselt");
    expect(t).toContain("anderen Weg");
  });
});

describe("die verbotenen Formulierungen kommen nirgends vor", () => {
  it.each(FAELLE.map((f) => [f.kennung]))("%s", (kennung) => {
    const t = darstellen(kennung as string).toLowerCase();

    // `spec/anzeige.md` §5. "sicher" fehlt hier bewusst: Es steckt in
    // "Sicherheit" und ähnlichen Wörtern und braucht eine genauere Prüfung.
    for (const verboten of [
      "garantiert metadatenfrei",
      "vollständig gelöscht",
      "völlig anonym",
    ]) {
      expect(t).not.toContain(verboten);
    }
  });
});
