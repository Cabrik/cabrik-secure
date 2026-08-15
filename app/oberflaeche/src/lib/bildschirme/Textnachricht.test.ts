/**
 * Die Textnachricht.
 *
 * # Die Zusicherung, die zählt
 *
 * **Das Eingabefeld ist nach dem Verschlüsseln leer.** Der Text ist ein
 * Geheimnis, das durch die Webansicht geht — wie das Passwort. Was dort
 * stehen bleibt, bleibt im Speicher, solange das Fenster steht, und
 * niemand tippt es weg, wenn es funktioniert hat.
 */

import { describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Textnachricht from "./Textnachricht.svelte";
import { kontaktspeicher } from "../kern/speicher.svelte";
import { MockBruecke } from "../kern/bruecke";
import { KONTAKTE } from "../kern/mock";

function zeigen(envelope: string | null = null) {
  kontaktspeicher.verbinde(new MockBruecke(KONTAKTE));
  kontaktspeicher.liste = KONTAKTE.map((k) => ({ ...k }));

  const gerufen: { text: string; empfaenger: string[]; signieren: boolean }[] =
    [];
  const ziel = document.createElement("div");
  document.body.append(ziel);
  const b = mount(Textnachricht, {
    target: ziel,
    props: {
      envelope,
      verschluesseln: (
        text: string,
        empfaenger: string[],
        signieren: boolean,
      ) => gerufen.push({ text, empfaenger, signieren }),
    },
  });
  return {
    ziel,
    gerufen,
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    feld: () => ziel.querySelector<HTMLTextAreaElement>("textarea")!,
    knopf: () =>
      ziel.querySelector<HTMLButtonElement>(
        '[data-pruefstelle="text-verschluesseln"]',
      ),
    tippen: (wert: string) => {
      const f = ziel.querySelector<HTMLTextAreaElement>("textarea")!;
      f.value = wert;
      f.dispatchEvent(new Event("input", { bubbles: true }));
      flushSync();
    },
    empfaengerWaehlen: () => {
      const feld = [...ziel.querySelectorAll("label")]
        .find((l) => l.textContent?.includes("Dr. Anna Beispiel"))
        ?.querySelector("input");
      feld?.click();
      flushSync();
    },
    abbauen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

describe("Textnachricht", () => {
  it("verlangt Text und einen Empfänger", () => {
    const s = zeigen();
    expect(s.knopf()!.disabled).toBe(true);

    s.tippen("Treffpunkt um acht");
    expect(s.knopf()!.disabled, "ohne Empfänger nicht").toBe(true);

    s.empfaengerWaehlen();
    expect(s.knopf()!.disabled, "mit beidem schon").toBe(false);
    s.abbauen();
  });

  it("reicht Text, Empfänger und Signaturwunsch weiter", () => {
    const s = zeigen();
    s.tippen("Treffpunkt um acht");
    s.empfaengerWaehlen();

    s.knopf()!.click();
    flushSync();

    expect(s.gerufen).toHaveLength(1);
    expect(s.gerufen[0]!.text).toBe("Treffpunkt um acht");
    expect(s.gerufen[0]!.empfaenger).toHaveLength(1);
    expect(s.gerufen[0]!.signieren).toBe(true);
    s.abbauen();
  });

  it("leert das Eingabefeld nach dem Verschlüsseln", () => {
    // Die Zusicherung, um die es geht. Der Text ist ein Geheimnis, das
    // durch die Webansicht geht — was hier stehen bleibt, bleibt im
    // Speicher, solange das Fenster steht.
    const s = zeigen();
    s.tippen("streng vertraulich");
    s.empfaengerWaehlen();

    s.knopf()!.click();
    flushSync();

    expect(s.feld().value).toBe("");
    s.abbauen();
  });

  it("nennt die Längenverschleierung, bevor jemand tippt", () => {
    // Die eine Eigenschaft, die Text von einer Datei unterscheidet — und
    // die niemand von selbst vermutet.
    const s = zeigen();

    expect(s.text()).toContain("Länge der Nachricht wird verschleiert");
    s.abbauen();
  });

  it("sagt beim Ergebnis, dass der Rahmen das Programm nennt", () => {
    // Der Zielkonflikt aus §14, ausgesprochen. Wer ihn nicht kennt, hält
    // Armor für die bessere Wahl — dabei verrät er, womit die Nachricht
    // gemacht wurde.
    const s = zeigen("-----BEGIN CABRIK ENVELOPE-----\nAAAA\n-----END CABRIK ENVELOPE-----");

    expect(s.text()).toContain("Rahmenzeilen nennen das Programm");
    s.abbauen();
  });

  it("zeigt den Envelope zum Kopieren", () => {
    const armor = "-----BEGIN CABRIK ENVELOPE-----\nAAAA\n-----END CABRIK ENVELOPE-----";
    const s = zeigen(armor);

    expect(s.feld().value).toBe(armor);
    expect(s.feld().readOnly).toBe(true);
    s.abbauen();
  });

  it("bleibt gesperrt, wenn nicht wirklich verschlüsselt werden kann", () => {
    // Im Browser gibt es keine Identität. Ein Knopf, der so täte, als
    // verschlüssele er, wäre eine Lüge über das eigene Programm.
    kontaktspeicher.liste = KONTAKTE.map((k) => ({ ...k }));
    const ziel = document.createElement("div");
    document.body.append(ziel);
    const b = mount(Textnachricht, { target: ziel, props: {} });

    const knopf = ziel.querySelector<HTMLButtonElement>(
      '[data-pruefstelle="text-verschluesseln"]',
    );
    expect(knopf!.disabled).toBe(true);
    expect(ziel.textContent).toContain("Nur im Fenster");
    unmount(b);
    ziel.remove();
  });
});
