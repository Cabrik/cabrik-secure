/**
 * Die Naht zwischen Oberfläche und Kern.
 *
 * # Warum sie vor Tauri kommt
 *
 * Leitprinzip 2 des Projekts lautet: **nie zwei Unbekannte gleichzeitig.**
 * Tauri einzuführen und im selben Zug sechs Bildschirme von Beispieldaten
 * auf echte umzuhängen, wären zwei — und wenn dann etwas nicht geht, weiß
 * niemand, an welchem von beiden es liegt.
 *
 * Also zuerst die Naht. Sie ändert an der Oberfläche genau eine Sache, und
 * zwar die einzige, die wirklich zählt:
 *
 * # Alles wird asynchron
 *
 * Das ist der strukturelle Unterschied zwischen Beispieldaten und einem
 * echten Kern, und er ist nicht nachträglich einzuziehen. Ein Aufruf über
 * die Brücke kann dauern, fehlschlagen, und zwischen Absenden und Antwort
 * kann der Nutzer weiterklicken. Eine Oberfläche, die synchron gebaut wurde,
 * müsste dafür an jeder Stelle aufgebrochen werden — genau die Art Umbau,
 * bei der Zustände wie „was ist eigentlich gerade bestätigt“ verlorengehen.
 *
 * Deshalb ist diese Schnittstelle asynchron, obwohl dahinter heute nur
 * Beispieldaten liegen. Der Aufwand fällt jetzt an, wo alles geprüft ist,
 * statt später, wo zusätzlich eine neue Abhängigkeit im Spiel ist.
 *
 * # Was nie über diese Naht geht
 *
 * Schlüsselmaterial. Kein Rückgabewert dieser Schnittstelle trägt einen
 * privaten Schlüssel oder ein abgeleitetes Geheimnis — dafür sorgt schon
 * `crates/cabrik-bruecke`, das gar keinen serialisierbaren Typ dafür führt
 * (`spec/anzeige.md` §6).
 *
 * Ein Passwort geht in die **andere** Richtung: Der Nutzer tippt es in der
 * Oberfläche, und es muss zum Kern. Es wird durchgereicht und nirgends
 * behalten — weder in einem Zustand noch in einem Zwischenspeicher. v1
 * hielt es dauerhaft im Klartext.
 */

import type { Kontakt, Nutzlastbefund, Verifikationsweg } from "./typen";
import { NUTZLASTEN } from "./mock";

/**
 * Was die Oberfläche vom Kern verlangen kann.
 *
 * Bewusst schmal: Jede Methode entspricht einer Handlung, die ein Mensch
 * auslöst. Es gibt kein allgemeines „lies mir dieses Feld“ — der Kern
 * entscheidet, was er herausgibt, nicht die Oberfläche, was sie sich holt.
 */
export interface Bruecke {
  /** Alle Kontakte des Speichers. */
  kontakte(): Promise<Kontakt[]>;

  /**
   * Liest eine Austausch-Nutzlast, **ohne** etwas aufzunehmen.
   *
   * Getrennt vom Aufnehmen, weil es zwei Vorgänge sind: erst ansehen, was
   * drinsteht, dann entscheiden. Ein Bildschirm, der beides in einem
   * Aufruf erledigt, kann den Befund gar nicht zeigen, bevor er handelt.
   */
  nutzlastLesen(nutzlast: string): Promise<Nutzlastbefund>;

  /**
   * Nimmt einen Kontakt aus einer Austausch-Nutzlast auf.
   *
   * **Immer als `gesehen`.** Es gibt keinen Parameter, mit dem sich das
   * umgehen ließe: Wer eine Nutzlast einliest, hat sie erhalten, nicht
   * geprüft. Die Unterscheidung an der ersten Stelle aufzuweichen machte
   * sie überall wertlos.
   *
   * **Die Nutzlast geht durch, nicht fertige Felder.** Aus ihr entstehen
   * die Schlüssel, und der Fingerprint wird im Kern neu berechnet — ihn
   * von hier zu übergeben hieße, dem Absender zu glauben.
   */
  kontaktAufnehmen(name: string, nutzlast: string): Promise<Kontakt>;

  /** Markiert einen Kontakt als verifiziert, mit dem benutzten Weg. */
  kontaktVerifizieren(
    fingerprint: string,
    weg: Verifikationsweg,
  ): Promise<Kontakt>;

  /**
   * Setzt einen Kontakt auf „nicht verifiziert“ zurück.
   *
   * Für den misslungenen Vergleich. **Nicht** widerrufen — das hieße
   * „dieser Schlüssel ist kompromittiert“, und das weiß niemand.
   */
  kontaktZuruecksetzen(fingerprint: string): Promise<Kontakt>;

  /** Markiert einen Schlüssel lokal als kompromittiert. */
  kontaktWiderrufen(fingerprint: string): Promise<Kontakt>;

