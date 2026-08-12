<!--
  Die Hülle des Prototyps.

  Sie ist bewusst eine Fallauswahl und kein Programmfenster: In Phase 3 geht
  es darum, JEDEN Zustand ansehen und beurteilen zu können — auch die, die
  im Alltag selten vorkommen und gerade deshalb schlecht gestaltet werden.
-->
<script lang="ts">
  import { FAELLE } from "./lib/kern/mock";
  import Empfangen from "./lib/bildschirme/Empfangen.svelte";
  import { darstellung } from "./lib/anzeige/darstellung.svelte";

  let gewaehlt = $state(FAELLE[0]!.kennung);
  const fall = $derived(FAELLE.find((f) => f.kennung === gewaehlt) ?? FAELLE[0]!);
</script>

<div class="mx-auto flex min-h-screen max-w-6xl gap-8 p-6">
  <nav class="w-72 shrink-0">
    <!-- Die Wortmarke in der Logofarbe. -->
    <div class="flex items-baseline justify-between px-3 pb-4">
      <p class="text-lg font-semibold tracking-tight">
        Cabrik<span class="text-bezug">Secure</span>
      </p>
      <button
        class="border-linie text-schrift-leise hover:text-schrift rounded border px-2 py-1 text-xs"
        onclick={() => darstellung.umschalten()}
        aria-label="Zwischen hellem und dunklem Modus wechseln"
      >
        {darstellung.modus === "dunkel" ? "hell" : "dunkel"}
      </button>
    </div>

    <p class="text-schrift-leise px-3 pb-2 text-xs font-semibold tracking-wide uppercase">
      Empfangen — Beispielfälle
    </p>
    <div class="space-y-1">
      {#each FAELLE as f (f.kennung)}
        <button
          class="w-full rounded-md px-3 py-2 text-left text-sm transition
                 {gewaehlt === f.kennung
            ? 'bg-schrift text-grund'
            : 'text-schrift hover:bg-flaeche'}"
          onclick={() => (gewaehlt = f.kennung)}
        >
          {f.titel}
        </button>
      {/each}
    </div>

    <p class="text-schrift-leise px-3 pt-6 text-xs leading-relaxed">
      Prototyp mit Beispieldaten. Keine Anbindung an den Kern — die folgt in
      Phase 4.
    </p>
  </nav>

  <main class="min-w-0 flex-1 space-y-4">
    <p class="border-linie bg-flaeche text-schrift-leise rounded-md border px-4 py-3 text-sm">
      <span class="text-schrift font-medium">Worum es hier geht:</span>
      {fall.worumEsGeht}
    </p>

    <div class="border-linie bg-flaeche rounded-xl border p-6">
      <Empfangen {fall} />
    </div>
  </main>
</div>
