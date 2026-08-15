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

import type {
  Geoeffnet,
  Loeschergebnis,
  Loeschkandidat,
  Identitaet,
  KdfStufe,
  Kontakt,
  Nutzlastbefund,
  Sendedatei,
  Sitzungsstand,
  Speicherergebnis,
  Versandbericht,
  Sperrfrist,
  Verifikationsweg,
  Ziehereignis,
} from "./typen";
import { FRIST_SEKUNDEN } from "./typen";
import { FAELLE, IDENTITAET, NUTZLASTEN, STAPEL } from "./mock";

/**
 * Was die Oberfläche vom Kern verlangen kann.
 *
 * Bewusst schmal: Jede Methode entspricht einer Handlung, die ein Mensch
 * auslöst. Es gibt kein allgemeines „lies mir dieses Feld“ — der Kern
 * entscheidet, was er herausgibt, nicht die Oberfläche, was sie sich holt.
 */
export interface Bruecke {
  // --- Sitzung (spec/entsperrung.md) ---------------------------------------

  /**
   * Wie es um die Sitzung steht.
   *
   * **`null` heißt „auf diesem Rechner liegt keine Identität“** — etwas
   * ganz anderes als gesperrt. Im einen Fall führt der Weg zur
   * Einrichtung, im anderen zum Passwortfeld.
   */
  sitzungsstand(): Promise<Sitzungsstand | null>;

  /**
   * Entsperrt mit einem Passwort.
   *
   * Das Passwort ist der einzige Wert, der über diese Naht **hinein**
   * geht. Es wird durchgereicht und nirgends behalten — der Aufrufer leert
   * sein Eingabefeld unmittelbar danach (`spec/entsperrung.md` §5.1).
   *
   * Ein Fehlschlag sagt nicht, *wie* falsch das Passwort war (§4.3).
   */
  entsperren(passwort: string): Promise<void>;

  /** Sperrt sofort. */
  sperren(): Promise<void>;

  /** Stellt die Frist ein. Das ist selbst eine Handlung, die sie neu startet. */
  fristSetzen(frist: Sperrfrist): Promise<void>;

  /**
   * Meldet, dass jemand gehandelt hat — Taste, Klick, Rollen.
   *
   * Ohne das liefe die Frist ab, während jemand eine lange Nachricht
   * schreibt: In dieser Zeit wird kein einziger anderer Befehl ausgelöst.
   *
   * **Bloße Mausbewegung zählt nicht** (§9.2) — ein Ärmel auf dem Tisch
   * sagt nichts darüber, ob noch jemand da ist.
   */
  taetigkeit(): Promise<void>;

  // --- Identität -----------------------------------------------------------

  /**
   * Die eigene Identität — **nur im entsperrten Zustand**.
   *
   * Die Bezeichnung steht im verschlüsselten Teil der Schlüsseldatei. Es
   * gibt sie also gesperrt nicht bloß nicht zu sehen, sondern gar nicht:
   * Der Sperrbildschirm kann nicht verraten, wessen Rechner das ist, weil
   * niemand es wüsste (`spec/entsperrung.md` §4.1).
   */
  identitaet(): Promise<Identitaet>;

  /**
   * Legt eine Identität an — und ist danach **entsperrt**.
   *
   * Wer gerade ein Passwort gesetzt hat, hat es eben getippt; ihn danach
   * auf den Sperrbildschirm zu schicken, verlangt dieselbe Eingabe ein
   * zweites Mal und schützt vor nichts.
   *
   * **Schlägt fehl, wenn schon eine da ist.** Eine neue über eine
   * bestehende zu schreiben, machte alles dauerhaft unlesbar, was an die
   * bisherige gerichtet war — auch das, was noch gar nicht angekommen ist.
   */
  identitaetAnlegen(
    bezeichnung: string | null,
    passwort: string,
    mitSignierschluessel: boolean,
    stufe: KdfStufe,
  ): Promise<Identitaet>;

