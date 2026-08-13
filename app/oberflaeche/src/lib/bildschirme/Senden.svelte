<!--
  Der Bildschirm „Senden“.

  DIE ENTSCHEIDENDE ANORDNUNG: Die Metadatenprüfung steht VOR dem
  Verschlüsseln, nicht danach. Wer erst verschlüsselt und dann berichtet, was
  drin war, stellt den Nutzer vor eine Datei, die er nicht mehr ändern kann,
  ohne von vorn zu beginnen.

  „Stör nur, wenn du wirklich etwas zu sagen hast“: Ist alles bereinigt,
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
  import { kontaktspeicher } from "../kern/speicher.svelte";

  // Derselbe Speicher wie im Verzeichnis: Ein Kontakt, den man gerade
  // aufgenommen hat, muss hier auftauchen. Ein Prototyp, dessen Teile
  // einander widersprechen, taugt nicht zum Beurteilen.
  const KONTAKTE = $derived(kontaktspeicher.liste);
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

  /**
   * Die vom Versand ausgenommenen Dateien.
   *
   * Ohne diese Möglichkeit gäbe es nur zwei Wege: alles senden oder von vorn
   * anfangen. Bei vierzig Dateien ist „von vorn“ so teuer, dass praktisch
   * jeder das Bestätigungshäkchen setzt — und dann erzieht die Bestätigung
   * genau zu dem Wegklicken, gegen das sie gebaut ist.
   *
   * Der dritte Weg — „diese drei eben nicht“ — muss deshalb der bequemste
   * sein, nicht der teuerste.
   *
   * (Der Name dient hier als Schlüssel. In Phase 4 kommt der volle Pfad aus
   * dem Kern; zwei gleichnamige Dateien aus verschiedenen Ordnern brauchen
   * dann eine stabile Kennung.)
   */
  /*
   * Je Stapel getrennt.
   *
   * Vorher war es eine einzige Liste. Wer im großen Stapel drei Dateien
   * herausnahm und dann umschaltete, nahm die drei Namen mit: Im neuen
   * Stapel gab es sie nicht, gefiltert wurde also nichts — aber die Zählung
   * meldete weiter „3 Dateien blieben hier“. Dieselbe Ursache wie beim
   * Bestätigungshäkchen: Ein Zustand, der zu etwas gehört, muss auch daran
   * hängen.
   */
  let ausgenommenJeStapel = $state<Record<string, string[]>>({});
  const ausgenommen = $derived(ausgenommenJeStapel[stapel.kennung] ?? []);

  const mitgesendet = $derived(stapel.dateien.filter((d) => !ausgenommen.includes(d.name)));
  const befund = $derived(fasseStapel(mitgesendet));

  /**
   * Alles, was nicht „unauffällig und dabei“ ist — in einem Block.
   *
   * Ausgenommene Dateien verschwinden **nicht**. Sonst hielte man das
   * Problem für gelöst, statt für umgangen: Die Datei ist ja noch da, sie
   * geht nur nicht mit.
   */
  const besonders = $derived(
    stapel.dateien.filter(
      (d) => ausgenommen.includes(d.name) || d.befund.fall !== "vollstaendig",
    ),
  );

  /** Die auffälligen, die noch mitgehen — nur sie verlangen eine Entscheidung. */
  const offeneAuffaellige = $derived(befund.auffaellig);
  const mussEntscheiden = $derived(brauchtEntscheidung(befund));

  /**
   * Wofür genau die Bestätigung erteilt wurde.
   *
   * Nicht `let gesehen = true` mit einem Rücksetzer: Die Bestätigung gilt für
   * **eine bestimmte Auswahl**, und das steht hier als Bedingung da statt als
   * Vorgang, der sie nachträglich wieder einsammelt. Ändert sich der Stapel
   * oder die Auswahl, passt die Kennung nicht mehr — und das Häkchen ist von
   * selbst weg, ohne dass irgendwo ein Rücksetzen vergessen werden kann.
   */
  let bestaetigtFuer = $state<string | null>(null);

  const auswahlKennung = $derived(
    `${stapel.kennung}|${[...ausgenommen].sort().join(",")}`,
  );
  const gesehen = $derived(bestaetigtFuer === auswahlKennung);

  function setzeAusgenommen(namen: string[]) {
    ausgenommenJeStapel = { ...ausgenommenJeStapel, [stapel.kennung]: namen };
  }

  function ausnehmen(name: string) {
    setzeAusgenommen(
      ausgenommen.includes(name)
        ? ausgenommen.filter((x) => x !== name)
        : [...ausgenommen, name],
    );
  }

  /** Der eine Klick, der die Sortierarbeit erspart. */
  function alleAuffaelligenAusnehmen() {
    setzeAusgenommen([...ausgenommen, ...offeneAuffaellige.map((d) => d.name)]);
  }

  // Anfangs der erste Kontakt. Nicht `KONTAKTE[0]` beim Initialisieren:
  // Das läse den Speicher einmal und bliebe dann stehen.
  let empfaenger = $state<string[]>(
    kontaktspeicher.liste[0] ? [kontaktspeicher.liste[0].fingerprint] : [],
  );
  let signieren = $state(true);

  const gewaehlt = $derived(KONTAKTE.filter((k) => empfaenger.includes(k.fingerprint)));

  // Ein Empfänger ohne Post-Quantum-Schlüssel zieht die ganze Nachricht auf
  // die klassische Suite herunter -- das muss dastehen.
  const ohnePq = $derived(gewaehlt.filter((k) => !k.hatPostQuantum));

  const gesamtGroesse = $derived(
    mitgesendet.reduce((summe, d) => summe + d.groesseBytes, 0),
  );

  const bereit = $derived(
    gewaehlt.length > 0 && mitgesendet.length > 0 && (!mussEntscheiden || gesehen),
  );

  /**
   * Ob der Vorgang gelaufen ist.
   *
   * An die Auswahl gebunden wie die Bestätigung: Ändert sich der Stapel,
   * gehört das Ergebnis nicht mehr dazu und verschwindet.
   */
  let fertigFuer = $state<string | null>(null);
  const fertig = $derived(fertigFuer === auswahlKennung);

  function verschluesseln() {
    if (!bereit) return;
    fertigFuer = auswahlKennung;
  }

  function umschalten(fp: string) {
    empfaenger = empfaenger.includes(fp)
      ? empfaenger.filter((x) => x !== fp)
      : [...empfaenger, fp];
  }
