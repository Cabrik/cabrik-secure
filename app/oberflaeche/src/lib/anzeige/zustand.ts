/**
 * Die vier Anzeigezustände und ihre Zuordnung (`spec/anzeige.md`).
 *
 * # Warum das Funktionen sind und keine Absichtserklärung
 *
 * Ein Anzeigevertrag, der nur in einem Dokument steht, wird beim dritten
 * Bildschirm gebrochen — nicht aus Nachlässigkeit, sondern weil niemand
 * beim Schreiben eines Knopfes ein Kapitel nachschlägt.
 *
 * Deshalb steht die Zuordnung hier als reine Funktion mit Tests. Wer eine
 * Nachricht anders einordnen will, muss den Test ändern, und das fällt in
 * einer Durchsicht auf.
 */

import type { Absender, Bereinigung, Fund, Schwere } from "../kern/typen";

/**
 * Die vier Zustände.
 *
 * `keineAussage` ist der wichtigste: Er entspricht der Flagge am künstlichen
 * Horizont, die erscheint, wenn das Instrument seine Eingangsdaten verliert.
 * Er ist **kein** abgestuftes Gelb — Gelb heißt „ich weiß etwas", Grau heißt
 * „ich weiß es nicht".
 */
export type Zustand = "bestaetigt" | "warnung" | "fehler" | "keineAussage";

/** Was eine Anzeige braucht: Zustand, Zeichen, Wort. Farbe allein genügt nie. */
export interface Marke {
  zustand: Zustand;
  /** Die Kurzform. Trägt die Bedeutung, wenn die Farbe nicht ankommt. */
  wort: string;
  /** Der Satz darunter. Nennt, was tatsächlich getan oder festgestellt wurde. */
  satz: string;
}

/**
 * Zeichen je Zustand.
 *
 * Bewusst Text und kein Bildzeichen: Es überlebt jede Schriftart und jeden
 * Bildschirmleser, und es lässt sich vorlesen.
 */
export const ZEICHEN: Record<Zustand, string> = {
  bestaetigt: "✓",
  warnung: "!",
  fehler: "✕",
  keineAussage: "?",
};

// ---------------------------------------------------------------------------
// Metadaten
// ---------------------------------------------------------------------------

/**
 * Ordnet ein Bereinigungsergebnis ein.
 *
 * Zwei Regeln aus `spec/anzeige.md` §4.1 sind hier zwingend:
 *
 * 1. Bei `vollstaendig` wird **das Format genannt**. „Alle bekannten
 *    Metadaten entfernt" ohne Bezug wäre eine stärkere Aussage, als der
 *    Kern deckt.
 * 2. Bei `unbekannt` darf nichts stehen, was nach Erfolg klingt.
 */
export function markeFuerBereinigung(b: Bereinigung): Marke {
  switch (b.fall) {
    case "vollstaendig":
      return {
        zustand: "bestaetigt",
        wort: "Bereinigt",
        satz: `Alle bekannten Metadaten entfernt (${b.format}).`,
      };
    case "teilweise":
      return {
        zustand: "warnung",
        wort: "Teilweise bereinigt",
        satz: b.grund,
      };
    case "unbekannt":
      return {
        zustand: "keineAussage",
        wort: "Keine Aussage",
        satz: b.formathinweis
          ? `Erkannt als ${b.formathinweis}, aber nicht verstanden — über den Inhalt lässt sich nichts sagen.`
          : "Format nicht verstanden — über den Inhalt lässt sich nichts sagen.",
      };
    case "fehler":
      return { zustand: "fehler", wort: "Fehler", satz: b.grund };
  }
}

// ---------------------------------------------------------------------------
// Absender
// ---------------------------------------------------------------------------

/**
 * Ordnet den Absender ein.
 *
 * `signaturVerlangt` bildet ab, was die CLI mit `--require-signature` tut:
 * **Dieselbe Lage wird anders bewertet, je nachdem was der Nutzer verlangt
 * hat** — nicht danach, was das Programm für richtig hält.
 *
 * Ohne diese Unterscheidung müsste man sich entscheiden, ob eine unsignierte
 * Nachricht gelb ist. Beide Antworten wären falsch: Für jemanden, der
 * anonyme Zusendungen erwartet, ist sie der Normalfall; für jemanden, der
 * eine Zusicherung braucht, ist sie unbrauchbar.
 */
