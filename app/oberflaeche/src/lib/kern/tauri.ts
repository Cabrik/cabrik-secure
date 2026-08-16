/**
 * Die Brücke, die den Kern tatsächlich ruft.
 *
 * # Warum sie so kurz ist
 *
 * Weil sie es sein soll. Sie erfüllt dieselbe Schnittstelle wie
 * [`MockBruecke`] und tut sonst nichts: Argumente hinüber, Antwort zurück.
 * Alles, was etwas entscheidet, steht in `cabrik-app` und ist dort ohne
 * Tauri geprüft.
 *
 * Wenn also etwas nicht geht, liegt es an dieser Datei oder an Tauri — nicht
 * an den Regeln darunter, denn die haben ihre Tests. Das war der ganze Sinn
 * der Reihenfolge (Leitprinzip 2).
 *
 * # Warum `invoke` erst beim Aufruf geholt wird
 *
 * `@tauri-apps/api` gibt es nur im Fenster. Im Browser — und in den Tests —
 * ist es nicht da, und ein Import an der Dateispitze risse alles mit. Der
 * dynamische Import macht diese Datei überall ladbar und scheitert nur
 * dort, wo tatsächlich gerufen wird.
 *
 * # Warum die Fehler weitergereicht werden
 *
 * Ein Tauri-Befehl gibt bei Fehlschlag eine **Zeichenkette** zurück — den
 * Satz, den `cabrik-app` für die Anzeige formuliert hat. Diese Datei fasst
 * ihn nicht neu: Wer hier umformuliert, verschiebt eine Entscheidung über
 * Wortlaut in eine Schicht, die keine Tests dafür hat.
 */

import type { Bruecke } from "./bruecke";
import type {
  Fortschritt,
  Fortschrittsmelder,
  Geoeffnet,
  QrCode,
  Loeschergebnis,
  Loeschkandidat,
  Kontakt,
  Nutzlastbefund,
  Identitaet,
  KdfStufe,
  Sendedatei,
  Sitzungsstand,
  Speicherergebnis,
  Sperrfrist,
  Versandbericht,
  Verifikationsweg,
  Ziehereignis,
} from "./typen";

/** Ob die Anwendung in einem Tauri-Fenster läuft. */
export function imFenster(): boolean {
  return (
    typeof globalThis !== "undefined" && "__TAURI_INTERNALS__" in globalThis
  );
}

type Aufruf = <T>(befehl: string, args?: Record<string, unknown>) => Promise<T>;

let geholt: Aufruf | null = null;

async function invoke(): Promise<Aufruf> {
  if (!geholt) {
    const api = await import("@tauri-apps/api/core");
    geholt = api.invoke as Aufruf;
  }
  return geholt;
}

/**
 * Verpackt einen Fortschritts-Rückruf in einen Tauri-Kanal.
 *
 * # Warum ein Kanal und kein Ereignis
 *
 * Ein globales Ereignis („fortschritt“) wäre für alle Stapel dasselbe. Zwei
 * Vorgänge gleichzeitig — Löschen läuft, jemand zieht Dateien ins
 * Sendefenster — schrieben dann in dieselbe Anzeige. Ein Kanal gehört zu
 * **diesem einen Aufruf** und endet mit ihm; es gibt keinen Zuhörer, der
 * hängen bleibt, und keinen Namen, der kollidiert.
 *
 * Er braucht auch keine Berechtigung in `capabilities/`: Ein Kanal läuft
 * über den Antwortweg des Aufrufs, nicht über das Ereignissystem.
 */
async function kanal(melden: Fortschrittsmelder): Promise<unknown> {
  const { Channel } = await import("@tauri-apps/api/core");
  const k = new Channel<Fortschritt>();
  k.onmessage = melden;
  return k;
}

/**
 * Die Brücke zum Kern.
 *
 * Die Namen der Befehle stimmen mit `crates/cabrik-fenster/src/main.rs`
 * überein. Sie stehen hier als Zeichenketten, und das ist die einzige
 * Stelle im ganzen Aufbau, an der etwas auseinanderlaufen kann, ohne dass
 * ein Übersetzer es merkt — deshalb prüft ein Test sie gegen die Rust-Datei.
 */
