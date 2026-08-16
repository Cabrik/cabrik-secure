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

import { KONTAKTE } from "./mock";
import { MockBruecke, type Bruecke } from "./bruecke";
import { TauriBruecke, imFenster } from "./tauri";
import type {
  Fortschrittsmelder,
  Stapelart,
  Stapelstand,
  Geoeffnet,
  QrCode,
  Loeschergebnis,
  Loeschkandidat,
  Identitaet,
  KdfStufe,
  Kontakt,
  Sendedatei,
  Sitzungsstand,
  Speicherergebnis,
  Versandbericht,
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
  /**
   * Lädt eine Austausch-Nutzlast aus einer Datei.
   *
   * Gibt den Text **zurück**, statt ihn zu behalten: Er gehört ins
   * Eingabefeld des Bildschirms und geht von dort durch dieselbe Prüfung
   * wie eine von Hand eingefügte. Zwei Wege herein dürfen nicht zu zwei
   * Urteilen führen.
   *
   * `null` heißt abgebrochen.
   */
  async nutzlastAusDatei(): Promise<string | null> {
    try {
      const text = await this.#bruecke.nutzlastAusDatei();
      this.fehler = null;
      return text;
    } catch (e) {
      this.fehler = e instanceof Error ? e.message : String(e);
      return null;
    }
  }

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
 * # Warum eine Liste, obwohl es genau eine gibt
 *
 * Weil es später mehrere sein werden, und weil die Form dann schon stimmt.
 * `spec/entsperrung.md` §7 hält die Regeln bereits fest: Entsperrt wird
 * **eine** Identität, nicht „der Speicher“; ein Wechsel sperrt die
 * vorherige.
 *
 * Der Grund dafür ist keine Bequemlichkeit, sondern eine Trennung: Der
 * Kontaktspeicher ist an die Identität versiegelt. Wer eine namentliche und
 * eine anonyme führt und unter beiden dieselben Kontakte hätte, hätte sie
 * damit verknüpft — und die anonyme wäre keine mehr.
 *
 * Heute führt die Ablage genau einen Pfad. Die Liste hat deshalb einen
 * Eintrag oder keinen. Was fehlt, ist die **Ablage**, nicht die Oberfläche —
 * und das ist die billigere Hälfte, wenn sie an der Reihe ist.
 */
class Identitaetsspeicher {
  /** Was der Kern zuletzt geantwortet hat. Leer heißt: noch keine angelegt. */
  liste = $state<Identitaet[]>([]);

  fehler = $state<string | null>(null);

  #bruecke: Bruecke;

  constructor(bruecke: Bruecke) {
    this.#bruecke = bruecke;
  }

  verbinde(bruecke: Bruecke) {
    this.#bruecke = bruecke;
    this.liste = [];
    this.nutzlast = null;
    this.nutzlastNach = null;
  }

  /**
   * Holt den Stand vom Kern.
   *
   * # Warum der Fehler hier stehenbleibt
   *
   * Weil er es bis eben nicht tat. Diese Methode fing jeden Fehlschlag ab
   * und setzte die Liste stumm auf leer — und damit sah eine gescheiterte
   * Abfrage genauso aus wie „es gibt noch keine Identität“. Der Bildschirm
   * zeigte „Keine Identität vorhanden“ und nannte keinen Grund; im Fenster
   * geschah das eine Sekunde nach dem Anlegen, also lange nach jedem
   * Klick.
   *
   * Ein Halter, der Fehler verschluckt, macht jeden späteren Fehler
   * unauffindbar. Das ist der teuerste Bequemlichkeitsfehler überhaupt.
   *
   * **Der Aufrufer ruft das nur bei entsperrter Sitzung auf.** Sonst wäre
   * „gesperrt“ ein Fehler, obwohl nichts vorgefallen ist — siehe
   * [`vergiss`].
   */
  async laden() {
    try {
      this.liste = [await this.#bruecke.identitaet()];
      this.fehler = null;
    } catch (e) {
      this.liste = [];
      this.fehler = e instanceof Error ? e.message : String(e);
    }
  }

  /**
   * Vergisst, was da war — ohne das für einen Fehler zu halten.
   *
   * Für den gesperrten Zustand und für den Rechner ohne Identität. Beides
   * ist kein Zwischenfall, sondern eine Lage, und der Weg führt dann zum
   * Passwortfeld oder zur Einrichtung.
   */
  vergiss() {
    this.liste = [];
    this.fehler = null;
  }

  /**
   * Legt eine Identität an.
   *
   * Das Passwort wird **durchgereicht, nicht gehalten** — weder hier noch
   * in der Brücke gibt es ein Feld dafür. Der Bildschirm leert sein
   * Eingabefeld unmittelbar nach diesem Aufruf.
   *
   * Gibt zurück, ob es geklappt hat. Der Fehler bleibt in `fehler` stehen,
   * statt geworfen zu werden: Ein Bildschirm, der beim Anlegen abstürzt,
   * sagt dem Nutzer nichts.
   */
  async anlegen(
    bezeichnung: string | null,
    passwort: string,
    kdf: KdfStufe,
    mitSignierschluessel: boolean,
  ): Promise<Identitaet | null> {
    try {
      const neu = await this.#bruecke.identitaetAnlegen(
        bezeichnung,
        passwort,
        mitSignierschluessel,
        kdf,
      );
      this.liste = [neu];
      this.fehler = null;
      return neu;
    } catch (e) {
      this.fehler = e instanceof Error ? e.message : String(e);
      return null;
    }
  }

  /**
   * Die eigene Austausch-Nutzlast. `null` heißt: noch nicht geholt.
   *
   * Sie wird auf Verlangen geholt, nicht beim Laden: Wer sie nie
   * weitergibt, braucht sie nie zu sehen.
   */
  nutzlast = $state<string | null>(null);

  /** Wohin sie zuletzt geschrieben wurde. */
  nutzlastNach = $state<string | null>(null);

  async nutzlastHolen() {
    try {
      this.nutzlast = await this.#bruecke.eigeneNutzlast();
      this.fehler = null;
    } catch (e) {
      this.fehler = e instanceof Error ? e.message : String(e);
    }
  }

  /** Der QR-Code zur eigenen Nutzlast. `null` heißt: noch nicht geholt. */
  qr = $state<QrCode | null>(null);

  async qrHolen() {
    try {
      this.qr = await this.#bruecke.nutzlastAlsQr();
      this.fehler = null;
    } catch (e) {
      this.fehler = e instanceof Error ? e.message : String(e);
    }
  }

  /** Nimmt den Code wieder weg. */
  qrSchliessen() {
    this.qr = null;
  }

  async nutzlastSpeichern() {
    try {
      const ziel = await this.#bruecke.nutzlastAlsDatei();
      if (ziel !== null) this.nutzlastNach = ziel;
      this.fehler = null;
    } catch (e) {
      this.fehler = e instanceof Error ? e.message : String(e);
    }
  }

  /** Wohin die Schlüsseldatei zuletzt gesichert wurde. */
  gesichertNach = $state<string | null>(null);

  /** Ob der letzte Passwortwechsel geklappt hat. */
  passwortGewechselt = $state(false);

  async schluesselSichern() {
    try {
      const ziel = await this.#bruecke.schluesselSichern();
      if (ziel !== null) this.gesichertNach = ziel;
      this.fehler = null;
    } catch (e) {
      this.fehler = e instanceof Error ? e.message : String(e);
    }
  }

  /**
   * Ändert das Passwort. Gibt zurück, ob es geklappt hat.
   *
   * **Der Aufrufer leert seine Felder danach — auch bei Fehlschlag.**
   * Stehengebliebene Passwörter sind Passwörter im Speicher der
   * Webansicht, unabhängig vom Ergebnis.
   */
  async passwortAendern(alt: string, neu: string): Promise<boolean> {
    this.passwortGewechselt = false;
    try {
      await this.#bruecke.passwortAendern(alt, neu);
      this.passwortGewechselt = true;
      this.fehler = null;
      return true;
    } catch (e) {
      this.fehler = e instanceof Error ? e.message : String(e);
      return false;
    }
  }

  /**
   * Löscht die Identität — der folgenschwerste Vorgang des Programms.
   *
   * Es gibt keine Sicherung beim Hersteller, keinen Wiederherstellungs-
   * schlüssel und keinen Weg zurück. Alles, was je an diesen Fingerprint
   * verschlüsselt wurde, ist danach dauerhaft unlesbar — auch das, was
   * noch gar nicht angekommen ist, denn die Gegenseite verschlüsselt
   * weiter an einen Schlüssel, den es nicht mehr gibt.
   *
   * Der Kontaktspeicher geht mit. Er ist an die Identität versiegelt und
   * ohne sie nicht mehr zu öffnen.
   */
  async loeschen() {
    try {
      await this.#bruecke.identitaetLoeschen();
      this.liste = [];
      this.nutzlast = null;
      this.nutzlastNach = null;
      this.fehler = null;
    } catch (e) {
      this.fehler = e instanceof Error ? e.message : String(e);
    }
  }
}

/**
 * Die Dateien, die verschickt werden sollen.
 *
 * # Warum der Halter die Kennung führt und nicht der Bildschirm
 *
 * Weil dieselbe Auswahl mehrere Bildschirme überlebt: aussuchen, ansehen,
 * entscheiden, senden. Läge sie im Bildschirm, wäre sie beim ersten
 * Wechsel fort — und der Nutzer wählte vierzig Dateien zweimal aus.
 *
 * # Der Pfad ist die Kennung
 *
 * Zweimal dieselbe Datei hinzuzufügen soll sie nicht verdoppeln. Über den
 * Namen ginge das schief: `Rechnung.pdf` aus zwei Ordnern sind zwei
 * Dateien, nicht eine.
 */
class Sendespeicher {
  /** Was ausgewählt ist, samt Befund. */
  dateien = $state<Sendedatei[]>([]);

  /** Läuft gerade eine Prüfung? Bei vierzig Bildern dauert das spürbar. */
  arbeitet = $state(false);

  /**
   * Wie weit der laufende Stapel ist. `null` heißt: keiner läuft.
   *
   * Getrennt von `arbeitet`, weil die beiden Verschiedenes sagen.
   * `arbeitet` heißt „es passiert etwas“ — auch während ein Dialog offen
   * steht oder ein Versandplan geprüft wird. `fortschritt` heißt „und zwar
   * an Datei drei von vierzig“.
   *
   * Die **Art** steckt mit drin und nicht in einem zweiten Feld: Zwei
   * Zustände, die zusammengehören, laufen irgendwann auseinander — und dann
   * stünde „Wird gelöscht“ über einem Prüflauf.
   */
  fortschritt = $state<Stapelstand | null>(null);

  /**
   * Baut den Melder für einen bestimmten Stapel.
   *
   * Die Art wird an der Aufrufstelle gesetzt, wo sie bekannt ist — nicht
   * hinterher erraten.
   */
  #melder(art: Stapelart): Fortschrittsmelder {
    return (f) => {
      this.fortschritt = { ...f, art };
    };
  }


  fehler = $state<string | null>(null);

  #bruecke: Bruecke;

  constructor(bruecke: Bruecke) {
    this.#bruecke = bruecke;
  }

  verbinde(bruecke: Bruecke) {
    this.#bruecke = bruecke;
    this.dateien = [];
    this.fehler = null;
    this.gespeichert = [];
  }

  /**
   * Öffnet den Dateidialog und nimmt auf, was ausgewählt wurde.
   *
   * Ein Abbruch ergibt eine leere Liste und **ändert nichts** — wer den
   * Dialog schließt, wollte die bisherige Auswahl nicht verwerfen.
   */
  async waehlen() {
    try {
      const pfade = await this.#bruecke.dateienWaehlen();
      if (pfade.length > 0) await this.hinzufuegen(pfade);
    } catch (e) {
      this.fehler = e instanceof Error ? e.message : String(e);
    }
  }

  /**
   * Nimmt Pfade auf — aus dem Dialog oder aus dem Fenster gezogen.
   *
   * Schon Vorhandenes wird **nicht erneut geprüft**: Bei vierzig Bildern
   * ist das der Unterschied zwischen einem Augenblick und einer Wartezeit,
   * und der Befund wäre derselbe.
   */
  async hinzufuegen(pfade: string[]) {
    const bekannt = new Set(this.dateien.map((d) => d.pfad));
    const neue = pfade.filter((p) => !bekannt.has(p));
    if (neue.length === 0) return;

    this.arbeitet = true;
    try {
      const geprueft = await this.#bruecke.dateienPruefen(neue, this.#melder("pruefen"));
      this.dateien = [...this.dateien, ...geprueft];
      this.fehler = null;
    } catch (e) {
      this.fehler = e instanceof Error ? e.message : String(e);
    } finally {
      this.arbeitet = false;
      this.fortschritt = null;
    }
  }

  /**
   * Was zuletzt gespeichert wurde. Leer heißt: noch nichts.
   *
   * Bleibt stehen, bis der nächste Vorgang läuft — wer speichert und dann
   * wegsieht, soll beim Zurückkommen noch lesen können, wohin.
   */
  gespeichert = $state<Speicherergebnis[]>([]);

  /**
   * Speichert die bereinigten Fassungen, ohne zu verschlüsseln.
   *
   * Eine leere Antwort heißt **abgebrochen** und löscht das vorige
   * Ergebnis nicht: Wer den Dialog versehentlich öffnet und schließt, soll
   * nicht verlieren, was er eben gelesen hat.
   */
  async bereinigtSpeichern(pfade: string[]) {
    if (pfade.length === 0) return;
    this.arbeitet = true;
    try {
      const ergebnis = await this.#bruecke.bereinigtSpeichern(pfade, this.#melder("speichern"));
      if (ergebnis.length > 0) this.gespeichert = ergebnis;
      this.fehler = null;
    } catch (e) {
      this.fehler = e instanceof Error ? e.message : String(e);
    } finally {
      this.arbeitet = false;
      this.fortschritt = null;
    }
  }

  /**
   * Der Bericht des letzten Versands. `null` heißt: noch keiner.
   */
  versand = $state<Versandbericht | null>(null);

  /**
   * Verschlüsselt und behält den Bericht.
   *
   * Ein Fehlschlag heißt hier **es ist nichts entstanden**: Der Kern prüft
   * die Empfänger, bevor er die erste Datei anfasst.
   */
  async verschluesseln(
    pfade: string[],
    empfaenger: string[],
    signieren: boolean,
    original: string[],
  ) {
    this.arbeitet = true;
    try {
      this.versand = await this.#bruecke.verschluesseln(
        pfade,
        empfaenger,
        signieren,
        original,
        this.#melder("verschluesseln"),
      );
      this.fehler = null;
    } catch (e) {
      this.versand = null;
      this.fehler = e instanceof Error ? e.message : String(e);
    } finally {
      this.arbeitet = false;
      this.fortschritt = null;
    }
  }

  /**
   * Der Armor-Text der letzten Nachricht. `null` heißt: keine.
   *
   * **Der Klartext steht hier nicht.** Er wird durchgereicht und im
   * Bildschirm sofort geleert; was hier liegt, ist bereits verschlüsselt.
   */
  textEnvelope = $state<string | null>(null);

  /** Verschlüsselt eine Textnachricht. */
  async textVerschluesseln(
    text: string,
    empfaenger: string[],
    signieren: boolean,
  ) {
    this.arbeitet = true;
    try {
      this.textEnvelope = await this.#bruecke.textVerschluesseln(
        text,
        empfaenger,
        signieren,
      );
      this.fehler = null;
    } catch (e) {
      this.textEnvelope = null;
      this.fehler = e instanceof Error ? e.message : String(e);
    } finally {
      this.arbeitet = false;
    }
  }

  /** Nimmt den Envelope vom Bildschirm. */
  textSchliessen() {
    this.textEnvelope = null;
  }

  /** Nimmt den Versandbericht vom Bildschirm. */
  versandSchliessen() {
    this.versand = null;
  }

  /** Nimmt das Ergebnis vom Bildschirm. */
  ergebnisSchliessen() {
    this.gespeichert = [];
  }

  /** Nimmt eine Datei wieder aus der Auswahl. */
  entfernen(pfad: string) {
    this.dateien = this.dateien.filter((d) => d.pfad !== pfad);
  }

  leeren() {
    this.dateien = [];
    this.fehler = null;
    this.gespeichert = [];
  }

  /**
   * Ob gerade etwas über dem Fenster hängt.
   *
   * Der Grund, warum es diesen Zustand gibt: Ein Fenster, das erst beim
   * Loslassen reagiert, sieht bis dahin aus wie eines, das nichts annimmt
   * — und dann lässt niemand los.
   */
  ziehtDrueber = $state(false);

  /**
   * Wird gesetzt, wenn gerade etwas fallengelassen wurde.
   *
   * Die Hülle liest das und wechselt zum Sendebildschirm. Ohne das
   * verschwinden hineingezogene Dateien in einen Halter, den gerade
   * niemand ansieht — was von außen aussieht, als sei nichts passiert.
   */
  zuletztGefallen = $state(0);

  /**
   * Hängt sich an das Ziehen-und-Fallenlassen. Gibt den Abmelder zurück.
   */
  beobachten(): () => void {
    let abmelden: (() => void) | null = null;
    let abgebaut = false;

    void this.#bruecke
      .aufDateienGezogen((e) => {
        switch (e.art) {
          case "drueber":
            this.ziehtDrueber = true;
            break;
          case "weg":
            this.ziehtDrueber = false;
            break;
          case "fallen":
            this.ziehtDrueber = false;
            void this.hinzufuegen(e.pfade).then(() => {
              this.zuletztGefallen = Date.now();
            });
            break;
        }
      })
      .then((weg) => {
        // Wurde in der Zwischenzeit abgebaut, sofort wieder abmelden --
        // sonst bliebe ein Empfänger für ein Fenster stehen, das es nicht
        // mehr gibt.
        if (abgebaut) weg();
        else abmelden = weg;
      })
      .catch((e: unknown) => {
        // **Nicht verschlucken.** Ein Ziehen-und-Fallenlassen, das sich
        // nicht anmelden lässt, sieht sonst genauso aus wie eines, das
        // funktioniert und bei dem der Nutzer danebengezielt hat.
        this.fehler = e instanceof Error ? e.message : String(e);
      });

    return () => {
      abgebaut = true;
      abmelden?.();
    };
  }
}

