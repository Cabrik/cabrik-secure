/**
 * Der Kontaktspeicher des Prototyps.
 *
 * # Was er ist
 *
 * Ein **Zwischenhalter** über der Brücke, kein Datenspeicher. Er hält, was
 * der Kern zuletzt geantwortet hat, damit die Bildschirme etwas anzuzeigen
 * haben, ohne bei jedem Zeichnen zu fragen.
 *
 * # Warum die Methoden asynchron sind, obwohl dahinter nichts wartet
 *
 * Weil dahinter bald etwas wartet. Ein Aufruf über die Brücke kann dauern
 * und fehlschlagen, und eine Oberfläche, die synchron gebaut wurde, müsste
 * dafür an jeder Stelle aufgebrochen werden. Der Aufwand fällt jetzt an,
 * wo alles geprüft ist — nicht später, wo zusätzlich eine neue
 * Abhängigkeit im Spiel ist (Leitprinzip 2).
 *
 * # Warum er trotzdem eine Liste führt
 *
 * Svelte zeichnet synchron. Ein `{#each}` kann kein Versprechen abwarten.
 * Der Halter nimmt die Antwort entgegen und macht daraus einen Zustand —
 * das ist genau seine Aufgabe und der Grund, warum es ihn gibt.
 */

import { IDENTITAET, IDENTITAET_V1, KONTAKTE } from "./mock";
import { MockBruecke, type Bruecke } from "./bruecke";
import type { Identitaet, KdfStufe, Verifikationsweg, Kontakt } from "./typen";

class Kontaktspeicher {
  /** Was der Kern zuletzt geantwortet hat. */
  liste = $state<Kontakt[]>(KONTAKTE.map((k) => ({ ...k })));

  /**
   * Der letzte Fehler, oder `null`.
   *
   * Er wird gehalten und nicht geworfen: Ein Bildschirm, der beim Laden
   * abstürzt, sagt dem Nutzer nichts. Einer, der die Meldung anzeigt,
   * schon.
   */
  fehler = $state<string | null>(null);

  #bruecke: Bruecke;

  constructor(bruecke: Bruecke) {
    this.#bruecke = bruecke;
  }

  /** Tauscht die Brücke aus — für Tests und später für den echten Kern. */
  verbinde(bruecke: Bruecke) {
    this.#bruecke = bruecke;
  }

  /** Holt den Stand vom Kern. */
  async laden() {
    await this.fuehreAus(async () => {
      this.liste = await this.#bruecke.kontakte();
    });
  }

  /**
   * Nimmt einen Kontakt auf — **immer als `gesehen`**.
   *
   * Die Regel steht in der Brücke, nicht hier: Es gibt dort keinen
   * Parameter, mit dem sich das umgehen ließe.
   */
  async aufnehmen(name: string, fingerprint: string, hatPostQuantum: boolean) {
    await this.fuehreAus(async () => {
      await this.#bruecke.kontaktAufnehmen(name, fingerprint, hatPostQuantum);
      this.liste = await this.#bruecke.kontakte();
    });
  }

  async verifizieren(fingerprint: string, weg: Verifikationsweg) {
    await this.fuehreAus(async () => {
      await this.#bruecke.kontaktVerifizieren(fingerprint, weg);
      this.liste = await this.#bruecke.kontakte();
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
  async zuruecksetzen(fingerprint: string) {
    await this.fuehreAus(async () => {
      await this.#bruecke.kontaktZuruecksetzen(fingerprint);
      this.liste = await this.#bruecke.kontakte();
    });
  }

  async widerrufen(fingerprint: string) {
    await this.fuehreAus(async () => {
      await this.#bruecke.kontaktWiderrufen(fingerprint);
      this.liste = await this.#bruecke.kontakte();
    });
  }

  /**
   * Entfernt einen Kontakt.
   *
   * **Nicht dasselbe wie widerrufen, und die Verwechslung ist gefährlich.**
   * Widerrufen heißt: „Dieser Schlüssel ist kompromittiert“ — der Eintrag
   * bleibt und warnt künftig. Löschen heißt: „Ich kenne diese Person
   * nicht“ — der Eintrag verschwindet, **und mit ihm die Warnung**.
   */
  async loeschen(fingerprint: string) {
    await this.fuehreAus(async () => {
      await this.#bruecke.kontaktLoeschen(fingerprint);
      this.liste = await this.#bruecke.kontakte();
    });
  }

  /**
   * Führt einen Aufruf aus und behält den Fehler, statt ihn zu werfen.
   *
   * Alles an einer Stelle, weil sonst spätestens beim fünften Aufruf einer
   * ohne Behandlung durchrutscht.
   */
  private async fuehreAus(tun: () => Promise<void>) {
    try {
      await tun();
      this.fehler = null;
    } catch (e) {
      this.fehler = e instanceof Error ? e.message : String(e);
    }
  }
}

/**
 * Die eigenen Identitäten.
 *
 * Mehrere sind ausdrücklich vorgesehen: Wer aus Version 1 kommt, behält
 * den alten Schlüssel neben dem neuen, sonst wären ältere Nachrichten
 * unlesbar. Und wer getrennte Rollen führt — namentlich und anonym —,
 * braucht ohnehin zwei.
 *
 * Noch ohne Brücke: Der Kern führt die dafür nötigen Typen bisher nicht in
 * dieser Form. Sobald er es tut, kommt hier dieselbe Naht wie oben.
 */
class Identitaetsspeicher {
  liste = $state<Identitaet[]>([{ ...IDENTITAET }, { ...IDENTITAET_V1 }]);

  /**
   * Legt eine Identität an.
   *
   * Im Prototyp entsteht dabei nur ein Fingerprint zum Ansehen. Das
   * eigentliche Erzeugen — Schlüsselpaar, Argon2 über das Passwort, Datei
   * schreiben — gehört in den Kern. Das Passwort taucht hier bewusst nicht
   * auf: Es hat im Frontend nichts zu suchen und wird auch später nur
   * durchgereicht, nie gehalten.
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
      pfad: `C:\\Users\\name\\AppData\\Roaming\\Cabrik\\${bezeichnung
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

export const kontaktspeicher = new Kontaktspeicher(new MockBruecke(KONTAKTE));
export const identitaetsspeicher = new Identitaetsspeicher();
