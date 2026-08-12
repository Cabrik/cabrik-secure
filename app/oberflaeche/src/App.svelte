<!--
  Die Hülle des Prototyps.

  Sie ist bewusst eine Fallauswahl und kein Programmfenster: In Phase 3 geht
  es darum, JEDEN Zustand ansehen und beurteilen zu können — auch die, die
  im Alltag selten vorkommen und gerade deshalb schlecht gestaltet werden.
-->
<script lang="ts">
  import { FAELLE, IDENTITAET, IDENTITAET_V1, STAPEL } from "./lib/kern/mock";
  import Empfangen from "./lib/bildschirme/Empfangen.svelte";
  import Senden from "./lib/bildschirme/Senden.svelte";
  import Kontakte from "./lib/bildschirme/Kontakte.svelte";
  import Identitaet from "./lib/bildschirme/Identitaet.svelte";
  import Onboarding from "./lib/bildschirme/Onboarding.svelte";
  import Werkzeuge from "./lib/bildschirme/Werkzeuge.svelte";
  import { darstellung } from "./lib/anzeige/darstellung.svelte";

  type Bereich =
    | "empfangen"
    | "senden"
    | "kontakte"
    | "identitaet"
    | "onboarding"
    | "werkzeuge";

  let bereich = $state<Bereich>("empfangen");
  let fallKennung = $state(FAELLE[0]!.kennung);
  let stapelKennung = $state(STAPEL[0]!.kennung);
  let identitaetV1 = $state(false);

  const fall = $derived(FAELLE.find((f) => f.kennung === fallKennung) ?? FAELLE[0]!);
  const stapel = $derived(STAPEL.find((s) => s.kennung === stapelKennung) ?? STAPEL[0]!);

  const BEREICHE: { kennung: Bereich; name: string }[] = [
    { kennung: "empfangen", name: "Empfangen" },
    { kennung: "senden", name: "Senden" },
    { kennung: "kontakte", name: "Kontakte" },
    { kennung: "identitaet", name: "Identität" },
    { kennung: "werkzeuge", name: "Werkzeuge" },
    { kennung: "onboarding", name: "Einrichtung" },
  ];

  const ERLAEUTERUNG: Record<Exclude<Bereich, "empfangen" | "senden">, string> = {
    kontakte:
      "Beim Empfangen ist Vertrauen eine Anzeige. Hier ist es eine Handlung — " +
      "und deshalb ist „nicht verifiziert“ hier grau statt gelb: Als Eintrag im " +
      "Verzeichnis ist es erwartbar, denn so fängt jeder Kontakt an.",
    identitaet:
      "Der Bildschirm, dessen wichtigste Eigenschaft ist, was er nicht kann: " +
      "Es gibt keinen Knopf, der den privaten Schlüssel zeigt — und der Typ " +
      "dahinter hat gar kein Feld dafür.",
    werkzeuge:
      "Sicheres Löschen ist der Fall, an dem sich Ehrlichkeit entscheidet. " +
      "Version 1 hatte drei Überschreibdurchgänge voreingestellt und " +
      "suggerierte damit einen Nutzen, den es auf heutigen Datenträgern nicht " +
      "gibt.",
    onboarding:
      "Hier steht die unbequemste Entscheidung des Entwurfs: keine " +
      "Passwort-Stärkeanzeige. Ein Programm kann nicht wissen, ob ein Passwort " +
      "gut ist — es kennt die Liste nicht, in der es vielleicht steht.",
  };

  const erlaeuterung = $derived(
    bereich === "empfangen"
      ? fall.worumEsGeht
      : bereich === "senden"
        ? stapel.worumEsGeht
        : ERLAEUTERUNG[bereich],
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

    <!--
      Bereiche.

      `flex-wrap` ist hier nicht Kosmetik: Ohne den Umbruch laufen die
      Knöpfe aus der 18rem breiten Spalte heraus und verschwinden hinter
      dem Inhaltsbereich. Mit drei Bereichen fiel das nicht auf, mit sechs
      sofort.
    -->
    <div class="border-linie mb-4 flex flex-wrap gap-1 border-b px-1 pb-3">
      {#each BEREICHE as b (b.kennung)}
        <button
          class="rounded-md px-3 py-1.5 text-sm whitespace-nowrap transition
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
    {:else if bereich === "identitaet"}
      <p class="text-schrift-leise px-3 pb-2 text-xs font-semibold tracking-wide uppercase">
        Beispielidentität
      </p>
      <div class="space-y-1">
        {#each [{ v1: false, t: "Frisch erzeugt (v2)" }, { v1: true, t: "Aus Version 1 übernommen" }] as w (w.t)}
          <button
            class="w-full rounded-md px-3 py-2 text-left text-sm transition
                   {identitaetV1 === w.v1
              ? 'bg-schrift text-grund'
              : 'text-schrift hover:bg-flaeche'}"
            onclick={() => (identitaetV1 = w.v1)}
          >
            {w.t}
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
      {:else if bereich === "kontakte"}
        <Kontakte />
      {:else if bereich === "identitaet"}
        <Identitaet identitaet={identitaetV1 ? IDENTITAET_V1 : IDENTITAET} />
      {:else if bereich === "werkzeuge"}
        <Werkzeuge />
      {:else}
        <Onboarding />
      {/if}
    </div>
  </main>
</div>