export class TauriBruecke implements Bruecke {
  // --- Sitzung -------------------------------------------------------------

  /**
   * Wie es um die Sitzung steht.
   *
   * `null` heißt: Es gibt noch **keine Identität** auf diesem Rechner. Das
   * ist etwas anderes als „gesperrt“ — im einen Fall führt der Weg zur
   * Einrichtung, im anderen zum Passwortfeld.
   */
  async sitzungsstand(): Promise<Sitzungsstand | null> {
    return (await invoke())("sitzungsstand");
  }

  /**
   * Entsperrt mit einem Passwort.
   *
   * Das Passwort geht als gewöhnliches Argument hinüber. Die Kopien, die
   * dabei entstehen — die JavaScript-Zeichenkette und der Übergabepuffer —
   * lassen sich nicht überschreiben; erst der Kern fasst es in `Zeroizing`
   * (`spec/entsperrung.md` §5.1). Der Aufrufer leert das Eingabefeld
   * unmittelbar danach.
   */
  async entsperren(passwort: string): Promise<void> {
    return (await invoke())("entsperren", { passwort });
  }

  async sperren(): Promise<void> {
    return (await invoke())("sperren");
  }

  async fristSetzen(frist: Sperrfrist): Promise<void> {
    return (await invoke())("frist_setzen", { frist });
  }

  /**
   * Meldet Tätigkeit. Gedrosselt vom Aufrufer, nicht hier.
   *
   * Der Befehl im Fenster gibt auch dann nichts zurück, wenn es gar keine
   * Sitzung gibt: Eine Fehlermeldung, die bei jedem Tastendruck erscheinen
   * kann, wäre binnen Minuten unerträglich.
   */
  async taetigkeit(): Promise<void> {
    return (await invoke())("taetigkeit");
  }

  // --- Identität -----------------------------------------------------------

  async identitaet(): Promise<Identitaet> {
    return (await invoke())("identitaet");
  }

  /**
   * Das Passwort geht denselben Weg wie beim Entsperren — durch, nicht
   * hinein. Der Aufrufer leert sein Eingabefeld unmittelbar danach.
   */
  async identitaetAnlegen(
    bezeichnung: string | null,
    passwort: string,
    mitSignierschluessel: boolean,
    stufe: KdfStufe,
  ): Promise<Identitaet> {
    return (await invoke())("identitaet_anlegen", {
      bezeichnung,
      passwort,
      mitSignierschluessel,
      stufe,
    });
  }

  async identitaetLoeschen(): Promise<void> {
    return (await invoke())("identitaet_loeschen");
  }

  // --- Dateien -------------------------------------------------------------

  async dateienWaehlen(): Promise<string[]> {
    return (await invoke())("dateien_waehlen");
  }

  /**
   * Hängt sich an das Ziehen-und-Fallenlassen des Fensters.
   *
   * `over` wird **nicht** weitergereicht: Es feuert bei jeder Mausbewegung
   * über dem Fenster, und der Aufrufer soll daraus keine Arbeit machen.
   * `enter` und `leave` genügen, um zu zeigen, dass das Fenster annimmt.
   */
  async aufDateienGezogen(
    melde: (e: Ziehereignis) => void,
  ): Promise<() => void> {
    const { getCurrentWebview } = await import("@tauri-apps/api/webview");
    return getCurrentWebview().onDragDropEvent((e) => {
      switch (e.payload.type) {
        case "enter":
          melde({ art: "drueber" });
          break;
        case "leave":
          melde({ art: "weg" });
          break;
        case "drop":
          melde({ art: "fallen", pfade: e.payload.paths });
          break;
        default:
          break;
      }
    });
  }

  async dateienPruefen(
    pfade: string[],
    melden: Fortschrittsmelder,
  ): Promise<Sendedatei[]> {
    const fortschritt = await kanal(melden);
    return (await invoke())("dateien_pruefen", { pfade, fortschritt });
  }

  async bereinigtSpeichern(
    pfade: string[],
    melden: Fortschrittsmelder,
  ): Promise<Speicherergebnis[]> {
    const fortschritt = await kanal(melden);
    return (await invoke())("bereinigt_speichern", { pfade, fortschritt });
  }

