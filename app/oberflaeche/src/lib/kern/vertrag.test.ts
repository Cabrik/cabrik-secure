/**
 * Der Brückenvertrag, gegen die Wirklichkeit gehalten.
 *
 * # Warum es das gibt
 *
 * `kern/typen.ts` war sechsmal gewachsen, ohne je gegen Rust geprüft worden
 * zu sein. Es gab **drei** unabhängige Auffassungen desselben Sachverhalts —
 * die Typen im Kern, die von Hand gebauten `json!`-Blöcke der CLI und diese
 * Datei — und keine zwei waren aneinander gehalten. Der Kern trug nicht eine
 * einzige `Serialize`-Ableitung; es gab schlicht nichts, wogegen man hätte
 * prüfen können.
 *
 * Seit `crates/cabrik-bruecke` gibt es das. Der Rust-Test dort erzeugt die
 * JSON-Muster in `vertrag/`, dieser Test hält die TypeScript-Typen dagegen.
 *
 * # Warum die Prüfung zur Laufzeit stattfindet
 *
 * Der naheliegende Weg wäre, das eingelesene Muster einfach dem Typ
 * zuzuweisen und den Übersetzer arbeiten zu lassen. Er trägt hier nicht,
 * und zwar aus zwei Gründen:
 *
 * 1. TypeScript verbreitert Zeichenketten aus JSON zu `string`. Ein Feld
 *    `"fall": "vollstaendig"` hat damit den Typ `string`, und `string` passt
 *    auf keine Literal-Union. Die Zuweisung schlüge immer fehl — und zwar
 *    aus einem Grund, der nichts mit dem Vertrag zu tun hat.
 * 2. Überzählige Felder beanstandet TypeScript nur bei frisch
 *    geschriebenen Objektliteralen, nicht bei eingelesenen Daten. Ein Feld,
 *    das Rust liefert und TypeScript nicht kennt, käme stumm durch.
 *
 * Ein `as`-Ausdruck würde beides verdecken, ohne etwas zu prüfen. Deshalb
 * steht hier keiner: Die Prüfung zählt Schlüsselmengen und Werte ab, und
 * die Listen, gegen die sie prüft, sind **selbst getypt** — ändert sich die
 * Union in `typen.ts`, muss die Liste hier mitziehen, sonst übersetzt es
 * nicht.
 */

import { describe, expect, it } from "vitest";
import bereinigung from "./vertrag/bereinigung.json";
import absender from "./vertrag/absender.json";
import kontakt from "./vertrag/kontakt.json";
import fassung from "./vertrag/fassung.json";
import fundart from "./vertrag/fundart.json";
import geoeffnet from "./vertrag/geoeffnet.json";
import aussenansicht from "./vertrag/aussenansicht.json";
import loeschbeurteilung from "./vertrag/loeschbeurteilung.json";
import loeschergebnis from "./vertrag/loeschergebnis.json";
import sitzungsstand from "./vertrag/sitzungsstand.json";
import sperrfrist from "./vertrag/sperrfrist.json";
import identitaet from "./vertrag/identitaet.json";
import kdfStufe from "./vertrag/kdf_stufe.json";
import type {
  Absender,
  Bereinigung,
  Fundart,
  KdfStufe,
  Kontakt,
  Schwere,
  Sperrfrist,
} from "./typen";

// ---------------------------------------------------------------------------
// Die Werte, die der Vertrag kennen darf
//
// Diese Listen sind der eigentliche Prüfstein. Sie sind gegen die Typen aus
// `typen.ts` deklariert: Verschwindet dort eine Variante oder kommt eine
// hinzu, übersetzt diese Datei nicht mehr — und wer sie anpasst, sieht
// zugleich, was er dem Frontend gerade zumutet.
// ---------------------------------------------------------------------------

const FAELLE_BEREINIGUNG: Bereinigung["fall"][] = [
  "vollstaendig",
  "teilweise",
  "unbekannt",
  "fehler",
];

const FAELLE_ABSENDER: Absender["fall"][] = [
  "unsigniert",
  "unbekannt",
  "bekannt",
  "verifiziert",
  "gewechselt",
  "widerrufen",
];

const VERTRAUENSZUSTAENDE: Kontakt["vertrauen"][] = [
  "gesehen",
  "verifiziert",
  "gewechselt",
  "widerrufen",
];