export function markeFuerAbsender(a: Absender, signaturVerlangt = false): Marke {
  switch (a.fall) {
    case "verifiziert":
      return {
        zustand: "bestaetigt",
        wort: a.name,
        satz: `Verifiziert am ${datum(a.verifiziertAm)}. Die Signatur stammt vom Inhaber des geprüften Schlüssels.`,
      };

    case "bekannt":
      return {
        zustand: "warnung",
        wort: `${a.name} — nicht verifiziert`,
        satz:
          "Der Name stammt aus Ihrem Kontaktspeicher, nicht aus einer Prüfung. " +
          "Vergleichen Sie die Safety Number, bevor Sie sich darauf verlassen.",
      };

    case "unbekannt":
      return {
        zustand: "keineAussage",
        wort: "Unbekannter Absender",
        satz:
          "Die Signatur ist gültig, aber der Schlüssel steht in keinem Ihrer Kontakte. " +
          "Das sagt nichts darüber, wer geschickt hat.",
      };

    case "gewechselt":
      return {
        zustand: "warnung",
        wort: `${a.name} — Schlüssel gewechselt`,
        satz: a.vorherVerifiziert
          ? "Der bisherige Schlüssel dieses Kontakts war von Ihnen verifiziert. Der neue ist es nicht. Fragen Sie auf einem anderen Weg nach, bevor Sie antworten."
          : "Dieser Kontakt benutzt einen anderen Schlüssel als bisher.",
      };

    case "widerrufen":
      return {
        zustand: "fehler",
        wort: `${a.name} — Schlüssel widerrufen`,
        satz:
          "Sie haben diesen Schlüssel als kompromittiert markiert. " +
          "Die Signatur ist gültig — das heißt hier gerade nichts Gutes.",
      };

    case "unsigniert":
      // Anonymer Versand ist ein legitimer Modus. Erst wenn der Nutzer eine
      // Signatur ausdrücklich verlangt hat, ist ihr Fehlen ein Fehler.
      return signaturVerlangt
        ? {
            zustand: "fehler",
            wort: "Nicht signiert",
            satz: "Sie haben eine Signatur verlangt. Diese Nachricht trägt keine.",
          }
        : {
            zustand: "keineAussage",
            wort: "Nicht signiert",
            satz: "Die Nachricht sagt nichts darüber, wer sie geschickt hat.",
          };
  }
}

// ---------------------------------------------------------------------------
// Einzelne Funde
// ---------------------------------------------------------------------------

/**
 * Wie ein einzelner Fund dargestellt wird.
 *
 * Die Schwere färbt **den Fund**, nicht das Gesamturteil. Ein einzelner
 * kritischer Fund in einer vollständig bereinigten Datei ist kein Grund für
 * eine Warnung — er ist ja weg.
 */
export const SCHWERE_RANG: Record<Schwere, number> = {
  kritisch: 0,
  beachtlich: 1,
  gering: 2,
};

/** Sortiert Funde: Schwerwiegendes zuerst, sonst nach Fundstelle. */
export function nachSchwere(funde: readonly Fund[]): Fund[] {
  return [...funde].sort(
    (a, b) => SCHWERE_RANG[a.schwere] - SCHWERE_RANG[b.schwere] || a.ort.localeCompare(b.ort),
  );
}

/** Menschenlesbarer Name einer Fundart. */
export const FUNDART_TEXT: Record<Fund["art"], string> = {
  ortsangabe: "Ortsangabe",
  personenname: "Personenname",
  geraet: "Gerät oder Seriennummer",
  software: "erzeugende Software",
  zeitangabe: "Zeitangabe",
  organisation: "Firmen- oder Organisationsname",
  vorschaubild: "eingebettetes Vorschaubild",
  zugeschnittenes_bild: "zugeschnittenes Bild",
  nachverfolgte_aenderung: "nachverfolgte Änderung",
  farbprofil: "Farbprofil",
  kommentar: "Kommentar",
  bearbeitungssitzung: "Bearbeitungssitzung",
  dateiname: "ursprünglicher Dateiname",
  unbekannte_erweiterung: "unbekannte Erweiterung",
};

// ---------------------------------------------------------------------------
// Kleinkram
// ---------------------------------------------------------------------------

function datum(unixSekunden: number): string {
  return new Date(unixSekunden * 1000).toLocaleDateString("de-DE", {
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}

/** Größenangabe, die sich vorlesen lässt. */
export function groesse(bytes: number): string {
  if (bytes < 1024) return `${bytes} Bytes`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
}