</script>

{#if fertig}
  <!-- ===================================================================
       Danach

       Was hier steht, entscheidet mit darüber, ob jemand sicher arbeitet:
       Die Klartextdatei liegt nach dem Verschlüsseln UNVERÄNDERT weiter auf
       der Platte. Wer das nicht sagt, lässt den Nutzer im Glauben, er habe
       etwas geschützt — dabei hat er nur eine zweite, verschlüsselte Kopie
       daneben gelegt.
       =================================================================== -->
  <article class="space-y-5">
    <Zustandsmarke
      marke={{
        zustand: "bestaetigt",
        wort:
          mitgesendet.length === 1
            ? "Verschlüsselt"
            : `${mitgesendet.length} Dateien verschlüsselt`,
        satz: `Für ${gewaehlt.length} ${
          gewaehlt.length === 1 ? "Empfänger" : "Empfänger"
        }, ${signieren ? "mit Ihrer Signatur" : "ohne Signatur"}.`,
      }}
      gross
    />

    <dl class="border-linie bg-flaeche grid gap-4 rounded-lg border p-4 sm:grid-cols-3">
      <Bezugswert beschriftung="Geschrieben nach" fest>
        {mitgesendet.length === 1
          ? `${mitgesendet[0]!.name}.cab`
          : "Ausgangsordner"}
      </Bezugswert>
      <Bezugswert beschriftung="Suite">
        {ohnePq.length > 0 ? "klassisch (0x0001)" : "Post-Quantum-Hybrid (0x0002)"}
      </Bezugswert>
      <Bezugswert beschriftung="Kapseln">{gewaehlt.length}</Bezugswert>
    </dl>

    <!--
      DER SATZ, DEN VERSCHLÜSSELUNGSWERKZEUGE GERN WEGLASSEN.
    -->
    <Zustandsmarke
      marke={{
        zustand: "warnung",
        wort: "Die Ausgangsdateien liegen unverschlüsselt weiter da",
        satz:
          "Verschlüsseln legt eine zweite Datei daneben, es ersetzt die erste " +
          "nicht. Wer den Rechner durchsucht, findet den Klartext genauso wie " +
          "vorher — sicher ist erst, was auch gelöscht wurde.",
      }}
      gross
    />

    {#if ausgenommen.length > 0}
      <Sollwert>
        {ausgenommen.length}
        {ausgenommen.length === 1 ? "Datei blieb" : "Dateien blieben"} hier und
        {ausgenommen.length === 1 ? "wurde" : "wurden"} nicht verschlüsselt
      </Sollwert>
    {/if}

    <p class="text-schrift-leise text-sm">
      Was ein Mitleser an der fertigen Datei erkennt: dass es ein Envelope ist
      und wie viele Kapseln er trägt. Nicht den Namen, nicht die Größe, nicht
      den Absender. Nachsehen können Sie das unter
      <span class="text-schrift">Werkzeuge → Außenansicht</span>.
    </p>

    <div class="border-linie flex flex-wrap gap-3 border-t pt-4">
      <button class="border-linie hover:bg-grund rounded-md border px-4 py-2 text-sm">
        Ordner öffnen
      </button>
      <button class="border-fehler text-fehler rounded-md border px-4 py-2 text-sm">
        Ausgangsdateien sicher löschen
      </button>
      <button
        class="border-linie hover:bg-flaeche rounded-md border px-4 py-2 text-sm"
        onclick={() => (fertigFuer = null)}
      >
        Zurück
      </button>
    </div>
  </article>
{:else}
<article class="space-y-5">
  <header class="flex flex-wrap items-baseline justify-between gap-2">
    <h2 class="text-xl font-semibold">
      {#if stapel.dateien.length === 1}
        {stapel.dateien[0]!.name}
      {:else if ausgenommen.length > 0}
        <!-- Beide Zahlen. „38 Dateien“ allein verschwiege die drei anderen. -->
        {mitgesendet.length} von {stapel.dateien.length} Dateien
      {:else}
        {stapel.dateien.length} Dateien
      {/if}
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

    {#if besonders.length === 0}
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
      {#if !mussEntscheiden}
        <!--
          Alles Auffällige ist ausgenommen. Das ist der Erfolgsfall dieses
          Bildschirms und gehört gesagt: Es geht sauber hinaus, weil eine
          Entscheidung getroffen wurde — nicht, weil sich etwas erledigt hat.
        -->
        <Zustandsmarke
          marke={{
            zustand: "bestaetigt",
            wort: `Was hinausgeht, ist bereinigt`,
            satz: `${ausgenommen.length} ${
              ausgenommen.length === 1 ? "Datei bleibt" : "Dateien bleiben"
            } hier. Von den übrigen ${mitgesendet.length} wurden alle bekannten Metadaten entfernt.`,
          }}
          gross
        />
      {/if}
      {#if befund.vollstaendig > 0}
        <!--
          Zugeklappt, aber vorhanden. „Nicht stören“ heißt nicht „nicht
          nachsehen können“: Wer wissen will, was aus den unauffälligen
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
                <label class="flex min-w-0 cursor-pointer items-baseline gap-2">
                  <input
                    type="checkbox"
                    checked={true}
                    onchange={() => ausnehmen(datei.name)}
                    aria-label="{datei.name} mitsenden"
                  />
                  <span class="text-schrift-leise break-all">{datei.name}</span>
                </label>
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

      <!--
        Jede dieser Dateien lässt sich einzeln abwählen. Das ist der Grund,
        warum die Zusammenfassung überhaupt tragfähig ist: Ohne diese
        Möglichkeit bliebe nur „alles senden“ oder „von vorn anfangen“.
      -->
      <div class="space-y-2" data-pruefstelle="besonders">
        {#each besonders as datei (datei.name)}
          {@const raus = ausgenommen.includes(datei.name)}
          {@const marke = markeFuerBereinigung(datei.befund)}
          <div
            class="space-y-2 rounded-lg border p-3
                   {raus ? 'border-sollwert/40 bg-transparent' : 'border-linie bg-flaeche'}"
          >
            <div class="flex flex-wrap items-baseline justify-between gap-2">
              <label class="flex min-w-0 cursor-pointer items-baseline gap-2">
                <input
                  type="checkbox"
                  checked={!raus}
                  onchange={() => ausnehmen(datei.name)}
                  aria-label="{datei.name} mitsenden"
                />
                <span class="break-all font-medium {raus ? 'text-schrift-leise line-through' : ''}">
                  {datei.name}
                </span>
              </label>
              <p class="text-bezug text-xs">{groesse(datei.groesseBytes)}</p>
            </div>

            {#if raus}
              <!--
                Magenta, nicht grau: Das ist keine Feststellung des
                Programms, sondern ein vom Nutzer eingestellter Sollwert.
                Die Datei ist nicht in Ordnung — sie geht nur nicht mit.
              -->
              <Sollwert>Bleibt hier — wird nicht verschlüsselt und nicht versendet</Sollwert>
              <p class="text-schrift-leise text-xs">
                {marke.wort}: {marke.satz}
              </p>
            {:else}
              <Zustandsmarke {marke} />
              {#if datei.befund.fall === "teilweise" && datei.befund.geblieben.length > 0}
                <Fundliste
                  funde={datei.befund.geblieben}
                  ueberschrift="Bleibt in der Datei"
                  offen
                />
              {/if}
            {/if}
          </div>
        {/each}
      </div>

      {#if mussEntscheiden}
        <!--
          Zwei Wege, und der sichere ist der bequemere. Ein einzelner Klick
          nimmt alle Auffälligen heraus — genau die Arbeit, die man sonst
          von Hand durch Neusortieren der Dateiauswahl erledigen müsste.
        -->
        {#if offeneAuffaellige.length > 1}
          <button
            class="border-sollwert/50 text-sollwert hover:bg-sollwert/10 w-full rounded-lg border border-dashed px-4 py-2.5 text-sm"
            onclick={alleAuffaelligenAusnehmen}
          >
            Diese {offeneAuffaellige.length} nicht mitsenden — die übrigen
            {stapel.dateien.length - ausgenommen.length - offeneAuffaellige.length} verschlüsseln
          </button>
        {/if}

        <!--
          Die Bestätigung ist kein Ritual. Sie ist die Stelle, an der aus
          „gezeigt“ ein „gesehen“ wird — und ohne sie geht es nicht weiter.
        -->
        <label
          class="border-warnung-rand bg-warnung-grund flex cursor-pointer items-start gap-3 rounded-lg border p-3"
        >
          <input
            type="checkbox"
            checked={gesehen}
            onchange={(e) =>
              (bestaetigtFuer = e.currentTarget.checked ? auswahlKennung : null)}
            class="mt-1"
          />
          <span class="text-sm">
            Ich habe gesehen, was in
            {offeneAuffaellige.length === 1 ? "dieser Datei" : "diesen Dateien"}
            bleibt, und will sie trotzdem verschlüsseln.
          </span>
        </label>
      {/if}
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
      <Bezugswert beschriftung="Dateien">
        {mitgesendet.length}{#if ausgenommen.length > 0}<span class="text-schrift-leise">
            &nbsp;von {stapel.dateien.length}</span
          >{/if}
      </Bezugswert>
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
        onclick={verschluesseln}
        data-pruefstelle="senden"
      >
        Verschlüsseln
      </button>
      {#if mitgesendet.length === 0}
        <span class="text-schrift-leise text-sm">
          Alle Dateien sind ausgenommen — es bleibt nichts zu verschlüsseln.
        </span>
      {:else if gewaehlt.length === 0}
        <span class="text-schrift-leise text-sm">Wählen Sie mindestens einen Empfänger.</span>
      {:else if mussEntscheiden && !gesehen}
        <span class="text-schrift-leise text-sm">
          Bestätigen Sie oben, dass Sie den Befund gesehen haben.
        </span>
      {/if}
    </div>
  </section>
</article>
{/if}
