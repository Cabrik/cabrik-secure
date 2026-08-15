<!--
  Eine Textnachricht verschlüsseln.

  # Wofür

  Für Kanäle, die keine Dateien nehmen: ein Chatfenster, eine E-Mail, ein
  Ticket. Das Ergebnis ist Text zum Einfügen, kein Anhang.

  # Was hier ehrlich gesagt werden muss

  Der Rahmen nennt das Produkt (`spec/envelope-v2.md` §14). Wer verbergen
  will, WOMIT etwas verschlüsselt wurde, nimmt eine Datei. Das steht
  darunter, statt es zu verschweigen — es ist ein Zielkonflikt, den die
  Spezifikation bewusst so entschieden hat.
-->
<script lang="ts">
  import Empfaengerwahl from "../anzeige/Empfaengerwahl.svelte";
  import Zustandsmarke from "../anzeige/Zustandsmarke.svelte";
  import { kontaktspeicher } from "../kern/speicher.svelte";

  interface Props {
    /**
     * Verschlüsselt wirklich — nur im Fenster gesetzt.
     *
     * Fehlt er, bleibt der Knopf gesperrt: Ein Knopf, der so täte, als
     * verschlüssele er, wäre eine Lüge über das eigene Programm.
     */
    verschluesseln?: (
      text: string,
      empfaenger: string[],
      signieren: boolean,
    ) => void;
    /** Das Ergebnis, sobald es da ist. */
    envelope?: string | null;
    /** Nimmt das Ergebnis wieder weg. */
    schliessen?: () => void;
    arbeitet?: boolean;
  }
  let {
    verschluesseln,
    envelope = null,
    schliessen,
    arbeitet = false,
  }: Props = $props();

  const KONTAKTE = $derived(kontaktspeicher.liste);

  /**
   * Die Nachricht.
   *
   * **Sie wird nach dem Verschlüsseln geleert.** Wie das Passwort ist sie
   * ein Geheimnis, das durch die Webansicht geht; was hier stehen bleibt,
   * bleibt im Speicher, solange das Fenster steht. Die Kopien davor können
   * wir nicht überschreiben — das ist dieselbe Lücke wie bei der
   * Passworteingabe und wird mit demselben Schritt geschlossen.
   */
  let text = $state("");
  let empfaenger = $state<string[]>([]);
  let signieren = $state(true);

  const bereit = $derived(
    text.trim().length > 0 && empfaenger.length > 0 && !arbeitet,
  );

  function umschalten(fp: string) {
    empfaenger = empfaenger.includes(fp)
      ? empfaenger.filter((x) => x !== fp)
      : [...empfaenger, fp];
  }

  function los() {
    if (!bereit || !verschluesseln) return;
    const nachricht = text;
    // Erst leeren, dann verschlüsseln -- dazwischen liegt ein Aufruf über
    // die Brücke, und in der Zeit soll nichts im Feld stehen.
    text = "";
    verschluesseln(nachricht, empfaenger, signieren);
  }

  let kopiert = $state(false);
  async function kopieren() {
    if (!envelope) return;
    await navigator.clipboard.writeText(envelope);
    kopiert = true;
    setTimeout(() => (kopiert = false), 2000);
  }
</script>

<article class="space-y-5">
  {#if envelope}
    <!--
      Das Ergebnis. Es steht ganz oben, weil es das ist, wofür jemand
      hergekommen ist.
    -->
    <section class="space-y-3">
      <div class="flex flex-wrap items-baseline justify-between gap-2">
        <h2 class="text-bestaetigt flex items-center gap-2 text-lg font-semibold">
          <span aria-hidden="true">✓</span> Verschlüsselt
        </h2>
        <div class="flex gap-2">
          <button
            class="bg-schrift text-grund rounded-md px-4 py-2 text-sm font-medium"
            onclick={kopieren}
          >
            {kopiert ? "Kopiert" : "In die Zwischenablage"}
          </button>
          {#if schliessen}
            <button
              class="border-linie hover:bg-grund rounded-md border px-4 py-2 text-sm"
              onclick={schliessen}
            >
              Schließen
            </button>
          {/if}
        </div>
      </div>

      <textarea
        readonly
        rows="10"
        class="border-linie bg-grund w-full rounded-lg border p-3 font-mono text-xs"
        value={envelope}
      ></textarea>

      <!--
        Der Zielkonflikt aus §14, ausgesprochen. Wer ihn nicht kennt, hält
        Armor für die bessere Wahl -- dabei verrät er, womit die Nachricht
        gemacht wurde.
      -->
      <p class="text-schrift-leise text-xs leading-relaxed">
        Die Rahmenzeilen nennen das Programm. Wer verbergen will, womit eine
        Nachricht verschlüsselt wurde, verschickt eine Datei statt eines
        Textes.
      </p>
    </section>
  {:else}
    <section class="space-y-2">
      <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
        Nachricht
      </h3>
      <textarea
        bind:value={text}
        rows="8"
        placeholder="Was verschlüsselt werden soll …"
        class="border-linie bg-grund focus:border-bezug w-full rounded-lg border p-3 text-sm outline-none"
      ></textarea>
      <!--
        Die eine Eigenschaft, die Text von einer Datei unterscheidet — und
        die niemand von selbst vermutet.
      -->
      <p class="text-schrift-leise text-xs leading-relaxed">
        Die Länge der Nachricht wird verschleiert: Ein kurzes „ja“ ergibt
        einen ebenso großen Envelope wie ein langer Absatz.
      </p>
    </section>

    <Empfaengerwahl
      kontakte={KONTAKTE}
      gewaehlt={empfaenger}
      {umschalten}
      {signieren}
      setzeSignieren={(ja) => (signieren = ja)}
    />

    <section class="border-linie flex flex-wrap items-center gap-3 border-t pt-4">
      <button
        class="bg-schrift text-grund rounded-md px-5 py-2.5 text-sm font-medium
               disabled:cursor-not-allowed disabled:opacity-40"
        disabled={!bereit || !verschluesseln}
        data-pruefstelle="text-verschluesseln"
        onclick={los}
      >
        {arbeitet ? "Wird verschlüsselt…" : "Verschlüsseln"}
      </button>
      {#if !verschluesseln}
        <Zustandsmarke
          marke={{
            zustand: "keineAussage",
            wort: "Nur im Fenster",
            satz:
              "Im Browser gibt es keine Identität, mit der sich verschlüsseln ließe.",
          }}
        />
      {:else if empfaenger.length === 0}
        <span class="text-schrift-leise text-sm">
          Wählen Sie mindestens einen Empfänger.
        </span>
      {:else if text.trim().length === 0}
        <span class="text-schrift-leise text-sm">Es gibt nichts zu verschlüsseln.</span>
      {/if}
    </section>
  {/if}
</article>