/**
 * Was gerade geöffnet ist.
 *
 * # Warum der Inhalt hier nicht steht
 *
 * Weil er in Rust bleibt. Dieser Halter führt den **Bericht** — wer
 * geschickt hat, wie die Datei heißt, wie groß sie ist. Der entschlüsselte
 * Inhalt liegt in der Sitzung des Kerns, bis jemand sagt, wohin er soll;
 * ihn hierher zu holen hieße, ihn in eine Webansicht zu legen, die wir
 * weder überschreiben noch begrenzen können.
 */
class Empfangsspeicher {
  /** Der Bericht zum zuletzt geöffneten Envelope. */
  geoeffnet = $state<Geoeffnet | null>(null);

  /** Woher er kam — für die Anzeige. */
  quelle = $state<string | null>(null);

  /** Wohin zuletzt gespeichert wurde. */
  gespeichertNach = $state<string | null>(null);

  /**
   * Ob eine Signatur verlangt wurde.
   *
   * **Dieselbe Lage wird anders bewertet, je nachdem was der Nutzer
   * verlangt hat** — nicht danach, was das Programm für richtig hält. Ohne
   * diese Unterscheidung müsste man entscheiden, ob eine unsignierte
   * Nachricht gelb ist; beide Antworten wären falsch.
   */
  signaturVerlangt = $state(false);

