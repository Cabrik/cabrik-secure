<!--
  Wer die Nachricht bekommt — und ob signiert wird.

  # Warum das ein eigenes Bauteil ist

  Weil es zweimal gebraucht wird: für Dateien und für Text. Zweimal
  dieselbe Liste zu schreiben hieße, dass die eine beim nächsten Zusatz
  stehenbleibt — und dann zeigt ein Bildschirm einen Vorbehalt an, den der
  andere verschweigt.

  # Was hier NICHT entschieden wird

  Ob ein Empfänger zulässig ist. Das prüft der Kern, und zwar bevor ein
  einziges Byte entsteht: An einen widerrufenen Schlüssel wird nicht
  verschlüsselt. Dieser Bildschirm zeigt an, er urteilt nicht — sonst gäbe
  es zwei Stellen, an denen dieselbe Regel steht, und eine davon liefe der
  anderen davon.
-->
<script lang="ts">
  import { markeFuerKontakt } from "./zustand";
  import Zustandsmarke from "./Zustandsmarke.svelte";
  import type { Kontakt } from "../kern/typen";

  interface Props {
    kontakte: Kontakt[];
    /** Die Fingerprints der Gewählten. */
    gewaehlt: string[];
    umschalten: (fingerprint: string) => void;
    signieren: boolean;
    setzeSignieren: (ja: boolean) => void;
    /** Ob die Identität überhaupt signieren kann. */
    kannSignieren?: boolean;
  }
  let {
    kontakte,
    gewaehlt,
    umschalten,
    signieren,
    setzeSignieren,
    kannSignieren = true,
  }: Props = $props();

  /**
   * Gewählte ohne Post-Quantum-Schlüssel.
   *
   * Einer genügt, um die ganze Nachricht auf das klassische Verfahren zu
   * ziehen: Ein Envelope trägt ein Verfahren für alle Kapseln. Das muss
   * dastehen, bevor jemand auf „Verschlüsseln“ drückt.
   */
  const ohnePq = $derived(
    kontakte.filter(
      (k) => gewaehlt.includes(k.fingerprint) && !k.hatPostQuantum,
    ),
  );
</script>

<section class="space-y-2">
  <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
    Empfänger
  </h3>

  {#if kontakte.length === 0}
    <!--
      Ohne Kontakte gibt es nichts zu wählen. Der Weg dorthin gehört
      dazugesagt, statt eine leere Liste hinzustellen.
    -->
    <p class="border-linie text-schrift-leise rounded-lg border border-dashed p-4 text-sm">
      Noch keine Kontakte. Unter <span class="text-schrift font-medium">Kontakte</span>
      lässt sich einer aus einer Austausch-Nutzlast aufnehmen.
    </p>
  {/if}

  <div class="space-y-1.5">
    {#each kontakte as k (k.fingerprint)}
      {@const marke = markeFuerKontakt(k)}
      <label
        class="border-linie bg-flaeche flex cursor-pointer items-center gap-3 rounded-lg border p-3"
      >
        <input
          type="checkbox"
          checked={gewaehlt.includes(k.fingerprint)}
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

<section class="space-y-2">
  <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
    Absender
  </h3>
  <label
    class="border-linie bg-flaeche flex cursor-pointer items-start gap-3 rounded-lg border p-3"
  >
    <input
      type="checkbox"
      checked={signieren}
      disabled={!kannSignieren}
      onchange={() => setzeSignieren(!signieren)}
      class="mt-1"
    />
    <span class="text-sm">
      <span class="block font-medium">Mit meiner Identität signieren</span>
      <span class="text-schrift-leise block">
        {#if kannSignieren}
          Der Empfänger kann prüfen, dass die Nachricht von Ihnen stammt.
        {:else}
          <!--
            Ein gewählter Modus, kein Mangel — aber er darf nicht
            stillschweigend unterbleiben. Wer das Häkchen setzt und nichts
            signiert bekommt, glaubt an eine Zusicherung, die es nicht gibt.
          -->
          Diese Identität hat keinen Signierschlüssel. Nachrichten von ihr
          sind niemandem zuzuordnen — auch Ihnen nicht.
        {/if}
      </span>
    </span>
  </label>
</section>
