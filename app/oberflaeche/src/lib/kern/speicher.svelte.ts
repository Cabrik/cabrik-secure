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
import { TauriBruecke, imFenster } from "./tauri";
import type {
  Identitaet,
  KdfStufe,
  Kontakt,
  Sitzungsstand,
  Sperrfrist,
  Verifikationsweg,
} from "./typen";

/**
 * Der Zustand der Sperre (`spec/entsperrung.md`).
 *
 * # Was er nicht hat
 *
 * Ein Feld für das Passwort. Es wird an [`entsperren`] übergeben, geht
 * durch die Brücke und ist danach fort — der Halter sieht es nur als
 * Argument. Das ist der Grund, warum diese Methode einen Wahrheitswert
 * zurückgibt statt das Passwort zu behalten und selbst zu wiederholen.
 *
 * # Warum er selbst einen Takt führt
 *
 * Weil die Frist im Kern abläuft, nicht hier. Ohne regelmäßiges Nachfragen
 * zeigte das Fenster „entsperrt“ an, bis jemand etwas anfasst — und der
 * erste Klick nach zwei Stunden liefe dann ins Leere, ohne dass vorher
 * irgendetwas darauf hingedeutet hätte.
 *
 * Nachfragen ist **keine** Handlung: Der Kern setzt die Messung dabei
 * nicht zurück (dafür gibt es dort einen eigenen Test). Sonst hielte sich
 * die Sitzung durch das Anzeigen ihrer eigenen Restzeit selbst offen.
 */
class Sitzungsspeicher {
  /**
   * Der Stand, oder `null` für „auf diesem Rechner liegt keine Identität“.
   *
   * Der Unterschied trägt den ganzen Bildschirmwechsel: `null` führt zur
   * Einrichtung, `gesperrt` zum Passwortfeld. Beides zu vermengen hieße,
   * jemandem ohne Schlüssel ein Passwortfeld hinzustellen.
   */
  stand = $state<Sitzungsstand | null>(null);

  /**
   * Ob überhaupt schon gefragt wurde.
   *
   * Ohne das zeigte das Fenster im ersten Augenblick die Einrichtung —
   * `stand` ist ja anfangs `null` — und spränge dann zum Sperrbildschirm.
   * Ein Aufflackern der Aufforderung „legen Sie eine Identität an“ bei
   * jemandem, der längst eine hat, ist keine Kleinigkeit.
   */
  geladen = $state(false);

  /** Was zuletzt schiefging — namentlich das falsche Passwort. */
  fehler = $state<string | null>(null);

  /** Läuft gerade eine Ableitung? Argon2 braucht spürbar Zeit. */
  arbeitet = $state(false);

  #bruecke: Bruecke;
  /** Wann zuletzt Tätigkeit gemeldet wurde — für die Drosselung. */
  #zuletztGemeldet = 0;

  constructor(bruecke: Bruecke) {
    this.#bruecke = bruecke;
  }

  verbinde(bruecke: Bruecke) {
    this.#bruecke = bruecke;
  }

  /** Fragt den Kern. Ändert nichts und gilt dort nicht als Handlung. */
  async laden() {
    try {
      this.stand = await this.#bruecke.sitzungsstand();
    } catch (e) {
      this.stand = null;
      this.fehler = e instanceof Error ? e.message : String(e);
    }
    this.geladen = true;
  }

  /**
   * Entsperrt. Gibt zurück, ob es geklappt hat.
   *
   * **Der Aufrufer leert sein Eingabefeld danach unbedingt** — auch bei
   * Fehlschlag (`spec/entsperrung.md` §5.1). Ein stehengebliebenes
   * Passwortfeld ist ein Passwort im Speicher der Webansicht, egal wie das
   * Ergebnis ausfiel.
   */
  async entsperren(passwort: string): Promise<boolean> {
    this.arbeitet = true;
    try {
      await this.#bruecke.entsperren(passwort);
      this.fehler = null;
      await this.laden();
      return true;
    } catch (e) {
      // Die Meldung kommt wörtlich aus dem Kern und sagt bewusst nicht,
      // wie falsch das Passwort war.
      this.fehler = e instanceof Error ? e.message : String(e);
      await this.laden();
      return false;
    } finally {
      this.arbeitet = false;
    }
  }