  /**
   * Entfernt einen Kontakt.
   *
   * Nicht dasselbe wie widerrufen: Löschen entfernt den Eintrag **und mit
   * ihm jede spätere Warnung**.
   */
  kontaktLoeschen(fingerprint: string): Promise<void>;
}

// ---------------------------------------------------------------------------
// Die Umsetzung mit Beispieldaten
// ---------------------------------------------------------------------------

/**
 * Eine Safety Number für den Prototyp.
 *
 * **Nur zum Ansehen.** Im Kern ist sie eine paarweise Ableitung beider
 * Fingerprints, sortiert, damit beide Seiten dieselbe sehen. Das gehört
 * nach Rust und kommt von dort, sobald die Brücke steht.
 */
function safetyNummerAus(fingerprint: string): string {
  let wert = 0;
  for (const zeichen of fingerprint) {
    wert = (wert * 31 + zeichen.charCodeAt(0)) % 100_000;
  }
  return Array.from({ length: 12 }, (_, i) => {
    wert = (wert * 31 + i * 7919 + 13) % 100_000;
    return String(wert).padStart(5, "0");
  }).join(" ");
}

/**
 * Die Brücke mit Beispieldaten dahinter.
 *
 * Sie hält die Kontakte selbst, weil es hinter ihr noch nichts gibt, das
 * sie hielte. Sobald der Kern antwortet, wird daraus ein Aufruf — und die
 * Bildschirme merken davon nichts, weil sie schon heute nur die
 * Schnittstelle kennen.
 */
export class MockBruecke implements Bruecke {
  private daten: Kontakt[];

  constructor(anfang: readonly Kontakt[]) {
    this.daten = anfang.map((k) => ({ ...k }));
  }

  async kontakte(): Promise<Kontakt[]> {
    return this.daten.map((k) => ({ ...k }));
  }

  async nutzlastLesen(nutzlast: string): Promise<Nutzlastbefund> {
    const treffer = NUTZLASTEN.find((n) => n.text.trim() === nutzlast.trim());
    if (treffer) return treffer.befund;
    return {
      fall: "unlesbar",
      grund:
        "Das ist keine Cabrik-Austausch-Nutzlast. Sie beginnt mit " +
        "„cabrik:v2:“ und ist rund 2050 Zeichen lang.",
    };
  }

  async kontaktAufnehmen(name: string, nutzlast: string): Promise<Kontakt> {
    const befund = await this.nutzlastLesen(nutzlast);
    if (befund.fall !== "gelesen") {
      // Wie im Kern: Was sich nicht lesen laesst, wird nicht aufgenommen.
      throw new Error(befund.grund);
    }
    const neu: Kontakt = {
      name,
      fingerprint: befund.fingerprint,
      vertrauen: "gesehen",
      seit: Math.floor(Date.now() / 1000),
      verifiziertAm: null,
      verifiziertUeber: null,
      notiz: null,
      hatPostQuantum: befund.hatPostQuantum,
      safetyNumber: safetyNummerAus(befund.fingerprint),
    };
    this.daten = [...this.daten, neu];
    return { ...neu };
  }

  async kontaktVerifizieren(
    fingerprint: string,
    weg: Verifikationsweg,
  ): Promise<Kontakt> {
    return this.aendern(fingerprint, {
      vertrauen: "verifiziert",
      verifiziertAm: Math.floor(Date.now() / 1000),
      verifiziertUeber: weg,
    });
  }

  async kontaktZuruecksetzen(fingerprint: string): Promise<Kontakt> {
    return this.aendern(fingerprint, {
      vertrauen: "gesehen",
      verifiziertAm: null,
      verifiziertUeber: null,
    });
  }

  async kontaktWiderrufen(fingerprint: string): Promise<Kontakt> {
    return this.aendern(fingerprint, { vertrauen: "widerrufen" });
  }

  async kontaktLoeschen(fingerprint: string): Promise<void> {
    this.daten = this.daten.filter((k) => k.fingerprint !== fingerprint);
  }

  private aendern(fingerprint: string, teil: Partial<Kontakt>): Kontakt {
    const alt = this.daten.find((k) => k.fingerprint === fingerprint);
    if (!alt) {
      // Kein stilles Nichtstun: Ein Aufruf auf einen Kontakt, den es nicht
      // gibt, ist ein Fehler im Aufrufer — und in Phase 4 käme genau hier
      // eine Antwort des Kerns zurück, die auch nicht schweigt.
      throw new Error(`Kontakt ${fingerprint} gibt es nicht`);
    }
    const neu = { ...alt, ...teil };
    this.daten = this.daten.map((k) =>
      k.fingerprint === fingerprint ? neu : k,
    );
    return { ...neu };
  }
}
