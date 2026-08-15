<!--
  Die einzelnen Funde.

  Die Schwere färbt den FUND, nicht das Gesamturteil. Ein kritischer Fund in
  einer vollständig bereinigten Datei ist kein Grund für eine Warnung — er
  ist ja weg. Deshalb steht hier nirgends eine Zustandsmarke.

  Die Fundstelle (`Video:udta/©xyz`) steht in Cyan: Sie ist ein Wert, den
  das Programm gelesen hat, kein Urteil.
-->
<script lang="ts">
  import type { Fund } from "../kern/typen";
  import { FUNDART_TEXT, nachSchwere } from "./zustand";

  interface Props {
    funde: Fund[];
    /**
     * Was die Liste zeigt — beim Senden „Entfernt“ oder „Geblieben“, beim
     * Empfangen „Das steht in der Datei“. Der Unterschied ist der Kern der
     * Sache: Dieselben Funde bedeuten je nach Richtung etwas anderes.
     */
    ueberschrift: string;
    /**
     * Aufgeklappt starten.
     *
     * Zusammenklappen darf sich nur, was erledigt ist — Entferntes. Was
     * bleibt oder was gerade angekommen ist, zählt Eintrag für Eintrag.
     */
    offen?: boolean;
  }

  let { funde, ueberschrift, offen = false }: Props = $props();
  const sortiert = $derived(nachSchwere(funde));

  const punkt = {
    kritisch: "bg-fehler",
    beachtlich: "bg-warnung",
    gering: "bg-keine",
  } as const;
</script>

{#if funde.length > 0}
  <details class="border-linie bg-flaeche rounded-lg border" open={offen}>
    <summary
      class="hover:bg-grund cursor-pointer list-none px-4 py-3 text-sm font-medium select-none"
    >
      {ueberschrift}
      <span class="text-schrift-leise ml-1">({funde.length})</span>
    </summary>

    <ul class="border-linie border-t">
      <!--
      Ohne Schlüssel. Ein Schlüssel muss eindeutig sein, und Fundstelle
      und Art sind es nicht: Ein großes ICC-Farbprofil wird über
      mehrere APP2-Segmente verteilt, und jedes ergibt einen eigenen
      Fund an derselben Stelle. Svelte bricht dann beim Zeichnen ab --
      und ein Bildschirm, der mitten im Aufbau abbricht, bleibt einfach
      stehen. Von außen sah das aus wie ein toter Knopf.

      Diese Liste braucht ohnehin keine Kennung: Sie wird nicht
      umsortiert und nicht teilweise ersetzt, sondern ganz neu gebaut,
      wenn die Datei wechselt.
    -->
    {#each sortiert as fund}
        <li class="border-linie flex gap-3 border-b px-4 py-2.5 last:border-0">
          <span
            class="mt-1.5 h-2 w-2 shrink-0 rounded-full {punkt[fund.schwere]}"
            aria-hidden="true"
          ></span>
          <div class="min-w-0">
            <p class="text-sm">
              <span class="font-medium">{FUNDART_TEXT[fund.art]}</span>
              <span class="text-schrift-leise">— {fund.schwere}</span>
            </p>
            {#if fund.wert}
              <p class="text-schrift mt-0.5 text-sm break-words opacity-90">{fund.wert}</p>
            {/if}
            <p class="text-bezug mt-0.5 font-mono text-xs opacity-80">{fund.ort}</p>
          </div>
        </li>
      {/each}
    </ul>
  </details>
{/if}