  /**
   * Löscht die Identität — **nur im entsperrten Zustand**.
   *
   * Das schützt die Datei nicht; wer am Rechner sitzt, kann sie auch im
   * Dateimanager wegwerfen. Es schützt dagegen, dass das Programm selbst
   * einen Knopf anbietet, mit dem jemand ohne Passwort in zwei Klicks
   * alles vernichtet, was an diesen Schlüssel gerichtet war.
   *
   * Der Kontaktspeicher geht mit: Er ist an die Identität versiegelt und
   * ohne sie nicht mehr zu öffnen.
   */
  identitaetLoeschen(): Promise<void>;

  // --- Dateien -------------------------------------------------------------

  /**
   * Lässt Dateien auswählen und gibt ihre Pfade zurück.
   *
   * **Eine leere Liste heißt abgebrochen**, nicht „keine gefunden“. Wer
   * den Dialog schließt, hat sich entschieden — eine Meldung darüber wäre
   * eine Störung ohne Vorfall.
   *
   * Der Dialog steht in Rust, nicht in der Webansicht: Die Naht bleibt
   * damit, wie sie überall ist, und die Webansicht bekommt keine
   * Berechtigung, die sie sonst nicht hätte.
   */
  dateienWaehlen(): Promise<string[]>;

  /**
   * Meldet Dateien, die ins Fenster gezogen wurden.
   *
   * Gibt zurück, wie man sich wieder abmeldet.
   *
   * **Nur im Fenster.** Im Browser liefert das Ziehen von Dateien keine
   * Pfade, sondern Inhalte — und Inhalte will diese Naht nicht. Die
   * Attrappe meldet deshalb nie etwas, statt so zu tun.
   */
  aufDateienGezogen(melde: (e: Ziehereignis) => void): Promise<() => void>;

  /**
   * Sieht Dateien an, **ohne etwas zu verändern**.
   *
   * Über diese Naht geht der **Befund**, nicht der Inhalt. Eine Oberfläche,
   * die Dateiinhalte hält, hätte sie in einem Speicher, den wir weder
   * überschreiben noch begrenzen können — und bei vierzig Bildern wären
   * das hunderte Megabyte in einer Webansicht.
   *
   * **Jede Datei darf einzeln scheitern.** Was sich nicht lesen ließ,
   * kommt mit `befund.fall === "fehler"` zurück und steht sichtbar im
   * Stapel, statt ihn ganz zum Scheitern zu bringen.
   */
  dateienPruefen(pfade: string[]): Promise<Sendedatei[]>;

  /**
   * Speichert die bereinigten Fassungen — **ohne zu verschlüsseln**.
   *
   * Metadaten zu entfernen ist ein eigener Zweck: Wer ein Foto hochlädt,
   * will kein Envelope, sondern ein Bild ohne Ortsangabe.
   *
   * **Die Ausgangsdatei bleibt liegen.** Danach stehen zwei
   * unverschlüsselte Fassungen auf der Platte — eine davon mit allem, was
   * drinstand.
   *
   * Eine leere Liste heißt **abgebrochen**, nicht „nichts gespeichert“:
   * Wer den Dialog schließt, hat sich entschieden.
   */
  bereinigtSpeichern(pfade: string[]): Promise<Speicherergebnis[]>;

  /**
   * Verschlüsselt die Dateien für die genannten Empfänger.
   *
   * **Schlägt ganz fehl, wenn ein Empfänger widerrufen ist** — dann
   * entsteht keine einzige Datei. Ein Vorgang, der bei Datei
   * siebenunddreißig abbricht, hinterließe sechsunddreißig Envelopes, die
   * niemand bestellt hat.
   *
   * `original` nennt die Dateien, deren **unbereinigte** Fassung
   * hinausgehen soll. Alle übrigen werden vorher bereinigt.
   */
  verschluesseln(
    pfade: string[],
    empfaenger: string[],
    signieren: boolean,
    original: string[],
  ): Promise<Versandbericht>;

