/**
 * Jeder Beispielfall muss sich darstellen lassen — und das Richtige sagen.
 *
 * Die Typprüfung sagt nur, dass es sich übersetzt. Dieser Test rendert jeden
 * der acht Fälle serverseitig und liest nach, was tatsächlich dasteht. Damit
 * wird der Anzeigevertrag nicht nur in den Zuordnungsfunktionen geprüft,
 * sondern bis zur fertigen Ausgabe.
 */

import { describe, expect, it } from "vitest";
import { mount, unmount } from "svelte";
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
  const fall = FAELLE.find((f) => f.kennung === kennung)!;
  const ziel = document.createElement("div");
  document.body.append(ziel);
  const b = mount(Empfangen, { target: ziel, props: { fall } });
  const t = (ziel.textContent ?? "").replace(/\s+/g, " ").trim();
  unmount(b);
  ziel.remove();
  return t;
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
    expect(t).toContain("Alle bekannten Metadaten entfernt");
  });

  it("beim MP3 steht der Grund für das Verbleibende, nicht nur die Zahl", () => {
    const t = darstellen("mp3-rest");

    expect(t).toContain("Teilweise bereinigt");
    expect(t).toContain("Neuberechnen des Tons");
    expect(t).toContain("Geblieben");
  });

  it("die Rohdatei nennt den Ausweg", () => {
    const t = darstellen("rohdatei");

    expect(t).toContain("SubIFD");
    expect(t).toContain("JPEG");
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
