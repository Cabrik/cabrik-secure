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
import type { Kontakt, Nutzlastbefund, Verifikationsweg } from "./typen";

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
