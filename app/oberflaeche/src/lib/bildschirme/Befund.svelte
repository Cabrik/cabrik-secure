<!--
  Der vollständige Metadatenbefund einer Datei — und die Wahl, welche
  Fassung hinausgeht.

  WARUM ALLES GEZEIGT WIRD, NICHT NUR DAS VERBLEIBENDE.
  Zeigt man nur, was nach dem Bereinigen übrig ist, sieht eine sauber
  bereinigte Datei aus wie eine, in der nie etwas stand. Der Nutzer erfährt
  nie, dass sein Name, die Seriennummer seiner Kamera und der Aufnahmeort
  drin waren — und lernt dadurch auch nie, dass er so etwas mit sich
  herumträgt. Der vollständige Befund ist nicht nur Kontrolle, er ist die
  einzige Stelle, an der jemand das über seine eigenen Dateien erfährt.

  WARUM ES DAS ORIGINAL ÜBERHAUPT ZUR WAHL GIBT.
  Manchmal sind die Metadaten der Zweck: ein Foto mit Urheberangabe, ein
  Dokument, dessen Bearbeitungsverlauf den Empfänger angeht. Wer das
  braucht und es nicht bekommt, umgeht das Programm — und schickt die Datei
  ungeprüft über einen anderen Weg. Eine sichtbare, benannte Wahl ist
  besser als eine Umgehung.

  Die Wahl ist deshalb MAGENTA und nicht rot: Sie ist ein eingestellter
  Sollwert des Nutzers, kein Fehler des Programms.
-->
<script lang="ts">
  import type { Fund, Sendedatei } from "../kern/typen";
  import { FUNDART_TEXT, groesse, nachSchwere } from "../anzeige/zustand";
  import Zustandsmarke from "../anzeige/Zustandsmarke.svelte";
  import Sollwert from "../anzeige/Sollwert.svelte";

  interface Props {
    datei: Sendedatei;
    /** Ob derzeit das Original hinausgeht statt der bereinigten Fassung. */
    original: boolean;
    waehle: (original: boolean) => void;
    schliessen: () => void;
  }
  let { datei, original, waehle, schliessen }: Props = $props();

  const b = $derived(datei.befund);

  /** Alles, was gefunden wurde — unabhängig davon, ob es entfernt wird. */
  const alleFunde = $derived.by((): { fund: Fund; bleibt: boolean }[] => {
    if (b.fall === "vollstaendig") {
      return b.entfernt.map((fund) => ({ fund, bleibt: false }));
    }
    if (b.fall === "teilweise") {
      return [
        ...b.entfernt.map((fund) => ({ fund, bleibt: false })),
        ...b.geblieben.map((fund) => ({ fund, bleibt: true })),
      ];
    }
    return [];
  });

  const sortiert = $derived(
    nachSchwere(alleFunde.map((e) => e.fund)).map(
      (fund) => alleFunde.find((e) => e.fund === fund)!,
    ),
  );

  const entferntZahl = $derived(alleFunde.filter((e) => !e.bleibt).length);
  const bleibtZahl = $derived(alleFunde.filter((e) => e.bleibt).length);

  /**
   * Ob es überhaupt zwei Fassungen gibt.
   *
   * Bei einem nicht verstandenen Format gibt es keine bereinigte — das
   * Programm weiß ja nicht, was es entfernen sollte. Dann eine Wahl
   * anzubieten wäre eine Behauptung.
   */
  const zweiFassungen = $derived(b.fall === "vollstaendig" || b.fall === "teilweise");

  const punkt = {
    kritisch: "bg-fehler",
    beachtlich: "bg-warnung",
    gering: "bg-keine",
  } as const;
</script>

