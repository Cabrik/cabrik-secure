<!--
  Der Bildschirm „Kontakte“.

  Beim Empfangen ist Vertrauen eine ANZEIGE. Hier ist es eine HANDLUNG — und
  das ist der Bildschirm, an dem sich entscheidet, ob jemand den Unterschied
  zwischen „bekannt“ und „verifiziert“ begreift.

  DESHALB IST „NICHT VERIFIZIERT“ HIER GRAU, NICHT GELB. Als Eintrag im
  Verzeichnis ist es erwartbar: So fängt jeder Kontakt an. Erst wenn eine
  Nachricht kommt und man sich auf den Namen verlassen soll, wird daraus eine
  Warnung. Derselbe Sachverhalt, zwei Bewertungen — je nachdem, worum es
  gerade geht.

  DIE SAFETY NUMBER STEHT GROSS UND ZUM VORLESEN DA. Sie ist kein Beiwerk,
  sondern das einzige Mittel, aus „bekannt“ ein „verifiziert“ zu machen.
  Sechzig Ziffern in Zwölfergruppen, sprachunabhängig — man liest sie am
  Telefon vor, und zwar beide Seiten dieselbe.
-->
<script lang="ts">
  import type { Kontakt } from "../kern/typen";
  import { kontaktspeicher } from "../kern/speicher.svelte";
  import { markeFuerKontakt } from "../anzeige/zustand";
  import Zustandsmarke from "../anzeige/Zustandsmarke.svelte";
  import Bezugswert from "../anzeige/Bezugswert.svelte";
  import Aufnehmen from "./Aufnehmen.svelte";

  const KONTAKTE = $derived(kontaktspeicher.liste);

  let aufnahme = $state(false);
  let gewaehlt = $state<string>(kontaktspeicher.liste[0]!.fingerprint);
  const kontakt = $derived(KONTAKTE.find((k) => k.fingerprint === gewaehlt) ?? KONTAKTE[0]!);

  /**
   * Was nach dem Vergleich geschehen ist.
   *
   * An denselben Kontakt gebunden wie der Vergleich selbst — aus demselben
   * Grund: Ein Ergebnis, das beim Umschalten stehen bliebe, gehörte zum
   * falschen Schlüssel.
   */
  let abgleichFehlerFuer = $state<string | null>(null);
  const abgleichFehlgeschlagen = $derived(abgleichFehlerFuer === gewaehlt);

  let widerrufFragtFuer = $state<string | null>(null);
  const widerrufFragt = $derived(widerrufFragtFuer === gewaehlt);

  function stimmtUeberein() {
    kontaktspeicher.verifizieren(gewaehlt, "safetyNumber");
    vergleichtFuer = null;
    abgleichFehlerFuer = null;
  }

  function stimmtNichtUeberein() {
    // NICHT widerrufen. Widerrufen hieße „dieser Schlüssel ist
    // kompromittiert“ — das weiß niemand. Bekannt ist nur, dass die
    // Prüfung fehlgeschlagen ist.
    kontaktspeicher.zuruecksetzen(gewaehlt);
    vergleichtFuer = null;
    abgleichFehlerFuer = gewaehlt;
  }

  function widerrufen() {
    kontaktspeicher.widerrufen(gewaehlt);
    widerrufFragtFuer = null;
  }
  const marke = $derived(markeFuerKontakt(kontakt));

  /**
   * Für welchen Kontakt der Vergleich läuft — nicht „ob“.
   *
   * Ein `$effect`, der ein Flag beim Kontaktwechsel zurücksetzt, täte
   * scheinbar dasselbe. Er ist aber davon abhängig, wann er läuft, und läuft
   * unter Umständen öfter als gedacht: In `Senden.svelte` löschte genau so
   * ein Rücksetzer die Bestätigung bei jedem Klick sofort wieder, und der
   * Bildschirm ließ sich nicht mehr bedienen.
   *
   * Als Vergleich formuliert kann das nicht passieren — die Zugehörigkeit
   * steht in der Bedingung selbst.
   */
  let vergleichtFuer = $state<string | null>(null);
  const vergleicht = $derived(vergleichtFuer === gewaehlt);

  const punkt: Record<Kontakt["vertrauen"], string> = {
    verifiziert: "bg-bestaetigt",
    gesehen: "bg-keine",
    gewechselt: "bg-warnung",
    widerrufen: "bg-fehler",
  };

  function datum(u: number): string {
    return new Date(u * 1000).toLocaleDateString("de-DE", {
      year: "numeric",
      month: "long",
      day: "numeric",
    });
  }

  /** Die zwölf Gruppen zu fünf Ziffern, wie sie vorgelesen werden. */
  const gruppen = $derived(kontakt.safetyNumber.trim().split(/\s+/));
