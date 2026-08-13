/**
 * Kleine Hilfen für Tests, die Runen brauchen.
 *
 * `$state` gibt es nur in `.svelte`- und `.svelte.ts`-Dateien. Ein Test in
 * einer gewöhnlichen `.ts` kann deshalb keine veränderlichen Props bauen —
 * und genau das braucht man, um zu prüfen, ob ein Bildschirm beim Wechsel
 * seiner Eingabe richtig nachzieht. Das war der Fehler, den diese Datei
 * prüfbar macht: Ausnahmen aus einem Stapel wirkten im nächsten weiter.
 */

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
