<!--
  Die einzelnen Funde.

  Die Schwere färbt den FUND, nicht das Gesamturteil. Ein kritischer Fund in
  einer vollständig bereinigten Datei ist kein Grund für eine Warnung — er
  ist ja weg. Deshalb steht hier nirgends eine Zustandsmarke.
-->
<script lang="ts">
  import type { Fund } from "../kern/typen";
  import { FUNDART_TEXT, nachSchwere } from "./zustand";

  interface Props {
    funde: Fund[];
    /** „Entfernt" oder „Geblieben" — der Unterschied ist der Kern der Sache. */
    ueberschrift: string;
    /** Bei Geblieben­em zählt jeder Eintrag; Entferntes darf sich zusammenklappen. */
    offen?: boolean;
  }

  let { funde, ueberschrift, offen = false }: Props = $props();
  const sortiert = $derived(nachSchwere(funde));

  const punkt = {
    kritisch: "bg-fehler",
    beachtlich: "bg-warnung",
    gering: "bg-slate-400",
  } as const;
</script>

{#if funde.length > 0}
  <details class="rounded-lg border border-slate-200 bg-white" open={offen}>
    <summary
      class="cursor-pointer list-none px-4 py-3 text-sm font-medium select-none hover:bg-slate-50"
    >
      {ueberschrift}
      <span class="ml-1 text-slate-500">({funde.length})</span>
    </summary>

    <ul class="border-t border-slate-100">
      {#each sortiert as fund (fund.ort + fund.art)}
        <li class="flex gap-3 border-b border-slate-50 px-4 py-2.5 last:border-0">
          <span
            class="mt-1.5 h-2 w-2 shrink-0 rounded-full {punkt[fund.schwere]}"
            aria-hidden="true"
          ></span>
          <div class="min-w-0">
            <p class="text-sm">
              <span class="font-medium">{FUNDART_TEXT[fund.art]}</span>
              <span class="text-slate-500">— {fund.schwere}</span>
            </p>
            {#if fund.wert}
              <p class="mt-0.5 text-sm break-words text-slate-700">{fund.wert}</p>
            {/if}
            <p class="mt-0.5 font-mono text-xs text-slate-400">{fund.ort}</p>
          </div>
        </li>
      {/each}
    </ul>
  </details>
{/if}
