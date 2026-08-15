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
import { MockBruecke } from "../kern/bruecke";
import { KONTAKTE } from "../kern/mock";
import { abgewickelt } from "../kern/pruefstand.svelte";
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

    s.aufraeumen();
  });

  it("mehr Durchgänge sperren den Knopf nicht — es ist ein Hinweis, kein Verbot", () => {
    // Mit echten Dateien und erteilter Bestätigung. Ohne beides ist der
    // Knopf ohnehin gesperrt, und dann bewiese der Test nichts über die
    // Durchgänge.
    const kandidaten = [
      {
        pfad: "C:\Fotos\Alt.jpg",
        name: "Alt.jpg",
        groesseBytes: 1000,
        beurteilung: {
          faehigkeit: "bestEffort" as const,
          vorbehalte: [{ art: "kopienMoeglich" as const }],
        },
      },
    ];
    const s = einhaengen(Werkzeuge, { kandidaten, loeschen: () => {} });
    // Bestätigen.
    const haken = [
      ...s.ziel.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'),
    ].at(-1)!;
    s.klick(haken);

    const zahl = s.ziel.querySelector<HTMLInputElement>('input[type="number"]')!;
    s.tippen(zahl, "5");

    const knopf = s.ziel.querySelector<HTMLButtonElement>(
      '[data-pruefstelle="endgueltig-loeschen"]',
    );
    expect(knopf!.disabled, "mehr Durchgänge sind kein Verbot").toBe(false);
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
  // Frische Brücke je Test: Die Attrappe führt eine Sitzung mit genau einer
  // Identität, wie das Fenster auch.
  beforeEach(async () => {
    identitaetsspeicher.verbinde(new MockBruecke(KONTAKTE));
    await identitaetsspeicher.laden();
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
    expect(s.text()).toContain("dauerhaft unlesbar");
    s.aufraeumen();
  });

  it("hält, solange die Bezeichnung nicht abgetippt ist", () => {
    const s = beiDerAbfrage();

    expect(loeschKnopf(s)?.disabled).toBe(true);
    s.tippen(s.ziel.querySelector("input")!, "irgendwas");
    expect(loeschKnopf(s)?.disabled).toBe(true);

    expect(identitaetsspeicher.liste).toHaveLength(1);

    s.aufraeumen();
  });

  it("und geht auf, wenn sie stimmt — die Gegenprobe", async () => {
    const s = beiDerAbfrage();
    s.tippen(s.ziel.querySelector("input")!, IDENTITAET.bezeichnung!);

    expect(loeschKnopf(s)?.disabled).toBe(false);

    s.klick(loeschKnopf(s)!);
    await abgewickelt();

    expect(identitaetsspeicher.liste).toHaveLength(0);
    s.aufraeumen();
  });

  it("nimmt den Kontaktspeicher mit", async () => {
    // Er ist an die Identität versiegelt und ohne sie nicht mehr zu
    // öffnen. Ihn liegen zu lassen hieße, eine Datei zurückzulassen, die
    // niemand je wieder lesen kann und die trotzdem aussieht, als
    // enthielte sie etwas.
    const bruecke = new MockBruecke(KONTAKTE);
    identitaetsspeicher.verbinde(bruecke);
    await identitaetsspeicher.laden();
    expect(await bruecke.kontakte()).not.toHaveLength(0);

    await identitaetsspeicher.loeschen();

    expect(await bruecke.kontakte()).toHaveLength(0);
  });

  it("ohne Bezeichnung tritt der kurze Fingerprint an ihre Stelle", () => {
    // Sonst „stimmte“ ein leeres Feld, sobald man nichts eintippt — und
    // das ausgerechnet beim folgenschwersten Knopf des Programms.
    const ohneNamen = { ...IDENTITAET, bezeichnung: null };
    const s = einhaengen(Identitaet, { identitaet: ohneNamen });
    s.klick(s.knopf("Identität löschen"));

    expect(loeschKnopf(s)?.disabled).toBe(true);
    s.tippen(s.ziel.querySelector("input")!, "");
    expect(loeschKnopf(s)?.disabled).toBe(true);

    s.tippen(s.ziel.querySelector("input")!, ohneNamen.fingerprintKurz);
    expect(loeschKnopf(s)?.disabled).toBe(false);

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Die Einrichtung legt tatsächlich etwas an
// ---------------------------------------------------------------------------

describe("am Ende der Einrichtung steht eine Identität", () => {
  beforeEach(() => {
    // Ohne Identität: die Lage beim allerersten Start.
    const leer = new MockBruecke(KONTAKTE);
    void leer.identitaetLoeschen();
    identitaetsspeicher.verbinde(leer);
  });

  async function durchlaufen(bezeichnung: string | null = "Zweitrechner") {
    const s = einhaengen(Onboarding);
    // 1. Bezeichnung
    if (bezeichnung !== null) {
      s.tippen(s.ziel.querySelector("input")!, bezeichnung);
    }
    s.klick(s.knopf("Weiter"));
    // 2. Passwort
    s.tippen(s.feld("password", 0)!, "vierwortpasswortmitlaenge");
    s.tippen(s.feld("password", 1)!, "vierwortpasswortmitlaenge");
    s.klick(s.feld("checkbox", 0));
    s.klick(
      s.ziel.querySelector<HTMLButtonElement>(
        'button[data-pruefstelle="weiter"]',
      )!,
    );
    // 3. Optionen -- hier entsteht der Schlüssel.
    s.klick(
      s.ziel.querySelector<HTMLButtonElement>(
        'button[data-pruefstelle="weiter"]',
      )!,
    );
    await abgewickelt();
    return s;
  }

  it("die Identität landet im Speicher", async () => {
    // Der gemeldete Fehler: Schritt 4 erschien, aber es entstand nichts.
    // Die Ursache war, dass das Passwort abgefragt und nie weitergereicht
    // wurde -- der Aufruf hatte gar keins.
    const s = await durchlaufen();

    expect(identitaetsspeicher.liste).toHaveLength(1);
    expect(identitaetsspeicher.liste[0]!.bezeichnung).toBe("Zweitrechner");

    s.aufraeumen();
  });

  it("das Passwortfeld ist danach leer", async () => {
    // Dieselbe Zusicherung wie auf dem Sperrbildschirm: Was hier
    // stehenbliebe, wäre ein Passwort im Speicher der Webansicht — und
    // zwar so lange, wie das Fenster offen ist.
    //
    // Geprüft am **Fehlschlag**, und das ist kein Zufall: Nach dem
    // Gelingen zeigt der Bildschirm den Abschluss, und die Felder sind gar
    // nicht mehr im Dokument. Ein Test, der dort nachsähe, fände nichts
    // vor und bliebe grün, auch wenn nie etwas geleert wird — er war es,
    // bis diese Fassung ihn ersetzt hat. Nach einem Fehlschlag bleibt der
    // Nutzer im Ablauf, und genau dann zählt es.
    identitaetsspeicher.verbinde(new MockBruecke(KONTAKTE));
    await identitaetsspeicher.laden();

    const s = await durchlaufen("Noch eine");
    // Zurück zum Passwortschritt -- dort stehen die Felder.
    s.klick(s.knopf("Zurück"));

    const felder = [
      ...s.ziel.querySelectorAll<HTMLInputElement>('input[type="password"]'),
    ];
    expect(felder).toHaveLength(2);
    for (const feld of felder) expect(feld.value).toBe("");

    s.aufraeumen();
  });

  it("und ihr Fingerprint steht gleich da", async () => {
    const s = await durchlaufen();
    const neu = identitaetsspeicher.liste[0]!;

    expect(s.text()).toContain("Ihr Fingerprint");
    // Dreizehn Gruppen zu vier Zeichen, mit Bindestrich getrennt — so
    // gruppiert `Fingerprint::display_full` im Kern. Die Attrappe hatte
    // zehn Gruppen mit Leerzeichen; das sah plausibel aus und stimmte an
    // keiner Stelle.
    const gruppen = neu.fingerprint.split("-");
    expect(gruppen).toHaveLength(13);
    for (const g of gruppen) expect(g).toMatch(/^[0-9A-HJKMNP-TV-Z]{4}$/);

    s.aufraeumen();
  });

  it("ohne Bezeichnung entsteht keine erfundene", async () => {
    // Bis eben stand hier „Ohne Bezeichnung“ als Text in der Datei. Das
    // ist ein Name, den niemand vergeben hat — und er stünde dann im
    // verschlüsselten Teil der Schlüsseldatei, als hätte jemand ihn
    // gewollt.
    const s = await durchlaufen(null);

    expect(identitaetsspeicher.liste[0]!.bezeichnung).toBeNull();
    s.aufraeumen();
  });

  it("die Wahl beim Signieren wird übernommen", async () => {
    const s = einhaengen(Onboarding);
    s.klick(s.knopf("Weiter"));
    s.tippen(s.feld("password", 0)!, "vierwortpasswortmitlaenge");
    s.tippen(s.feld("password", 1)!, "vierwortpasswortmitlaenge");
    s.klick(s.feld("checkbox", 0));
    s.klick(
      s.ziel.querySelector<HTMLButtonElement>(
        'button[data-pruefstelle="weiter"]',
      )!,
    );
    // Signierschlüssel abwählen.
    s.klick(s.feld("checkbox", 0));
    s.klick(
      s.ziel.querySelector<HTMLButtonElement>(
        'button[data-pruefstelle="weiter"]',
      )!,
    );
    await abgewickelt();

    expect(identitaetsspeicher.liste[0]!.hatSignierschluessel).toBe(false);

    s.aufraeumen();
  });

  it("erst der letzte Schritt legt an, nicht schon die Eingabe", async () => {
    const s = einhaengen(Onboarding);
    s.tippen(s.ziel.querySelector("input")!, "Nichts davon");
    s.klick(s.knopf("Weiter"));
    await abgewickelt();

    expect(identitaetsspeicher.liste).toHaveLength(0);

    s.aufraeumen();
  });

  it("eine zweite Identität wird abgelehnt, statt die erste zu überschreiben", async () => {
    // Der folgenschwerste Fehlgriff, den dieses Programm zulassen könnte.
    // Die Prüfung steht auch im Fenster und in der Ablage — hier zählt,
    // dass der Bildschirm sie ZEIGT statt weiterzugehen.
    identitaetsspeicher.verbinde(new MockBruecke(KONTAKTE));
    await identitaetsspeicher.laden();

    const s = await durchlaufen("Noch eine");

    expect(s.text()).toContain("liegt bereits eine Identität");
    expect(s.text()).not.toContain("Ihr Fingerprint");
    s.aufraeumen();
  });
});
