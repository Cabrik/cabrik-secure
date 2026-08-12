<!--
  Die Hülle des Prototyps.

  Sie ist bewusst eine Fallauswahl und kein Programmfenster: In Phase 3 geht
  es darum, JEDEN Zustand ansehen und beurteilen zu können — auch die, die
  im Alltag selten vorkommen und gerade deshalb schlecht gestaltet werden.
-->
<script lang="ts">
  import { FAELLE } from "./lib/kern/mock";
  import Empfangen from "./lib/bildschirme/Empfangen.svelte";

  let gewaehlt = $state(FAELLE[0]!.kennung);
  const fall = $derived(FAELLE.find((f) => f.kennung === gewaehlt) ?? FAELLE[0]!);
</script>

<div class="mx-auto flex min-h-screen max-w-6xl gap-8 p-6">
  <nav class="w-72 shrink-0 space-y-1">
    <p class="px-3 pb-2 text-xs font-semibold tracking-wide text-slate-500 uppercase">
      Empfangen — Beispielfälle
    </p>
    {#each FAELLE as f (f.kennung)}
      <button
        class="w-full rounded-md px-3 py-2 text-left text-sm transition
               {gewaehlt === f.kennung
          ? 'bg-slate-900 text-white'
          : 'text-slate-700 hover:bg-slate-200'}"
        onclick={() => (gewaehlt = f.kennung)}
      >
        {f.titel}
      </button>
    {/each}

    <p class="px-3 pt-6 text-xs leading-relaxed text-slate-500">
      Prototyp mit Beispieldaten. Keine Anbindung an den Kern — die folgt in
      Phase 4.
    </p>
  </nav>

  <main class="min-w-0 flex-1 space-y-4">
    <p class="rounded-md border border-slate-200 bg-white px-4 py-3 text-sm text-slate-600">
      <span class="font-medium text-slate-900">Worum es hier geht:</span>
      {fall.worumEsGeht}
    </p>

    <div class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
      <Empfangen {fall} />
    </div>
  </main>
</div>