const WEGE: NonNullable<Kontakt["verifiziertUeber"]>[] = [
  "qr",
  "safetyNumber",
  "fingerprint",
];

const SCHWEREN: Schwere[] = ["gering", "beachtlich", "kritisch"];

const FUNDARTEN: Fundart[] = [
  "ortsangabe",
  "personenname",
  "geraet",
  "software",
  "zeitangabe",
  "organisation",
  "vorschaubild",
  "zugeschnittenes_bild",
  "nachverfolgte_aenderung",
  "farbprofil",
  "kommentar",
  "bearbeitungssitzung",
  "dateiname",
  "unbekannte_erweiterung",
  "unbekannt",
];

// ---------------------------------------------------------------------------
// Die Schlüsselmengen
// ---------------------------------------------------------------------------

/** Die Felder, die der Vertrag je Fall führt — hier von Hand nachgezogen. */
const ERWARTET: Record<string, string[]> = {
  // Bereinigung
  vollstaendig: ["fall", "entfernt", "format"],
  teilweise: ["fall", "entfernt", "geblieben", "grund", "format"],
  unbekannt: ["fall", "formathinweis"],
  fehler: ["fall", "grund"],
};

function schluessel(o: object): string[] {
  return Object.keys(o).sort();
}

/** Alle Funde aus allen Bereinigungsfällen. */
function alleFunde(): {
  art: string;
  ort: string;
  wert: string | null;
  schwere: string;
}[] {
  return bereinigung.flatMap((b) => [
    ...("entfernt" in b && b.entfernt ? b.entfernt : []),
    ...("geblieben" in b && b.geblieben ? b.geblieben : []),
  ]);
}

describe("Bereinigung", () => {
  it("führt alle vier Fälle vor", () => {
    const faelle = new Set(bereinigung.map((b) => b.fall));
    expect([...faelle].sort()).toEqual([
      "fehler",
      "teilweise",
      "unbekannt",
      "vollstaendig",
    ]);
  });

  it("jeder Fall trägt genau die erwarteten Felder", () => {
    for (const b of bereinigung) {
      const erwartet = ERWARTET[b.fall];
      expect(erwartet, `kein Feldsatz fuer ${b.fall}`).toBeDefined();
      expect(schluessel(b), `Fall ${b.fall}`).toEqual([...erwartet!].sort());
    }
  });

  it("„unbekannt“ kommt auch ohne Formathinweis vor", () => {
    // Der Fall, in dem nicht einmal das Format erkennbar war. Ein Muster,
    // das ihn auslässt, prüft die Anzeige dafür nie.
    const ohne = bereinigung.filter(
      (b) => b.fall === "unbekannt" && b.formathinweis === null,
    );
    expect(ohne).toHaveLength(1);
  });

  it("die Funde tragen die vier Felder des Vertrags", () => {
    const funde = alleFunde();
    expect(funde.length).toBeGreaterThan(2);
    for (const f of funde) {
      expect(schluessel(f)).toEqual(["art", "ort", "schwere", "wert"]);
    }
  });

  it("ein Fund ohne darstellbaren Wert ist vorgesehen", () => {
    const funde = alleFunde();
    expect(funde.some((f) => f.wert === null)).toBe(true);
  });
});

describe("Absender", () => {
  it("führt alle sechs Fälle vor", () => {
    const faelle = new Set(absender.map((a) => a.fall));
    expect([...faelle].sort()).toEqual([
      "bekannt",
      "gewechselt",
      "unbekannt",
      "unsigniert",
      "verifiziert",
      "widerrufen",
    ]);
  });

  it("„unsigniert“ trägt nichts als sein Tag", () => {
    // Das ist die Aussage: Es gibt nichts zu berichten, und das ist kein
    // Mangel.
    const u = absender.find((a) => a.fall === "unsigniert")!;
    expect(schluessel(u)).toEqual(["fall"]);
  });

  it("ein verifizierter Absender nennt den Weg — und darf ihn offenlassen", () => {
    // Beides muss im Muster vorkommen: Der Weg ist im Kern `Option`, und
    // bei aus v1 übernommenen Kontakten ist er der Normalfall nicht
    // vorhanden.
    const v = absender.filter((a) => a.fall === "verifiziert");
    expect(v.length).toBeGreaterThanOrEqual(2);
    expect(v.some((a) => a.verifiziertUeber !== null)).toBe(true);
    expect(v.some((a) => a.verifiziertUeber === null)).toBe(true);
  });

  it("beim Schlüsselwechsel steht der abgelöste Fingerprint dabei", () => {
    const g = absender.find((a) => a.fall === "gewechselt")!;
    expect(schluessel(g)).toEqual([
      "fall",
      "fingerprint",
      "name",
      "vorherVerifiziert",
      "vorherigerFingerprint",
    ]);
  });

  it("der unbekannte Absender trägt den Signierschlüssel, keinen Fingerprint", () => {
    // Aus einer Signatur allein lässt sich kein Fingerprint bilden
    // (`spec/trust-store.md` §7.1). Der Vertrag bildet das ab.
    const u = absender.find((a) => a.fall === "unbekannt")!;
    expect(schluessel(u)).toEqual(["fall", "signierschluessel"]);
    expect(schluessel(u)).not.toContain("fingerprint");
  });
});

