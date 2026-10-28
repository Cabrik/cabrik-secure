/**
 * Sichern — mit oder ohne Kontakte.
 *
 * # Was hier auf dem Spiel steht
 *
 * 1. **Die Kontakte gehen mit, wenn niemand etwas anderes sagt.** Eine
 *    Sicherung ohne sie kostet beim Einspielen jeden Kontakt und jede
 *    Prüfung.
 * 2. **Aber es ist eine Wahl**, und sie kommt beim Kern an. Die Kontaktdatei
 *    verrät jedem, der die Sicherung mit dem Passwort öffnet, mit wem man
 *    zu tun hat.
 * 3. **Wer zwei Dateien bekommt, erfährt, dass sie zusammengehören.** Sonst
 *    zieht jemand nur die Schlüsseldatei auf den Stick und merkt es erst
 *    beim Einspielen.
 */

import { describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Identitaet from "./Identitaet.svelte";
import { IDENTITAET } from "../kern/mock";
import type { Sicherung } from "../kern/typen";

function einhaengen(gesichert: Sicherung | null = null) {
  const ziel = document.createElement("div");
  document.body.append(ziel);
  const gerufen: boolean[] = [];
  const b = mount(Identitaet, {
    target: ziel,
    props: {
      identitaet: IDENTITAET,
      sichern: (mitKontakten: boolean) => gerufen.push(mitKontakten),
      gesichert,
    },
  });
  const haken = () =>
    [...ziel.querySelectorAll("label")]
      .find((l) => l.textContent?.includes("Kontakte mitsichern"))!
      .querySelector<HTMLInputElement>('input[type="checkbox"]')!;
  return {
    gerufen,
    haken,
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    sichern: () => {
      [...ziel.querySelectorAll("button")]
        .find((k) => k.textContent?.includes("Schlüsseldatei sichern"))!
        .click();
      flushSync();
    },
    aufraeumen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

describe("Sichern", () => {
  it("nimmt die Kontakte mit, wenn niemand etwas anderes sagt", () => {
    const s = einhaengen();
    expect(s.haken().checked).toBe(true);
    s.sichern();
    expect(s.gerufen).toEqual([true]);
    s.aufraeumen();
  });

  it("lässt sie weg, wenn der Haken fehlt", () => {
    const s = einhaengen();
    s.haken().click();
    flushSync();
    s.sichern();
    expect(s.gerufen).toEqual([false]);
    s.aufraeumen();
  });

  it("sagt beim Haken, was die Kontaktdatei verrät", () => {
    const s = einhaengen();
    expect(s.text()).toContain("mit wem Sie zu tun haben");
    s.aufraeumen();
  });

  it("zeigt beide Pfade und dass sie zusammengehören", () => {
    const s = einhaengen({
      schluessel: "E:\\Sicherung\\identity.cabrik-key",
      kontakte: "E:\\Sicherung\\identity.cabrik-contacts",
    });
    const text = s.text();
    expect(text).toContain("identity.cabrik-key");
    expect(text).toContain("identity.cabrik-contacts");
    expect(text).toContain("Bewahren Sie beide Dateien zusammen auf");
    s.aufraeumen();
  });

  it("ohne Kontaktdatei gibt es nichts zusammenzuhalten", () => {
    const s = einhaengen({ schluessel: "E:\\Sicherung\\identity.cabrik-key", kontakte: null });
    expect(s.text()).toContain("identity.cabrik-key");
    expect(s.text()).not.toContain("Bewahren Sie beide Dateien zusammen auf");
    s.aufraeumen();
  });
});
