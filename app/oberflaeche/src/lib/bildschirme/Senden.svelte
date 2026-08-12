<!--
  Der Bildschirm „Senden".

  DIE ENTSCHEIDENDE ANORDNUNG: Die Metadatenprüfung steht VOR dem
  Verschlüsseln, nicht danach. Wer erst verschlüsselt und dann berichtet, was
  drin war, stellt den Nutzer vor eine Datei, die er nicht mehr ändern kann,
  ohne von vorn zu beginnen.

  „Stör nur, wenn du wirklich etwas zu sagen hast": Ist alles bereinigt,
  bleibt der Befund eine zugeklappte Zeile. Gibt es etwas zu entscheiden,
  steht es offen da — und der Knopf zum Verschlüsseln verlangt vorher eine
  Bestätigung.

  KEIN ÜBERSPRINGEN BEI VIELEN DATEIEN. Dort ist die Prüfung am wichtigsten:
  Wer vierzig Dateien schickt und drei sind nur teilweise bereinigt,
  übersieht beim Überspringen genau die drei. Stattdessen wird das
  Unauffällige zu einer Zeile zusammengefasst.
-->
<script lang="ts">
  import type { Stapel } from "../kern/mock";
  import { KONTAKTE } from "../kern/mock";
  import {
    brauchtEntscheidung,
    fasseStapel,
    groesse,
    markeFuerBereinigung,
    markeFuerKontakt,
  } from "../anzeige/zustand";
  import Zustandsmarke from "../anzeige/Zustandsmarke.svelte";
  import Fundliste from "../anzeige/Fundliste.svelte";
  import Bezugswert from "../anzeige/Bezugswert.svelte";
  import Sollwert from "../anzeige/Sollwert.svelte";

  interface Props {
    stapel: Stapel;
  }
  let { stapel }: Props = $props();

  const befund = $derived(fasseStapel(stapel.dateien));
  const mussEntscheiden = $derived(brauchtEntscheidung(befund));

  // Die Bestätigung wird bei jedem Stapelwechsel zurückgesetzt: Sie gilt für
  // das, was der Nutzer gesehen hat, nicht für das nächste.
  let gesehen = $state(false);
  $effect(() => {
    stapel.kennung;
    gesehen = false;
  });

  let empfaenger = $state<string[]>([KONTAKTE[0]!.fingerprint]);
  let signieren = $state(true);

  const gewaehlt = $derived(KONTAKTE.filter((k) => empfaenger.includes(k.fingerprint)));

  // Ein Empfänger ohne Post-Quantum-Schlüssel zieht die ganze Nachricht auf
  // die klassische Suite herunter -- das muss dastehen.
  const ohnePq = $derived(gewaehlt.filter((k) => !k.hatPostQuantum));

  const gesamtGroesse = $derived(
    stapel.dateien.reduce((summe, d) => summe + d.groesseBytes, 0),
  );

  const bereit = $derived(
    gewaehlt.length > 0 && (!mussEntscheiden || gesehen),
  );

  function umschalten(fp: string) {
    empfaenger = empfaenger.includes(fp)
      ? empfaenger.filter((x) => x !== fp)
      : [...empfaenger, fp];
  }
</script>

