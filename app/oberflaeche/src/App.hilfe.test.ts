/**
 * Der Start, die Hilfe und die Beispiele — in der ganzen Anwendung.
 *
 * # Was hier geprüft wird, und warum in der Hülle
 *
 * Am 27.10.2026 lief der Installer zum ersten Mal auf einem echten Rechner.
 * Er öffnete auf einem Beispielfall — einer Nachricht von „Dr. Anna
 * Beispiel“, mit Fingerprint und Zeitstempel —, und unten stand „Alles
 * andere ist noch Beispieldaten“, was seit Phase 4 nicht mehr stimmte.
 * Kein Bildschirmtest konnte das sehen: Jeder Bildschirm stellte richtig
 * dar, was er bekam. Falsch war, WAS die Hülle ihm gab.
 */
import { beforeEach, describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import App from "./App.svelte";
import { MockBruecke } from "./lib/kern/bruecke";
import { FAELLE, KONTAKTE, STAPEL } from "./lib/kern/mock";
import { KAPITEL } from "./lib/hilfe/handbuch";
import {
  empfangsspeicher,
  identitaetsspeicher,
  kontaktspeicher,
  sendespeicher,
  sitzungsspeicher,
} from "./lib/kern/speicher.svelte";
import { abgewickelt } from "./lib/kern/pruefstand.svelte";

let bruecke: MockBruecke;

async function verbinden(mitIdentitaet: boolean) {
  bruecke = new MockBruecke(KONTAKTE);
  // Die Attrappe beginnt MIT einer Identität und entsperrt -- der Prototyp
  // soll ohne Passwort durchsehbar sein. Den frisch installierten Rechner
  // stellt erst das Löschen her, wie in `App.einrichtung.test.ts`.
  if (!mitIdentitaet) await bruecke.identitaetLoeschen();
  for (const s of [
    sitzungsspeicher,
    kontaktspeicher,
    identitaetsspeicher,
    sendespeicher,
    empfangsspeicher,
  ]) {
    s.verbinde(bruecke);
  }
  sitzungsspeicher.geladen = false;
  await sitzungsspeicher.laden();
}

function anhaengen() {
  document.body.innerHTML = '<div id="app"></div>';
  const ziel = document.getElementById("app")!;
  const a = mount(App, { target: ziel });
  flushSync();
  const knopf = (teil: string) =>
    [...ziel.querySelectorAll("button")].find((k) => k.textContent?.includes(teil));
  return {
    ziel,
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    knopf,
    klick: (teil: string) => {
      const k = knopf(teil);
      expect(k, `kein Knopf „${teil}“`).toBeDefined();
      k!.click();
      flushSync();
    },
    band: () => ziel.querySelector('[data-pruefstelle="beispielband"]'),
    abbauen: () => unmount(a),
  };
}

describe("der erste Start", () => {
  beforeEach(() => verbinden(false));

  it("führt ohne Identität zur Einrichtung", async () => {
    const s = anhaengen();
    await abgewickelt();
    flushSync();
    expect(s.text()).toContain("Sie erzeugen gleich einen Schlüssel");
    expect(s.text(), "kein erfundener Absender").not.toContain("Dr. Anna Beispiel");
    s.abbauen();
  });

  it("holt nicht zurück, wer danach woandershin wechselt", async () => {
    const s = anhaengen();
    await abgewickelt();
    s.klick("Hilfe");
    await abgewickelt();
    flushSync();
    expect(s.text()).not.toContain("Sie erzeugen gleich einen Schlüssel");
    expect(s.text()).toContain(KAPITEL[0]!.titel);
    s.abbauen();
  });
});

describe("mit Identität", () => {
  beforeEach(() => verbinden(true));

  it("öffnet auf Empfangen — leer, ohne erfundene Nachricht", async () => {
    const s = anhaengen();
    await abgewickelt();
    flushSync();
    expect(s.text()).toContain("Noch keine Nachricht geöffnet");
    expect(s.text()).not.toContain("Dr. Anna Beispiel");
    expect(s.text()).not.toContain(FAELLE[0]!.titel);
    expect(s.band()).toBeNull();
    // Der Satz, der bis zum 27.10.2026 unten links stand.
    expect(s.text()).not.toContain("noch Beispieldaten");
    s.abbauen();
  });

  it("ein Beispiel aus dem Handbuch trägt das Band — der Bereich links verlässt es", async () => {
    const s = anhaengen();
    await abgewickelt();
    s.klick("Hilfe");
    s.klick("Beispiele");
    s.klick(FAELLE[0]!.titel);

    expect(s.band(), "das Band fehlt").not.toBeNull();
    expect(s.text()).toContain("keine echte Nachricht");
    expect(s.text()).toContain("Dr. Anna Beispiel");

    // Wer links auf „Empfangen“ klickt, will sein eigenes Arbeiten sehen.
    s.klick("Empfangen");
    expect(s.band()).toBeNull();
    expect(s.text()).toContain("Noch keine Nachricht geöffnet");
    expect(s.text()).not.toContain("Dr. Anna Beispiel");
    s.abbauen();
  });

  it("„Zurück zu den Beispielen“ führt dorthin zurück", async () => {
    const s = anhaengen();
    await abgewickelt();
    s.klick("Hilfe");
    s.klick("Beispiele");
    s.klick(FAELLE[1]!.titel);
    s.klick("Zurück zu den Beispielen");
    expect(s.band()).toBeNull();
    expect(s.text()).toContain(FAELLE[2]!.titel);
    s.abbauen();
  });

  it("auch ein Beispielstapel trägt es, und dort wird nichts verschlüsselt", async () => {
    const s = anhaengen();
    await abgewickelt();
    s.klick("Hilfe");
    s.klick("Beispiele");
    s.klick(STAPEL[0]!.titel);

    expect(s.band()).not.toBeNull();
    expect(s.text()).toContain("Verschlüsselt und gespeichert wird hier nichts");
    // Keine Auswahl ist markiert: Weder „Ausgewählte Dateien“ noch
    // „Textnachricht“ ist das, was gerade dasteht.
    expect(s.knopf("Ausgewählte Dateien")!.className).not.toContain("bg-schrift");

    s.klick("Ausgewählte Dateien");
    expect(s.band()).toBeNull();
    expect(s.text()).toContain("Noch nichts ausgewählt");
    s.abbauen();
  });

  it("die Werkzeug-Beispiele gehen mit dem gewählten Werkzeug auf", async () => {
    const s = anhaengen();
    await abgewickelt();
    s.klick("Hilfe");
    s.klick("Beispiele");
    s.klick("Was ein Mitleser");

    expect(s.band()).not.toBeNull();
    expect(s.text()).toContain("Gelöscht wird hier nichts");
    expect(s.text()).toContain("Was ein Mitleser sieht");

    // Und ohne Beispiel: kein erfundener Pfad, keine Außenansicht.
    s.klick("Werkzeuge");
    expect(s.band()).toBeNull();
    expect(s.text()).toContain("Noch keine Dateien ausgewählt");
    expect(s.text()).not.toContain("Was ein Mitleser sieht");
    s.abbauen();
  });

  it("jedes Kapitel lässt sich links öffnen", async () => {
    const s = anhaengen();
    await abgewickelt();
    s.klick("Hilfe");
    for (const k of KAPITEL) {
      s.klick(k.titel);
      expect(s.ziel.querySelector('[data-pruefstelle="handbuch"] h2')?.textContent).toBe(
        k.titel,
      );
    }
    s.abbauen();
  });

  it("ein Verweis im Handbuch führt in seinen Bereich", async () => {
    const s = anhaengen();
    await abgewickelt();
    s.klick("Hilfe");
    s.klick("Zur Einrichtung");
    expect(s.text()).toContain("Hier legen Sie Ihre Identität an");
    s.abbauen();
  });
});
