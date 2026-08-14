/**
 * Zwei Zusicherungen, die der Sendebildschirm nie brechen darf.
 *
 * 1. **Der Bericht ist erreichbar.** Auch für eine vollständig bereinigte
 *    Datei — gerade dort erfährt man sonst nie, was drinstand.
 * 2. **Keine Datei verschwindet.** Was ausgewählt ist, steht irgendwo auf
 *    dem Bildschirm. Ein Häkchen darf eine Zeile verschieben, aber nie
 *    verschlucken: Wer zehn Dateien abzählt und neun verschickt bekommt,
 *    hat keinen Anhaltspunkt, wo die zehnte geblieben ist.
 */

import { describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Senden from "./Senden.svelte";
import type { Stapel } from "../kern/mock";
import type { Sendedatei } from "../kern/typen";

/** Eine vollständig bereinigte Datei mit lesbaren Funden. */
function sauber(pfad: string, name: string, bytes: number): Sendedatei {
  return {
    pfad,
    name,
    groesseBytes: bytes,
    fassungen: [],
    befund: {
      fall: "vollstaendig",
      format: "JPEG",
      entfernt: [
        {
          art: "geraet",
          ort: "EXIF:Model",
          wert: "Pixel 8 Pro",
          schwere: "beachtlich",
        },
        {
          art: "zeitangabe",
          ort: "EXIF:DateTime",
          wert: "2026:08:14 21:03:11",
          schwere: "beachtlich",
        },
      ],
    },
  };
}

function zeigen(dateien: Sendedatei[]) {
  const stapel: Stapel = {
    kennung: "auswahl",
    titel: "Ausgewählte Dateien",
    worumEsGeht: "Prüfung.",
    dateien,
  };
  const ziel = document.createElement("div");
  document.body.append(ziel);
  const b = mount(Senden, { target: ziel, props: { stapel } });
  return {
    ziel,
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    knopf: (teil: string) =>
      [...ziel.querySelectorAll("button")].find((k) =>
        k.textContent?.includes(teil),
      ),
    kaestchen: () => [
      ...ziel.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'),
    ],
    /** Jede Datei, die irgendwo auf dem Bildschirm benannt ist. */
    sichtbar: (alle: Sendedatei[]) =>
      alle.filter((d) => (ziel.textContent ?? "").includes(d.name)),
    klick: (el: HTMLElement | undefined) => {
      el!.click();
      flushSync();
    },
    abbauen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

describe("der Befund einer bereinigten Datei", () => {
  it("ist über die zugeklappte Zeile erreichbar", () => {
    // Der gemeldete Fehler: „ich bekomme den Bericht nicht, wenn ich auf
    // 13 Funde klicke“.
    const d = sauber("C:\\Fotos\\Test.jpg", "Test.jpg", 1_300_000);
    const s = zeigen([d]);

    s.klick(s.knopf("Funde entfernt"));

    expect(s.text()).toContain("Pixel 8 Pro");
    s.abbauen();
  });

  it("nennt jeden Fund mit seinem Wert, nicht nur die Zahl", () => {
    // Der Punkt, um den es geht: „2 Funde entfernt“ ist eine Mengenangabe.
    const d = sauber("C:\\Fotos\\Test.jpg", "Test.jpg", 1_300_000);
    const s = zeigen([d]);

    s.klick(s.knopf("Funde entfernt"));
    const text = s.text();

    expect(text).toContain("EXIF:Model");
    expect(text).toContain("Pixel 8 Pro");
    expect(text).toContain("2026:08:14 21:03:11");
    s.abbauen();
  });
});

describe("keine Datei verschwindet", () => {
  const zwei = [
    sauber("C:\\Fotos\\Eins.jpg", "Eins.jpg", 1_300_000),
    sauber("C:\\Fotos\\Zwei.jpg", "Zwei.jpg", 572_300),
  ];

  it("beide stehen da, solange beide mitgehen", () => {
    const s = zeigen(zwei);

    expect(s.sichtbar(zwei)).toHaveLength(2);
    s.abbauen();
  });

  it("eine abgewählte Datei bleibt sichtbar — sie geht nur nicht mit", () => {
    // Sie zu verstecken hieße, das Problem für gelöst zu halten statt für
    // umgangen: Die Datei ist ja noch da, sie geht nur nicht mit.
    const s = zeigen(zwei);
    const kaestchen = s.kaestchen()[0]!;

    s.klick(kaestchen);

    expect(
      s.sichtbar(zwei),
      "beide Namen müssen weiter auf dem Bildschirm stehen",
    ).toHaveLength(2);
    s.abbauen();
  });

  it("die Zählung nennt danach beide Zahlen", () => {
    // „1 Datei“ allein verschwiege die andere.
    const s = zeigen(zwei);

    s.klick(s.kaestchen()[0]!);

    expect(s.text()).toContain("1 von 2");
    s.abbauen();
  });

  it("und wieder anwählen führt zurück", () => {
    const s = zeigen(zwei);
    s.klick(s.kaestchen()[0]!);

    const wieder = s
      .kaestchen()
      .find((k) => !k.checked)!;
    s.klick(wieder);

    expect(s.text()).not.toContain("1 von 2");
    expect(s.sichtbar(zwei)).toHaveLength(2);
    s.abbauen();
  });
});
