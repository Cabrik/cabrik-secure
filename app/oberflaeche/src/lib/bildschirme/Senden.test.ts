/**
 * „Stör nur, wenn du wirklich etwas zu sagen hast“ — ausführbar.
 *
 * Die Regel klingt einfach und ist beim Bauen leicht zu brechen: Ein
 * Bestätigungshäkchen, das immer da ist, kostet nichts und wirkt gründlich.
 * Es erzieht aber zum Wegklicken, und dann wirkt es auch dort nicht mehr,
 * wo es zählt.
 *
 * Diese Tests halten beide Hälften fest: dass **nicht** gestört wird, wenn
 * es nichts zu sagen gibt, und dass sich der Vorgang **nicht fortsetzen
 * lässt**, wenn es etwas gibt.
 */

import { describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Senden from "./Senden.svelte";
import { STAPEL } from "../kern/mock";
import type { Stapel } from "../kern/mock";
import { reaktiv } from "../kern/pruefstand.svelte";
import { brauchtEntscheidung, fasseStapel } from "../anzeige/zustand";
import type { Sendedatei } from "../kern/typen";

/**
 * Wählt den ersten Empfänger.
 *
 * Nötig, seit **kein Empfänger mehr vorausgewählt** ist: Im Prototyp war
 * der erste Kontakt angehakt, was bequem war, solange nichts wirklich
 * hinausging. Jetzt wäre es ein versehentlicher Versand an den, der
 * zufällig oben im Verzeichnis steht.
 *
 * Tests, die den Versandknopf prüfen, müssen den Empfänger deshalb
 * ausdrücklich wählen — genau wie ein Mensch.
 */
function empfaengerWaehlen(ziel: HTMLElement) {
  // Die Empfänger stehen als `<label>` mit Kästchen da, nicht als Knöpfe.
  const feld = [...ziel.querySelectorAll("label")]
    .find((l) => l.textContent?.includes("Dr. Anna Beispiel"))
    ?.querySelector("input");
  feld?.click();
  flushSync();
}

function darstellen(kennung: string) {
  const stapel = STAPEL.find((s) => s.kennung === kennung)!;
  const ziel = document.createElement("div");
  document.body.append(ziel);
  const b = mount(Senden, { target: ziel, props: { dateien: stapel.dateien, kennung: stapel.kennung } });
  empfaengerWaehlen(ziel);

  // Alles als Funktion, nicht als Momentaufnahme: Der Bildschirm hat jetzt
  // Zustand, und ein Test, der beim Einhängen abliest, prüft die Vergangenheit.
  const karten = () => [
    ...(ziel.querySelector('[data-pruefstelle="besonders"]')?.children ?? []),
  ];
  const kaestchenFuer = (name: string) =>
    ziel.querySelector<HTMLInputElement>(
      `input[type="checkbox"][aria-label="${name} mitsenden"]`,
    );

  return {
    ziel,
    text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
    /** Nur der Bereich, in dem einzeln aufgeführt wird. */
    besonders: () =>
      (
        ziel.querySelector('[data-pruefstelle="besonders"]')?.textContent ?? ""
      ).replace(/\s+/g, " "),
    /** Die Namen der einzeln aufgeführten Dateien, in Reihenfolge. */
    namen: () =>
      karten().map((k) => k.querySelector("label span")?.textContent?.trim()),
    klappe: () => ziel.querySelector("details"),
    knopf: () =>
      ziel.querySelector<HTMLButtonElement>(
        'button[data-pruefstelle="senden"]',
      ),
    sammelknopf: () =>
      [...ziel.querySelectorAll("button")].find((k) =>
        k.textContent?.includes("nicht mitsenden"),
      ),
    knopfAlle: () => [...ziel.querySelectorAll("button")],
    /** Klickt den ersten Knopf, dessen Beschriftung den Text enthält. */
    klickText: (teil: string) => {
      const k = [...ziel.querySelectorAll("button")].find((x) =>
        x.textContent?.includes(teil),
      );
      k?.click();
      flushSync();
      return k;
    },
    radio: (nr: number) =>
      [...ziel.querySelectorAll<HTMLInputElement>('input[type="radio"]')][nr],
    loeschHaken: () =>
      [
        ...ziel.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'),
      ].find((k) =>
        k.closest("label")?.textContent?.includes("sicher löschen"),
      ),
    bestaetigung: () =>
      [
        ...ziel.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'),
      ].filter((k) =>
        k.closest("label")?.textContent?.includes("Ich habe gesehen"),
      ),
    /** Nimmt eine Datei aus dem Versand oder wieder hinein. */
    umschalten: (name: string) => {
      kaestchenFuer(name)!.click();
      flushSync();
    },
    kaestchenFuer,
    aufraeumen: () => {
      unmount(b);
      ziel.remove();
    },
  };
}

// ---------------------------------------------------------------------------
// Die Regel selbst
// ---------------------------------------------------------------------------

describe("stör nur, wenn du wirklich etwas zu sagen hast", () => {
  it("ohne Befund gibt es nichts zu bestätigen", () => {
    const s = darstellen("eine-saubere");

    expect(s.text()).toContain("Es gibt nichts zu entscheiden");
    expect(s.text()).not.toContain("Ich habe gesehen");
    expect(s.knopf()?.disabled).toBe(false);

    s.aufraeumen();
  });

  it("mit Befund lässt sich nicht verschlüsseln, ohne ihn zu bestätigen", () => {
    const s = darstellen("eine-mit-rest");

    expect(s.text()).toContain("Ich habe gesehen");
    expect(s.knopf()?.disabled).toBe(true);
    expect(s.text()).toContain("Bestätigen Sie oben");

    s.aufraeumen();
  });

  it("und mit der Bestätigung geht es weiter", () => {
    // Die Gegenprobe zum vorigen Test — und sie fehlte lange. Geprüft war
    // nur, dass die Sperre HÄLT, nie dass sie sich ÖFFNET. Genau darin
    // versteckte sich ein Bildschirm, der sich gar nicht bedienen ließ:
    // Ein $effect setzte die Bestätigung bei jeder Änderung sofort wieder
    // zurück, der Knopf blieb dauerhaft gesperrt.
    const s = darstellen("eine-mit-rest");

    s.bestaetigung()[0]!.click();
    flushSync();

    expect(s.knopf()?.disabled).toBe(false);
    expect(s.text()).not.toContain("Bestätigen Sie oben");

    s.aufraeumen();
  });

  it("der Grund für das Verbleibende steht da, nicht nur die Zahl", () => {
    const s = darstellen("eine-mit-rest");

    expect(s.text()).toContain("Neuberechnen des Tons");
    expect(s.text()).toContain("Bleibt in der Datei");

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Der Stapel
// ---------------------------------------------------------------------------

describe("bei vielen Dateien wird zusammengefasst statt übersprungen", () => {
  it("das Unauffällige schrumpft auf eine Zeile, das Auffällige bleibt einzeln", () => {
    const s = darstellen("grosser-stapel");

    // 38 vollständig bereinigte: eine Zeile. Darunter fällt auch
    // Interview.wav, aus der ein Name und eine Gerätekennung entfernt
    // wurden — was weg ist, ist keine Entscheidung mehr.
    expect(s.text()).toContain("38");
    expect(s.text()).toContain("vollständig bereinigt");

    // Die drei, bei denen etwas offenbleibt: einzeln und mit Namen.
    expect(s.namen()).toEqual([
      "Uebersicht.psd",
      "DSC_0042.NEF",
      "Notiz.txt.gpg",
    ]);

    // Und die 38 gerade nicht einzeln.
    expect(s.besonders()).not.toContain("Scan_001.jpg");
    expect(s.besonders()).not.toContain("Interview.wav");

    s.aufraeumen();
  });

  it("das Zusammengefasste ist zugeklappt, aber auffindbar", () => {
    const s = darstellen("grosser-stapel");

    // Zugeklappt: es stört niemanden.
    expect(s.klappe()?.open).toBe(false);
    // Auffindbar: jede der 38 steht drin, mit der Zahl ihrer Funde.
    expect(s.klappe()?.textContent).toContain("Interview.wav");
    expect(s.klappe()?.textContent).toContain("Scan_001.jpg");
    expect(s.klappe()?.textContent).toContain("2 Funde entfernt");

    s.aufraeumen();
  });

  it("es gibt keinen Weg, die Vorschau zu überspringen", () => {
    const s = darstellen("grosser-stapel");

    for (const wort of [
      "überspringen",
      "Überspringen",
      "später",
      "ignorieren",
    ]) {
      expect(s.text()).not.toContain(wort);
    }
    expect(s.knopf()?.disabled).toBe(true);

    s.aufraeumen();
  });

  it("auch bei 41 Dateien hängt alles an einer einzigen Bestätigung", () => {
    const s = darstellen("grosser-stapel");

    expect(s.bestaetigung()).toHaveLength(1);

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Die Zusammenfassung als Funktion
// ---------------------------------------------------------------------------

describe("fasseStapel", () => {
  const datei = (name: string, befund: Sendedatei["befund"]): Sendedatei => ({
    // Im Test darf der Pfad aus dem Namen kommen: Hier geht es um die
    // Zusammenfassung, nicht um Namensgleichheit.
    pfad: `C:\Test\${name}`,
    name,
    groesseBytes: 1000,
    befund,
    fassungen: [],
  });

  const sauber = (n: string) =>
    datei(n, { fall: "vollstaendig", format: "JPEG", entfernt: [] });

  it("zählt das Unauffällige und listet das Auffällige", () => {
    const b = fasseStapel([
      sauber("a.jpg"),
      sauber("b.jpg"),
      datei("c.psd", { fall: "unbekannt", formathinweis: "PSD" }),
    ]);

    expect(b.gesamt).toBe(3);
    expect(b.vollstaendig).toBe(2);
    expect(b.auffaellig.map((d) => d.name)).toEqual(["c.psd"]);
  });

  it("ein reiner Stapel braucht keine Entscheidung", () => {
    expect(brauchtEntscheidung(fasseStapel([sauber("a.jpg")]))).toBe(false);
  });

  it("eine einzige unverstandene Datei unter vierzig genügt", () => {
    const viele = [
      ...Array.from({ length: 40 }, (_, i) => sauber(`s${i}.jpg`)),
      datei("x.psd", { fall: "unbekannt", formathinweis: "PSD" }),
    ];

    expect(brauchtEntscheidung(fasseStapel(viele))).toBe(true);
  });

  it("ein Lesefehler zählt ebenfalls als auffällig", () => {
    const b = fasseStapel([
      sauber("a.jpg"),
      datei("b.bin", { fall: "fehler", grund: "kaputt" }),
    ]);
    expect(b.auffaellig).toHaveLength(1);
  });
});

// ---------------------------------------------------------------------------
// Der Empfänger ohne Post-Quantum-Schlüssel
// ---------------------------------------------------------------------------

describe("ein Empfänger aus Version 1 zieht die ganze Nachricht herunter", () => {
  it("die Suite wird genannt und der Grund dazu", () => {
    const stapel = STAPEL[0]!;
    const ziel = document.createElement("div");
    document.body.append(ziel);
    const b = mount(Senden, { target: ziel, props: { dateien: stapel.dateien, kennung: stapel.kennung } });
  empfaengerWaehlen(ziel);

    // Voreingestellt ist der erste Kontakt, der Post-Quantum kann.
    expect(ziel.textContent).toContain("Post-Quantum-Hybrid");

    // Den v1-Kontakt hinzuwählen.
    const kaestchen = [...ziel.querySelectorAll('input[type="checkbox"]')].find(
      (k) => k.closest("label")?.textContent?.includes("Archiv"),
    ) as HTMLInputElement;
    // Svelte bündelt DOM-Änderungen. Ohne dieses Ausspülen läse der
    // Test den Stand von vor dem Klick.
    kaestchen.click();
    flushSync();

    const text = (ziel.textContent ?? "").replace(/\s+/g, " ");
    expect(text).toContain("Ohne Post-Quantum-Schutz");
    expect(text).toContain("klassisch");

    unmount(b);
    ziel.remove();
  });
});

// ---------------------------------------------------------------------------
// Der dritte Weg
// ---------------------------------------------------------------------------

/**
 * Ohne diesen Ausweg gäbe es nur zwei: alles senden oder von vorn anfangen.
 *
 * Bei einundvierzig Dateien ist „von vorn“ so teuer, dass praktisch jeder
 * das Bestätigungshäkchen setzt — und dann erzieht die Bestätigung genau zu
 * dem Wegklicken, gegen das sie gebaut ist. Der sichere Weg muss der
 * bequemere sein.
 */
describe("auffällige Dateien lassen sich einzeln vom Versand ausnehmen", () => {
  it("wer alle drei ausnimmt, muss nichts mehr bestätigen", () => {
    const s = darstellen("grosser-stapel");
    expect(s.knopf()?.disabled).toBe(true);

    for (const name of ["Uebersicht.psd", "DSC_0042.NEF", "Notiz.txt.gpg"]) {
      s.umschalten(name);
    }

    expect(s.bestaetigung()).toHaveLength(0);
    expect(s.knopf()?.disabled).toBe(false);
    expect(s.text()).toContain("Was hinausgeht, ist bereinigt");

    s.aufraeumen();
  });

  it("ein einziger Klick nimmt alle auffälligen heraus", () => {
    const s = darstellen("grosser-stapel");

    s.sammelknopf()!.click();
    flushSync();

    expect(s.knopf()?.disabled).toBe(false);
    expect(s.text()).toContain("38 von 41 Dateien");

    s.aufraeumen();
  });

  it("das Ausgenommene verschwindet nicht — es bleibt sichtbar", () => {
    const s = darstellen("grosser-stapel");
    s.umschalten("DSC_0042.NEF");

    // Weiterhin aufgeführt, und der Grund steht dabei.
    expect(s.namen()).toContain("DSC_0042.NEF");
    expect(s.besonders()).toContain("Bleibt hier");
    expect(s.besonders()).toContain("SubIFD");

    s.aufraeumen();
  });

  it("die Zählung nennt immer beide Zahlen", () => {
    const s = darstellen("grosser-stapel");
    s.umschalten("Uebersicht.psd");

    // Nicht „40 Dateien“ — das verschwiege die eine.
    expect(s.text()).toContain("40 von 41 Dateien");

    s.aufraeumen();
  });

  it("eine ausgenommene Datei lässt sich wieder aufnehmen", () => {
    const s = darstellen("grosser-stapel");

    s.sammelknopf()!.click();
    flushSync();
    expect(s.knopf()?.disabled).toBe(false);

    s.umschalten("DSC_0042.NEF");
    expect(s.bestaetigung()).toHaveLength(1);
    expect(s.knopf()?.disabled).toBe(true);

    s.aufraeumen();
  });

  it("eine schon erteilte Bestätigung gilt nicht für eine geänderte Auswahl", () => {
    const s = darstellen("grosser-stapel");

    s.bestaetigung()[0]!.click();
    flushSync();
    expect(s.knopf()?.disabled).toBe(false);

    // Eine Datei herausnehmen: Der Stapel ist ein anderer als der bestätigte.
    s.umschalten("Uebersicht.psd");

    expect(s.bestaetigung()[0]?.checked).toBe(false);
    expect(s.knopf()?.disabled).toBe(true);

    s.aufraeumen();
  });

  it("auch eine unauffällige Datei lässt sich ausnehmen", () => {
    const s = darstellen("grosser-stapel");

    // Aus der zugeklappten Sammelzeile heraus.
    s.umschalten("Interview.wav");

    // Sie wandert aus der Sammelzeile in die sichtbare Liste.
    expect(s.namen()).toContain("Interview.wav");
    expect(s.text()).toContain("37");

    s.aufraeumen();
  });

  it("bleibt nichts übrig, wird nicht verschlüsselt", () => {
    const s = darstellen("eine-mit-rest");
    s.umschalten("Mitschnitt.mp3");

    expect(s.knopf()?.disabled).toBe(true);
    expect(s.text()).toContain("es bleibt nichts zu verschlüsseln");

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Danach
// ---------------------------------------------------------------------------

/**
 * Was nach dem Verschlüsseln dasteht.
 *
 * Der Bildschirm war bis eben nicht zu beurteilen: Der Knopf tat nichts,
 * also ließ sich nicht ansehen, was er auslöst. Für einen Prototyp, dessen
 * Zweck das Beurteilen ist, war das die schlimmere Lücke.
 */
describe("nach dem Verschlüsseln", () => {
  function abgeschickt(kennung: string) {
    const s = darstellen(kennung);
    if (s.bestaetigung().length > 0) {
      s.bestaetigung()[0]!.click();
      flushSync();
    }
    s.knopf()!.click();
    flushSync();
    return s;
  }

  it("nennt Suite, Kapselzahl und Ziel", () => {
    const s = abgeschickt("eine-saubere");
    const text = s.text();

    expect(text).toContain("Verschlüsselt");
    expect(text).toContain("Protokoll.pdf.cab");
    expect(text).toContain("Post-Quantum-Hybrid");

    s.aufraeumen();
  });

  it("sagt, dass die Ausgangsdatei unverschlüsselt liegen bleibt", () => {
    // Der Satz, den Verschlüsselungswerkzeuge gern weglassen. Wer ihn nicht
    // liest, hält eine Datei für geschützt, von der der Klartext daneben
    // liegt.
    const s = abgeschickt("eine-saubere");
    const text = s.text();

    expect(text).toContain("liegen unverschlüsselt weiter da");
    expect(text).toContain("es ersetzt die erste nicht");
    expect(text).toContain("sicher ist erst, was auch gelöscht wurde");

    s.aufraeumen();
  });

  it("wiederholt, was hiergeblieben ist", () => {
    const s = darstellen("grosser-stapel");
    s.sammelknopf()!.click();
    flushSync();
    s.knopf()!.click();
    flushSync();

    expect(s.text()).toContain("3 Dateien blieben hier");

    s.aufraeumen();
  });

  it("verweist auf die Außenansicht statt Verborgenes zu behaupten", () => {
    const s = abgeschickt("eine-saubere");
    expect(s.text()).toContain("Außenansicht");
    expect(s.text()).toContain("wie viele Kapseln er trägt");
    s.aufraeumen();
  });

  it("der Rückweg führt zum unveränderten Stapel", () => {
    const s = abgeschickt("eine-saubere");
    const zurueck = [...s.knopfAlle()].find(
      (k) => k.textContent?.trim() === "Zurück",
    )!;
    zurueck.click();
    flushSync();

    expect(s.text()).toContain("Vor dem Verschlüsseln");

    s.aufraeumen();
  });
});

describe("Ausnahmen gehören zu ihrem Stapel", () => {
  /**
   * Ein gemeldeter Fehler: Wer im großen Stapel drei Dateien herausnahm und
   * dann auf „Eine Datei, alles bereinigt“ umschaltete, bekam nach dem
   * Verschlüsseln „3 Dateien blieben hier“ zu lesen — obwohl dort nichts
   * ausgenommen war.
   *
   * Dieselbe Ursache wie beim Bestätigungshäkchen: ein Zustand, der zu
   * etwas gehört, aber an nichts hängt.
   *
   * Der erste Anlauf dieses Tests prüfte die Kopfzeile und blieb deshalb
   * grün, auch mit wieder eingebautem Fehler: Bei einer Einzeldatei zeigt
   * die Kopfzeile den Dateinamen, nie die Zählung. Sichtbar wird es allein
   * im Ergebnis — also muss der Test dorthin.
   */
  function amPruefstand() {
    const gross = STAPEL.find((s) => s.kennung === "grosser-stapel")!;
    const klein = STAPEL.find((s) => s.kennung === "eine-saubere")!;
    const ziel = document.createElement("div");
    document.body.append(ziel);
    const props = reaktiv({ dateien: gross.dateien, kennung: gross.kennung });
    const b = mount(Senden, { target: ziel, props });
  empfaengerWaehlen(ziel);

    const text = () => (ziel.textContent ?? "").replace(/\s+/g, " ");
    const klick = (teil: string) => {
      [...ziel.querySelectorAll("button")]
        .find((k) => k.textContent?.includes(teil))!
        .click();
      flushSync();
    };
    return {
      text,
      klick,
      wechsleZu: (s: typeof klein) => {
        props.dateien = s.dateien;
        props.kennung = s.kennung;
        flushSync();
      },
      klein,
      gross,
      verschluesseln: () => {
        ziel
          .querySelector<HTMLButtonElement>(
            'button[data-pruefstelle="senden"]',
          )!
          .click();
        flushSync();
      },
      aufraeumen: () => {
        unmount(b);
        ziel.remove();
      },
    };
  }

  it("eine Ausnahme im einen Stapel wirkt nicht im Ergebnis des anderen", () => {
    const s = amPruefstand();

    s.klick("nicht mitsenden");
    expect(s.text()).toContain("38 von 41");

    s.wechsleZu(s.klein);
    expect(s.text()).toContain("Protokoll.pdf");

    // Hier zeigte sich der Fehler: „3 Dateien blieben hier“.
    s.verschluesseln();
    expect(s.text()).toContain("Verschlüsselt");
    expect(s.text()).not.toContain("blieben hier");
    expect(s.text()).not.toContain("blieb hier");

    s.aufraeumen();
  });

  it("und ist beim Zurückschalten wieder da", () => {
    // Die Ausnahme ist eine Entscheidung, kein Versehen — sie soll nicht
    // verlorengehen, nur weil man kurz woanders hinsieht.
    const s = amPruefstand();

    s.klick("nicht mitsenden");
    s.wechsleZu(s.klein);
    s.wechsleZu(s.gross);

    expect(s.text()).toContain("38 von 41");

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Der Befund im Sendeablauf
// ---------------------------------------------------------------------------

describe("der Befund ist von überall erreichbar", () => {
  it("auch für eine Datei, die vollständig bereinigt wurde", () => {
    // Der eigentliche Punkt: Gerade dort erfährt man sonst nie, was drin
    // war. Der Weg dorthin führt über die zugeklappte Sammelzeile.
    const s = darstellen("grosser-stapel");
    s.klickText("2 Funde entfernt");

    expect(s.text()).toContain("Gefunden");
    expect(s.text()).toContain("Welche Fassung soll hinausgehen?");

    s.aufraeumen();
  });

  it("und für jede einzeln aufgeführte", () => {
    const s = darstellen("grosser-stapel");
    s.klickText("Bericht ansehen");

    // Die erste einzeln aufgeführte ist Uebersicht.psd — ohne Befund, und
    // genau das muss dastehen statt einer leeren Liste.
    expect(s.text()).toContain("Uebersicht.psd");
    expect(s.text()).toContain("Welche Fassung soll hinausgehen?");
    expect(s.text()).toContain("Kein Befund möglich");

    s.aufraeumen();
  });
});

describe("das Original als gewählte Fassung", () => {
  it("wird im Stapel sichtbar, nicht nur im Befund", () => {
    const s = darstellen("eine-saubere");
    s.klickText("2 Funde entfernt");
    s.radio(1)!.click();
    flushSync();
    s.klickText("Schließen");

    const text = s.text();
    expect(text).toContain("Original — nichts wird entfernt");
    expect(text).toContain("unverändert hinaus");

    s.aufraeumen();
  });

  it("hebt die grüne Gesamtaussage auf", () => {
    // Grün hieße hier „alles bereinigt“ — über eine Datei, an der bewusst
    // nichts geändert wurde, wäre das eine Behauptung.
    const s = darstellen("eine-saubere");
    expect(s.text()).toContain("Es gibt nichts zu entscheiden");

    s.klickText("2 Funde entfernt");
    s.radio(1)!.click();
    flushSync();
    s.klickText("Schließen");

    expect(s.text()).not.toContain("Es gibt nichts zu entscheiden");

    s.aufraeumen();
  });

  it("erscheint nicht doppelt — einzeln und in der Sammelzeile", () => {
    const s = darstellen("grosser-stapel");
    s.klickText("2 Funde entfernt");
    s.radio(1)!.click();
    flushSync();
    s.klickText("Schließen");

    // Eine Datei weniger in der Sammelzeile, dafür einzeln aufgeführt.
    expect(s.text()).toContain("37 Dateien vollständig bereinigt");
    expect(s.namen()).toContain("Interview.wav");

    s.aufraeumen();
  });

  it("steht auch im Ergebnis noch da", () => {
    const s = darstellen("eine-saubere");
    s.klickText("2 Funde entfernt");
    s.radio(1)!.click();
    flushSync();
    s.klickText("Schließen");
    s.knopf()!.click();
    flushSync();

    expect(s.text()).toContain(
      "unverändert hinaus, mit allen gefundenen Angaben",
    );

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Löschen: eine Entscheidung, zwei Zeitpunkte
// ---------------------------------------------------------------------------

describe("die Ausgangsdateien", () => {
  it("bleiben voreingestellt liegen — und das steht da", () => {
    const s = darstellen("eine-saubere");

    expect(s.text()).toContain("Nach dem Verschlüsseln sicher löschen");
    expect(s.text()).toContain("es ersetzt die erste nicht");

    s.aufraeumen();
  });

  it("wer es vorher wählt, bekommt den Vorbehalt gleich mit", () => {
    // Ohne ihn verspricht das Häkchen mehr, als es hält: Auf SSD ist
    // Überschreiben nicht verlässlich, und das ist der Normalfall.
    const s = darstellen("eine-saubere");
    s.loeschHaken()!.click();
    flushSync();

    expect(s.text()).toContain("nicht überall verlässlich");
    expect(s.text()).toContain("versprochen wird nichts");

    s.aufraeumen();
  });

  it("und danach den Bericht statt des roten Knopfes", () => {
    const s = darstellen("eine-saubere");
    s.loeschHaken()!.click();
    flushSync();
    s.knopf()!.click();
    flushSync();

    const text = s.text();
    expect(text).toContain("gelöscht und überschrieben");
    expect(text).toContain("garantiert ist das Überschreiben nicht");
    expect(text).not.toContain("liegen unverschlüsselt weiter da");

    s.aufraeumen();
  });

  it("ohne die Wahl vorher steht der Knopf nachher da", () => {
    const s = darstellen("eine-saubere");
    s.knopf()!.click();
    flushSync();

    expect(s.text()).toContain("liegen unverschlüsselt weiter da");
    expect(
      s.knopfAlle().some((k) => k.textContent?.includes("sicher löschen")),
    ).toBe(true);

    s.aufraeumen();
  });
});

// ---------------------------------------------------------------------------
// Die formatabhängigen Entscheidungen im Stapel
// ---------------------------------------------------------------------------

describe("was im Befund gewählt wurde, steht auch im Stapel", () => {
  /**
   * Wer eine Entscheidung im Befund trifft und sie in der Übersicht nicht
   * wiederfindet, muss jede Datei einzeln aufmachen, um sich zu
   * vergewissern. Bei einundvierzig Dateien tut das niemand.
   */
  function historieBehalten() {
    const s = darstellen("eine-saubere");
    s.klickText("2 Funde entfernt");
    // Der letzte Wahlknopf im PDF-Block ist „Historie behalten“.
    const knoepfe = [
      ...s.ziel.querySelectorAll<HTMLInputElement>('input[name="revision"]'),
    ];
    knoepfe.at(-1)!.click();
    flushSync();
    return s;
  }

  it("„Historie behalten“ erscheint als Sollwert", () => {
    const s = historieBehalten();
    s.klickText("Schließen");

    expect(s.text()).toContain("Änderungshistorie bleibt");
    expect(s.text()).toContain("frühere Fassungen gehen mit");

    s.aufraeumen();
  });

  it("und hebt die grüne Gesamtaussage auf", () => {
    // „Vollständig bereinigt“ wäre falsch: In der Datei bleiben frühere
    // Fassungen mitsamt allem, was aus ihnen entfernt wurde.
    const s = darstellen("eine-saubere");
    expect(s.text()).toContain("Es gibt nichts zu entscheiden");

    s.klickText("2 Funde entfernt");
    [...s.ziel.querySelectorAll<HTMLInputElement>('input[name="revision"]')]
      .at(-1)!
      .click();
    flushSync();
    s.klickText("Schließen");

    expect(s.text()).not.toContain("Es gibt nichts zu entscheiden");

    s.aufraeumen();
  });

  it("eine gewählte Fassung erscheint mit ihrer Nummer", () => {
    const s = darstellen("eine-saubere");
    s.klickText("2 Funde entfernt");
    [
      ...s.ziel.querySelectorAll<HTMLInputElement>('input[name="revision"]'),
    ][1]!.click();
    flushSync();
    s.klickText("Schließen");

    expect(s.text()).toContain("Fassung 1 statt der angezeigten");

    s.aufraeumen();
  });

  it("die Office-Schalter erscheinen einzeln", () => {
    const s = darstellen("mit-verlauf");
    s.klickText("Bericht ansehen");

    const kaesten = [
      ...s.ziel.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'),
    ];
    for (const k of kaesten) {
      if (k.closest("label")?.textContent?.includes("Anmerkungen entfernen")) {
        k.click();
        flushSync();
      }
    }
    s.klickText("Schließen");

    expect(s.text()).toContain("Anmerkungen werden zusätzlich entfernt");

    s.aufraeumen();
  });

  it("gehören zum Stapel, nicht zum Bildschirm", () => {
    // Dieselbe Regel wie bei den Ausnahmen: ein Zustand, der zu etwas
    // gehört, muss auch daran hängen.
    const gross = STAPEL.find((s) => s.kennung === "grosser-stapel")!;
    const klein = STAPEL.find((s) => s.kennung === "eine-saubere")!;
    const ziel = document.createElement("div");
    document.body.append(ziel);
    const props = reaktiv({ dateien: klein.dateien, kennung: klein.kennung });
    const b = mount(Senden, { target: ziel, props });
  empfaengerWaehlen(ziel);

    const klick = (teil: string) =>
      [...ziel.querySelectorAll("button")]
        .find((k) => k.textContent?.includes(teil))
        ?.click();

    klick("2 Funde entfernt");
    flushSync();
    [...ziel.querySelectorAll<HTMLInputElement>('input[name="revision"]')]
      .at(-1)!
      .click();
    flushSync();
    klick("Schließen");
    flushSync();
    expect(ziel.textContent).toContain("Änderungshistorie bleibt");

    props.dateien = gross.dateien;
    props.kennung = gross.kennung;
    flushSync();
    expect(ziel.textContent).not.toContain("Änderungshistorie bleibt");

    unmount(b);
    ziel.remove();
  });
});

// ---------------------------------------------------------------------------
// Zwei Dateien, ein Name
// ---------------------------------------------------------------------------

/**
 * Der Fall, den ein echter Dateidialog jederzeit liefert.
 *
 * Solange der Bildschirm mit dem **Namen** rechnete, war er hier falsch:
 * Eine Ausnahme traf beide Dateien oder keine. Mit Beispieldaten fiel das
 * nie auf, weil dort jeder Name genau einmal vorkommt — der Fehler wartete
 * auf den ersten echten Stapel.
 */
describe("zwei Dateien mit demselben Namen", () => {
  const gleichnamig: Stapel = {
    kennung: "gleichnamig",
    titel: "Zweimal Rechnung.pdf",
    worumEsGeht: "Zwei Ordner, ein Name.",
    dateien: [
      {
        pfad: "C:\Arbeit\Rechnung.pdf",
        name: "Rechnung.pdf",
        groesseBytes: 1000,
        befund: { fall: "unbekannt", formathinweis: null },
        fassungen: [],
      },
      {
        pfad: "C:\Privat\Rechnung.pdf",
        name: "Rechnung.pdf",
        groesseBytes: 2000,
        befund: { fall: "unbekannt", formathinweis: null },
        fassungen: [],
      },
    ],
  };

  function zeigen() {
    const ziel = document.createElement("div");
    document.body.append(ziel);
    const b = mount(Senden, { target: ziel, props: { dateien: gleichnamig.dateien, kennung: gleichnamig.kennung } });
    return {
      ziel,
      text: () => (ziel.textContent ?? "").replace(/\s+/g, " ").trim(),
      kaestchen: () => [
        ...ziel.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'),
      ],
      abbauen: () => {
        unmount(b);
        ziel.remove();
      },
    };
  }

  it("unterscheidet sie in der Beschriftung durch den Ordner", () => {
    // Sonst stehen zwei identische Zeilen da, und ein Bildschirmleser
    // liest zweimal dasselbe vor.
    const s = zeigen();
    const marken = s
      .kaestchen()
      .map((k) => k.getAttribute("aria-label"))
      .filter((l) => l?.includes("Rechnung.pdf"));

    expect(new Set(marken).size, `doppeldeutig: ${marken.join(" | ")}`).toBe(
      marken.length,
    );
    expect(s.text()).toContain("Arbeit");
    expect(s.text()).toContain("Privat");
    s.abbauen();
  });

  it("nimmt nur die eine aus, nicht beide", () => {
    // Der eigentliche Fehler. Mit dem Namen als Kennung traf dieser Klick
    // beide Dateien -- und die zweite verschwand stillschweigend aus dem
    // Stapel, ohne dass jemand sie abgewählt hätte.
    const s = zeigen();
    const kaestchen = s
      .kaestchen()
      .filter((k) => k.getAttribute("aria-label")?.includes("Rechnung.pdf"));
    expect(kaestchen).toHaveLength(2);

    kaestchen[0]!.click();
    flushSync();

    const gesetzt = s
      .kaestchen()
      .filter((k) => k.getAttribute("aria-label")?.includes("Rechnung.pdf"))
      .map((k) => k.checked);
    expect(gesetzt).toEqual([false, true]);
    s.abbauen();
  });
});
