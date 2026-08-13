<!--
  „Kontakt aufnehmen“.

  DER MOMENT, IN DEM EIN FREMDER SCHLÜSSEL IN DEN SPEICHER KOMMT. Alles,
  was später über Vertrauen angezeigt wird, hängt daran, dass hier nichts
  behauptet wird, was nicht geprüft wurde.

  DREI SÄTZE, DIE HIER FALLEN MÜSSEN:

  1. **Die Prüfsumme ist keine Sicherheitsprüfung.** Sie stellt fest, dass
     die Zeichenfolge unterwegs nicht zerrissen wurde — mehr nicht. Wer
     „Prüfsumme stimmt“ liest und daraus schließt, der Absender sei echt,
     hat genau das Gegenteil verstanden. Deshalb steht sie hier gar nicht
     als Erfolgsmeldung, sondern nur ihr Ausbleiben als Fehler.

  2. **Der Name ist Ihrer.** Die Nutzlast trägt keinen. Was hier eingetippt
     wird, ist eine Notiz an sich selbst und keine Angabe des Gegenübers.

  3. **Der Kontakt beginnt als „nicht verifiziert“.** Es gibt auf diesem
     Bildschirm keinen Weg, das zu überspringen. Wer eine Nutzlast einliest,
     hat sie erhalten — nicht geprüft.
-->
<script lang="ts">
  import type { Nutzlastbefund } from "../kern/typen";
  import { NUTZLASTEN } from "../kern/mock";
  import { kontaktspeicher } from "../kern/speicher.svelte";
  import Zustandsmarke from "../anzeige/Zustandsmarke.svelte";
  import Bezugswert from "../anzeige/Bezugswert.svelte";

  interface Props {
    /** Wird nach dem Aufnehmen mit dem Fingerprint gerufen. */
    fertig: (fingerprint: string | null) => void;
  }
  let { fertig }: Props = $props();

  let eingabe = $state("");
  let name = $state("");

  /**
   * Der Befund kommt aus dem Kern.
   *
   * Nicht mehr aus den Beispieldaten: `nutzlastLesen` geht über die
   * Brücke. Im Fenster zerlegt `parse_qr` die Zeichenfolge und berechnet
   * den Fingerprint neu; im Browser antwortet die Attrappe mit demselben
   * Typ. Der Bildschirm merkt den Unterschied nicht — und das ist der
   * Zweck der Naht.
   */
  let befund = $state<Nutzlastbefund | null>(null);

  $effect(() => {
    const text = eingabe.trim();
    if (text.length === 0) {
      befund = null;
      return;
    }
    // Das Ergebnis gehört zu DIESER Eingabe. Tippt jemand weiter, während
    // die Antwort unterwegs ist, darf die alte sie nicht überschreiben.
    let gilt = true;
    void kontaktspeicher.nutzlastLesen(text).then((b) => {
      if (gilt) befund = b;
    });
    return () => {
      gilt = false;
    };
  });

  const gelesen = $derived(befund?.fall === "gelesen" ? befund : null);

  /** Ohne Namen kein Eintrag: Ein Verzeichnis aus Fingerprints ist keins. */
  const bereit = $derived(gelesen !== null && name.trim().length > 0);

  async function aufnehmen() {
    if (!gelesen || !bereit) return;
    // Erst wenn der Kontakt wirklich im Speicher steht, darf der
    // aufrufende Bildschirm auf ihn umschalten -- sonst zeigt er auf
    // etwas, das es dort noch nicht gibt.
    await kontaktspeicher.aufnehmen(name.trim(), eingabe.trim());
    // Lehnt der Kern ab, bleibt der Bildschirm stehen. Ihn zu schließen
    // und die Meldung woanders anzuzeigen hieße, den Nutzer den
    // Zusammenhang selbst herstellen zu lassen.
    if (kontaktspeicher.fehler === null) {
      fertig(gelesen.fingerprint);
    }
  }
</script>

