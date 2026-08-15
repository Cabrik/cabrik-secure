/**
 * Der Bericht muss **jede** Fundart zeichnen können.
 *
 * # Der Bericht, der dazu führte
 *
 * Eine Datei mit dreizehn Funden ließ ihren Bericht nicht öffnen; eine mit
 * dreien schon. Kein Hinweis, keine Meldung — der Klick tat nichts. Das
 * ist das Bild eines Fehlers **beim Zeichnen**: Bricht ein Bauteil mitten
 * im Aufbau ab, bleibt der Bildschirm stehen, wie er war.
 *
 * Seit der JPEG-Pfad die einzelnen EXIF-Einträge ausliest, kommen Arten
 * vor, die vorher nie eine Datei erzeugt hat.
 */

import { expect, it } from "vitest";
import { mount, unmount } from "svelte";
import Befund from "./Befund.svelte";
import { WAHL_VOREINSTELLUNG } from "../kern/typen";
import type { Fund, Sendedatei } from "../kern/typen";

/** Jede Art, die die Brücke überhaupt liefern kann. */
const ALLE_ARTEN: Fund["art"][] = [
  "ortsangabe",
  "personenname",
  "geraet",
  "software",
  "zeitangabe",
  "organisation",
  "vorschaubild",
  "zugeschnittenes_bild",
  "nachverfolgte_aenderung",
  "farbprofil",
  "kommentar",
  "bearbeitungssitzung",
  "dateiname",
  "unbekannte_erweiterung",
  "unbekannt",
];

function zeichnen(funde: Fund[]) {
  const datei: Sendedatei = {
    pfad: "C:\Fotos\TegTest1.jpg",
    name: "TegTest1.jpg",
    groesseBytes: 1_300_000,
    fassungen: [],
    befund: { fall: "vollstaendig", format: "JPEG", entfernt: funde },
  };
  const ziel = document.createElement("div");
  document.body.append(ziel);
  const b = mount(Befund, {
    target: ziel,
    props: {
      datei,
      original: false,
      waehle: () => {},
      wahl: WAHL_VOREINSTELLUNG,
      setzeWahl: () => {},
      schliessen: () => {},
    },
  });
  const text = ziel.textContent ?? "";
  unmount(b);
  ziel.remove();
  return text;
}

it("zeichnet jede einzelne Fundart ohne abzubrechen", () => {
  // Einzeln, damit die Meldung sagt WELCHE Art es ist.
  for (const art of ALLE_ARTEN) {
    const funde: Fund[] = [
      { art, ort: "EXIF:Probe", wert: "ein Wert", schwere: "beachtlich" },
    ];
    expect(() => zeichnen(funde), `Fundart „${art}“`).not.toThrow();
  }
});

it("kommt auch ohne Wert zurecht", () => {
  // `wert: null` heißt „nicht darstellbar“ und kommt bei EXIF-Einträgen
  // vor, die keine Zeichenkette sind.
  for (const art of ALLE_ARTEN) {
    const funde: Fund[] = [
      { art, ort: "EXIF:Probe", wert: null, schwere: "gering" },
    ];
    expect(() => zeichnen(funde), `Fundart „${art}“ ohne Wert`).not.toThrow();
  }
});

it("zeichnet dreizehn Funde auf einmal", () => {
  // Der gemeldete Fall.
  const funde: Fund[] = ALLE_ARTEN.slice(0, 13).map((art, i) => ({
    art,
    ort: `EXIF:Eintrag${i}`,
    wert: i % 3 === 0 ? null : `Wert ${i}`,
    schwere: (["kritisch", "beachtlich", "gering"] as const)[i % 3]!,
  }));

  const text = zeichnen(funde);

  expect(text).toContain("Gefunden (13)");
});
