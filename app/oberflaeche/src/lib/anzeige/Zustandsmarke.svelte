<!--
  Die Anzeige eines Zustands: Farbe UND Zeichen UND Wort.

  Der Kern der ganzen Oberfläche steht in dieser Datei. Wer sie umgeht und
  irgendwo eine grüne Fläche malt, umgeht den Anzeigevertrag.

  Farbe ist hier Beschleuniger, nicht Träger: Nimmt man sie weg, bleibt eine
  vollständige Aussage stehen. Genau das ist die Probe, ob eine Anzeige
  taugt.

  HIER ERSCHEINT NIE CYAN ODER MAGENTA. Die beiden gehören zur
  Informationsachse (`app.css`) — sie sagen, WOHER ein Wert stammt, nicht
  WIE ES STEHT. In einer Zustandsmarke wären sie ein fünfter und sechster
  Zustand, und die Ampel hätte sechs Lichter.
-->
<script lang="ts">
  import { ZEICHEN, type Marke } from "./zustand";

  interface Props {
    marke: Marke;
    /** Groß für das Gesamturteil, klein für einen Punkt in einer Liste. */
    gross?: boolean;
  }

  let { marke, gross = false }: Props = $props();

  const stil = {
    bestaetigt: "bg-bestaetigt-grund border-bestaetigt-rand text-bestaetigt",
    warnung: "bg-warnung-grund border-warnung-rand text-warnung",
    fehler: "bg-fehler-grund border-fehler-rand text-fehler",
    keineAussage: "bg-keine-grund border-keine-rand text-keine rand-keine",
  } as const;

  // Für Vorlesegeräte: Die Farbe kommt dort nicht an, das Wort schon.
  const rolle = $derived(marke.zustand === "fehler" ? "alert" : "status");
</script>

<div
  class="flex items-start gap-3 rounded-lg border {stil[marke.zustand]} {gross
    ? 'p-4'
    : 'px-3 py-2'}"
  role={rolle}
>
  <span
    aria-hidden="true"
    class="flex shrink-0 items-center justify-center rounded-full border border-current font-bold {gross
      ? 'mt-0.5 h-7 w-7 text-base'
      : 'h-5 w-5 text-xs'}"
  >
    {ZEICHEN[marke.zustand]}
  </span>

  <div class="min-w-0">
    <p class="font-semibold {gross ? 'text-base' : 'text-sm'}">{marke.wort}</p>
    <p class="text-schrift mt-0.5 opacity-90 {gross ? 'text-sm' : 'text-xs'}">
      {marke.satz}
    </p>
  </div>
</div>