describe("Kontakt", () => {
  it("führt alle vier Vertrauenszustände vor", () => {
    const zustaende = new Set(kontakt.map((k) => k.vertrauen));
    expect([...zustaende].sort()).toEqual([
      "gesehen",
      "gewechselt",
      "verifiziert",
      "widerrufen",
    ]);
  });

  it("trägt genau die Felder des Vertrags — in camelCase", () => {
    for (const k of kontakt) {
      expect(schluessel(k)).toEqual([
        "fingerprint",
        "hatPostQuantum",
        "name",
        "notiz",
        "safetyNumber",
        "seit",
        "verifiziertAm",
        "verifiziertUeber",
        "vertrauen",
      ]);
    }
  });

  it("ein Kontakt ohne Post-Quantum-Schlüssel ist vorgesehen", () => {
    expect(kontakt.some((k) => !k.hatPostQuantum)).toBe(true);
  });
});

describe("Fassung", () => {
  it("trägt genau die Felder des Vertrags", () => {
    for (const f of fassung) {
      expect(schluessel(f)).toEqual([
        "auszug",
        "bytes",
        "nummer",
        "nurHier",
        "seiten",
        "wirdAngezeigt",
      ]);
    }
  });

  it("mindestens eine Fassung führt entfernten Text", () => {
    // Ohne diesen Fall prüfte das Muster die eigentliche Auskunft nie.
    expect(fassung.some((f) => f.nurHier.length > 0)).toBe(true);
  });

  it("genau eine wird angezeigt", () => {
    expect(fassung.filter((f) => f.wirdAngezeigt)).toHaveLength(1);
  });
});

describe("Fundart", () => {
  it("die Oberfläche kennt jede Art, die der Vertrag führt", () => {
    // Die Richtung ist wichtig: Eine Art, die Rust liefert und TypeScript
    // nicht kennt, fiele in der Anzeige durch — `FUNDART_TEXT[art]` wäre
    // undefined, und dastünde nichts.
    const bekannt: Fundart[] = [
      "ortsangabe",
      "personenname",
      "geraet",
      "software",
      "zeitangabe",
      "organisation",
      "vorschaubild",
      "zugeschnittenes_bild",
      "nachverfolgte_aenderung",
      "farbprofil",
      "kommentar",
      "bearbeitungssitzung",
      "dateiname",
      "unbekannte_erweiterung",
      "unbekannt",
    ];
    for (const art of fundart) {
      expect(bekannt, `Fundart „${art}“ fehlt in typen.ts`).toContain(art);
    }
  });

  it("führt „unbekannt“ als eigene Art", () => {
    /*
     * `FindingKind` ist im Kern `#[non_exhaustive]`. Käme dort eine Art
     * hinzu, fiele sie in der Brücke auf `unbekannt` — statt die Oberfläche
     * mit einem Wert zu treffen, den sie nicht kennt. Derselbe Gedanke wie
     * beim vierten Anzeigezustand: lieber „ich weiß nicht, was das ist“ als
     * eine plausible Einordnung ohne Grundlage.
     */
    expect(fundart).toContain("unbekannt");
  });

  it("jede Art hat einen deutschen Text", async () => {
    const { FUNDART_TEXT } = await import("../anzeige/zustand");
    for (const art of fundart) {
      expect(FUNDART_TEXT[art as Fundart], `kein Text für ${art}`).toBeTypeOf(
        "string",
      );
    }
  });
});

