<!--
  Die Hülle des Prototyps.

  Sie ist bewusst eine Fallauswahl und kein Programmfenster: In Phase 3 geht
  es darum, JEDEN Zustand ansehen und beurteilen zu können — auch die, die
  im Alltag selten vorkommen und gerade deshalb schlecht gestaltet werden.
-->
<script lang="ts">
  import { FAELLE, STAPEL } from "./lib/kern/mock";
  import Empfangen from "./lib/bildschirme/Empfangen.svelte";
  import Senden from "./lib/bildschirme/Senden.svelte";
  import Kontakte from "./lib/bildschirme/Kontakte.svelte";
  import { darstellung } from "./lib/anzeige/darstellung.svelte";

  type Bereich = "empfangen" | "senden" | "kontakte";

  let bereich = $state<Bereich>("empfangen");
  let fallKennung = $state(FAELLE[0]!.kennung);
  let stapelKennung = $state(STAPEL[0]!.kennung);

  const fall = $derived(FAELLE.find((f) => f.kennung === fallKennung) ?? FAELLE[0]!);
  const stapel = $derived(STAPEL.find((s) => s.kennung === stapelKennung) ?? STAPEL[0]!);

  const BEREICHE: { kennung: Bereich; name: string }[] = [
    { kennung: "empfangen", name: "Empfangen" },
    { kennung: "senden", name: "Senden" },
    { kennung: "kontakte", name: "Kontakte" },
  ];

  const erlaeuterung = $derived(
    bereich === "empfangen"
      ? fall.worumEsGeht
      : bereich === "senden"
        ? stapel.worumEsGeht
        : "Beim Empfangen ist Vertrauen eine Anzeige. Hier ist es eine Handlung — " +
          "und deshalb ist „nicht verifiziert“ hier grau statt gelb: Als Eintrag im " +
          "Verzeichnis ist es erwartbar, denn so fängt jeder Kontakt an.",
  );
</script>

<div class="mx-auto flex min-h-screen max-w-7xl gap-8 p-6">
  <nav class="w-72 shrink-0">
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

    <!-- Bereiche -->
    <div class="border-linie mb-4 flex gap-1 border-b px-1 pb-3">
      {#each BEREICHE as b (b.kennung)}
        <button
          class="rounded-md px-3 py-1.5 text-sm transition
                 {bereich === b.kennung
            ? 'bg-schrift text-grund font-medium'
            : 'text-schrift-leise hover:bg-flaeche'}"
          onclick={() => (bereich = b.kennung)}
        >
          {b.name}
        </button>
      {/each}
    </div>

    {#if bereich === "empfangen"}
      <p class="text-schrift-leise px-3 pb-2 text-xs font-semibold tracking-wide uppercase">
        Beispielfälle
      </p>
      <div class="space-y-1">
        {#each FAELLE as f (f.kennung)}
          <button
            class="w-full rounded-md px-3 py-2 text-left text-sm transition
                   {fallKennung === f.kennung
              ? 'bg-schrift text-grund'
              : 'text-schrift hover:bg-flaeche'}"
            onclick={() => (fallKennung = f.kennung)}
          >
            {f.titel}
          </button>
        {/each}
      </div>
    {:else if bereich === "senden"}
      <p class="text-schrift-leise px-3 pb-2 text-xs font-semibold tracking-wide uppercase">
        Beispielstapel
      </p>
      <div class="space-y-1">
        {#each STAPEL as s (s.kennung)}
          <button
            class="w-full rounded-md px-3 py-2 text-left text-sm transition
                   {stapelKennung === s.kennung
              ? 'bg-schrift text-grund'
              : 'text-schrift hover:bg-flaeche'}"
            onclick={() => (stapelKennung = s.kennung)}
          >
            {s.titel}
          </button>
        {/each}
      </div>
    {/if}

    <p class="text-schrift-leise px-3 pt-6 text-xs leading-relaxed">
      Prototyp mit Beispieldaten. Keine Anbindung an den Kern — die folgt in
      Phase 4.
    </p>
  </nav>

  <main class="min-w-0 flex-1 space-y-4">
    <p class="border-linie bg-flaeche text-schrift-leise rounded-md border px-4 py-3 text-sm">
      <span class="text-schrift font-medium">Worum es hier geht:</span>
      {erlaeuterung}
    </p>

    <div class="border-linie bg-flaeche rounded-xl border p-6">
      {#if bereich === "empfangen"}
        <Empfangen {fall} />
      {:else if bereich === "senden"}
        <Senden {stapel} />
      {:else}
        <Kontakte />
      {/if}
    </div>
  </main>
</div>
