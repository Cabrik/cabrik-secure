/**
 * Was die ganze Anwendung tut, wenn der Start gescheitert ist.
 *
 * # Die Zusicherung, um die es geht
 *
 * **Dann erscheint keine Einrichtung.** Das ist der teure Fall: Wer die
 * Aufforderung „Legen Sie eine Identität an“ sieht, während seine
 * Schlüsseldatei nur *beschädigt* ist, legt womöglich eine neue an — und
 * dann ist tatsächlich alles fort, was an die alte gerichtet war. Auch das,
 * was noch gar nicht angekommen ist.
 *
 * Der Bildschirm für sich ist in `Startfehler.test.ts` geprüft. Was dort
 * niemand sieht: ob die Hülle ihn auch wirklich vorzieht. Genau dazwischen
 * lag schon einmal ein Fehler dieses Projekts.
 */
// @vitest-environment happy-dom

import { beforeEach, describe, expect, it } from "vitest";
import { mount, unmount } from "svelte";
import App from "./App.svelte";
import { MockBruecke } from "./lib/kern/bruecke";
import { KONTAKTE } from "./lib/kern/mock";
import {
  identitaetsspeicher,
  kontaktspeicher,
  sitzungsspeicher,
} from "./lib/kern/speicher.svelte";
import { abgewickelt } from "./lib/kern/pruefstand.svelte";
import type { Startfehler } from "./lib/kern/typen";

const FEHLER: Startfehler = {
  meldung: "Die Schlüsseldatei ließ sich nicht lesen: unerwartetes Ende.",
  pfad: "C:\\Users\\jemand\\AppData\\Roaming\\cabrik\\identity.cabrik-key",
  rat: "Legen Sie die Datei beiseite, statt sie zu löschen.",
};

/** Eine Brücke, deren Start gescheitert ist — und die deshalb sonst nichts weiß. */
class MitStartfehler extends MockBruecke {
  constructor(private wasSchiefging: Startfehler | null) {
    super(KONTAKTE);
  }

  override async startfehler(): Promise<Startfehler | null> {
    return this.wasSchiefging;
  }
}

async function anwendung(fehler: Startfehler | null) {
  const b = new MitStartfehler(fehler);
  // Ein gescheiterter Start heisst: keine Identitaet gelesen. Genau die
  // Lage, in der die Einrichtung erscheinen wuerde.
  await b.identitaetLoeschen();
  for (const s of [sitzungsspeicher, kontaktspeicher, identitaetsspeicher]) {
    s.verbinde(b);
  }
  sitzungsspeicher.startfehler = null;
  await sitzungsspeicher.laden();

  document.body.innerHTML = '<div id="app"></div>';
  const ziel = document.getElementById("app")!;
  const a = mount(App, { target: ziel });
  await abgewickelt();
  return {
    ziel,
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    abbauen: () => {
      unmount(a);
      document.body.innerHTML = "";
    },
  };
}

beforeEach(() => {
  sitzungsspeicher.startfehler = null;
});

describe("ein gescheiterter Start", () => {
  it("zeigt den Fehler statt irgendetwas anderem", async () => {
    const s = await anwendung(FEHLER);

    expect(s.text()).toContain("Cabrik konnte nicht starten");
    expect(s.text()).toContain(FEHLER.pfad!);
    s.abbauen();
  });

  it("zeigt KEINE Einrichtung", async () => {
    /*
     * Die teuerste Verwechslung des ganzen Programms. Eine beschädigte
     * Schlüsseldatei sieht von außen aus wie gar keine — und wer daraufhin
     * eine neue Identität anlegt, hat alles verloren, was an die alte
     * gerichtet war.
     */
    const s = await anwendung(FEHLER);
    const text = s.text();

    expect(text).not.toContain("Einrichtung");
    expect(text).not.toContain("Identität anlegen");
    expect(text).not.toContain("Keine Identität vorhanden");
    s.abbauen();
  });

  it("zeigt kein Passwortfeld", async () => {
    // Es wäre eine Aufforderung zu etwas, das gerade nicht geht.
    const s = await anwendung(FEHLER);

    expect(s.ziel.querySelector('input[type="password"]')).toBeNull();
    s.abbauen();
  });

  it("und ohne Startfehler läuft alles wie gehabt", async () => {
    // Die Gegenprobe: Ein Bildschirm, der immer erschiene, sperrte das
    // ganze Programm aus.
    const s = await anwendung(null);

    expect(s.text()).not.toContain("Cabrik konnte nicht starten");
    s.abbauen();
  });
});
