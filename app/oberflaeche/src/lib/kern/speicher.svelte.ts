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

import { IDENTITAET, IDENTITAET_V1, KONTAKTE } from "./mock";
import type { Identitaet, KdfStufe, Kontakt, Verifikationsweg } from "./typen";

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

  /**
   * Entfernt einen Kontakt.
   *
   * **Nicht dasselbe wie widerrufen, und die Verwechslung ist gefährlich.**
   * Widerrufen heißt: „Dieser Schlüssel ist kompromittiert“ — der Eintrag
   * bleibt und warnt künftig. Löschen heißt: „Ich kenne diese Person
   * nicht“ — der Eintrag verschwindet, **und mit ihm die Warnung**.
   *
   * Wer einen verdächtigen Schlüssel löscht, tritt beim nächsten Mal
   * wieder als unbekannter Absender auf und lässt sich arglos neu
   * aufnehmen. Genau davor schützt der Widerruf, und genau das nimmt das
   * Löschen zurück.
   */
  loeschen(fingerprint: string) {
    this.liste = this.liste.filter((k) => k.fingerprint !== fingerprint);
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

/**
 * Die eigenen Identitäten.
 *
 * Mehrere sind ausdrücklich vorgesehen: Wer aus Version 1 kommt, behält
 * den alten Schlüssel neben dem neuen, sonst wären ältere Nachrichten
 * unlesbar. Und wer getrennte Rollen führt — namentlich und anonym —,
 * braucht ohnehin zwei.
 */
class Identitaetsspeicher {
  liste = $state<Identitaet[]>([{ ...IDENTITAET }, { ...IDENTITAET_V1 }]);

  /**
   * Legt eine Identität an.
   *
   * Im Prototyp entsteht dabei nur ein Fingerprint zum Ansehen. Das
   * eigentliche Erzeugen — Schlüsselpaar, Argon2 über das Passwort, Datei
   * schreiben — gehört in den Kern und kommt in Phase 4 von dort. Das
   * Passwort taucht hier bewusst nicht auf: Es hat im Frontend nichts zu
   * suchen und wird auch später nur durchgereicht, nie gehalten.
   */
  anlegen(bezeichnung: string, kdf: KdfStufe, mitSignierschluessel: boolean) {
    const neu: Identitaet = {
      bezeichnung,
      fingerprint: neuerFingerprint(),
      fingerprintKurz: "",
      erzeugtAm: Math.floor(Date.now() / 1000),
      kdf,
      hatSignierschluessel: mitSignierschluessel,
      hatPostQuantum: true,
      pfad: `C:\Users\name\AppData\Roaming\Cabrik\${bezeichnung
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")}.key`,
    };
    neu.fingerprintKurz = neu.fingerprint.split(" ").slice(0, 3).join(" ");
    this.liste = [...this.liste, neu];
    return neu;
  }

  /**
   * Löscht eine Identität — der folgenschwerste Vorgang des Programms.
   *
   * Es gibt keine Sicherung beim Hersteller, keinen Wiederherstellungs-
   * schlüssel und keinen Weg zurück. Alles, was je an diesen Fingerprint
   * verschlüsselt wurde, ist danach dauerhaft unlesbar — auch das, was
   * noch gar nicht angekommen ist, denn die Gegenseite verschlüsselt
   * weiter an einen Schlüssel, den es nicht mehr gibt.
   */
  loeschen(fingerprint: string) {
    this.liste = this.liste.filter((i) => i.fingerprint !== fingerprint);
  }
}

/**
 * Ein Fingerprint zum Ansehen.
 *
 * Crockford-Base32 wie im Kern, zehn Gruppen zu vier Zeichen. Die Ziffern
 * stammen aus `crypto.getRandomValues` und nicht aus `Math.random` — nicht
 * weil hier etwas davon abhinge, sondern weil an dieser Stelle später
 * echtes Schlüsselmaterial steht und ein schwacher Zufall in einer Vorlage
 * eine schlechte Saat ist.
 */
function neuerFingerprint(): string {
  const ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
  const roh = new Uint8Array(40);
  globalThis.crypto.getRandomValues(roh);
  const zeichen = [...roh].map((b) => ALPHABET[b % 32]);
  return Array.from({ length: 10 }, (_, i) =>
    zeichen.slice(i * 4, i * 4 + 4).join(""),
  ).join(" ");
}

export const identitaetsspeicher = new Identitaetsspeicher();
