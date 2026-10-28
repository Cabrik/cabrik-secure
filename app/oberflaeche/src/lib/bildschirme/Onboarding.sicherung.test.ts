/**
 * Eine Sicherung einspielen — der dritte Weg in der Einrichtung.
 *
 * # Was hier auf dem Spiel steht
 *
 * 1. **Eine falsche Datei wird benannt, bevor ein Passwort fällt.** Wer die
 *    Kontaktdatei statt der Schlüsseldatei erwischt, soll das hören — und
 *    nicht, sein Passwort sei falsch.
 * 2. **Kein Schritt ohne Entscheidung.** Signierschlüssel und Ableitung
 *    stehen in der Sicherung; „Optionen“ fällt weg.
 * 3. **Die Kontakte werden ausgesprochen.** Mit ihnen kommt der Stand ihrer
 *    Prüfung herein, und auch der Fall, in dem eine Kontaktdatei dalag und
 *    NICHT genommen wurde, muss dastehen.
 */

import { beforeEach, describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Onboarding from "./Onboarding.svelte";
import { identitaetsspeicher } from "../kern/speicher.svelte";
import { MockBruecke } from "../kern/bruecke";
import { IDENTITAET, KONTAKTE } from "../kern/mock";
import type { Eingespielt, Kontaktherkunft } from "../kern/typen";

function einhaengen() {
  const ziel = document.createElement("div");
  document.body.append(ziel);
  const b = mount(Onboarding, { target: ziel, props: {} });
  return {
    ziel,
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    knopf: (teil: string) =>
      [...ziel.querySelectorAll("button")].find((k) => k.textContent?.includes(teil)),
    passwortfelder: () => [...ziel.querySelectorAll<HTMLInputElement>('input[type="password"]')],
    tippen: (feld: HTMLInputElement, wert: string) => {
      feld.value = wert;
      feld.dispatchEvent(new Event("input", { bubbles: true }));
      flushSync();
    },
    klick: (el: HTMLElement | undefined) => {
      el!.click();
      flushSync();
    },
    /** Für Knöpfe mit `async`-Rückruf — siehe `Onboarding.v1.test.ts`. */
    klickWarten: async (el: HTMLElement | undefined) => {
      el!.click();
      await new Promise((f) => setTimeout(f, 0));
      flushSync();
    },
    aufraeumen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

/**
 * Eine Attrappe ohne Identität, deren Dateidialog `pfad` liefert und deren
 * Einspielen — wenn verlangt — eine feste Herkunft meldet.
 */
function verbinden(pfad = "E:\\Sicherung\\identity.cabrik-key", herkunft?: Kontaktherkunft) {
  class Attrappe extends MockBruecke {
    override async sicherungWaehlen() {
      return pfad;
    }
    override async sicherungEinspielen(
      quelle: string,
      passwort: string,
      zulassen: boolean,
    ): Promise<Eingespielt> {
      const echt = await super.sicherungEinspielen(quelle, passwort, zulassen);
      if (!herkunft) return echt;
      const mit = herkunft === "sicherung" || herkunft === "programmordner";
      return { ...echt, herkunft, kontakte: mit ? 7 : 0, verifiziert: mit ? 5 : 0 };
    }
  }
  const b = new Attrappe(KONTAKTE);
  void b.identitaetLoeschen();
  identitaetsspeicher.verbinde(b);
}

const PASSWORT = "vier zufaellige woerter hier";

/** Bis zum Passwortfeld der Sicherung. */
async function bisZumPasswort(s: ReturnType<typeof einhaengen>) {
  s.klick(s.knopf("Sicherung einspielen"));
  s.klick(s.knopf("Weiter"));
  await s.klickWarten(s.knopf("Datei wählen"));
}

beforeEach(() => verbinden());

describe("Sicherung einspielen", () => {
  it("steht als dritte Wahl im ersten Schritt", () => {
    const s = einhaengen();
    const sicherung = s.knopf("Sicherung einspielen")!;
    expect(sicherung.getAttribute("aria-pressed")).toBe("false");

    s.klick(sicherung);
    expect(sicherung.getAttribute("aria-pressed")).toBe("true");
    expect(s.text()).toContain("dieselbe Identität");
    // Die Bezeichnung steht in der Sicherung -- ein Feld dafür kaeme
    // nirgends an.
    expect(s.ziel.querySelector('input[placeholder="Arbeitsrechner"]')).toBeNull();
    s.aufraeumen();
  });

  it("kommt ohne den Schritt „Optionen“ aus", () => {
    const s = einhaengen();
    expect(s.text()).toContain("Optionen");
    s.klick(s.knopf("Sicherung einspielen"));
    expect(s.text()).not.toContain("Optionen");
    s.aufraeumen();
  });

  it("fragt erst nach der Datei, dann nach dem Passwort", async () => {
    const s = einhaengen();
    s.klick(s.knopf("Sicherung einspielen"));
    s.klick(s.knopf("Weiter"));

    expect(s.text()).toContain("Noch keine gewählt");
    expect(s.passwortfelder()).toHaveLength(0);

    await s.klickWarten(s.knopf("Datei wählen"));
    expect(s.text()).toContain("identity.cabrik-key");
    expect(s.passwortfelder()).toHaveLength(1);
    s.aufraeumen();
  });

  it("nennt die Kontaktdatei als solche, bevor ein Passwort fällt", async () => {
    verbinden("E:\\Sicherung\\identity.cabrik-contacts");
    const s = einhaengen();
    await bisZumPasswort(s);

    expect(s.text()).toContain("Das ist die Kontaktdatei");
    expect(s.passwortfelder()).toHaveLength(0);
    expect(s.knopf("Sicherung einspielen")!.hasAttribute("disabled")).toBe(true);
    s.aufraeumen();
  });

  it("lässt erst mit Passwort weiter", async () => {
    const s = einhaengen();
    await bisZumPasswort(s);

    const knopf = () => s.knopf("Sicherung einspielen")!;
    expect(knopf().hasAttribute("disabled")).toBe(true);
    s.tippen(s.passwortfelder()[0]!, PASSWORT);
    expect(knopf().hasAttribute("disabled")).toBe(false);
    s.aufraeumen();
  });

  it("fragt bei ungewöhnlichen Werten zuerst — und zeigt erst dann das Feld", async () => {
    verbinden("E:\\Sicherung\\ungewoehnlich.cabrik-key");
    const s = einhaengen();
    await bisZumPasswort(s);

    expect(s.ziel.querySelector('[data-pruef="rueckfrage"]')).not.toBeNull();
    expect(s.passwortfelder()).toHaveLength(0);

    s.klick(s.knopf("Trotzdem fortfahren"));
    expect(s.ziel.querySelector('[data-pruef="rueckfrage"]')).toBeNull();
    expect(s.passwortfelder()).toHaveLength(1);
    s.aufraeumen();
  });

  it("bleibt bei einem falschen Passwort stehen und leert das Feld", async () => {
    const s = einhaengen();
    await bisZumPasswort(s);
    // Die Attrappe lehnt alles unter vier Zeichen ab.
    s.tippen(s.passwortfelder()[0]!, "ab");
    await s.klickWarten(s.knopf("Sicherung einspielen"));

    expect(s.text()).toContain("passt nicht zu dieser Sicherung");
    expect(s.text()).not.toContain("Sicherung eingespielt");
    expect(s.passwortfelder()[0]!.value).toBe("");
    s.aufraeumen();
  });

  it("nennt am Ende denselben Fingerprint", async () => {
    const s = einhaengen();
    await bisZumPasswort(s);
    s.tippen(s.passwortfelder()[0]!, PASSWORT);
    await s.klickWarten(s.knopf("Sicherung einspielen"));

    const text = s.text();
    expect(text).toContain("Sicherung eingespielt");
    expect(text).toContain("Derselbe Fingerprint wie vorher");
    expect(text).toContain(IDENTITAET.fingerprint.split("-")[0]!);
    // Was nach dem Anlegen gilt, gilt hier nicht: Gesichert ist sie ja.
    expect(text).not.toContain("Zwei Dinge jetzt gleich");
    s.aufraeumen();
  });

  it("nennt die Ableitung aus der Sicherung, nicht die Voreinstellung", async () => {
    // Die Einrichtung steht auf „empfohlen“. Bei einer Sicherung hat
    // niemand gewaehlt -- was dasteht, muss aus der Datei kommen.
    class Stark extends MockBruecke {
      override async sicherungEinspielen(q: string, p: string, z: boolean) {
        const echt = await super.sicherungEinspielen(q, p, z);
        return {
          ...echt,
          identitaet: { ...echt.identitaet, kdf: "stark" as const, kdfSpeicherMib: 1024 },
        };
      }
    }
    const b = new Stark(KONTAKTE);
    void b.identitaetLoeschen();
    identitaetsspeicher.verbinde(b);

    const s = einhaengen();
    await bisZumPasswort(s);
    s.tippen(s.passwortfelder()[0]!, PASSWORT);
    await s.klickWarten(s.knopf("Sicherung einspielen"));

    expect(s.text()).toContain("Stark — 1 GiB");
    expect(s.text()).not.toContain("Empfohlen — 256 MiB");
    s.aufraeumen();
  });

  it.each([
    ["sicherung", "7 Kontakte übernommen, davon 5 verifiziert"],
    ["programmordner", "noch auf diesem Rechner lag"],
    ["unpassend", "Übernommen wurde sie nicht"],
    ["keine", "Nehmen Sie Ihre Kontakte neu auf"],
  ] as const)("sagt, woher die Kontakte kommen: %s", async (herkunft, satz) => {
    verbinden(undefined, herkunft);
    const s = einhaengen();
    await bisZumPasswort(s);
    s.tippen(s.passwortfelder()[0]!, PASSWORT);
    await s.klickWarten(s.knopf("Sicherung einspielen"));

    const kasten = s.ziel.querySelector('[data-pruef="kontakte"]');
    expect(kasten?.textContent?.replace(/\s+/g, " ")).toContain(satz);
    s.aufraeumen();
  });
});