<article class="space-y-5">
  <header class="flex flex-wrap items-baseline justify-between gap-2">
    <div class="min-w-0">
      <h2 class="text-xl font-semibold break-all">{datei.name}</h2>
      <p class="text-schrift-leise mt-0.5 text-sm">
        {groesse(datei.groesseBytes)}
        {#if b.fall === "vollstaendig" || b.fall === "teilweise"}
          · {b.format}
        {/if}
      </p>
    </div>
    <button
      class="border-linie hover:bg-flaeche rounded-md border px-3 py-1.5 text-sm"
      onclick={schliessen}
    >
      Schließen
    </button>
  </header>

  <!-- ===================================================================
       1. Was gefunden wurde — alles
       =================================================================== -->
  {#if b.fall === "unbekannt"}
    <Zustandsmarke
      marke={{
        zustand: "keineAussage",
        wort: "Kein Befund möglich",
        satz: b.formathinweis
          ? `Erkannt als ${b.formathinweis}, aber nicht verstanden. Was in dieser Datei steht, kann dieses Programm nicht sagen — auch nicht, dass nichts drin ist.`
          : "Format nicht verstanden. Was in dieser Datei steht, kann dieses Programm nicht sagen.",
      }}
      gross
    />
  {:else if b.fall === "fehler"}
    <Zustandsmarke
      marke={{ zustand: "fehler", wort: "Nicht lesbar", satz: b.grund }}
      gross
    />
  {:else}
    <section class="space-y-2">
      <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
        Gefunden ({alleFunde.length})
      </h3>

      {#if alleFunde.length === 0}
        <p class="border-linie bg-flaeche text-schrift-leise rounded-lg border px-4 py-3 text-sm">
          In dieser Datei wurde nichts gefunden, was dieses Programm als
          Metadatum kennt. Das heißt nicht, dass nichts drin ist — nur, dass
          nichts Bekanntes drin ist.
        </p>
      {:else}
        <ul class="border-linie divide-linie bg-flaeche divide-y rounded-lg border">
          {#each sortiert as eintrag (eintrag.fund.ort + eintrag.fund.art)}
            <li class="flex gap-3 px-4 py-3">
              <span
                class="mt-1.5 h-2 w-2 shrink-0 rounded-full {punkt[eintrag.fund.schwere]}"
                aria-hidden="true"
              ></span>
              <div class="min-w-0 flex-1">
                <p class="text-sm">
                  <span class="font-medium">{FUNDART_TEXT[eintrag.fund.art]}</span>
                  <span class="text-schrift-leise">— {eintrag.fund.schwere}</span>
                </p>
                {#if eintrag.fund.wert}
                  <p class="mt-0.5 text-sm break-words">{eintrag.fund.wert}</p>
                {/if}
                <p class="text-bezug mt-0.5 font-mono text-xs opacity-80">
                  {eintrag.fund.ort}
                </p>
              </div>
              <!--
                Je Fund, was mit ihm geschieht. Das ist die Angabe, die
                sonst nirgends steht: nicht nur, dass etwas gefunden wurde,
                sondern ob es die Datei verlässt.
              -->
              <span
                class="shrink-0 self-start text-xs {eintrag.bleibt
                  ? 'text-warnung'
                  : 'text-bestaetigt'}"
              >
                {eintrag.bleibt ? "! bleibt" : "✓ wird entfernt"}
              </span>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/if}

  <!-- ===================================================================
       2. Welche Fassung hinausgeht
       =================================================================== -->
  <section class="space-y-2" data-pruefstelle="fassung">
    <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
      Welche Fassung soll hinausgehen?
    </h3>

    {#if !zweiFassungen}
      <p class="border-linie bg-flaeche text-schrift-leise rounded-lg border px-4 py-3 text-sm">
        Es gibt nur eine: Ohne verstandenes Format lässt sich nichts
        bereinigen. Die Datei geht unverändert hinaus — oder gar nicht.
      </p>
    {:else}
      <label
        class="flex cursor-pointer items-start gap-3 rounded-lg border p-3
               {!original ? 'border-schrift-leise bg-flaeche' : 'border-linie'}"
      >
        <input
          type="radio"
          checked={!original}
          onchange={() => waehle(false)}
          class="mt-1"
          name="fassung"
        />
        <span class="text-sm">
          <span class="block font-medium">Bereinigt</span>
          <span class="text-schrift-leise block">
            {entferntZahl}
            {entferntZahl === 1 ? "Angabe wird" : "Angaben werden"} entfernt{#if bleibtZahl > 0}, {bleibtZahl}
              {bleibtZahl === 1 ? "bleibt" : "bleiben"} in der Datei{/if}.
          </span>
          {#if b.fall === "teilweise"}
            <span class="text-schrift-leise mt-1 block">{b.grund}</span>
          {/if}
        </span>
      </label>

      <label
        class="flex cursor-pointer items-start gap-3 rounded-lg border p-3
               {original ? 'border-sollwert/60 bg-flaeche' : 'border-linie'}"
      >
        <input
          type="radio"
          checked={original}
          onchange={() => waehle(true)}
          class="mt-1"
          name="fassung"
        />
        <span class="text-sm">
          <span class="block font-medium">Original, unverändert</span>
          <span class="text-schrift-leise block">
            {#if alleFunde.length === 0}
              Nichts wird angefasst. Gefunden wurde ohnehin nichts.
            {:else}
              Alle {alleFunde.length}
              {alleFunde.length === 1 ? "gefundene Angabe geht" : "gefundenen Angaben gehen"}
              mit hinaus — auch die, die sonst entfernt würden.
            {/if}
          </span>
          <span class="text-schrift-leise mt-1 block">
            Sinnvoll, wenn die Angaben der Zweck sind: eine Urheberangabe im
            Foto, ein Bearbeitungsverlauf, der den Empfänger angeht.
          </span>
        </span>
      </label>

      {#if original}
        <Sollwert>
          Sie senden das Original — {entferntZahl}
          {entferntZahl === 1 ? "Angabe" : "Angaben"} mehr als nötig
        </Sollwert>
      {/if}
    {/if}
  </section>
</article>