</script>

{#if aufnahme}
  <Aufnehmen
    fertig={(fp) => {
      aufnahme = false;
      if (fp) gewaehlt = fp;
    }}
  />
{:else}
<div class="grid gap-6 lg:grid-cols-[18rem_1fr]">
  <!-- ===================================================================
       Das Verzeichnis
       =================================================================== -->
  <section class="space-y-1.5">
    <h3 class="text-schrift-leise px-1 pb-1 text-xs font-semibold tracking-wide uppercase">
      {KONTAKTE.length} Kontakte
    </h3>

    {#each KONTAKTE as k (k.fingerprint)}
      <button
        class="flex w-full items-center gap-3 rounded-lg border px-3 py-2.5 text-left transition
               {gewaehlt === k.fingerprint
          ? 'border-schrift-leise bg-flaeche'
          : 'border-linie hover:bg-flaeche'}"
        onclick={() => (gewaehlt = k.fingerprint)}
      >
        <span class="h-2.5 w-2.5 shrink-0 rounded-full {punkt[k.vertrauen]}" aria-hidden="true"
        ></span>
        <span class="min-w-0 flex-1">
          <span class="block truncate text-sm font-medium">{k.name}</span>
          <span class="text-schrift-leise block text-xs">
            {markeFuerKontakt(k).wort}
          </span>
        </span>
      </button>
    {/each}

    <button
      class="border-linie text-schrift-leise hover:text-schrift mt-3 w-full rounded-lg border border-dashed px-3 py-2.5 text-sm"
      onclick={() => (aufnahme = true)}
    >
      + Kontakt aufnehmen
    </button>
  </section>

  <!-- ===================================================================
       Der einzelne Kontakt
       =================================================================== -->
  <section class="min-w-0 space-y-5">
    <header>
      <h2 class="text-xl font-semibold">{kontakt.name}</h2>
      {#if kontakt.notiz}
        <p class="text-schrift-leise mt-0.5 text-sm">{kontakt.notiz}</p>
      {/if}
    </header>

    <Zustandsmarke {marke} gross />

    <dl class="border-linie bg-flaeche grid gap-4 rounded-lg border p-4 sm:grid-cols-2">
      <Bezugswert beschriftung="Bekannt seit">{datum(kontakt.seit)}</Bezugswert>
      {#if kontakt.verifiziertAm}
        <Bezugswert beschriftung="Verifiziert am">{datum(kontakt.verifiziertAm)}</Bezugswert>
      {/if}
      <Bezugswert beschriftung="Fingerprint" fest>{kontakt.fingerprint}</Bezugswert>
      <Bezugswert beschriftung="Verschlüsselung">
        {kontakt.hatPostQuantum ? "Post-Quantum-Hybrid" : "nur klassisch (aus Version 1)"}
      </Bezugswert>
    </dl>

    {#if !kontakt.hatPostQuantum}
      <Zustandsmarke
        marke={{
          zustand: "warnung",
          wort: "Kein Post-Quantum-Schlüssel",
          satz:
            "Dieser Kontakt stammt aus Version 1. An ihn verschickte Nachrichten sind " +
            "gegen einen künftigen Quantenrechner nicht geschützt. Bitten Sie ihn, " +
            "seine Identität neu zu erzeugen.",
        }}
      />
    {/if}

    <!-- =================================================================
         Die Safety Number
         ================================================================= -->
    <section class="space-y-3">
      <div class="flex flex-wrap items-baseline justify-between gap-2">
        <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
          Safety Number
        </h3>
        {#if kontakt.vertrauen !== "verifiziert"}
          <button
            class="bg-schrift text-grund rounded-md px-4 py-2 text-sm font-medium"
            onclick={() => (vergleichtFuer = vergleicht ? null : gewaehlt)}
          >
            {vergleicht ? "Abbrechen" : "Jetzt vergleichen"}
          </button>
        {/if}
      </div>

      <!--
        Die Ziffern in Zwölfergruppen, groß und in fester Zeichenbreite.
        Cyan, weil es ein gelesener Wert ist und kein Urteil.
      -->
      <div class="border-linie bg-flaeche rounded-lg border p-4">
        <div class="grid grid-cols-3 gap-x-6 gap-y-2 sm:grid-cols-4 md:grid-cols-6">
          {#each gruppen as gruppe, i (i)}
            <span class="text-bezug font-mono text-lg tracking-wider tabular-nums">
              {gruppe}
            </span>
          {/each}
        </div>
      </div>

      {#if vergleicht}
        <div class="border-linie bg-flaeche space-y-3 rounded-lg border p-4">
          <p class="text-sm">
            <span class="font-medium">Rufen Sie {kontakt.name} an</span> — auf einem Weg, den
            Sie nicht über dieses Programm hergestellt haben. Lesen Sie die Ziffern
            gegenseitig vor. Beide Seiten sehen dieselben.
          </p>
          <p class="text-schrift-leise text-sm">
            Stimmen sie überein, hört niemand dazwischen mit. Stimmen sie nicht überein,
            brechen Sie ab und verwenden Sie diesen Kontakt nicht.
          </p>
          <div class="flex flex-wrap gap-2 pt-1">
            <button
              class="bg-bestaetigt text-grund rounded-md px-4 py-2 text-sm font-medium"
              onclick={stimmtUeberein}
            >
              Sie stimmen überein
            </button>
            <button
              class="border-fehler text-fehler rounded-md border px-4 py-2 text-sm font-medium"
              onclick={stimmtNichtUeberein}
            >
              Sie stimmen nicht überein
            </button>
          </div>
        </div>
      {:else if abgleichFehlgeschlagen}
        <!--
          Der Fall, für den die Safety Number überhaupt gebaut ist -- und
          der bisher keinen Bildschirm hatte. Er sagt ausdrücklich NICHT,
          dass jemand mithört: Ein Zahlendreher beim Vorlesen sieht genauso
          aus. Was er sagt, ist, was zu tun ist.
        -->
        <Zustandsmarke
          marke={{
            zustand: "fehler",
            wort: "Die Nummern stimmen nicht überein",
            satz:
              "Schicken Sie diesem Kontakt vorerst nichts. Häufigste Ursache " +
              "ist ein Zahlendreher beim Vorlesen — versuchen Sie es ruhig noch " +
              "einmal. Bleibt es dabei, sitzt jemand zwischen Ihnen, und der " +
              "Schlüssel oben gehört nicht dem, den Sie am Telefon haben.",
          }}
          gross
        />
      {:else if kontakt.vertrauen === "gesehen"}
        <p class="text-schrift-leise text-sm">
          Ohne diesen Vergleich ist der Name oben nur eine Behauptung Ihres eigenen
          Speichers. Er sagt nichts darüber, wer den Schlüssel wirklich besitzt.
        </p>
      {:else if kontakt.vertrauen === "gewechselt"}
        <p class="text-schrift-leise text-sm">
          Dieser Kontakt tritt mit einem anderen Schlüssel auf. Die Ziffern oben gehören
          zum <span class="text-schrift font-medium">neuen</span> Schlüssel — vergleichen
          Sie sie erneut, bevor Sie ihm wieder schreiben.
        </p>
      {/if}
    </section>

    {#if kontakt.vertrauen !== "widerrufen"}
      <section class="border-linie space-y-2 border-t pt-4">
        {#if widerrufFragt}
          <p class="text-sm">
            <span class="font-medium">{kontakt.name}</span> wird künftig rot
            angezeigt, und Nachrichten von diesem Schlüssel gelten als Fehler —
            auch wenn ihre Signatur gültig ist.
          </p>
          <div class="flex flex-wrap gap-2">
            <button
              class="border-fehler text-fehler rounded-md border px-4 py-2 text-sm font-medium"
              onclick={widerrufen}
            >
              Ja, widerrufen
            </button>
            <button
              class="border-linie hover:bg-flaeche rounded-md border px-4 py-2 text-sm"
              onclick={() => (widerrufFragtFuer = null)}
            >
              Abbrechen
            </button>
          </div>
        {:else}
          <button
            class="border-fehler text-fehler rounded-md border px-4 py-2 text-sm"
            onclick={() => (widerrufFragtFuer = gewaehlt)}
          >
            Schlüssel als kompromittiert markieren
          </button>
        {/if}
        <p class="text-schrift-leise mt-2 text-xs">
          Wirkt nur bei Ihnen. Ein Widerruf ohne Verteilweg erreicht niemanden sonst.
        </p>
      </section>
    {/if}
  </section>
</div>
{/if}