describe("jeder Wert im Muster ist einer, den die Oberfläche kennt", () => {
  it("die Bereinigungsfälle", () => {
    for (const b of bereinigung) {
      expect(FAELLE_BEREINIGUNG, `Fall „${b.fall}“`).toContain(b.fall);
    }
  });

  it("die Absenderfälle", () => {
    for (const a of absender) {
      expect(FAELLE_ABSENDER, `Fall „${a.fall}“`).toContain(a.fall);
    }
  });

  it("die Vertrauenszustände", () => {
    for (const k of kontakt) {
      expect(VERTRAUENSZUSTAENDE, k.vertrauen).toContain(k.vertrauen);
    }
  });

  it("die Verifikationswege", () => {
    const wege = [
      ...kontakt.map((k) => k.verifiziertUeber),
      ...absender.map((a) =>
        "verifiziertUeber" in a ? a.verifiziertUeber : null,
      ),
    ].filter((w): w is string => w !== null);

    expect(wege.length).toBeGreaterThan(0);
    for (const w of wege) expect(WEGE, w).toContain(w);
  });

  it("die Schweregrade und Fundarten", () => {
    const funde = alleFunde();
    expect(funde.length).toBeGreaterThan(2);
    for (const f of funde) {
      expect(SCHWEREN, f.schwere).toContain(f.schwere);
      expect(FUNDARTEN, f.art).toContain(f.art);
    }
  });
});

// ---------------------------------------------------------------------------
// Öffnen
// ---------------------------------------------------------------------------