  /**
   * Verschlüsselt einen Text und gibt ihn zum Kopieren zurück.
   *
   * **Der Text ist ein Geheimnis und geht in die andere Richtung** — wie
   * das Passwort. Er wird durchgereicht und nirgends behalten; der
   * Aufrufer leert sein Eingabefeld, sobald der Envelope da ist.
   *
   * Das Ergebnis ist Armor (`spec/envelope-v2.md` §14): Text zum
   * Einfügen, ein Drittel größer als die Binärform, und die Rahmenzeilen
   * nennen das Produkt. Beides steht dort und wird bewusst gezahlt.
   */
  textVerschluesseln(
    text: string,
    empfaenger: string[],
    signieren: boolean,
  ): Promise<string>;

  /** Öffnet einen eingefügten Armor-Text. */
  textOeffnen(text: string, signaturVerlangt: boolean): Promise<Geoeffnet>;

  // --- Empfangen -----------------------------------------------------------

  /** Lässt einen Envelope auswählen. `null` heißt abgebrochen. */
  envelopeWaehlen(): Promise<string | null>;

  /**
   * Öffnet einen Envelope. **Der Klartext bleibt im Kern.**
   *
   * Zurück geht ein Bericht: Wer geschickt hat, wie die Datei heißt, wie
   * groß sie ist. Der Inhalt liegt in der Sitzung, bis jemand sagt, wohin
   * er soll — oder bis gesperrt wird, dann ist er fort.
   *
   * `signaturVerlangt` macht aus einer unsignierten Nachricht einen
   * Fehler. Das ist eine Entscheidung des Nutzers, keine des Programms.
   */
  envelopeOeffnen(pfad: string, signaturVerlangt: boolean): Promise<Geoeffnet>;

  /**
   * Legt die geöffnete Nutzlast ab. `null` heißt abgebrochen.
   *
   * **Ein zweiter Schritt.** Wer eine Nachricht von einem unbekannten
   * Absender öffnet, will vielleicht nur wissen, was drinsteht — und
   * nicht, dass sie danach auf der Platte liegt.
   */
  nutzlastSpeichern(): Promise<string | null>;

  /** Wirft den geöffneten Klartext weg. */
  nutzlastVerwerfen(): Promise<void>;

  /**
   * Die eigene Austausch-Nutzlast — zum Weitergeben.
   *
   * **Ausschließlich öffentliche Angaben.** Kein Name, keine Bezeichnung:
   * Die vergibt der Empfänger selbst.
   *
   * Ohne sie wäre das Programm einseitig — man könnte Kontakte aufnehmen,
   * aber niemand könnte einem schreiben.
   */
  eigeneNutzlast(): Promise<string>;

  /** Legt die eigene Nutzlast als Textdatei ab. `null` heißt abgebrochen. */
  nutzlastAlsDatei(): Promise<string | null>;

  /**
   * Liest eine Austausch-Nutzlast aus einer Datei. `null` heißt abgebrochen.
   *
   * Der Inhalt kommt **ungeprüft** zurück: Beurteilt wird er von
   * [`nutzlastLesen`], derselben Prüfung wie beim Einfügen von Hand. Zwei
   * Wege herein dürfen nicht zu zwei Urteilen führen.
   */
  nutzlastAusDatei(): Promise<string | null>;

  /**
   * Legt eine Kopie der Schlüsseldatei ab. `null` heißt abgebrochen.
   *
   * **Unbedenklich**: Die Datei ist mit dem Passwort verschlüsselt. Sie
   * schützt gegen eine kaputte Platte und ein versehentliches Löschen —
   * **nicht** gegen ein vergessenes Passwort.
   */
  schluesselSichern(): Promise<string | null>;

  /**
   * Ändert das Passwort. **Die Identität bleibt dieselbe.**
   *
   * Derselbe Fingerprint, dieselben Kontakte, dieselben alten Envelopes.
   * Wer einen neuen Schlüssel will, legt eine neue Identität an.
   *
   * Beide Passwörter gehen in die **andere** Richtung über diese Naht —
   * wie beim Entsperren. Der Aufrufer leert seine Felder danach.
   */
  passwortAendern(alt: string, neu: string): Promise<void>;