  async sperren() {
    await this.#bruecke.sperren();
    this.fehler = null;
    await this.laden();
  }

  async fristSetzen(frist: Sperrfrist) {
    await this.#bruecke.fristSetzen(frist);
    await this.laden();
  }

  /**
   * Meldet Tätigkeit — höchstens alle fünf Sekunden.
   *
   * Die Drosselung ist der Grund, warum das hier steht und nicht im
   * Bildschirm: Ungedrosselt liefe bei jedem Tastendruck ein Aufruf über
   * die Brücke, also hunderte je Absatz. Fünf Sekunden Ungenauigkeit sind
   * bei einer Frist von Minuten belanglos.
   *
   * Feuert und vergisst: Ein Fehler wird verschluckt, weil eine Meldung,
   * die beim Tippen erscheinen kann, unerträglich wäre.
   */
  taetigkeit() {
    const jetzt = Date.now();
    if (jetzt - this.#zuletztGemeldet < 5_000) return;
    this.#zuletztGemeldet = jetzt;
    void this.#bruecke.taetigkeit().catch(() => {});
  }

  /**
   * Hängt sich an Fenster und Uhr. Gibt zurück, wie man das rückgängig macht.
   *
   * **Tastatur, Klick und Rollen zählen als Tätigkeit, bloße Mausbewegung
   * nicht** (`spec/entsperrung.md` §9.2). Eine verschobene Maus sagt nicht,
   * ob noch jemand da ist — ein Ärmel oder ein angestoßener Tisch genügt.
   * Wer die Bewegung mitzählte, hätte praktisch keine Sperre mehr.
   */
  beobachten(): () => void {
    const melden = () => this.taetigkeit();
    const takt = setInterval(() => void this.laden(), 1_000);

    // `pointerdown` statt `click`: Es feuert auch dort, wo nichts anklickbar
    // ist -- und wer irgendwohin tippt, ist anwesend.
    globalThis.addEventListener("keydown", melden);
    globalThis.addEventListener("pointerdown", melden);
    globalThis.addEventListener("wheel", melden, { passive: true });

    void this.laden();

    return () => {
      clearInterval(takt);
      globalThis.removeEventListener("keydown", melden);
      globalThis.removeEventListener("pointerdown", melden);
      globalThis.removeEventListener("wheel", melden);
    };
  }
}

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
  /** Liest eine Nutzlast, ohne etwas zu veraendern. */
  async nutzlastLesen(nutzlast: string) {
    return this.#bruecke.nutzlastLesen(nutzlast);
  }

  async aufnehmen(name: string, nutzlast: string) {
    await this.fuehreAus(async () => {
      await this.#bruecke.kontaktAufnehmen(name, nutzlast);
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

/**
 * Welche Brücke gilt.
 *
 * Im Fenster der Kern, sonst die Attrappe. Die Unterscheidung steht hier
 * und an genau einer Stelle: Kein Bildschirm fragt danach, keiner darf es.
 *
 * Im Browser bleibt der Prototyp mit seinen Beispielfällen benutzbar —
 * das ist kein Übergangszustand, sondern nützlich: Die seltenen Zustände
 * lassen sich dort ansehen, ohne sie im Kern herstellen zu müssen.
 */
function bruecke(): Bruecke {
  return imFenster() ? new TauriBruecke() : new MockBruecke(KONTAKTE);
}

/**
 * Eine Brücke für alle Halter.
 *
 * Nicht je Halter eine eigene: Die Attrappe führt eine Sitzung, und zwei
 * Attrappen führten zwei — der eine Halter hielte für gesperrt, was der
 * andere für offen hält.
 */
const GETEILT = bruecke();

export const sitzungsspeicher = new Sitzungsspeicher(GETEILT);
export const kontaktspeicher = new Kontaktspeicher(GETEILT);
export const identitaetsspeicher = new Identitaetsspeicher();