  arbeitet = $state(false);
  fehler = $state<string | null>(null);

  #bruecke: Bruecke;

  constructor(bruecke: Bruecke) {
    this.#bruecke = bruecke;
  }

  verbinde(bruecke: Bruecke) {
    this.#bruecke = bruecke;
    this.geoeffnet = null;
    this.quelle = null;
    this.gespeichertNach = null;
    this.fehler = null;
  }

  /** Lässt einen Envelope auswählen und öffnet ihn gleich. */
  async waehlenUndOeffnen() {
    try {
      const pfad = await this.#bruecke.envelopeWaehlen();
      // `null` heißt abgebrochen und verwirft nichts.
      if (pfad === null) return;
      await this.oeffnen(pfad);
    } catch (e) {
      this.fehler = e instanceof Error ? e.message : String(e);
    }
  }

  /**
   * Öffnet einen eingefügten Armor-Text.
   *
   * Der Text ist bereits verschlüsselt — er ist kein Geheimnis, und ihn
   * hier durchzureichen ist unbedenklich. Der **Klartext** kommt nicht
   * zurück; er bleibt im Kern wie bei einer Datei.
   */
  async textOeffnen(text: string) {
    this.arbeitet = true;
    try {
      this.geoeffnet = await this.#bruecke.textOeffnen(
        text,
        this.signaturVerlangt,
      );
      this.quelle = "eingefügter Text";
      this.gespeichertNach = null;
      this.fehler = null;
    } catch (e) {
      this.geoeffnet = null;
      this.quelle = null;
      this.fehler = e instanceof Error ? e.message : String(e);
    } finally {
      this.arbeitet = false;
    }
  }

  async oeffnen(pfad: string) {
    this.arbeitet = true;
    try {
      this.geoeffnet = await this.#bruecke.envelopeOeffnen(
        pfad,
        this.signaturVerlangt,
      );
      this.quelle = pfad;
      this.gespeichertNach = null;
      this.fehler = null;
    } catch (e) {
      // Ein Fehlschlag verwirft das Vorige: Sonst stünde ein Bericht da,
      // der zu einer anderen Datei gehört.
      this.geoeffnet = null;
      this.quelle = null;
      this.fehler = e instanceof Error ? e.message : String(e);
    } finally {
      this.arbeitet = false;
    }
  }

  async speichern() {
    try {
      const ziel = await this.#bruecke.nutzlastSpeichern();
      if (ziel !== null) this.gespeichertNach = ziel;
      this.fehler = null;
    } catch (e) {
      this.fehler = e instanceof Error ? e.message : String(e);
    }
  }

  /**
   * Schließt und wirft den Klartext im Kern weg.
   *
   * Ein entschlüsselter Inhalt, der liegen bleibt, ist eine Kopie ohne
   * Zweck.
   */
  async schliessen() {
    this.geoeffnet = null;
    this.quelle = null;
    this.gespeichertNach = null;
    this.fehler = null;
    await this.#bruecke.nutzlastVerwerfen().catch(() => {});
  }

  // --- Doppelklick im Explorer ---------------------------------------------

  /**
   * Eine Datei, die das Betriebssystem hereingereicht hat und die noch
   * nicht geöffnet werden konnte. `null` heißt: es wartet nichts.
   *
   * **Der Fall, um den es geht, ist der gesperrte.** Wer eine `.cabrik`
   * doppelklickt, während das Fenster gesperrt ist, soll nicht ins Leere
   * greifen: Der Pfad wartet hier, der Sperrbildschirm nennt ihn, und nach
   * dem Entsperren geht die Datei von selbst auf.
   *
   * Ein Pfad, kein Inhalt — gelesen wird erst beim Öffnen.
   */
  wartendeDatei = $state<string | null>(null);

  /**
   * Fragt den Kern, ob etwas hereingereicht wurde, und öffnet es, sobald
   * es geht.
   *
   * `entsperrt` entscheidet, ob geöffnet oder gewartet wird. Diese Auskunft
   * kommt von außen und wird hier nicht selbst geholt: Zwei Halter, die
   * einander befragen, sind eine Schleife, die niemand mehr auseinandernimmt.
   */
  async hereingereichtePruefen(entsperrt: boolean) {
    try {
      // Erst abholen -- auch im gesperrten Zustand. Das Fach im Kern muss
      // geleert werden, sonst kaeme derselbe Pfad bei jedem Takt erneut.
      const neu = await this.#bruecke.dateiAbholen();
      if (neu !== null) this.wartendeDatei = neu;
    } catch (e) {
      this.fehler = e instanceof Error ? e.message : String(e);
      return;
    }

    if (this.wartendeDatei === null || !entsperrt) return;

    const pfad = this.wartendeDatei;
    // Erst wegnehmen, dann oeffnen. Andersherum bliebe der Pfad bei einem
    // Fehlschlag stehen und wuerde beim naechsten Takt erneut versucht --
    // eine Schleife aus Fehlermeldungen, die niemand abstellen kann.
    this.wartendeDatei = null;
    await this.oeffnen(pfad);
  }

  /**
   * Hängt sich an die Meldung des Kerns. Gibt zurück, wie man das löst.
   *
   * Das Ereignis trägt den Pfad **nicht** — es ist nur der Anstoß,
   * nachzufragen. Zwei Wege zu derselben Auskunft liefen auseinander.
   */
  async aufHereingereichtHorchen(entsperrt: () => boolean): Promise<() => void> {
    try {
      return await this.#bruecke.aufDateiHereingereicht(() => {
        void this.hereingereichtePruefen(entsperrt());
      });
    } catch {
      // Im Browser gibt es das Ereignis nicht. Das ist kein Fehler, den
      // jemand sehen muesste -- dort wird auch nie etwas hereingereicht.
      return () => {};
    }
  }
}

