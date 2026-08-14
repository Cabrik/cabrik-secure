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
  Kontakt,
  Nutzlastbefund,
  Identitaet,
  KdfStufe,
  Sitzungsstand,
  Sperrfrist,
  Verifikationsweg,
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
