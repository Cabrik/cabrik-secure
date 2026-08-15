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
  import type { Bereinigungswahl, Fund, Sendedatei } from "../kern/typen";
  import { FUNDART_TEXT, groesse, nachSchwere } from "../anzeige/zustand";
  import Zustandsmarke from "../anzeige/Zustandsmarke.svelte";
  import Sollwert from "../anzeige/Sollwert.svelte";

  interface Props {
    datei: Sendedatei;
    /** Ob derzeit das Original hinausgeht statt der bereinigten Fassung. */
    original: boolean;
    waehle: (original: boolean) => void;
    /** Die formatabhängigen Entscheidungen dieser Datei. */
    wahl: Bereinigungswahl;
    setzeWahl: (wahl: Bereinigungswahl) => void;
    schliessen: () => void;
  }
  let { datei, original, waehle, wahl, setzeWahl, schliessen }: Props = $props();

  function aendere(teil: Partial<Bereinigungswahl>) {
    setzeWahl({ ...wahl, ...teil });
  }

  /**
   * Ob es frühere Fassungen gibt.
   *
   * Eine einzelne Fassung ist der Normalfall und keine Nachricht — dann
   * steckt in der Datei nichts, was ein Leser nicht anzeigt.
   */
  const mehrereFassungen = $derived(datei.fassungen.length > 1);

  /** Zeilen, die nur in früheren Fassungen stehen -- also entfernter Text. */
  const entfernterText = $derived(
    datei.fassungen.flatMap((f) => f.nurHier),
  );

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

  /**
   * Welche Funde die Office-Schalter betreffen.
   *
   * Die Schalter werden nur angeboten, wenn es tatsächlich etwas zu
   * schalten gibt. Ein Häkchen ohne Wirkung ist eine Behauptung über die
   * Datei.
   */
  const hatKommentare = $derived(
    alleFunde.some((e) => e.fund.art === "kommentar"),
  );
  const hatAenderungen = $derived(
    alleFunde.some((e) => e.fund.art === "nachverfolgte_aenderung"),
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
        <ul
          class="border-linie divide-linie bg-flaeche divide-y rounded-lg border"
          data-pruefstelle="funde"
        >
          <!--
            Ohne Schlüssel. Ein Schlüssel muss eindeutig sein, und Fundstelle
            und Art sind es nicht: Ein großes ICC-Farbprofil wird über
            mehrere APP2-Segmente verteilt, und jedes ergibt einen eigenen
            Fund an derselben Stelle. Svelte bricht dann beim Zeichnen ab --
            und ein Bildschirm, der mitten im Aufbau abbricht, bleibt einfach
            stehen. Von außen sah das aus wie ein toter Knopf.

            Diese Liste braucht ohnehin keine Kennung: Sie wird nicht
            umsortiert und nicht teilweise ersetzt, sondern ganz neu gebaut,
            wenn die Datei wechselt.
          -->
          {#each sortiert as eintrag}
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
       1b. Frühere Fassungen

       KEIN METADATUM, SONDERN INHALT, DER NOCH MITFÄHRT.
       PDFs werden inkrementell fortgeschrieben: Jede Bearbeitung hängt
       hinten an, statt zu ersetzen. Wer Namen aus einem Dokument entfernt
       und es speichert, hat die vorige Fassung mit den Namen weiterhin in
       der Datei. Ein Leser zeigt sie nicht an. Ein Werkzeug schon.

       Deshalb steht das hier gesondert und nicht in der Fundliste: Es ist
       kein Eintrag in einem Kopfbereich, es ist Text.
       =================================================================== -->
  {#if mehrereFassungen}
    <section class="space-y-3" data-pruefstelle="fassungen">
      <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
        Frühere Fassungen ({datei.fassungen.length})
      </h3>

      {#if entfernterText.length > 0}
        <Zustandsmarke
          marke={{
            zustand: "warnung",
            wort: "In dieser Datei steckt Text, den jemand entfernt hat",
            satz:
              `Die Datei enthält alle ${datei.fassungen.length} Fassungen; angezeigt wird nur die letzte. ` +
              `${entfernterText.length} ${entfernterText.length === 1 ? "Zeile steht" : "Zeilen stehen"} ` +
              "nur in früheren Fassungen — sie wurden herausgenommen und fahren trotzdem mit.",
          }}
          gross
        />
      {:else}
        <Zustandsmarke
          marke={{
            zustand: "keineAussage",
            wort: `${datei.fassungen.length} Fassungen`,
            satz:
              "Die Datei enthält alle; angezeigt wird nur die letzte. Aus " +
              "keiner früheren wurde Text entfernt, den die aktuelle nicht " +
              "mehr enthält.",
          }}
        />
      {/if}

      <ul class="border-linie divide-linie bg-flaeche divide-y rounded-lg border">
        {#each datei.fassungen as f (f.nummer)}
          <li class="space-y-1.5 px-4 py-3">
            <div class="flex flex-wrap items-baseline justify-between gap-2">
              <p class="text-sm font-medium">
                Fassung {f.nummer}
                {#if f.wirdAngezeigt}
                  <span class="text-bestaetigt ml-1 text-xs">wird angezeigt</span>
                {/if}
              </p>
              <p class="text-bezug text-xs">
                {groesse(f.bytes)} · {f.seiten}
                {f.seiten === 1 ? "Seite" : "Seiten"}
              </p>
            </div>

            {#if f.nurHier.length > 0}
              <!--
                Die eigentliche Auskunft. Nicht „wie sah diese Fassung aus“,
                sondern „was wurde herausgenommen und fährt trotzdem mit“.
              -->
              <p class="text-warnung text-xs font-medium">
                Nur hier — später entfernt:
              </p>
              <ul class="space-y-0.5">
                {#each f.nurHier as zeile, i (i)}
                  <li class="border-warnung-rand border-l-2 pl-3 text-sm break-words">
                    {zeile}
                  </li>
                {/each}
              </ul>
            {:else if f.auszug}
              <p class="text-schrift-leise text-sm break-words">{f.auszug}</p>
            {/if}
          </li>
        {/each}
      </ul>
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

  <!-- ===================================================================
       3. Was das Format sonst noch zur Wahl stellt

       KEINE SCHALTER, SONDERN ZIELKONFLIKTE. Jeder ist manchmal richtig
       und manchmal fatal — und keiner darf voreingestellt sein, der den
       Inhalt verändert.
       =================================================================== -->
  {#if !original && (mehrereFassungen || hatKommentare || hatAenderungen)}
    <section class="space-y-2" data-pruefstelle="wahl">
      <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
        Beim Bereinigen außerdem
      </h3>

      {#if mehrereFassungen}
        <!-- PDF: welche Fassung eingeflacht wird. -->
        <div class="border-linie bg-flaeche space-y-2 rounded-lg border p-3">
          <p class="text-sm font-medium">Welche Fassung eingeflacht wird</p>
          <label class="flex cursor-pointer items-baseline gap-2 text-sm">
            <input
              type="radio"
              name="revision"
              checked={wahl.fassung === null && !wahl.historieBehalten}
              onchange={() => aendere({ fassung: null, historieBehalten: false })}
            />
            <span>
              Die angezeigte Fassung
              <span class="text-schrift-leise">
                — die Historie verschwindet, samt allem, was aus ihr entfernt
                wurde
              </span>
            </span>
          </label>
          {#each datei.fassungen.filter((f) => !f.wirdAngezeigt) as f (f.nummer)}
            <label class="flex cursor-pointer items-baseline gap-2 text-sm">
              <input
                type="radio"
                name="revision"
                checked={wahl.fassung === f.nummer}
                onchange={() =>
                  aendere({ fassung: f.nummer, historieBehalten: false })}
              />
              <span>
                Fassung {f.nummer}
                <span class="text-schrift-leise">
                  — diese wird zur einzigen; spätere Bearbeitungen gehen
                  verloren
                </span>
              </span>
            </label>
          {/each}

          <label
            class="flex cursor-pointer items-baseline gap-2 border-t border-linie pt-2 text-sm"
          >
            <input
              type="radio"
              name="revision"
              checked={wahl.historieBehalten}
              onchange={() =>
                aendere({ historieBehalten: true, fassung: null })}
            />
            <span>
              Historie behalten
              <span class="text-schrift-leise">
                — für Beweismittel und Archivierung, wo das Dokument nicht
                verändert werden darf
              </span>
            </span>
          </label>

          {#if wahl.historieBehalten}
            <!--
              Die Folge, und zwar sofort: Wer die Historie behält, sendet
              alles mit, was je darin stand. Das kann genau richtig sein --
              es darf nur nicht unbemerkt geschehen.
            -->
            <Zustandsmarke
              marke={{
                zustand: "warnung",
                wort: "Frühere Fassungen bleiben wiederherstellbar",
                satz:
                  entfernterText.length > 0
                    ? `Auch die ${entfernterText.length} ${entfernterText.length === 1 ? "Zeile" : "Zeilen"}, die später entfernt wurden, gehen mit hinaus.`
                    : "Alles, was je in dieser Datei stand, geht mit hinaus.",
              }}
            />
          {:else if wahl.fassung !== null}
            <Sollwert>Fassung {wahl.fassung} wird zur einzigen</Sollwert>
          {/if}
        </div>
      {/if}

      {#if hatKommentare}
        <label
          class="border-linie bg-flaeche flex cursor-pointer items-start gap-3 rounded-lg border p-3"
        >
          <input
            type="checkbox"
            checked={wahl.kommentareEntfernen}
            onchange={(e) =>
              aendere({ kommentareEntfernen: e.currentTarget.checked })}
            class="mt-1"
          />
          <span class="text-sm">
            <span class="block font-medium">Anmerkungen entfernen</span>
            <span class="text-schrift-leise block">
              Betrifft nur die Anmerkungen. Der Text bleibt Zeichen für
              Zeichen erhalten.
            </span>
          </span>
        </label>
      {/if}

      {#if hatAenderungen}
        <label
          class="border-linie bg-flaeche flex cursor-pointer items-start gap-3 rounded-lg border p-3"
        >
          <input
            type="checkbox"
            checked={wahl.aenderungenAnnehmen}
            onchange={(e) =>
              aendere({ aenderungenAnnehmen: e.currentTarget.checked })}
            class="mt-1"
          />
          <span class="text-sm">
            <span class="block font-medium">
              Nachverfolgte Änderungen annehmen
            </span>
            <span class="text-schrift-leise block">
              Wie „Alle Änderungen annehmen“ in Word: Einfügungen bleiben,
              Löschungen verschwinden samt Text.
            </span>
          </span>
        </label>
        {#if wahl.aenderungenAnnehmen}
          <!--
            Der einzige Schalter, der den INHALT verändert. Das gehoert
            gesagt, und zwar nicht im Kleingedruckten: Der Empfänger
            bekommt ein anderes Dokument, als Sie geöffnet haben.
          -->
          <Zustandsmarke
            marke={{
              zustand: "warnung",
              wort: "Das verändert den Inhalt",
              satz:
                "Der Empfänger bekommt ein anderes Dokument, als Sie hier " +
                "geöffnet haben. Gelöschter Text ist danach fort — prüfen " +
                "Sie das Ergebnis, bevor Sie es aus der Hand geben.",
            }}
          />
        {/if}
      {/if}
    </section>
  {/if}
</article>