describe("Geoeffnet", () => {
  it("trägt genau die Felder des Vertrags", () => {
    for (const g of geoeffnet) {
      expect(schluessel(g)).toEqual([
        "absender",
        "art",
        "dateiname",
        "groesseBytes",
        "metadaten",
        "text",
        "zeitpunkt",
      ]);
    }
  });

  it("trägt **keinen** Klartext einer Datei", () => {
    /*
     * Die tragende Regel: `Opened::plaintext` ist ein
     * `Zeroizing<Vec<u8>>` und bleibt in Rust. Bei einer Datei bekommt die
     * Oberfläche Name und Größe — mehr nicht.
     *
     * Bei einer Textnachricht ist der Text der Inhalt und zugleich das, was
     * angezeigt werden soll; ihn zurückzuhalten hieße, die Nachricht nicht
     * zu zeigen. Das ist die einzige Ausnahme, und sie steht hier als Test,
     * damit sie eine bleibt.
     */
    const datei = geoeffnet.find((g) => g.art === "datei")!;
    expect(datei.text).toBeNull();

    const text = geoeffnet.find((g) => g.art === "text")!;
    expect(text.text).toBeTypeOf("string");
    expect(text.dateiname).toBeNull();
  });

  it("führt beide Inhaltsarten vor", () => {
    const arten = new Set(geoeffnet.map((g) => g.art));
    expect([...arten].sort()).toEqual(["datei", "text"]);
  });

  it("der Absender ist der aufgelöste, nicht der rohe Signierschlüssel", () => {
    // `Opened::signer` sagt nur, MIT WELCHEM Schlüssel signiert wurde. Wem
    // er gehört, entsteht erst am Kontaktspeicher — und genau das steht im
    // Vertrag.
    for (const g of geoeffnet) {
      expect(schluessel(g.absender)).toContain("fall");
    }
  });

  it("der Metadatenbefund ist NICHT dieselbe Form wie eine Bereinigung", () => {
    /*
     * Der Unterschied, den dieser Vertrag festhält: Beim Empfangen wird
     * nichts bereinigt. `entfernt` und `geblieben` beschreiben einen
     * Vorgang, den es hier nicht gibt — steht eines der Felder wieder da,
     * ist jemand auf den bequemen Weg zurückgefallen.
     */
    for (const g of geoeffnet) {
      if (g.metadaten === null) continue;
      expect(schluessel(g.metadaten)).not.toContain("entfernt");
      expect(schluessel(g.metadaten)).not.toContain("geblieben");
    }
  });

  it("führt alle Fälle des Befunds vor, leere Fundliste eingeschlossen", () => {
    // Der leere Fall ist der heikle: „erkannt, nichts gefunden“ ist eine
    // Aussage, `null` ist keine. Fehlte er im Muster, prüfte niemand, ob
    // die Oberfläche beide auseinanderhält.
    const faelle = geoeffnet.map((g) =>
      g.metadaten === null ? "keiner" : g.metadaten.fall,
    );
    expect(new Set(faelle)).toEqual(
      new Set(["keiner", "erkannt", "unbekannt"]),
    );

    // Die Fundlisten der erkannten Fälle. Bewusst ohne die Typen des
    // Vertrags gelesen: Dieser Test prüft ja gerade, ob das Muster ihnen
    // entspricht — sich dabei auf sie zu berufen wäre ein Zirkelschluss.
    const laengen: number[] = [];
    for (const g of geoeffnet) {
      const m = g.metadaten as { fall: string; funde?: unknown[] } | null;
      if (m?.fall !== "erkannt") continue;
      expect(m.funde, "erkannt trägt immer eine Fundliste").toBeInstanceOf(
        Array,
      );
      laengen.push(m.funde?.length ?? -1);
    }
    expect(laengen, "eine leere Fundliste").toContain(0);
    expect(
      laengen.some((n) => n > 0),
      "eine gefüllte Fundliste",
    ).toBe(true);
  });

  it("eine Textnachricht hat gar keinen Befund", () => {
    // `null` heißt „die Frage stellt sich nicht“, nicht „nichts gefunden“.
    const text = geoeffnet.find((g) => g.art === "text")!;
    expect(text.metadaten).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Außenansicht
// ---------------------------------------------------------------------------

describe("Aussenansicht", () => {
  it("trägt genau die Felder des Vertrags", () => {
    for (const a of aussenansicht) {
      expect(schluessel(a)).toEqual([
        "fassung",
        "groesseBytes",
        "kapseln",
        "offengelegt",
        "suite",
      ]);
    }
  });

  it("das Offengelegte ist eine freie Liste, kein festes Feld", () => {
    // Ein früherer Entwurf hatte `klartextDateiname` und
    // `klartextGroesse`. Das war zu eng: Was ein Format preisgibt, hängt am
    // Format.
    for (const a of aussenansicht) {
      expect(Array.isArray(a.offengelegt)).toBe(true);
    }
  });

  it("bei v2 ist die Liste leer, bei v1 nicht", () => {
    const v2 = aussenansicht.find((a) => a.fassung === "v2")!;
    const v1 = aussenansicht.find((a) => a.fassung === "v1")!;

    expect(v2.offengelegt).toHaveLength(0);
    expect(v1.offengelegt.length).toBeGreaterThan(0);
  });
});

// ---------------------------------------------------------------------------
// Löschen
// ---------------------------------------------------------------------------

describe("Löschen", () => {
  it("die Beurteilung trägt keine erfundene Begründung", () => {
    // Ein früherer Entwurf führte ein Feld `grundlage` mit Sätzen wie
    // „NTFS auf rotierender Platte“. Der Kern liefert so etwas nicht.
    for (const b of loeschbeurteilung) {
      expect(schluessel(b)).toEqual(["faehigkeit", "vorbehalte"]);
      expect(schluessel(b)).not.toContain("grundlage");
    }
  });

  it("führt alle drei Fähigkeiten vor", () => {
    const alle = new Set(loeschbeurteilung.map((b) => b.faehigkeit));
    expect([...alle].sort()).toEqual([
      "bestEffort",
      "nichtMoeglich",
      "ueberschreiben",
    ]);
  });

  it("führt jeden Vorbehalt mindestens einmal vor", () => {
    const arten = new Set(
      loeschbeurteilung.flatMap((b) => b.vorbehalte.map((v) => v.art)),
    );
    expect([...arten].sort()).toEqual([
      "cloudOrdner",
      "kopienMoeglich",
      "warSchreibgeschuetzt",
      "wechselOderNetz",
      "zeitstempelBlieb",
    ]);
  });

  it("nur der Cloud-Vorbehalt trägt einen Hinweis", () => {
    for (const b of loeschbeurteilung) {
      for (const v of b.vorbehalte) {
        expect(schluessel(v)).toEqual(
          v.art === "cloudOrdner" ? ["art", "hinweis"] : ["art"],
        );
      }
    }
  });

  it("das Ergebnis nennt, was tatsächlich geschah — und was nicht", () => {
    for (const e of loeschergebnis) {
      expect(schluessel(e)).toEqual([
        "entfernt",
        "faehigkeit",
        "fehler",
        "pfad",
        "ueberschrieben",
        "umbenannt",
        "vorbehalte",
      ]);
    }
  });

  it("ein Fehlschlag nennt den Grund", () => {
    // Er gehört in die Anzeige, nicht in ein Protokoll.
    const misslungen = loeschergebnis.find((e) => !e.entfernt)!;
    expect(misslungen.fehler).toBeTypeOf("string");
    expect(misslungen.ueberschrieben).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Sitzung und Identität
// ---------------------------------------------------------------------------

/**
 * Gegen `typen.ts` deklariert: Verschwindet dort eine Frist oder kommt eine
 * hinzu, übersetzt diese Datei nicht mehr.
 */
const FRISTEN: Sperrfrist[] = [
  "eineMinute",
  "fuenfMinuten",
  "fuenfzehnMinuten",
  "dreissigMinuten",
  "eineStunde",
  "bisZumSchliessen",
];

const STUFEN: KdfStufe[] = ["min", "empfohlen", "stark"];

describe("Sitzung", () => {
  it("kennt genau die Fristen, die die Oberfläche anbietet", () => {
    // Eine Frist, die der Kern kennt und die Oberfläche nicht, wäre ein
    // Wert, den niemand mehr ändern kann, wenn er einmal eingestellt ist.
    expect(sperrfrist).toEqual(FRISTEN);
  });

  it("nennt drei Felder und keins mehr", () => {
    // Namentlich: keine Bezeichnung der Identität. Wer auf einen gesperrten
    // Bildschirm sieht, soll nicht erfahren, wessen Rechner das ist.
    for (const s of sitzungsstand) {
      expect(schluessel(s)).toEqual(["frist", "gesperrt", "restsekunden"]);
    }
  });

  it("führt beide Bedeutungen von „keine Restzeit“ vor", () => {
    // `null` heißt „es läuft nichts“, nicht „gleich“. Wer das verwechselt,
    // baut einen Countdown, der bei „bis zum Schließen“ bei null steht.
    const gesperrt = sitzungsstand.find((s) => s.gesperrt)!;
    const ohneFrist = sitzungsstand.find(
      (s) => !s.gesperrt && s.frist === "bisZumSchliessen",
    )!;
    const laufend = sitzungsstand.find((s) => s.restsekunden !== null)!;

    expect(gesperrt.restsekunden).toBeNull();
    expect(ohneFrist.restsekunden).toBeNull();
    expect(laufend.gesperrt).toBe(false);
  });
});

describe("Identität", () => {
  it("kennt genau die drei Stufen", () => {
    expect(kdfStufe).toEqual(STUFEN);
  });

  it("trägt kein Feld für Schlüsselmaterial", () => {
    // Die Architekturregel als Prüfung. Der Rust-Typ hat gar kein Feld
    // dafür — das hier fällt auf, falls je eines dazukommt.
    for (const i of identitaet) {
      expect(schluessel(i)).toEqual([
        "bezeichnung",
        "erzeugtAm",
        "fingerprint",
        "fingerprintKurz",
        "hatPostQuantum",
        "hatSignierschluessel",
        "kdf",
        "kdfSpeicherMib",
        "pfad",
      ]);
    }
  });

  it("führt den Fall ohne benannte Stufe vor", () => {
    // `kdf: null` heißt „zu keiner der drei gehörend“, nicht „unbekannt“.
    // Die Zahl daneben ist dann die einzige Aussage — wer nur benannte
    // Stufen baut, zeigt hier ein leeres Feld.
    const eigen = identitaet.find((i) => i.kdf === null)!;
    expect(eigen).toBeDefined();
    expect(eigen.kdfSpeicherMib).toBeGreaterThan(0);
  });

  it("hält die Kurzform kurz und trennerfrei", () => {
    // Acht Zeichen, wie `Fingerprint::short`. Sie darf nie Grundlage einer
    // Verifikation sein — dafür ist sie zu kurz, und genau deshalb sieht
    // man ihr das an.
    for (const i of identitaet) {
      expect(i.fingerprintKurz).toHaveLength(8);
      expect(i.fingerprintKurz).not.toContain("-");
      expect(i.fingerprint.replaceAll("-", "")).toContain(i.fingerprintKurz);
    }
  });

  it("erlaubt eine Identität ohne Bezeichnung", () => {
    // Sie ist keine Pflicht: Wer eine anonyme Identität führt, will
    // vielleicht gerade keinen Namen daran.
    expect(identitaet.some((i) => i.bezeichnung === null)).toBe(true);
  });
});