<article class="space-y-5">
  <header class="flex flex-wrap items-baseline justify-between gap-2">
    <h2 class="text-xl font-semibold">Kontakt aufnehmen</h2>
    <button
      class="border-linie hover:bg-flaeche rounded-md border px-3 py-1.5 text-sm"
      onclick={() => fertig(null)}
    >
      Abbrechen
    </button>
  </header>

  <!-- ===================================================================
       1. Die Nutzlast
       =================================================================== -->
  <section class="space-y-2">
    <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
      Austausch-Nutzlast
    </h3>
    <p class="text-sm">
      Die Zeichenfolge, die Ihr Gegenüber unter „Weitergeben“ erzeugt hat —
      eingefügt, aus einer Datei geladen oder als QR-Code abgelesen.
    </p>

    <textarea
      class="border-linie bg-grund h-24 w-full rounded-md border px-3 py-2 font-mono text-xs"
      bind:value={eingabe}
      placeholder="cabrik:v2:…"
    ></textarea>

    <div class="flex flex-wrap gap-2">
      <button class="border-linie hover:bg-flaeche rounded-md border px-3 py-1.5 text-sm">
        Aus Datei laden
      </button>
      <button class="border-linie hover:bg-flaeche rounded-md border px-3 py-1.5 text-sm">
        QR-Code abscannen
      </button>
    </div>

    <!-- Beispiele, damit sich im Prototyp jeder Ausgang ansehen lässt. -->
    <div class="border-linie mt-2 space-y-1.5 border-t pt-3">
      <p class="text-schrift-leise text-xs">Beispiele zum Ausprobieren:</p>
      <div class="flex flex-wrap gap-1.5">
        {#each NUTZLASTEN as n (n.kennung)}
          <button
            class="border-linie text-schrift-leise hover:text-schrift rounded-md border px-2.5 py-1 text-xs"
            onclick={() => (eingabe = n.text)}
          >
            {n.titel}
          </button>
        {/each}
      </div>
    </div>
  </section>

  <!-- ===================================================================
       2. Was drinsteht
       =================================================================== -->
  {#if befund}
    <section class="space-y-3" data-pruefstelle="befund">
      {#if befund.fall === "beschaedigt"}
        <!--
          Ein Übertragungsfehler, kein Angriff — und das gehört dazugesagt.
          Wer hier „Warnung: Prüfsumme falsch“ liest, denkt an Manipulation
          und ruft womöglich beunruhigt an, obwohl ein Mailprogramm nur
          einen Zeilenumbruch eingefügt hat.
        -->
        <Zustandsmarke
          marke={{
            zustand: "fehler",
            wort: "Die Nutzlast ist unvollständig angekommen",
            satz: `${befund.grund} Lassen Sie sie sich noch einmal schicken.`,
          }}
          gross
        />
      {:else if befund.fall === "unlesbar"}
        <Zustandsmarke
          marke={{
            zustand: "fehler",
            wort: "Nicht lesbar",
            satz: befund.grund,
          }}
          gross
        />
      {:else}
        <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
          Was darin steht
        </h3>

        <dl class="border-linie bg-flaeche grid gap-4 rounded-lg border p-4 sm:grid-cols-2">
          <Bezugswert beschriftung="Fingerprint" fest>
            {befund.fingerprint}
          </Bezugswert>
          <Bezugswert beschriftung="Verschlüsselung">
            {befund.hatPostQuantum
              ? "Post-Quantum-Hybrid (X-Wing)"
              : "nur klassisch (X25519)"}
          </Bezugswert>
          <Bezugswert beschriftung="Signierschlüssel">
            {befund.hatSignierschluessel ? "vorhanden" : "keiner"}
          </Bezugswert>
        </dl>

        <!--
          Der Fingerprint wird aus den Schlüsseln NEU BERECHNET
          (spec/trust-store.md §5.1). Der mitgelieferte Wert ist nur eine
          Prüfsumme, und ihm zu vertrauen verbietet die Spezifikation
          ausdrücklich. Das steht hier, weil sonst niemand auf die Idee
          käme, dass es ein Unterschied ist.
        -->
        <p class="text-schrift-leise text-xs">
          Der Fingerprint stammt nicht aus der Nutzlast, sondern wurde aus den
          enthaltenen Schlüsseln neu berechnet. Der mitgeschickte Kurzwert ist
          nur eine Prüfsumme gegen Übertragungsfehler — er sagt nichts darüber,
          wer die Nutzlast geschickt hat.
        </p>

        {#if !befund.hatPostQuantum}
          <Zustandsmarke
            marke={{
              zustand: "warnung",
              wort: "Kein Post-Quantum-Schlüssel",
              satz:
                "An diesen Kontakt lässt sich nur klassisch verschlüsseln. " +
                "Nachrichten an ihn sind gegen einen künftigen Quantenrechner " +
                "nicht geschützt — vermutlich stammt seine Identität aus " +
                "Version 1.",
            }}
          />
        {/if}

        {#if !befund.hatSignierschluessel}
          <Zustandsmarke
            marke={{
              zustand: "keineAussage",
              wort: "Ohne Signierschlüssel",
              satz:
                "Dieser Kontakt kann empfangen, aber nicht signieren. Seine " +
                "Nachrichten werden immer als „nicht signiert“ ankommen. Ein " +
                "gewählter Modus, kein Mangel.",
            }}
          />
        {/if}

        <!--
          Der ernste Fall. Ein bekannter Kontakt mit anderem Schlüssel ist
          entweder ein neues Gerät — oder jemand anders.
        -->
        {#if befund.schonBekannt && !befund.schonBekannt.gleicherSchluessel}
          <Zustandsmarke
            marke={{
              zustand: "warnung",
              wort: `Ein Kontakt namens „${befund.schonBekannt.name}“ hat bereits einen anderen Schlüssel`,
              satz:
                "Das kann ein neues Gerät sein — oder jemand anders. Fragen Sie " +
                "auf einem Weg nach, den Sie nicht über dieses Programm " +
                "hergestellt haben, bevor Sie ihm etwas schicken.",
            }}
            gross
          />
        {:else if befund.schonBekannt}
          <Zustandsmarke
            marke={{
              zustand: "keineAussage",
              wort: `Bereits als „${befund.schonBekannt.name}“ im Verzeichnis`,
              satz: "Derselbe Schlüssel. Ein zweiter Eintrag bringt nichts.",
            }}
          />
        {/if}
      {/if}
    </section>
  {/if}

  <!-- ===================================================================
       3. Der Name
       =================================================================== -->
  {#if gelesen}
    <section class="border-linie space-y-2 border-t pt-4">
      <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
        Name
      </h3>
      <input
        class="border-linie bg-grund w-full rounded-md border px-3 py-2"
        bind:value={name}
        placeholder="Wie Sie diesen Kontakt nennen möchten"
      />
      <p class="text-schrift-leise text-sm">
        Die Nutzlast trägt keinen Namen. Was Sie hier eintragen, ist
        <span class="text-schrift">Ihre Notiz an sich selbst</span> — keine
        Angabe Ihres Gegenübers und keine Zusicherung. Nachprüfbar ist allein
        der Fingerprint darüber.
      </p>
    </section>

    <!-- ===============================================================
         4. Aufnehmen — und zwar als „nicht verifiziert“
         =============================================================== -->
    <section class="border-linie space-y-3 border-t pt-4">
      <!--
        Kein Häkchen „gleich als verifiziert markieren“. Es gäbe nichts, was
        eine solche Angabe stützt: Wer eine Nutzlast einliest, hat sie
        erhalten, nicht geprüft. Diese Unterscheidung an der ersten Stelle
        aufzuweichen, machte sie überall wertlos.
      -->
      <Zustandsmarke
        marke={{
          zustand: "keineAussage",
          wort: "Wird als „nicht verifiziert“ aufgenommen",
          satz:
            "So fängt jeder Kontakt an. Sie haben die Schlüssel erhalten, aber " +
            "nicht geprüft, von wem. Gleich danach können Sie die Safety Number " +
            "vergleichen — das dauert eine Minute und ist der einzige Schritt, " +
            "der aus „bekannt“ ein „verifiziert“ macht.",
        }}
      />

      <div class="flex flex-wrap items-center gap-3">
        <button
          class="bg-schrift text-grund rounded-md px-5 py-2.5 text-sm font-medium
                 disabled:cursor-not-allowed disabled:opacity-40"
          disabled={!bereit}
          onclick={aufnehmen}
          data-pruefstelle="aufnehmen"
        >
          Aufnehmen
        </button>
        {#if !bereit}
          <span class="text-schrift-leise text-sm">
            Geben Sie einen Namen an.
          </span>
        {/if}
      </div>

      {#if kontaktspeicher.fehler}
        <Zustandsmarke
          marke={{
            zustand: "fehler",
            wort: "Der Kern hat abgelehnt",
            satz: kontaktspeicher.fehler,
          }}
          gross
        />
      {/if}
    </section>
  {/if}
</article>
