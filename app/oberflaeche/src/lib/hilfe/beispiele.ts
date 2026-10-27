/**
 * Welche Beispiele es gibt — und wie man sie aufruft.
 *
 * # Warum sie nicht mehr neben dem echten Arbeiten stehen
 *
 * Bis zum 27.10.2026 öffnete das Programm auf einem erfundenen Fall: einer
 * Nachricht von „Dr. Anna Beispiel“, mit Fingerprint und Zeitstempel. Wer
 * Cabrik zum ersten Mal startet, kann das nicht von einer echten Nachricht
 * unterscheiden — und ein Befund ist eine Aussage über eine wirkliche
 * Datei. Dieselbe Lage beim Löschen: erfundene Pfade, die echt genug
 * aussehen, um für eigene gehalten zu werden.
 *
 * Die Beispiele bleiben, weil sie zeigen, was das Programm kann, und weil
 * sich manche Lagen — ein Netzlaufwerk, ein gewechselter Schlüssel — auf
 * dem eigenen Rechner kaum herstellen lassen. Aber sie stehen jetzt im
 * Handbuch, und wo eines aufgeht, sagt ein Band darüber, dass es eines ist.
 */
import { FAELLE, STAPEL } from "../kern/mock";

/** Ein aufgerufenes Beispiel. */
export type Beispielwahl =
  | { art: "empfangen"; kennung: string }
  | { art: "senden"; kennung: string }
  | { art: "werkzeuge"; werkzeug: "loeschen" | "aussenansicht" };

export interface Beispielgruppe {
  titel: string;
  eintraege: { titel: string; wahl: Beispielwahl }[];
}

/** Die Gruppen im Kapitel „Beispiele“ — aus denselben Daten wie die Tests. */
export const BEISPIELGRUPPEN: Beispielgruppe[] = [
  {
    titel: "Nachrichten öffnen",
    eintraege: FAELLE.map((f) => ({
      titel: f.titel,
      wahl: { art: "empfangen" as const, kennung: f.kennung },
    })),
  },
  {
    titel: "Dateien senden",
    eintraege: STAPEL.map((s) => ({
      titel: s.titel,
      wahl: { art: "senden" as const, kennung: s.kennung },
    })),
  },
  {
    titel: "Werkzeuge",
    eintraege: [
      {
        titel: "Sicher löschen: Netzlaufwerk, Cloud-Ordner, schreibgeschützt",
        wahl: { art: "werkzeuge", werkzeug: "loeschen" },
      },
      {
        titel: "Was ein Mitleser von einer verschlüsselten Datei sieht",
        wahl: { art: "werkzeuge", werkzeug: "aussenansicht" },
      },
    ],
  },
];

/** Ein Schlüssel je Beispiel — für `{#key}` und Vergleiche. */
export function beispielKennung(w: Beispielwahl): string {
  return w.art === "werkzeuge" ? `werkzeuge:${w.werkzeug}` : `${w.art}:${w.kennung}`;
}