/**
 * Die Dateien, die endgültig gelöscht werden sollen.
 *
 * # Warum das ein eigener Halter ist und nicht der Sendespeicher
 *
 * Weil es zwei verschiedene Absichten sind. Wer etwas verschickt, will es
 * behalten; wer etwas löscht, will es loswerden. Sie in einer Liste zu
 * führen hieße, dass ein Klick im einen Bildschirm den anderen verändert —
 * und der eine ist unwiderruflich.
 */
class Loeschspeicher {
  /** Was ausgewählt ist, samt Beurteilung. */
  kandidaten = $state<Loeschkandidat[]>([]);

  /** Was beim letzten Löschen herauskam. */
  ergebnisse = $state<Loeschergebnis[]>([]);

  arbeitet = $state(false);

  /**
   * Wie weit der laufende Stapel ist. `null` heißt: keiner läuft.
   *
   * Getrennt von `arbeitet`, weil die beiden Verschiedenes sagen.
   * `arbeitet` heißt „es passiert etwas“ — auch während ein Dialog offen
   * steht oder ein Versandplan geprüft wird. `fortschritt` heißt „und zwar
   * an Datei drei von vierzig“.
   *
   * Die **Art** steckt mit drin und nicht in einem zweiten Feld: Zwei
   * Zustände, die zusammengehören, laufen irgendwann auseinander — und dann
   * stünde „Wird gelöscht“ über einem Prüflauf.
   */
  fortschritt = $state<Stapelstand | null>(null);