  // --- Sicheres Löschen ----------------------------------------------------

  /**
   * Beurteilt, was Löschen bei diesen Dateien **erreicht** — ohne zu
   * löschen.
   *
   * Die Auskunft kommt vor der Tat: Wer erst löscht und dann erfährt, dass
   * Überschreiben auf einer SSD nichts ausrichtet, kann nichts mehr
   * entscheiden.
   */
  loeschenBeurteilen(pfade: string[]): Promise<Loeschkandidat[]>;

  /**
   * Löscht die Dateien. **Unwiderruflich.**
   *
   * Jeder Schritt wird einzeln gemeldet — überschrieben, umbenannt,
   * entfernt. Ein pauschales „Gelöscht“ wäre eine Behauptung über drei
   * verschiedene Dinge, von denen jedes einzeln scheitern kann.
   */
  loeschenAusfuehren(
    pfade: string[],
    durchgaenge: number,
  ): Promise<Loeschergebnis[]>;

  // --- Kontakte ------------------------------------------------------------

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

  /**
   * Die nachgestellte Sitzung.
   *
   * **Sie beginnt entsperrt**, anders als der echte Kern. Das ist kein
   * Versehen: Der Prototyp im Browser soll durchsehbar bleiben, ohne dass
   * jemand ein Passwort erfindet, das es gar nicht gibt. Der gesperrte
   * Zustand ist über „Jetzt sperren“ einen Klick entfernt.
   */
  private gesperrt = false;
  private frist: Sperrfrist = "fuenfzehnMinuten";
  private letzteHandlung = Date.now();

  constructor(anfang: readonly Kontakt[]) {
    this.daten = anfang.map((k) => ({ ...k }));
  }

  // --- Sitzung -------------------------------------------------------------

  async sitzungsstand(): Promise<Sitzungsstand | null> {
    // **`null`, solange es keine Identität gibt** -- wie das Fenster. Bis
    // eben meldete die Attrappe auch dann eine Sitzung, und damit fehlte
    // ihr der Übergang „keine Identität -> entsperrt“ ganz. Genau in dem
    // steckte ein Fehler, den kein Test sehen konnte.
    if (!this.eigene) return null;
    this.fristPruefen();
    return {
      gesperrt: this.gesperrt,
      frist: this.frist,
      restsekunden: this.restsekunden(),
    };
  }

  /**
   * Nimmt jedes Passwort ab vier Zeichen an.
   *
   * Es gibt hier nichts zu prüfen — im Browser liegt keine Schlüsseldatei.
   * Die Grenze existiert allein, damit der **abgelehnte** Fall erreichbar
   * ist: Ein Zustand, den man nie sehen kann, wird nie gestaltet.
   *
   * Die Meldung ist wörtlich die des Kerns, mitsamt dem, was sie nicht
   * sagt — nämlich wie falsch das Passwort war.
   */
  async entsperren(passwort: string): Promise<void> {
    if (passwort.trim().length < 4) throw new Error("Das Passwort passt nicht.");
    this.gesperrt = false;
    this.letzteHandlung = Date.now();
  }

  async sperren(): Promise<void> {
    this.gesperrt = true;
  }

  async fristSetzen(frist: Sperrfrist): Promise<void> {
    this.frist = frist;
    this.letzteHandlung = Date.now();
  }

  async taetigkeit(): Promise<void> {
    this.fristPruefen();
    if (!this.gesperrt) this.letzteHandlung = Date.now();
  }

  /** Wie `Sitzung::sperre_pruefen`: Nachfragen ist keine Handlung. */
  private fristPruefen() {
    const grenze = FRIST_SEKUNDEN[this.frist];
    if (grenze === null || this.gesperrt) return;
    if ((Date.now() - this.letzteHandlung) / 1000 >= grenze) this.gesperrt = true;
  }

