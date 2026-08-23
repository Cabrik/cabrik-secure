<!--
  Wie weit ein Stapel ist.

  CYAN, weil es ein gelesener Wert ist und kein Urteil — dieselbe Achse wie
  Format, Größe und Fingerprint. Ein Fortschritt bewertet nichts; er sagt,
  wo das Programm gerade steht.

  DIE ZAHLEN STEHEN DA, NICHT NUR DER BALKEN. Ein Balken allein ist Farbe
  allein, und die genügt in diesem Programm nirgends: Wer ihn nicht sieht,
  weil er zu blass ist, weil der Bildschirm schlecht ist oder weil ihm ein
  Bildschirmleser vorliest, bekommt „3 von 40, Foto.jpg“ und weiß dasselbe.

  UND DER DATEINAME STEHT DABEI. „3 von 40“ allein sagt nicht, ob es hakt
  oder läuft. Bleibt eine Minute lang derselbe Name stehen, weiß man
  wenigstens, WELCHE Datei aufhält — und dass es nicht das Programm ist.
-->
<script lang="ts">
  import type { Stapelstand } from "../kern/typen";
  import { SCHRITT_TEXT, STAPELART_TEXT } from "./zustand";

  interface Props {
    /**
     * Der Stand samt der Auskunft, **wozu** er gehört.
     *
     * Ohne die Art wäre der Balken bei allen fünf Stapeln derselbe. Beim
     * Löschen ist das keine Kleinigkeit: Der Vorgang ist unwiderruflich,
     * und wer ihn mit dem Prüfen verwechselt, wartet gelassen auf etwas
     * anderes, als gerade geschieht.
     */
    fortschritt: Stapelstand;
  }
  let { fortschritt }: Props = $props();

  const was = $derived(STAPELART_TEXT[fortschritt.art]);

  /**
   * Der laufende Schritt vor dem Dateinamen — sofern er einen Namen hat.
   *
   * `arbeiten` liefert eine leere Zeichenkette, und dann steht hier nur
   * der Dateiname. Das ist ehrlicher als ein erfundener Zwischenschritt.
   */
  const schrittwort = $derived(SCHRITT_TEXT[fortschritt.schritt] ?? "");

  /*
   * Der Anteil in Prozent.
   *
   * `erledigt` zählt die FERTIGEN — die laufende ist noch nicht dabei. Bei
   * der ersten Datei steht der Balken deshalb auf null, und das stimmt: Es
   * ist noch nichts fertig. Ihn dort schon ein Vierzigstel weit zu füllen
   * hieße, eine Datei als erledigt zu zeigen, die gerade erst anfängt.
   */
  const anteil = $derived(
    fortschritt.gesamt > 0
      ? Math.round((fortschritt.erledigt / fortschritt.gesamt) * 100)
      : 0,
  );
</script>

<div
  class="border-linie bg-flaeche space-y-2 rounded-lg border p-4"
  role="status"
  aria-live="polite"
>
  <div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
    <span class="text-sm font-medium">{was}</span>
    <span class="text-bezug text-sm">
      {fortschritt.erledigt} von {fortschritt.gesamt}
    </span>
    <!--
      Der Name bricht um statt abzuschneiden. Ein gekürzter Dateiname
      („Rechnung_2026_…“) beantwortet die Frage nicht, für die er dasteht.
    -->
    <span class="text-schrift-leise min-w-0 text-xs break-all">
      {schrittwort ? `${schrittwort} ${fortschritt.laeuft}` : fortschritt.laeuft}
    </span>
  </div>

  <div
    class="bg-grund h-1.5 w-full overflow-hidden rounded-full"
    role="progressbar"
    aria-valuenow={fortschritt.erledigt}
    aria-valuemin={0}
    aria-valuemax={fortschritt.gesamt}
    aria-label={was}
  >
    <div
      class="bg-bezug h-full rounded-full transition-[width] duration-150"
      style="width: {anteil}%"
    ></div>
  </div>
</div>
