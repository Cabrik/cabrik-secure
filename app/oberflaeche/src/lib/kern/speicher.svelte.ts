/**
 * Der Kontaktspeicher des Prototyps.
 *
 * # Warum es ihn gibt
 *
 * Die Bildschirme lasen die Beispielkontakte bisher jeder für sich aus
 * `mock.ts`. Solange nichts veränderlich war, ging das. Sobald man einen
 * Kontakt aufnehmen oder verifizieren kann, widerspräche sich der Prototyp
 * selbst: Ein neu aufgenommener Kontakt tauchte beim Senden nicht auf.
 *
 * Ein Prototyp, dessen Teile einander widersprechen, taugt nicht zum
 * Beurteilen — und beurteilen ist der ganze Zweck von Phase 3.
 *
 * # Was er nicht ist
 *
 * Keine Datenhaltung. In Phase 4 kommt der Inhalt aus `cabrik-core`, und
 * diese Klasse wird zu dem, was die Antworten der Brücke zwischenhält. Die
 * Formen der Methoden sind deshalb schon jetzt so geschnitten, wie die
 * Brücke sie brauchen wird: eine Änderung, ein Aufruf.
 */

import { KONTAKTE } from "./mock";
import type { Kontakt, Verifikationsweg } from "./typen";

class Kontaktspeicher {
  /** Kopien, nicht die Beispieldaten selbst — sonst hielte ein Neuladen nicht. */
  liste = $state<Kontakt[]>(KONTAKTE.map((k) => ({ ...k })));

  /**
   * Nimmt einen Kontakt auf — **immer als `gesehen`**.
   *
   * Es gibt bewusst keinen Weg, hier gleich `verifiziert` zu setzen. Wer
   * eine Nutzlast einliest, hat sie erhalten, nicht geprüft. Die
   * Unterscheidung ginge sonst schon im ersten Schritt verloren.
   */
  aufnehmen(name: string, fingerprint: string, hatPostQuantum: boolean) {
    this.liste = [
      ...this.liste,
      {
        name,
        fingerprint,
        vertrauen: "gesehen",
        seit: Math.floor(Date.now() / 1000),
        verifiziertAm: null,
        verifiziertUeber: null,
        notiz: null,
        hatPostQuantum,
        safetyNumber: safetyNummerAus(fingerprint),
      },
    ];
  }

  verifizieren(fingerprint: string, weg: Verifikationsweg) {
    this.aendern(fingerprint, {
      vertrauen: "verifiziert",
      verifiziertAm: Math.floor(Date.now() / 1000),
      verifiziertUeber: weg,
    });
  }

  /**
   * Setzt einen Kontakt auf „nicht verifiziert“ zurück.
   *
   * Für den Fall, dass der Vergleich **nicht** übereinstimmt. Er wird
   * nicht widerrufen — widerrufen heißt „dieser Schlüssel ist
   * kompromittiert“, und das weiß man nicht. Man weiß nur, dass die
   * Prüfung fehlgeschlagen ist.
   */
  zuruecksetzen(fingerprint: string) {
    this.aendern(fingerprint, {
      vertrauen: "gesehen",
      verifiziertAm: null,
      verifiziertUeber: null,
    });
  }

  widerrufen(fingerprint: string) {
    this.aendern(fingerprint, { vertrauen: "widerrufen" });
  }

  private aendern(fingerprint: string, aenderung: Partial<Kontakt>) {
    this.liste = this.liste.map((k) =>
      k.fingerprint === fingerprint ? { ...k, ...aenderung } : k,
    );
  }
}

/**
 * Eine Safety Number für den Prototyp.
 *
 * **Nur zum Ansehen.** Im Kern ist sie eine paarweise Ableitung beider
 * Fingerprints, sortiert, damit beide Seiten dieselbe sehen. Das gehört
 * nach Rust und kommt in Phase 4 von dort; hier geht es allein darum, dass
 * zwölf Fünfergruppen dastehen und die Anzeige beurteilbar ist.
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

export const kontaktspeicher = new Kontaktspeicher();