  private restsekunden(): number | null {
    const grenze = FRIST_SEKUNDEN[this.frist];
    if (grenze === null || this.gesperrt) return null;
    const verstrichen = Math.floor((Date.now() - this.letzteHandlung) / 1000);
    return Math.max(0, grenze - verstrichen);
  }

  // --- Identität -----------------------------------------------------------

  /**
   * Die nachgestellte Identität.
   *
   * `undefined` heißt hier dasselbe wie im Fenster ein fehlender Schlüssel:
   * Es gibt noch keine. Die Attrappe beginnt mit einer, damit der Prototyp
   * durchsehbar bleibt.
   */
  private eigene: Identitaet | undefined = { ...IDENTITAET };

  async identitaet(): Promise<Identitaet> {
    if (this.gesperrt) throw new Error("Die Sitzung ist gesperrt.");
    if (!this.eigene) throw new Error(KEINE_IDENTITAET);
    return { ...this.eigene };
  }

  async identitaetAnlegen(
    bezeichnung: string | null,
    passwort: string,
    mitSignierschluessel: boolean,
    stufe: KdfStufe,
  ): Promise<Identitaet> {
    if (this.eigene) {
      // Wörtlich wie im Fenster. Der Satz ist die eigentliche Zusicherung:
      // Er nennt die Folge, nicht bloß die Regel.
      throw new Error(
        "Auf diesem Rechner liegt bereits eine Identität. Eine zweite " +
          "anzulegen, würde die bisherige überschreiben — und damit alles " +
          "unlesbar machen, was an sie gerichtet ist.",
      );
    }
    if (passwort.trim().length < 4) throw new Error("Das Passwort passt nicht.");

    this.eigene = {
      bezeichnung,
      fingerprint: neuerFingerprint(),
      fingerprintKurz: "",
      erzeugtAm: Math.floor(Date.now() / 1000),
      kdf: stufe,
      kdfSpeicherMib: SPEICHER_JE_STUFE[stufe],
      hatSignierschluessel: mitSignierschluessel,
      hatPostQuantum: true,
      pfad: "C:\Users\name\AppData\Roaming\CabrikSecure\identity.cabrik-key",
    };
    this.eigene.fingerprintKurz = this.eigene.fingerprint
      .replaceAll("-", "")
      .slice(0, 8);
    // Wie im Kern: angelegt heißt offen, und die Frist läuft ab jetzt.
    this.gesperrt = false;
    this.letzteHandlung = Date.now();
    return { ...this.eigene };
  }

  async identitaetLoeschen(): Promise<void> {
    if (this.gesperrt) throw new Error("Die Sitzung ist gesperrt.");
    if (!this.eigene) throw new Error(KEINE_IDENTITAET);
    this.eigene = undefined;
    // Wie im Fenster: Der Kontaktspeicher ist ohne die Identität nicht
    // mehr zu oeffnen und bleibt deshalb nicht liegen.
    this.daten = [];
  }

  // --- Dateien -------------------------------------------------------------

  /**
   * Gibt die Pfade des gerade gezeigten Beispielstapels zurück.
   *
   * Im Browser gibt es keinen Dateidialog. Etwas zurückzugeben ist trotzdem
   * richtig: Der Prototyp soll den Weg zeigen können, den jemand geht —
   * auswählen, ansehen, entscheiden.
   */
  async dateienWaehlen(): Promise<string[]> {
    return STAPEL[0]?.dateien.map((d) => d.pfad) ?? [];
  }

  /**
   * Meldet nie etwas.
   *
   * Ehrlicher als eine nachgestellte Übergabe: Im Browser gäbe es keine
   * Pfade, und ein Bildschirm, der hier etwas bekäme, prüfte einen Weg,
   * den es so nicht gibt.
   */
  async aufDateienGezogen(
    _melde: (e: Ziehereignis) => void,
  ): Promise<() => void> {
    return () => {};
  }

