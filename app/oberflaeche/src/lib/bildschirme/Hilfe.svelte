<!--
  Der Bildschirm „Hilfe“: ein Kapitel des Handbuchs, und im letzten die
  Beispiele.

  Die Kapitel stehen links in der Leiste, wie die Unterpunkte der anderen
  Bereiche. Hier steht nur, was eines sagt — und wohin es weiterführt.
-->
<script lang="ts">
  import { KAPITEL, type Ziel } from "../hilfe/handbuch";
  import { BEISPIELGRUPPEN, beispielKennung } from "../hilfe/beispiele";

  interface Props {
    /** Welches Kapitel. Ein unbekanntes zeigt das erste. */
    kennung: string;
    /** Führt einen Verweis aus — in einen Bereich, ein Kapitel, ein Beispiel. */
    oeffnen: (ziel: Ziel) => void;
  }
  let { kennung, oeffnen }: Props = $props();

  const kapitel = $derived(KAPITEL.find((k) => k.kennung === kennung) ?? KAPITEL[0]!);

  /**
   * `**fett**` in Abschnitte zerlegt.
   *
   * Alles bleibt Text: Svelte setzt `{t.text}` als Textknoten ein, ein
   * `<` im Handbuch wäre ein Zeichen und kein Tag. `{@html}` gibt es hier
   * nicht — auch nicht für Text aus der eigenen Quelle, damit es nirgends
   * erst einmal steht.
   */
  function teile(text: string): { fett: boolean; text: string }[] {
    return text
      .split("**")
      .map((t, i) => ({ fett: i % 2 === 1, text: t }))
      .filter((t) => t.text !== "");
  }
</script>

{#snippet satz(text: string)}{#each teile(text) as t}{#if t.fett}<strong
        class="font-semibold">{t.text}</strong
      >{:else}{t.text}{/if}{/each}{/snippet}

<article class="max-w-3xl space-y-4" data-pruefstelle="handbuch">
  <h2 class="text-xl font-semibold">{kapitel.titel}</h2>

  {#each kapitel.inhalt as b, i (i)}
    {#if b.art === "absatz"}
      <p class="text-sm leading-relaxed">{@render satz(b.text)}</p>
    {:else if b.art === "hinweis"}
      <p class="border-linie bg-grund rounded-lg border p-3 text-sm leading-relaxed">
        {@render satz(b.text)}
      </p>
    {:else if b.art === "liste"}
      <ul class="list-disc space-y-1.5 pl-5 text-sm leading-relaxed">
        {#each b.punkte as p}
          <li>{@render satz(p)}</li>
        {/each}
      </ul>
    {:else if b.art === "schritte"}
      <ol class="list-decimal space-y-1.5 pl-5 text-sm leading-relaxed">
        {#each b.punkte as p}
          <li>{@render satz(p)}</li>
        {/each}
      </ol>
    {:else if b.art === "verweis"}
      <button
        class="border-linie hover:bg-grund rounded-md border px-3 py-1.5 text-sm"
        onclick={() => oeffnen(b.ziel)}
      >
        {b.text} →
      </button>
    {/if}
  {/each}

  {#if kapitel.kennung === "beispiele"}
    {#each BEISPIELGRUPPEN as g (g.titel)}
      <section class="space-y-2 pt-2">
        <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
          {g.titel}
        </h3>
        <div class="flex flex-col items-start gap-1">
          {#each g.eintraege as e (beispielKennung(e.wahl))}
            <button
              class="hover:bg-grund rounded-md px-3 py-1.5 text-left text-sm"
              onclick={() => oeffnen({ art: "beispiel", beispiel: e.wahl })}
            >
              {e.titel} →
            </button>
          {/each}
        </div>
      </section>
    {/each}
  {/if}
</article>
