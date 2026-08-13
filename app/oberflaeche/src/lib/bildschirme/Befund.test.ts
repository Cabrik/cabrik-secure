/**
 * Der vollständige Befund — und die Wahl der Fassung.
 *
 * Die tragende Regel dieses Bildschirms ist eine, die man beim Bauen leicht
 * anders trifft: **Es wird alles gezeigt, nicht nur das Verbleibende.**
 *
 * Zeigt man nur den Rest, sieht eine sauber bereinigte Datei aus wie eine,
 * in der nie etwas stand. Der Nutzer erfährt nie, dass sein Name, die
 * Seriennummer seiner Kamera und der Aufnahmeort drin waren — und lernt
 * dadurch nie, dass er das mit sich herumträgt.
 */

import { describe, expect, it, vi } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Befund from "./Befund.svelte";
import { STAPEL } from "../kern/mock";
import type { Sendedatei } from "../kern/typen";

function datei(stapel: string, name: string): Sendedatei {
  return STAPEL.find((s) => s.kennung === stapel)!.dateien.find(
    (d) => d.name === name,
  )!;
}

function darstellen(d: Sendedatei, original = false) {
  const ziel = document.createElement("div");
  document.body.append(ziel);
  const waehle = vi.fn();
  const schliessen = vi.fn();
  const b = mount(Befund, {
    target: ziel,
    props: { datei: d, original, waehle, schliessen },
  });
  return {
    ziel,
    waehle,
    schliessen,
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    fassung: () =>
      (
        ziel.querySelector('[data-pruefstelle="fassung"]')?.textContent ?? ""
      ).replace(/\s+/g, " "),
    eintraege: () => [...ziel.querySelectorAll("li")],
    funkKnopf: (nr: number) =>
      [...ziel.querySelectorAll<HTMLInputElement>('input[type="radio"]')][nr],
    aufraeumen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

// ---------------------------------------------------------------------------
// Alles zeigen, nicht nur den Rest
// ---------------------------------------------------------------------------

describe("der Befund zeigt jeden Fund, auch den entfernten", () => {
  it("bei einer vollständig bereinigten Datei stehen die Funde trotzdem da", () => {
    // Genau der Fall, der sonst wie „da war nie etwas“ aussähe.
    const s = darstellen(datei("eine-saubere", "Protokoll.pdf"));

    expect(s.text()).toContain("Gefunden (2)");
    expect(s.text()).toContain("Dr. Anna Beispiel");
    expect(s.text()).toContain("PDF:DocInfo/Author");

    s.aufraeumen();
  });

  it("je Fund steht dabei, ob er die Datei verlässt", () => {
    const s = darstellen(datei("eine-saubere", "Protokoll.pdf"));

    for (const e of s.eintraege()) {
      expect(e.textContent).toContain("wird entfernt");
    }

    s.aufraeumen();
  });

  it("bei teilweiser Bereinigung stehen beide Sorten nebeneinander", () => {
    const s = darstellen(datei("eine-mit-rest", "Mitschnitt.mp3"));
    const text = s.text();

    // Das Entfernte …
    expect(text).toContain("Dr. Anna Beispiel");
    expect(text).toContain("wird entfernt");
    // … und das Bleibende, mit Grund.
    expect(text).toContain("bleibt");
    expect(text).toContain("Neuberechnen des Tons");

    s.aufraeumen();
  });

  it("die Reihenfolge richtet sich nach der Schwere", () => {
    const s = darstellen(datei("eine-saubere", "Protokoll.pdf"));
    const erster = s.eintraege()[0]!.textContent ?? "";

    expect(erster).toContain("kritisch");

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Was das Programm nicht weiß
// ---------------------------------------------------------------------------

describe("ohne verstandenes Format gibt es keinen Befund", () => {
  it("und das wird gesagt, statt Leere als Sauberkeit auszugeben", () => {
    const s = darstellen(datei("grosser-stapel", "Uebersicht.psd"));
    const text = s.text();

    expect(text).toContain("Kein Befund möglich");
    expect(text).toContain("auch nicht, dass nichts drin ist");

    s.aufraeumen();
  });

  it("und es gibt dann auch keine Wahl zwischen zwei Fassungen", () => {
    const s = darstellen(datei("grosser-stapel", "Uebersicht.psd"));

    expect(s.funkKnopf(0)).toBeUndefined();
    expect(s.fassung()).toContain("Es gibt nur eine");

    s.aufraeumen();
  });

  it("eine unlesbare Datei meldet den Grund", () => {
    const s = darstellen(datei("grosser-stapel", "Notiz.txt.gpg"));

    expect(s.text()).toContain("Nicht lesbar");
    expect(s.text()).toContain("ließ sich nicht lesen");

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Die Fassungswahl
// ---------------------------------------------------------------------------

describe("welche Fassung hinausgeht", () => {
  it("beide Fassungen nennen, was sie bedeuten", () => {
    const s = darstellen(datei("eine-mit-rest", "Mitschnitt.mp3"));
    const text = s.fassung();

    expect(text).toContain("1 Angabe wird entfernt");
    expect(text).toContain("1 bleibt in der Datei");
    expect(text).toContain("gehen mit hinaus");

    s.aufraeumen();
  });

  it("voreingestellt ist die bereinigte Fassung", () => {
    const s = darstellen(datei("eine-saubere", "Protokoll.pdf"));

    expect(s.funkKnopf(0)?.checked).toBe(true);
    expect(s.funkKnopf(1)?.checked).toBe(false);

    s.aufraeumen();
  });

  it("die Wahl des Originals wird gemeldet", () => {
    const s = darstellen(datei("eine-saubere", "Protokoll.pdf"));

    s.funkKnopf(1)!.click();
    flushSync();

    expect(s.waehle).toHaveBeenCalledWith(true);

    s.aufraeumen();
  });

  it("und ist magenta, nicht rot — eine Einstellung, kein Fehler", () => {
    const s = darstellen(datei("eine-saubere", "Protokoll.pdf"), true);

    expect(s.text()).toContain("Sie senden das Original");
    expect(s.text()).toContain("2 Angaben mehr als nötig");
    // Kein Vorwurf.
    expect(s.text()).not.toContain("Fehler");
    expect(s.text()).not.toContain("unsicher");

    s.aufraeumen();
  });

  it("nennt einen Grund, warum jemand das Original wollen könnte", () => {
    // Ohne diesen Satz wirkt die Wahl wie eine Falle. Sie ist aber
    // manchmal genau das Richtige — und wer sie nicht bekommt, umgeht das
    // Programm.
    const s = darstellen(datei("eine-saubere", "Protokoll.pdf"));

    expect(s.fassung()).toContain("wenn die Angaben der Zweck sind");
    expect(s.fassung()).toContain("Urheberangabe");

    s.aufraeumen();
  });
});