  /**
   * Sucht die Beispieldateien zum Pfad heraus.
   *
   * Im Browser gibt es kein Dateisystem, also gibt es hier auch nichts zu
   * lesen. Was die Attrappe kann, ist die **Beispielfälle** ausliefern —
   * und darum geht es im Prototyp: die seltenen Zustände ansehen, ohne sie
   * herstellen zu müssen.
   *
   * Ein unbekannter Pfad ergibt einen Fehlerbefund, der sagt, woran es
   * liegt. Er stillschweigend zu übergehen hieße, eine Datei aus dem
   * Stapel verschwinden zu lassen.
   */
  async dateienPruefen(pfade: string[]): Promise<Sendedatei[]> {
    const bekannt = new Map(
      STAPEL.flatMap((s) => s.dateien).map((d) => [d.pfad, d]),
    );
    return pfade.map(
      (p) =>
        bekannt.get(p) ?? {
          pfad: p,
          // Beide Trenner: Auf Windows steht der Backslash, und ohne ihn
          // waere der „Name“ der ganze Pfad.
          name: p.split(/[\\/]/).at(-1) ?? p,
          groesseBytes: 0,
          befund: {
            fall: "fehler",
            grund:
              "Im Browser gibt es kein Dateisystem. Diese Datei lässt sich " +
              "nur im Fenster ansehen.",
          },
          fassungen: [],
        },
    );
  }

  /**
   * Speichert nichts — es gibt kein Dateisystem im Browser.
   *
   * Sie meldet das je Datei, statt eine leere Liste zurückzugeben: Eine
   * leere Liste hieße „abgebrochen“, und das wäre eine andere Aussage.
   */
  async bereinigtSpeichern(pfade: string[]): Promise<Speicherergebnis[]> {
    return pfade.map((quelle) => ({
      quelle,
      ziel: null,
      befund: { fall: "unbekannt", formathinweis: null },
      fehler:
        "Im Browser gibt es kein Dateisystem. Speichern geht nur im Fenster.",
    }));
  }

  /**
   * Verschlüsselt nichts — es gibt kein Dateisystem im Browser.
   *
   * Sie wirft, statt einen erfundenen Bericht zu liefern: Ein Bericht über
   * einen Versand, den es nicht gab, wäre die schlimmste Attrappe von
   * allen.
   */
  async verschluesseln(): Promise<Versandbericht> {
    throw new Error(
      "Im Browser gibt es kein Dateisystem. Verschlüsseln geht nur im Fenster.",
    );
  }

  async textVerschluesseln(): Promise<string> {
    throw new Error(
      "Im Browser gibt es keine Identität. Verschlüsseln geht nur im Fenster.",
    );
  }

  async textOeffnen(): Promise<Geoeffnet> {
    throw new Error(
      "Im Browser gibt es keine Identität. Öffnen geht nur im Fenster.",
    );
  }

  // --- Empfangen -----------------------------------------------------------

  async envelopeWaehlen(): Promise<string | null> {
    return null;
  }

  /**
   * Liefert den Beispielfall, dessen Kennung im „Pfad“ steht.
   *
   * Im Browser gibt es keine Envelopes. Die Beispielfälle sind aber genau
   * die seltenen Lagen, um derentwillen es den Prototyp gibt — ein
   * widerrufener Absender etwa lässt sich sonst nirgends ansehen.
   */
  async envelopeOeffnen(pfad: string): Promise<Geoeffnet> {
    const fall = FAELLE.find((f) => f.kennung === pfad) ?? FAELLE[0];
    if (!fall) throw new Error("Kein Beispielfall vorhanden.");
    return fall.daten;
  }

  async nutzlastSpeichern(): Promise<string | null> {
    throw new Error(
      "Im Browser gibt es kein Dateisystem. Speichern geht nur im Fenster.",
    );
  }

  async nutzlastVerwerfen(): Promise<void> {}