  /**
   * Baut den Melder für einen bestimmten Stapel.
   *
   * Die Art wird an der Aufrufstelle gesetzt, wo sie bekannt ist — nicht
   * hinterher erraten.
   */
  #melder(art: Stapelart): Fortschrittsmelder {
    return (f) => {
      this.fortschritt = { ...f, art };
    };
  }

  fehler = $state<string | null>(null);

  #bruecke: Bruecke;

  constructor(bruecke: Bruecke) {
    this.#bruecke = bruecke;
  }

  verbinde(bruecke: Bruecke) {
    this.#bruecke = bruecke;
    this.kandidaten = [];
    this.ergebnisse = [];
    this.fehler = null;
  }

  /** Lässt Dateien auswählen und beurteilt sie — **ohne** zu löschen. */
  async waehlen() {
    try {
      const pfade = await this.#bruecke.dateienWaehlen();
      // Ein Abbruch verwirft die bisherige Auswahl nicht.
      if (pfade.length === 0) return;
      this.arbeitet = true;
      const bekannt = new Set(this.kandidaten.map((k) => k.pfad));
      const neue = pfade.filter((p) => !bekannt.has(p));
      if (neue.length === 0) return;
      const beurteilt = await this.#bruecke.loeschenBeurteilen(neue, this.#melder("beurteilen"));
      this.kandidaten = [...this.kandidaten, ...beurteilt];
      this.ergebnisse = [];
      this.fehler = null;
    } catch (e) {
      this.fehler = e instanceof Error ? e.message : String(e);
    } finally {
      this.arbeitet = false;
      this.fortschritt = null;
    }
  }

  /**
   * Löscht. **Unwiderruflich.**
   *
   * Die Auswahl wird danach geleert: Was gelöscht ist, gehört nicht mehr in
   * eine Liste von Dateien, die gelöscht werden sollen. Das Ergebnis bleibt
   * stehen — es ist das Einzige, was von ihnen übrig ist.
   */
  async loeschen(durchgaenge: number) {
    if (this.kandidaten.length === 0) return;
    this.arbeitet = true;
    try {
      this.ergebnisse = await this.#bruecke.loeschenAusfuehren(
        this.kandidaten.map((k) => k.pfad),
        durchgaenge,
        this.#melder("loeschen"),
      );
      this.kandidaten = [];
      this.fehler = null;
    } catch (e) {
      this.fehler = e instanceof Error ? e.message : String(e);
    } finally {
      this.arbeitet = false;
      this.fortschritt = null;
    }
  }

  leeren() {
    this.kandidaten = [];
    this.ergebnisse = [];
    this.fehler = null;
  }
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
export const identitaetsspeicher = new Identitaetsspeicher(GETEILT);
export const sendespeicher = new Sendespeicher(GETEILT);
export const empfangsspeicher = new Empfangsspeicher(GETEILT);
export const loeschspeicher = new Loeschspeicher(GETEILT);
