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

it("verträgt mehrere Funde an derselben Stelle", () => {
  // Der Fehler aus dem Fenster, wörtlich:
  //
  //   each_key_duplicate — Keyed each block has duplicate key
  //   `APP2:ICCfarbprofil` at indexes 10 and 11
  //
  // Ein großes ICC-Farbprofil wird über mehrere APP2-Segmente verteilt,
  // und jedes ergibt einen eigenen Fund an derselben Stelle. Die Liste war
  // über Fundstelle und Art geschlüsselt — und ein Schlüssel muss
  // eindeutig sein.
  //
  // Svelte bricht dann beim Zeichnen ab, und ein Bildschirm, der mitten im
  // Aufbau abbricht, bleibt einfach stehen: kein Bericht, keine Meldung,
  // von außen ein toter Knopf. Danach wirkte der halb aufgebaute Zustand
  // weiter — deshalb geriet anschließend auch die Dateiliste durcheinander.
  const elfmal: Fund[] = Array.from({ length: 11 }, () => ({
    art: "farbprofil" as const,
    ort: "APP2:ICC",
    wert: "65524 Bytes",
    schwere: "gering" as const,
  }));

  expect(() => zeichnen(elfmal)).not.toThrow();
  expect(zeichnen(elfmal)).toContain("Gefunden (11)");
});

it("verträgt zwei völlig gleiche Funde", () => {
  // Die härtere Fassung: gleiche Art, gleiche Stelle, gleicher Wert.
  const zwei: Fund[] = [
    { art: "vorschaubild", ort: "EXIF:Thumbnail", wert: null, schwere: "kritisch" },
    { art: "vorschaubild", ort: "EXIF:Thumbnail", wert: null, schwere: "kritisch" },
  ];

  expect(() => zeichnen(zwei)).not.toThrow();
});