  /**
   * Eine Beispiel-Nutzlast — dieselbe, die auch als Beispiel angeboten wird.
   *
   * Im Browser gibt es keine Identität. Etwas zurückzugeben ist trotzdem
   * richtig: Der Prototyp soll den Weg zeigen können, den jemand geht.
   */
  async eigeneNutzlast(): Promise<string> {
    return NUTZLASTEN[0]?.text.trim() ?? "";
  }

  async nutzlastAlsDatei(): Promise<string | null> {
    throw new Error(
      "Im Browser gibt es kein Dateisystem. Speichern geht nur im Fenster.",
    );
  }

  async nutzlastAusDatei(): Promise<string | null> {
    throw new Error(
      "Im Browser gibt es kein Dateisystem. Laden geht nur im Fenster.",
    );
  }

  async schluesselSichern(): Promise<string | null> {
    throw new Error(
      "Im Browser gibt es kein Dateisystem. Sichern geht nur im Fenster.",
    );
  }

  async passwortAendern(): Promise<void> {
    throw new Error(
      "Im Browser gibt es keine Schlüsseldatei. Das geht nur im Fenster.",
    );
  }

  // --- Sicheres Löschen ----------------------------------------------------

  /**
   * Beurteilt nichts — im Browser gibt es kein Dateisystem.
   *
   * Sie meldet das je Datei, statt eine leere Liste zurückzugeben: Eine
   * leere Liste hieße „nichts ausgewählt“, und das wäre eine andere
   * Aussage.
   */
  async loeschenBeurteilen(pfade: string[]): Promise<Loeschkandidat[]> {
    return pfade.map((pfad) => ({
      pfad,
      name: pfad.split(/[\\/]/).at(-1) ?? pfad,
      groesseBytes: 0,
      beurteilung: {
        faehigkeit: "nichtMoeglich",
        vorbehalte: [{ art: "kopienMoeglich" }],
      },
    }));
  }

  async loeschenAusfuehren(): Promise<Loeschergebnis[]> {
    throw new Error(
      "Im Browser gibt es kein Dateisystem. Löschen geht nur im Fenster.",
    );
  }

  // --- Kontakte ------------------------------------------------------------

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

/** Was das Fenster sagt, wenn es keine Identität gibt. */
const KEINE_IDENTITAET =
  "Auf diesem Rechner liegt noch keine Identität. Legen Sie unter " +
  "„Einrichtung“ eine an.";

/**
 * Die Speicherwerte der drei Stufen — **nur für die Attrappe**.
 *
 * Im Fenster kommen sie aus der Schlüsseldatei, gelesen von
 * `cabrik_core::keyfile::params_of`. Hier gibt es keine Datei, also muss
 * etwas Plausibles dastehen. Es ist die einzige Stelle im Frontend, an der
 * diese Zahlen überhaupt vorkommen, und sie erreicht das Fenster nie.
 */
const SPEICHER_JE_STUFE: Record<KdfStufe, number> = {
  min: 64,
  empfohlen: 256,
  stark: 1024,
};

/**
 * Ein Fingerprint zum Ansehen.
 *
 * Crockford-Base32 wie im Kern, dreizehn Gruppen zu vier Zeichen, mit
 * Bindestrichen getrennt — so gruppiert `Fingerprint::display_full`. Die
 * Ziffern stammen aus `crypto.getRandomValues` und nicht aus `Math.random`:
 * nicht weil hier etwas davon abhinge, sondern weil an dieser Stelle im
 * Fenster echtes Schlüsselmaterial steht und ein schwacher Zufall in einer
 * Vorlage eine schlechte Saat ist.
 */
function neuerFingerprint(): string {
  const ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
  const roh = new Uint8Array(52);
  globalThis.crypto.getRandomValues(roh);
  const zeichen = [...roh].map((b) => ALPHABET[b % 32]);
  return Array.from({ length: 13 }, (_, i) =>
    zeichen.slice(i * 4, i * 4 + 4).join(""),
  ).join("-");
}
