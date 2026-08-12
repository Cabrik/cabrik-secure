/**
 * Heller oder dunkler Modus.
 *
 * Die Systemeinstellung ist die Voreinstellung — wer sein Betriebssystem
 * dunkel gestellt hat, will es hier auch. Umschalten muss trotzdem gehen:
 * In einem dunklen Raum mit hellem System soll niemand das ganze System
 * umstellen müssen, um eine Datei zu prüfen.
 */

export type Modus = "hell" | "dunkel";

const SCHLUESSEL = "cabrik.darstellung";

function ausSystem(): Modus {
  return globalThis.matchMedia?.("(prefers-color-scheme: dark)").matches ? "dunkel" : "hell";
}

function gespeichert(): Modus | null {
  const v = globalThis.localStorage?.getItem(SCHLUESSEL);
  return v === "hell" || v === "dunkel" ? v : null;
}

class Darstellung {
  modus = $state<Modus>(gespeichert() ?? ausSystem());

  constructor() {
    $effect.root(() => {
      $effect(() => {
        document.documentElement.classList.toggle("dunkel", this.modus === "dunkel");
        globalThis.localStorage?.setItem(SCHLUESSEL, this.modus);
      });
    });
  }

  umschalten() {
    this.modus = this.modus === "dunkel" ? "hell" : "dunkel";
  }
}

export const darstellung = new Darstellung();
