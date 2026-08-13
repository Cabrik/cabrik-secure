/**
 * Der Moment, in dem ein fremder Schlüssel in den Speicher kommt.
 *
 * Alles, was später über Vertrauen angezeigt wird, hängt daran, dass hier
 * nichts behauptet wird, was nicht geprüft wurde. Deshalb steht die
 * wichtigste Prüfung nicht in der Anzeige, sondern im Speicher: Es darf
 * **keinen Weg** geben, einen Kontakt gleich als verifiziert aufzunehmen.
 */

import { beforeEach, describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Aufnehmen from "./Aufnehmen.svelte";
import { NUTZLASTEN } from "../kern/mock";
import { kontaktspeicher } from "../kern/speicher.svelte";
import { MockBruecke } from "../kern/bruecke";
import { abgewickelt } from "../kern/pruefstand.svelte";

function darstellen() {
  const ziel = document.createElement("div");
  document.body.append(ziel);
  let letzterAusgang: string | null | undefined;
  const b = mount(Aufnehmen, {
    target: ziel,
    props: {
      fertig: (fp: string | null) => {
        letzterAusgang = fp;
      },
    },
  });
  const knopf = (teil: string) =>
    [...ziel.querySelectorAll("button")].find((k) =>
      k.textContent?.includes(teil),
    );
  return {
    ziel,
    knopf,
    ausgang: () => letzterAusgang,
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    befund: () =>
      (
        ziel.querySelector('[data-pruefstelle="befund"]')?.textContent ?? ""
      ).replace(/\s+/g, " "),
    aufnehmenKnopf: () =>
      ziel.querySelector<HTMLButtonElement>(
        'button[data-pruefstelle="aufnehmen"]',
      ),
    /** Wählt eine Beispielnutzlast über ihren Knopf. */
    waehlen: (titel: string) => {
      knopf(titel)!.click();
      flushSync();
    },
    tippen: (feld: HTMLInputElement | HTMLTextAreaElement, wert: string) => {
      feld.value = wert;
      feld.dispatchEvent(new Event("input", { bubbles: true }));
      flushSync();
    },
    namensfeld: () =>
      ziel.querySelector<HTMLInputElement>(
        'input[type="text"], input:not([type])',
      )!,
    textfeld: () => ziel.querySelector("textarea")!,
    aufraeumen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

const ANFANG = kontaktspeicher.liste.map((k) => ({ ...k }));

// Eine frische Bruecke je Test: Der Stand liegt jetzt DAHINTER, nicht mehr
// in der Liste. Nur die Liste zurueckzusetzen liesse die Bruecke mit den
// alten Daten stehen -- und der naechste Aufruf holte sie wieder.
beforeEach(async () => {
  kontaktspeicher.verbinde(new MockBruecke(ANFANG));
  await kontaktspeicher.laden();
});

// ---------------------------------------------------------------------------
// Die Regel, die alles trägt
// ---------------------------------------------------------------------------

describe("ein aufgenommener Kontakt beginnt als „nicht verifiziert“", () => {
  it("der Speicher kennt keinen anderen Weg", async () => {
    // Die eigentliche Absicherung: Nicht die Anzeige entscheidet das,
    // sondern die einzige Methode, die es zum Aufnehmen gibt.
    await kontaktspeicher.aufnehmen("Neu", "AAAA BBBB", true);
    const neu = kontaktspeicher.liste.at(-1)!;

    expect(neu.vertrauen).toBe("gesehen");
    expect(neu.verifiziertAm).toBeNull();
    expect(neu.verifiziertUeber).toBeNull();
  });

  it("und der Bildschirm sagt es, bevor man klickt", () => {
    const s = darstellen();
    s.waehlen("Vollständig");

    expect(s.text()).toContain("Wird als „nicht verifiziert“ aufgenommen");
    expect(s.text()).toContain("So fängt jeder Kontakt an");

    s.aufraeumen();
  });

  it("es gibt kein Häkchen, das das überspringt", () => {
    const s = darstellen();
    s.waehlen("Vollständig");

    const text = s.text();
    for (const wendung of [
      "als verifiziert markieren",
      "bereits verifiziert",
      "Verifikation überspringen",
    ]) {
      expect(text).not.toContain(wendung);
    }

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Der Name
// ---------------------------------------------------------------------------

describe("der Name ist Ihrer, nicht seiner", () => {
  it("das steht dabei, nicht im Kleingedruckten", () => {
    const s = darstellen();
    s.waehlen("Vollständig");

    expect(s.text()).toContain("Die Nutzlast trägt keinen Namen");
    expect(s.text()).toContain("Ihre Notiz an sich selbst");

    s.aufraeumen();
  });

  it("ohne Namen wird nicht aufgenommen", () => {
    const s = darstellen();
    s.waehlen("Vollständig");

    expect(s.aufnehmenKnopf()?.disabled).toBe(true);

    s.aufraeumen();
  });

  it("mit Namen schon — die Gegenprobe", async () => {
    const s = darstellen();
    s.waehlen("Vollständig");
    s.tippen(s.namensfeld(), "Neue Zuträgerin");

    expect(s.aufnehmenKnopf()?.disabled).toBe(false);

    const vorher = kontaktspeicher.liste.length;
    s.knopf("Aufnehmen")!.click();
    await abgewickelt();

    expect(kontaktspeicher.liste).toHaveLength(vorher + 1);
    expect(kontaktspeicher.liste.at(-1)!.name).toBe("Neue Zuträgerin");
    expect(s.ausgang()).toBe(kontaktspeicher.liste.at(-1)!.fingerprint);

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Die Prüfsumme
// ---------------------------------------------------------------------------

describe("die Prüfsumme ist keine Sicherheitsprüfung", () => {
  it("ihr Gelingen wird nirgends als Erfolg gemeldet", () => {
    const s = darstellen();
    s.waehlen("Vollständig");

    // Es gibt kein grünes „Prüfsumme stimmt“ — das läse sich wie
    // „Absender bestätigt“ und wäre das glatte Gegenteil.
    const text = s.text();
    expect(text).not.toContain("Prüfsumme stimmt");
    expect(text).not.toContain("Prüfsumme in Ordnung");

    s.aufraeumen();
  });

  it("dafür steht da, dass der Fingerprint neu berechnet wurde", () => {
    const s = darstellen();
    s.waehlen("Vollständig");

    expect(s.text()).toContain("neu berechnet");
    expect(s.text()).toContain(
      "sagt nichts darüber, wer die Nutzlast geschickt hat",
    );

    s.aufraeumen();
  });

  it("ihr Scheitern heißt Übertragungsfehler, nicht Angriff", () => {
    const s = darstellen();
    s.waehlen("Beim Kopieren abgeschnitten");

    const text = s.befund();
    expect(text).toContain("unvollständig angekommen");
    expect(text).toContain("Übertragungsfehler");
    // Gerade nicht:
    expect(text).not.toContain("manipuliert");
    expect(text).not.toContain("Angriff");

    // Und aufnehmen lässt sich nichts.
    expect(s.aufnehmenKnopf()).toBeNull();

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Die Sonderfälle in der Nutzlast
// ---------------------------------------------------------------------------

describe("was in der Nutzlast fehlt, wird benannt", () => {
  it("ohne Post-Quantum-Schlüssel: Warnung mit Grund", () => {
    const s = darstellen();
    s.waehlen("Ohne Post-Quantum-Schlüssel");

    expect(s.befund()).toContain("nur klassisch");
    expect(s.befund()).toContain("Quantenrechner");

    s.aufraeumen();
  });

  it("ohne Signierschlüssel: neutral, nicht als Mangel", () => {
    const s = darstellen();
    s.waehlen("Ohne Signierschlüssel");

    expect(s.befund()).toContain("Ohne Signierschlüssel");
    expect(s.befund()).toContain("gewählter Modus");

    s.aufraeumen();
  });

  it("bekannter Kontakt mit anderem Schlüssel: der ernste Fall", () => {
    const s = darstellen();
    s.waehlen("Bekannter Kontakt, anderer Schlüssel");

    const text = s.befund();
    expect(text).toContain("bereits einen anderen Schlüssel");
    expect(text).toContain("oder jemand anders");
    expect(text).toContain("nicht über dieses Programm hergestellt haben");

    s.aufraeumen();
  });

  it("etwas Fremdes im Feld ist kein Absturz, sondern ein Satz", () => {
    const s = darstellen();
    s.tippen(s.textfeld(), "irgendein Text aus der Zwischenablage");

    expect(s.befund()).toContain("keine Cabrik-Austausch-Nutzlast");
    expect(s.aufnehmenKnopf()).toBeNull();

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Der Speicher
// ---------------------------------------------------------------------------

describe("der Kontaktspeicher", () => {
  const bert = () =>
    kontaktspeicher.liste.find((k) => k.name === "Bert Muster")!;

  it("ein geglückter Vergleich verifiziert mit dem benutzten Weg", async () => {
    await kontaktspeicher.verifizieren(bert().fingerprint, "safetyNumber");

    expect(bert().vertrauen).toBe("verifiziert");
    expect(bert().verifiziertUeber).toBe("safetyNumber");
    expect(bert().verifiziertAm).toBeTypeOf("number");
  });

  it("ein misslungener Vergleich widerruft NICHT", async () => {
    // Widerrufen hieße „dieser Schlüssel ist kompromittiert“ — das weiß
    // niemand. Bekannt ist nur, dass die Prüfung fehlgeschlagen ist.
    await kontaktspeicher.verifizieren(bert().fingerprint, "safetyNumber");
    await kontaktspeicher.zuruecksetzen(bert().fingerprint);

    expect(bert().vertrauen).toBe("gesehen");
    expect(bert().vertrauen).not.toBe("widerrufen");
    expect(bert().verifiziertUeber).toBeNull();
  });

  it("jeder Beispielfall hat eine eigene Nutzlast", () => {
    const texte = new Set(NUTZLASTEN.map((n) => n.text));
    expect(texte.size).toBe(NUTZLASTEN.length);
  });
});
