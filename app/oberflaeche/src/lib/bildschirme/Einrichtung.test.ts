/**
 * Die drei übrigen Bildschirme.
 *
 * Zwei Regeln stehen hier auf dem Spiel, die beide leicht wieder
 * verschwinden, weil ihr Gegenteil sich besser anfühlt:
 *
 * 1. **Keine Passwort-Stärkeanzeige.** Ein Balken ist schnell eingebaut,
 *    sieht kompetent aus und ist geraten.
 * 2. **Kein Zusatznutzen durch mehr Überschreibdurchgänge.** Drei Durchgänge
 *    wirken gründlicher als einer. Version 1 hatte drei voreingestellt.
 *
 * Und die Gegenprobe, die zuletzt gefehlt hat: dass die Sperren sich auch
 * wieder **öffnen**.
 */

import { beforeEach, describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import type { Component } from "svelte";
import Onboarding from "./Onboarding.svelte";
import Identitaet from "./Identitaet.svelte";
import Werkzeuge from "./Werkzeuge.svelte";
import { IDENTITAET, IDENTITAET_V1 } from "../kern/mock";
import { identitaetsspeicher } from "../kern/speicher.svelte";
import type { Identitaet as IdentitaetTyp } from "../kern/typen";

function einhaengen<P extends Record<string, unknown>>(
  bauteil: Component<P, Record<string, never>, string>,
  props: P = {} as P,
) {
  const ziel = document.createElement("div");
  document.body.append(ziel);
  const b = mount(bauteil, { target: ziel, props });
  return {
    ziel,
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    knopf: (teil: string) =>
      [...ziel.querySelectorAll("button")].find((k) =>
        k.textContent?.includes(teil),
      ),
    feld: (typ: string, nr = 0) =>
      [...ziel.querySelectorAll<HTMLInputElement>(`input[type="${typ}"]`)][nr],
    tippen: (feld: HTMLInputElement, wert: string) => {
      feld.value = wert;
      feld.dispatchEvent(new Event("input", { bubbles: true }));
      flushSync();
    },
    klick: (el: HTMLElement | undefined) => {
      el!.click();
      flushSync();
    },
    aufraeumen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

// ---------------------------------------------------------------------------
// Onboarding
// ---------------------------------------------------------------------------

describe("es gibt keine Passwort-Stärkeanzeige", () => {
  function beimPasswort() {
    const s = einhaengen(Onboarding);
    s.klick(s.knopf("Weiter"));
    return s;
  }

  it("kein Urteil über die Güte, sondern der Grund, warum keins möglich ist", () => {
    const s = beimPasswort();
    const text = s.text();

    expect(text).toContain("kann dieses Programm nicht sagen");
    expect(text).toContain("Listen");

    // Genau die Wörter, die ein Balken benutzen würde.
    for (const urteil of ["Stark", "Schwach", "Mittel", "Sehr gut", "Sicher"]) {
      expect(text).not.toContain(urteil);
    }

    s.aufraeumen();
  });

  it("nennt stattdessen, was tatsächlich hilft", () => {
    const s = beimPasswort();
    expect(s.text()).toContain("zufällig gewählte");
    s.aufraeumen();
  });

  it("verkauft die Passwortableitung nicht als Ersatz für ein gutes Passwort", () => {
    const s = beimPasswort();
    // Der Satz, der sonst nie fällt.
    expect(s.text()).toContain("macht ein erratbares Passwort");
    s.aufraeumen();
  });

  it("sagt nur, was es wirklich weiß: Länge und Übereinstimmung", () => {
    const s = beimPasswort();

    s.tippen(s.feld("password", 0)!, "kurz");
    expect(s.text()).toContain("4 Zeichen");
    expect(s.text()).toContain("mindestens 12");

    s.aufraeumen();
  });
});

describe("die Sperre im Onboarding hält — und geht auf", () => {
  function beimPasswort() {
    const s = einhaengen(Onboarding);
    s.klick(s.knopf("Weiter"));
    return s;
  }

  const weiterKnopf = (s: ReturnType<typeof einhaengen>) =>
    s.ziel.querySelector<HTMLButtonElement>(
      'button[data-pruefstelle="weiter"]',
    );

  it("hält bei zu kurzem Passwort", () => {
    const s = beimPasswort();
    s.tippen(s.feld("password", 0)!, "kurz");
    expect(weiterKnopf(s)?.disabled).toBe(true);
    s.aufraeumen();
  });

  it("hält, solange die Wiederholung nicht stimmt", () => {
    const s = beimPasswort();
    s.tippen(s.feld("password", 0)!, "vierwortpasswortmitlaenge");
    s.tippen(s.feld("password", 1)!, "etwasanderes");
    expect(weiterKnopf(s)?.disabled).toBe(true);
    expect(s.text()).toContain("stimmt nicht überein");
    s.aufraeumen();
  });

  it("hält, solange die Unwiederbringlichkeit nicht bestätigt ist", () => {
    const s = beimPasswort();
    s.tippen(s.feld("password", 0)!, "vierwortpasswortmitlaenge");
    s.tippen(s.feld("password", 1)!, "vierwortpasswortmitlaenge");

    expect(weiterKnopf(s)?.disabled).toBe(true);
    expect(s.text()).toContain("keine Wiederherstellung");

    s.aufraeumen();
  });

  it("und geht auf, wenn alles drei erfüllt ist", () => {
    // Die Gegenprobe. Ohne sie bestünde auch eine Sperre den Test, die
    // sich nie öffnet — genau das war in Senden.svelte der Fall.
    const s = beimPasswort();
    s.tippen(s.feld("password", 0)!, "vierwortpasswortmitlaenge");
    s.tippen(s.feld("password", 1)!, "vierwortpasswortmitlaenge");
    s.klick(s.feld("checkbox", 0));

    expect(weiterKnopf(s)?.disabled).toBe(false);

    // Und der Weg führt tatsächlich weiter.
    s.klick(weiterKnopf(s)!);
    expect(s.text()).toContain("Zwei Entscheidungen");

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Identität
// ---------------------------------------------------------------------------

describe("Identität", () => {
  it("nennt die Bezeichnung als das, was sie ist: lokal", () => {
    const s = einhaengen(Identitaet, { identitaet: IDENTITAET });

    expect(s.text()).toContain("bleibt bei Ihnen");
    expect(s.text()).toContain("vergibt den Namen selbst");

    s.aufraeumen();
  });

  it("sagt, dass es keine Wiederherstellung gibt — nicht, dass es schwierig sei", () => {
    const s = einhaengen(Identitaet, { identitaet: IDENTITAET });
    const text = s.text();

    expect(text).toContain("Es gibt keine Wiederherstellung");
    expect(text).toContain("auch nicht bei uns");
    // Keine Weichmacher.
    for (const wort of ["schwierig", "kaum möglich", "nur mit Aufwand"]) {
      expect(text).not.toContain(wort);
    }

    s.aufraeumen();
  });

  it("zeigt das Fehlen eines Signierschlüssels neutral, nicht als Warnung", () => {
    // Dieselbe Regel wie bei `unsigniert`: ein gewählter Modus, kein Mangel.
    const s = einhaengen(Identitaet, { identitaet: IDENTITAET_V1 });

    expect(s.text()).toContain("Ohne Signierschlüssel");
    expect(s.text()).toContain("gewählter Modus");

    s.aufraeumen();
  });

  it("benennt beim v1-Schlüssel die fehlende Post-Quantum-Deckung", () => {
    const s = einhaengen(Identitaet, { identitaet: IDENTITAET_V1 });
    expect(s.text()).toContain("Kein Post-Quantum-Schlüssel");
    s.aufraeumen();
  });

  it("bietet nirgends an, den privaten Schlüssel zu zeigen", () => {
    for (const id of [IDENTITAET, IDENTITAET_V1] as IdentitaetTyp[]) {
      const s = einhaengen(Identitaet, { identitaet: id });
      s.klick(s.knopf("Weitergeben"));

      const text = s.text().toLowerCase();
      for (const wendung of ["privaten schlüssel", "geheimen schlüssel"]) {
        expect(text).not.toContain(wendung);
      }
      // Und die Nutzlast wird ausdrücklich als öffentlich bezeichnet.
      expect(s.text()).toContain("ausschließlich öffentliche");

      s.aufraeumen();
    }
  });
});

// ---------------------------------------------------------------------------
// Werkzeuge
// ---------------------------------------------------------------------------

describe("sicheres Löschen sagt, was es nicht erreicht", () => {
  it("der Normalfall ist gelb, aber mit Grund statt mit Vorwurf", () => {
    const s = einhaengen(Werkzeuge);
    s.klick(s.knopf("Notizen.txt"));

    const text = s.text();
    expect(text).toContain("nicht verlässlich");
    expect(text).toContain("Normalfall");
    expect(text).toContain("kein Fehler des Programms");

    s.aufraeumen();
  });

  it("beim Cloud-Ordner steht, woran es erkannt wurde", () => {
    const s = einhaengen(Werkzeuge);
    s.klick(s.knopf("Liste.xlsx"));

    expect(s.text()).toContain("Synchronisationsordner");
    expect(s.text()).toContain("Erkannt an");
    expect(s.text()).toContain("Serverkopien");

    s.aufraeumen();
  });

  it("mehr Durchgänge werden nicht verboten, aber auch nicht beschwiegen", () => {
    const s = einhaengen(Werkzeuge);
    const zahl = s.ziel.querySelector<HTMLInputElement>(
      'input[type="number"]',
    )!;

    // Voreinstellung: einer. Kein Hinweis nötig.
    expect(zahl.value).toBe("1");
    expect(s.text()).not.toContain("keinen Zusatznutzen");

    s.tippen(zahl, "3");

    expect(s.text()).toContain("keinen Zusatznutzen");
    expect(s.text()).toContain("Gutmann");
    // Und woher die Zahl drei kommt.
    expect(s.text()).toContain("Version 1 hatte drei voreingestellt");

    // Der Knopf bleibt trotzdem benutzbar — es ist ein Hinweis, kein Verbot.
    expect(s.knopf("Endgültig löschen")?.disabled).toBeFalsy();

    s.aufraeumen();
  });

  it("die Außenansicht sagt, was Verschlüsselung nicht verbirgt", () => {
    const s = einhaengen(Werkzeuge);
    s.klick(s.knopf("Außenansicht"));

    expect(s.text()).toContain("verbirgt den Inhalt, nicht den Vorgang");

    s.aufraeumen();
  });
});

describe("die Außenansicht zeigt den Unterschied zwischen den Fassungen", () => {
  it("bei Version 2 ist nur die Kapselzahl sichtbar", () => {
    const s = einhaengen(Werkzeuge);
    s.klick(s.knopf("Außenansicht"));

    expect(s.text()).toContain("Nichts als die Kapselzahl");
    expect(s.text()).not.toContain("Im Klartext lesbar");

    s.aufraeumen();
  });

  it("bei Version 1 steht der Dateiname im Klartext da", () => {
    // Der eigentliche Grund für dieses Werkzeug: Wer eine alte Datei
    // weitergibt, soll sehen, was sie über sich verrät.
    const s = einhaengen(Werkzeuge);
    s.klick(s.knopf("Außenansicht"));
    s.klick(s.knopf("Eine Datei aus Version 1"));

    expect(s.text()).toContain("Der Kopf steht im Klartext");
    expect(s.text()).toContain("Kuendigung-Mueller.pdf");

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Identität löschen
// ---------------------------------------------------------------------------

/**
 * Der folgenschwerste Vorgang des Programms.
 *
 * Ein Häkchen wäre hier zu billig — es erzieht zum Wegklicken, und dies ist
 * der eine Fall, bei dem Wegklicken nicht passieren darf. Deshalb muss die
 * Bezeichnung abgetippt werden: Wer sie abschreibt, hat sie gelesen.
 */
describe("eine Identität zu löschen verlangt mehr als einen Klick", () => {
  const ANFANG = identitaetsspeicher.liste.map((i) => ({ ...i }));
  beforeEach(() => {
    identitaetsspeicher.liste = ANFANG.map((i) => ({ ...i }));
  });

  const loeschKnopf = (s: ReturnType<typeof einhaengen>) =>
    s.ziel.querySelector<HTMLButtonElement>(
      'button[data-pruefstelle="identitaet-loeschen"]',
    );

  function beiDerAbfrage() {
    const s = einhaengen(Identitaet, { identitaet: IDENTITAET });
    s.klick(s.knopf("Identität löschen"));
    return s;
  }

  it("sagt vorher, dass danach alles unlesbar ist", () => {
    const s = beiDerAbfrage();
    const text = s.text();

    expect(text).toContain("Danach ist alles dauerhaft unlesbar");
    expect(text).toContain("auch nicht von uns");
    expect(text).toContain("keinen Wiederherstellungsschlüssel");

    s.aufraeumen();
  });

  it("nennt die Folge für die Gegenseite", () => {
    const s = beiDerAbfrage();
    // Der Punkt, den man leicht übersieht: Die anderen verschlüsseln weiter
    // an einen Schlüssel, den es nicht mehr gibt.
    expect(s.text()).toContain("verschlüsseln weiter an ihn");
    s.aufraeumen();
  });

  it("weist auf den Weg hin, der meistens gemeint ist", () => {
    const s = beiDerAbfrage();
    expect(s.text()).toContain("Legen Sie eine zweite Identität an");
    s.aufraeumen();
  });

  it("hält, solange die Bezeichnung nicht abgetippt ist", () => {
    const s = beiDerAbfrage();

    expect(loeschKnopf(s)?.disabled).toBe(true);
    s.tippen(s.ziel.querySelector("input")!, "irgendwas");
    expect(loeschKnopf(s)?.disabled).toBe(true);

    expect(identitaetsspeicher.liste).toHaveLength(2);

    s.aufraeumen();
  });

  it("und geht auf, wenn sie stimmt — die Gegenprobe", () => {
    const s = beiDerAbfrage();
    s.tippen(s.ziel.querySelector("input")!, IDENTITAET.bezeichnung);

    expect(loeschKnopf(s)?.disabled).toBe(false);

    s.klick(loeschKnopf(s)!);
    expect(identitaetsspeicher.liste).toHaveLength(1);
    expect(
      identitaetsspeicher.liste.some(
        (i) => i.fingerprint === IDENTITAET.fingerprint,
      ),
    ).toBe(false);

    s.aufraeumen();
  });

  it("ein Häkchen gibt es nicht — Abtippen ist der einzige Weg", () => {
    const s = beiDerAbfrage();
    const kaesten = s.ziel.querySelectorAll('input[type="checkbox"]');
    expect(kaesten).toHaveLength(0);
    s.aufraeumen();
  });
});