<article class="space-y-5">
  <header class="flex flex-wrap items-baseline justify-between gap-2">
    <h2 class="text-xl font-semibold">
      {stapel.dateien.length === 1
        ? stapel.dateien[0]!.name
        : `${stapel.dateien.length} Dateien`}
    </h2>
    <p class="text-schrift-leise text-sm">{groesse(gesamtGroesse)}</p>
  </header>

  <!-- ===================================================================
       1. Was verraten die Dateien?
       =================================================================== -->
  <section class="space-y-2">
    <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
      Vor dem Verschlüsseln
    </h3>

    {#if !mussEntscheiden}
      <!-- Nichts zu sagen: eine Zeile, und weiter. -->
      <Zustandsmarke
        marke={{
          zustand: "bestaetigt",
          wort:
            befund.gesamt === 1
              ? "Bereinigt"
              : `Alle ${befund.gesamt} Dateien bereinigt`,
          satz: "Alle bekannten Metadaten entfernt. Es gibt nichts zu entscheiden.",
        }}
        gross
      />
      {#if befund.gesamt === 1}
        {@const b = stapel.dateien[0]!.befund}
        {#if b.fall === "vollstaendig"}
          <Fundliste funde={b.entfernt} ueberschrift="Entfernt" />
        {/if}
      {/if}
    {:else}
      <!--
        Es gibt etwas zu sagen. Das Unauffällige schrumpft auf eine Zeile,
        das Auffällige steht einzeln — auch bei einundvierzig Dateien.
      -->
      {#if befund.vollstaendig > 0}
        <!--
          Zugeklappt, aber vorhanden. „Nicht stören" heißt nicht „nicht
          nachsehen können": Wer wissen will, was aus den unauffälligen
          Dateien entfernt wurde, klappt es auf. Wer nicht, sieht eine Zeile.
        -->
        <details class="border-linie bg-flaeche rounded-lg border px-4 py-2.5">
          <summary class="text-schrift-leise cursor-pointer text-sm">
            <span class="text-bestaetigt font-medium">{befund.vollstaendig}</span>
            {befund.vollstaendig === 1 ? "Datei" : "Dateien"} vollständig bereinigt.
          </summary>
          <ul class="border-linie mt-3 space-y-1 border-t pt-3">
            {#each befund.sauber as datei (datei.name)}
              {@const anzahl =
                datei.befund.fall === "vollstaendig" ? datei.befund.entfernt.length : 0}
              <li class="flex flex-wrap items-baseline justify-between gap-2 text-sm">
                <span class="text-schrift-leise">{datei.name}</span>
                <span class="text-bezug text-xs">
                  {anzahl === 0
                    ? "nichts gefunden"
                    : `${anzahl} ${anzahl === 1 ? "Fund" : "Funde"} entfernt`}
                </span>
              </li>
            {/each}
          </ul>
        </details>
      {/if}

      <div class="space-y-2" data-pruefstelle="auffaellig">
        {#each befund.auffaellig as datei (datei.name)}
          {@const marke = markeFuerBereinigung(datei.befund)}
          <div class="border-linie bg-flaeche space-y-2 rounded-lg border p-3">
            <div class="flex flex-wrap items-baseline justify-between gap-2">
              <p class="font-medium">{datei.name}</p>
              <p class="text-bezug text-xs">{groesse(datei.groesseBytes)}</p>
            </div>
            <Zustandsmarke {marke} />
            {#if datei.befund.fall === "teilweise" && datei.befund.geblieben.length > 0}
              <Fundliste funde={datei.befund.geblieben} ueberschrift="Bleibt in der Datei" offen />
            {/if}
          </div>
        {/each}
      </div>

      <!--
        Die Bestätigung ist kein Ritual. Sie ist die Stelle, an der aus
        „gezeigt" ein „gesehen" wird — und ohne sie geht es nicht weiter.
      -->
      <label
        class="border-warnung-rand bg-warnung-grund flex cursor-pointer items-start gap-3 rounded-lg border p-3"
      >
        <input type="checkbox" bind:checked={gesehen} class="mt-1" />
        <span class="text-sm">
          Ich habe gesehen, was in
          {befund.auffaellig.length === 1 ? "dieser Datei" : "diesen Dateien"}
          bleibt, und will trotzdem verschlüsseln.
        </span>
      </label>
    {/if}
  </section>

  <!-- ===================================================================
       2. An wen?
       =================================================================== -->
  <section class="space-y-2">
    <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">Empfänger</h3>

    <div class="space-y-1.5">
      {#each KONTAKTE as k (k.fingerprint)}
        {@const marke = markeFuerKontakt(k)}
        <label
          class="border-linie bg-flaeche flex cursor-pointer items-center gap-3 rounded-lg border p-3"
        >
          <input
            type="checkbox"
            checked={empfaenger.includes(k.fingerprint)}
            onchange={() => umschalten(k.fingerprint)}
          />
          <span class="min-w-0 flex-1">
            <span class="block font-medium">{k.name}</span>
            <span class="text-schrift-leise block text-xs">{marke.wort}</span>
          </span>
          {#if !k.hatPostQuantum}
            <span class="text-warnung shrink-0 text-xs">nur klassisch</span>
          {/if}
        </label>
      {/each}
    </div>

    {#if ohnePq.length > 0}
      <Zustandsmarke
        marke={{
          zustand: "warnung",
          wort: "Ohne Post-Quantum-Schutz",
          satz:
            `${ohnePq.map((k) => k.name).join(", ")} ` +
            `${ohnePq.length === 1 ? "führt" : "führen"} keinen Post-Quantum-Schlüssel — ` +
            "vermutlich aus Version 1 übernommen. Diese Nachricht ist gegen einen " +
            "künftigen Quantenrechner nicht geschützt.",
        }}
      />
    {/if}
  </section>

  <!-- ===================================================================
       3. Wie?
       =================================================================== -->
  <section class="space-y-2">
    <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">Absender</h3>
    <label
      class="border-linie bg-flaeche flex cursor-pointer items-start gap-3 rounded-lg border p-3"
    >
      <input type="checkbox" bind:checked={signieren} class="mt-1" />
      <span class="text-sm">
        <span class="block font-medium">Mit meiner Identität signieren</span>
        <span class="text-schrift-leise block">
          {signieren
            ? "Der Empfänger kann prüfen, dass die Nachricht von Ihnen stammt."
            : "Anonymer Versand. Die Nachricht sagt nichts darüber, wer sie geschickt hat — ein legitimer Modus."}
        </span>
      </span>
    </label>
    {#if !signieren}
      <Sollwert>Sie versenden anonym</Sollwert>
    {/if}
  </section>

  <!-- ===================================================================
       4. Los
       =================================================================== -->
  <section class="border-linie space-y-3 border-t pt-4">
    <dl class="grid gap-4 sm:grid-cols-3">
      <Bezugswert beschriftung="Dateien">{stapel.dateien.length}</Bezugswert>
      <Bezugswert beschriftung="Größe">{groesse(gesamtGroesse)}</Bezugswert>
      <Bezugswert beschriftung="Suite">
        {ohnePq.length > 0 ? "klassisch (0x0001)" : "Post-Quantum-Hybrid (0x0002)"}
      </Bezugswert>
    </dl>

    <div class="flex flex-wrap items-center gap-3">
      <button
        class="bg-schrift text-grund rounded-md px-5 py-2.5 text-sm font-medium
               disabled:cursor-not-allowed disabled:opacity-40"
        disabled={!bereit}
      >
        Verschlüsseln
      </button>
      {#if gewaehlt.length === 0}
        <span class="text-schrift-leise text-sm">Wählen Sie mindestens einen Empfänger.</span>
      {:else if mussEntscheiden && !gesehen}
        <span class="text-schrift-leise text-sm">
          Bestätigen Sie oben, dass Sie den Befund gesehen haben.
        </span>
      {/if}
    </div>
  </section>
</article>