  async verschluesseln(
    pfade: string[],
    empfaenger: string[],
    signieren: boolean,
    original: string[],
    melden: Fortschrittsmelder,
  ): Promise<Versandbericht> {
    const fortschritt = await kanal(melden);
    return (await invoke())("verschluesseln", {
      pfade,
      empfaenger,
      signieren,
      original,
      fortschritt,
    });
  }

  async textVerschluesseln(
    text: string,
    empfaenger: string[],
    signieren: boolean,
  ): Promise<string> {
    return (await invoke())("text_verschluesseln", {
      text,
      empfaenger,
      signieren,
    });
  }

  async textOeffnen(text: string, signaturVerlangt: boolean): Promise<Geoeffnet> {
    return (await invoke())("text_oeffnen", { text, signaturVerlangt });
  }

  // --- Empfangen -----------------------------------------------------------

  async envelopeWaehlen(): Promise<string | null> {
    return (await invoke())("envelope_waehlen");
  }

  async envelopeOeffnen(
    pfad: string,
    signaturVerlangt: boolean,
  ): Promise<Geoeffnet> {
    return (await invoke())("envelope_oeffnen", { pfad, signaturVerlangt });
  }

  async nutzlastSpeichern(): Promise<string | null> {
    return (await invoke())("nutzlast_speichern");
  }

  async nutzlastVerwerfen(): Promise<void> {
    return (await invoke())("nutzlast_verwerfen");
  }

  async eigeneNutzlast(): Promise<string> {
    return (await invoke())("eigene_nutzlast");
  }

  async nutzlastAlsQr(): Promise<QrCode> {
    return (await invoke())("nutzlast_als_qr");
  }

  async nutzlastAlsDatei(): Promise<string | null> {
    return (await invoke())("nutzlast_als_datei");
  }

  async nutzlastAusDatei(): Promise<string | null> {
    return (await invoke())("nutzlast_aus_datei");
  }

  async schluesselSichern(): Promise<string | null> {
    return (await invoke())("schluessel_sichern");
  }

  async passwortAendern(alt: string, neu: string): Promise<void> {
    return (await invoke())("passwort_aendern", { alt, neu });
  }

  // --- Sicheres Löschen ----------------------------------------------------

  async loeschenBeurteilen(
    pfade: string[],
    melden: Fortschrittsmelder,
  ): Promise<Loeschkandidat[]> {
    const fortschritt = await kanal(melden);
    return (await invoke())("loeschen_beurteilen", { pfade, fortschritt });
  }

  async loeschenAusfuehren(
    pfade: string[],
    durchgaenge: number,
    melden: Fortschrittsmelder,
  ): Promise<Loeschergebnis[]> {
    const fortschritt = await kanal(melden);
    return (await invoke())("loeschen_ausfuehren", {
      pfade,
      durchgaenge,
      fortschritt,
    });
  }

  // --- Kontakte ------------------------------------------------------------

  async kontakte(): Promise<Kontakt[]> {
    return (await invoke())("kontakte");
  }

  async nutzlastLesen(nutzlast: string): Promise<Nutzlastbefund> {
    return (await invoke())("nutzlast_lesen", { nutzlast });
  }

  async kontaktAufnehmen(name: string, nutzlast: string): Promise<Kontakt> {
    return (await invoke())("kontakt_aufnehmen", { name, nutzlast });
  }

  async kontaktVerifizieren(
    fingerprint: string,
    weg: Verifikationsweg,
  ): Promise<Kontakt> {
    return (await invoke())("kontakt_verifizieren", { fingerprint, weg });
  }

  async kontaktZuruecksetzen(fingerprint: string): Promise<Kontakt> {
    return (await invoke())("kontakt_zuruecksetzen", { fingerprint });
  }

  async kontaktWiderrufen(fingerprint: string): Promise<Kontakt> {
    return (await invoke())("kontakt_widerrufen", { fingerprint, grund: null });
  }

  async kontaktLoeschen(fingerprint: string): Promise<void> {
    return (await invoke())("kontakt_loeschen", { fingerprint });
  }
}
