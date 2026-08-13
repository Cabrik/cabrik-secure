/**
 * Kleine Hilfen für Tests, die Runen brauchen.
 *
 * `$state` gibt es nur in `.svelte`- und `.svelte.ts`-Dateien. Ein Test in
 * einer gewöhnlichen `.ts` kann deshalb keine veränderlichen Props bauen —
 * und genau das braucht man, um zu prüfen, ob ein Bildschirm beim Wechsel
 * seiner Eingabe richtig nachzieht. Das war der Fehler, den diese Datei
 * prüfbar macht: Ausnahmen aus einem Stapel wirkten im nächsten weiter.
 */

import { flushSync } from "svelte";

/**
 * Ein veränderlicher Behälter für Props.
 *
 * `$state` darf nur eine Variable oder ein Klassenfeld initialisieren,
 * nicht direkt zurückgegeben werden — daher die Klasse.
 */
class Behaelter<T> {
  wert = $state<T>() as T;

  constructor(anfang: T) {
    this.wert = anfang;
  }
}

/** Macht ein Objekt reaktiv, damit `mount` es als veränderliche Props nimmt. */
export function reaktiv<T extends object>(anfang: T): T {
  return new Behaelter(anfang).wert;
}

/**
 * Wartet ab, bis eine über die Brücke angestoßene Änderung angekommen ist.
 *
 * Seit der Speicher asynchron ist, genügt `flushSync()` nicht mehr: Es
 * spült Sveltes eigene Warteschlange, nicht die Versprechen davor. Erst
 * muss die Mikrotask-Schlange leer sein, dann darf gezeichnet werden.
 *
 * Dass jeder Test das braucht, ist keine Umständlichkeit, sondern die
 * Wahrheit über die Anwendung: Zwischen Klick und Anzeige liegt ein
 * Aufruf, der dauern kann.
 */
export async function abgewickelt() {
  await new Promise((weiter) => setTimeout(weiter, 0));
  flushSync();
}
