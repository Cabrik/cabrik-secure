/**
 * Der Bildschirm, wenn Cabrik gar nicht erst starten konnte.
 *
 * # Warum es ihn gibt
 *
 * Das Fenster läuft unter Windows ohne Konsole. Ein `eprintln!` beim Start
 * schrieb dort auf einen Ausgang, den es nicht gibt: Wer Cabrik
 * doppelklickte und dessen Schlüsseldatei beschädigt war, sah **gar
 * nichts**. Version 1 stürzte wenigstens sichtbar ab.
 *
 * # Die Zusicherung, die am meisten wiegt
 *
 * **Bei einem Startfehler erscheint keine Einrichtung.** Wer die sieht,
 * während seine Schlüsseldatei nur beschädigt ist, legt womöglich eine neue
 * Identität an — und dann ist tatsächlich alles fort, was an die alte
 * gerichtet war. Der Fehlerbildschirm ist deshalb ausschließlich.
 */

import { describe, expect, it } from "vitest";
import { mount, unmount } from "svelte";
import Startfehler from "./Startfehler.svelte";
import type { Startfehler as Fehler } from "../kern/typen";

const FEHLER: Fehler = {
  meldung: "Die Schlüsseldatei ließ sich nicht lesen: unerwartetes Ende.",
  pfad: "C:\\Users\\jemand\\AppData\\Roaming\\cabrik\\identity.cabrik-key",
  rat: "Legen Sie die Datei beiseite, statt sie zu löschen.",
};

function zeigen(fehler: Fehler = FEHLER) {
  const ziel = document.createElement("div");
  document.body.append(ziel);
  const b = mount(Startfehler, { target: ziel, props: { fehler } });
  return {
    ziel,
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    abbauen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

describe("Startfehler", () => {
  it("sagt überhaupt etwas — das ist der ganze Punkt", () => {
    const s = zeigen();

    expect(s.text()).toContain("Cabrik konnte nicht starten");
    expect(s.text()).toContain("unerwartetes Ende");
    s.abbauen();
  });

  it("nennt die betroffene Datei mit vollem Pfad", () => {
    /*
     * Ohne ihn sucht jemand an der falschen Stelle, und bei einer
     * Schlüsseldatei ist die falsche Stelle teuer. Der Unterschied zwischen
     * einem Rätsel und einer Aufgabe.
     */
    const s = zeigen();

    expect(s.text()).toContain(FEHLER.pfad!);
    s.abbauen();
  });

  it("sagt, was zu tun ist — nicht nur, was kaputt ist", () => {
    const s = zeigen();

    expect(s.text()).toContain("Was Sie tun können");
    expect(s.text()).toContain("beiseite");
    s.abbauen();
  });

  it("rät nicht zum Löschen", () => {
    // Der teuerste Rat, den man hier geben könnte. Solange die Datei da
    // ist, ist nichts endgültig verloren.
    const s = zeigen();
    const text = s.text().toLowerCase();

    expect(text).not.toContain("löschen sie");
    expect(text).not.toContain("neu anlegen");
    expect(text).not.toContain("neue identität");
    s.abbauen();
  });

  it("nimmt die Angst um die verschlüsselten Dateien", () => {
    // Wer diesen Bildschirm sieht, denkt zuerst, seine Daten seien fort —
    // und in diesem Zustand macht man Dinge, die es dann tatsächlich sind.
    const s = zeigen();

    expect(s.text()).toContain("Verschlüsselte Dateien sind hiervon nicht betroffen");
    s.abbauen();
  });

  it("kommt auch ohne Pfad zurecht", () => {
    // Beim fehlenden Konfigurationsverzeichnis gibt es keinen zu nennen.
    const s = zeigen({
      meldung: "Kein Verzeichnis gefunden.",
      pfad: null,
      rat: "Melden Sie sich neu an.",
    });

    expect(s.text()).toContain("Kein Verzeichnis gefunden");
    expect(s.text()).not.toContain("Betroffene Datei");
    s.abbauen();
  });

  it("meldet den Fehler an Bildschirmleser", () => {
    const s = zeigen();

    expect(s.ziel.querySelector('[role="alert"]')?.textContent).toContain(
      "nicht lesen",
    );
    s.abbauen();
  });
});
