<!--
  Der Bildschirm „Werkzeuge“.

  DAS SICHERE LÖSCHEN IST DER FALL, AN DEM SICH DIESER ENTWURF ENTSCHEIDET.
  v1 hatte drei Überschreibdurchgänge voreingestellt und suggerierte damit
  einen Nutzen, den zusätzliche Durchgänge auf heutigen Datenträgern nicht
  haben. Das ist die verbreitetste Unwahrheit dieser Werkzeugklasse: Sie
  zeigt Arbeit, wo keine Wirkung ist.

  Die ehrliche Fassung ist unbequem. Auf einer SSD, einem Copy-on-Write-
  Dateisystem oder bei Snapshots wirkt Überschreiben nicht verlässlich — und
  das ist der NORMALFALL, nicht die Ausnahme. Eine Oberfläche, die deshalb
  dauernd gelb leuchtet, erzieht zum Wegsehen (`spec/anzeige.md` §4.3). Der
  Hinweis gehört deshalb einmal deutlich an die Stelle, an der gelöscht
  wird, und nicht als Dauerzustand in die Kopfzeile.

  „Kopien können nicht ausgeschlossen werden“ erscheint fast immer. Das ist
  Absicht: Es ist ehrlicher als eine Anbieterliste, die nie vollständig
  wird.
-->
<script lang="ts">
  import type {
    Aussenansicht,
    Loeschbeurteilung,
    Loeschergebnis,
    Loeschkandidat,
    Loeschvorbehalt,
    Stapelstand,
  } from "../kern/typen";
  import { LOESCHFAELLE, AUSSENANSICHTEN } from "../kern/mock";
  import { groesse } from "../anzeige/zustand";
  import Zustandsmarke from "../anzeige/Zustandsmarke.svelte";
  import Bezugswert from "../anzeige/Bezugswert.svelte";
  import Fortschrittsbalken from "../anzeige/Fortschrittsbalken.svelte";

  interface Props {
    /**
     * Die ausgewählten Dateien, samt Beurteilung — nur im Fenster gesetzt.
     *
     * Ist sie leer, bleiben die Beispielfälle stehen: Sie zeigen die
     * Lagen, die man auf dem eigenen Rechner selten herstellen kann.
     */
    kandidaten?: Loeschkandidat[];
    /** Lässt Dateien auswählen. */
    waehlen?: () => void;
    /** Löscht sie. **Unwiderruflich.** */
    loeschen?: (durchgaenge: number) => void;
    /** Nimmt die Auswahl zurück. */
    leeren?: () => void;
    /** Was beim letzten Löschen herauskam. */
    ergebnisse?: Loeschergebnis[];
    arbeitet?: boolean;
    /**
     * Wie weit der laufende Stapel ist. `null` heißt: keiner läuft.
     *
     * Hier zählt das mehr als anderswo: Löschen ist **unwiderruflich** und
     * der langsamste Vorgang des Programms. Ein Fenster, das dabei nichts
     * sagt, ist von einem hängenden nicht zu unterscheiden — und wer es
     * für hängend hält, greift zum Task-Manager, mitten im Überschreiben.
     */
    fortschritt?: Stapelstand | null;
  }
  let {
    kandidaten = [],
    waehlen,
    loeschen,
    leeren,
    ergebnisse = [],
    arbeitet = false,
    fortschritt = null,
  }: Props = $props();

  /**
   * Wofür bestätigt wurde.
   *
   * An die **Auswahl** gebunden, nicht als Schalter: Ändert sich die
   * Liste, gilt die Bestätigung nicht mehr — und das steht hier als
   * Bedingung da statt als Vorgang, der sie nachträglich einsammelt.
   * Dieselbe Regel wie beim Senden.
   */
  let bestaetigtFuer = $state<string | null>(null);
  const auswahlKennung = $derived(kandidaten.map((k) => k.pfad).join("|"));
  const bestaetigt = $derived(
    kandidaten.length > 0 && bestaetigtFuer === auswahlKennung,
  );
  type Werkzeug = "loeschen" | "aussenansicht";

  let werkzeug = $state<Werkzeug>("loeschen");
  let fall = $state(0);

  /**
   * Was gezeigt wird: die echte Auswahl oder ein Beispiel.
   *
   * Echte Dateien haben Vorrang. Die Beispiele bleiben daneben stehen —
   * sie zeigen Lagen (Netzlaufwerk, Cloud-Ordner, schreibgeschützt), die
   * man auf dem eigenen Rechner selten herstellen kann.
   */
  const echt = $derived(kandidaten.length > 0);
  const gezeigt = $derived(
    echt ? kandidaten[Math.min(fall, kandidaten.length - 1)]! : null,
  );
  const fallDatei = $derived(
    gezeigt
      ? {
          pfad: gezeigt.pfad,
          groesseBytes: gezeigt.groesseBytes,
          beurteilung: gezeigt.beurteilung,
        }
      : LOESCHFAELLE[fall]!,
  );
  const befund = $derived(fallDatei.beurteilung);

  /**
   * Die Marke zum Löschbefund.
   *
   * `bestEffort` ist gelb, aber der Satz nennt den Grund — sonst liest man
   * es als Fehler des Programms statt als Eigenschaft des Datenträgers.
   */
  function markeFuerLoeschen(b: Loeschbeurteilung) {
    switch (b.faehigkeit) {
      case "ueberschreiben":
        return {
          zustand: "bestaetigt" as const,
          wort: "Überschreiben wirkt hier",
          satz:
            "Rotierender Datenträger, kein Copy-on-Write, keine erkennbaren " +
            "Snapshots. Die überschriebenen Stellen sind dieselben physischen.",
        };
      case "bestEffort":
        return {
          zustand: "warnung" as const,
          wort: "Überschreiben ist hier nicht verlässlich",
          satz:
            "Der Datenträger entscheidet selbst, wohin er schreibt — die alten " +
            "Blöcke bleiben womöglich stehen. Das ist der Normalfall auf " +
            "heutigen Systemen und kein Fehler des Programms.",
        };
      case "nichtMoeglich":
        return {
          zustand: "fehler" as const,
          wort: "Überschreiben nicht möglich",
          satz:
            "Netzlaufwerk, Schreibschutz oder kein Zugriff. Die Datei lässt " +
            "sich löschen, aber über ihren Verbleib ist nichts zu sagen.",
        };
    }
  }

  function vorbehaltText(v: Loeschvorbehalt): { wort: string; satz: string } {
    switch (v.art) {
      case "cloudOrdner":
        return {
          wort: "Liegt in einem Synchronisationsordner",
          satz: `Erkannt an: ${v.hinweis}. Serverkopien erreicht lokales Löschen nicht — dort muss gesondert gelöscht werden.`,
        };
      case "kopienMoeglich":
        return {
          wort: "Kopien außerhalb des Zugriffs sind nicht auszuschließen",
          satz:
            "Sicherungen, Vorschaubilder, Auslagerungsdatei, Papierkorb eines " +
            "anderen Programms. Dieser Hinweis erscheint immer, außer es steht " +
            "positiv fest, dass es sich um ein einfaches lokales Laufwerk handelt.",
        };
      case "wechselOderNetz":
        return {
          wort: "Wechselmedium oder Netzlaufwerk",
          satz: "Was dort tatsächlich geschieht, liegt außerhalb dieses Rechners.",
        };
      case "warSchreibgeschuetzt":
        return {
          wort: "Die Datei war schreibgeschützt",
          satz: "Das Attribut wurde entfernt, um überschreiben zu können.",
        };
      case "virtualisiert":
        return {
          wort: "Dieses System läuft virtualisiert",
          satz:
            `Erkannt an: ${v.hinweis}. Was unter dem virtuellen Laufwerk ` +
            "liegt, ist von hier aus nicht feststellbar — es kann eine SSD " +
            "sein, auch wenn das Gastsystem eine rotierende Platte meldet. " +
            "Überschreiben wird deshalb nicht zugesagt.",
        };
      case "zeitstempelBlieb":
        return {
          wort: "Der Zeitstempel blieb erhalten",
          satz:
            "Er konnte nicht normalisiert werden. Aus ihm lässt sich weiterhin " +
            "ablesen, wann die Datei zuletzt angefasst wurde.",
        };
    }
  }

  /** Die Zahl der Durchgänge — mit dem Grund, warum eins genügt. */
  let durchgaenge = $state(1);

  let aussenFall = $state(0);
  const aussen = $derived(AUSSENANSICHTEN[aussenFall]!);
  function aussenText(a: Aussenansicht): string {
    return a.fassung === "v1"
      ? "Version 1 schrieb den Kopf im Klartext. Wer die Datei abfängt, liest den Dateinamen und die Größe mit — ohne jeden Schlüssel."
      : "Sichtbar ist nur, dass es sich um einen Envelope handelt, und wie viele Kapseln er trägt. Namen, Größe und Absender stecken im verschlüsselten Teil.";
  }
</script>

<article class="space-y-5">
  <!-- Werkzeugwahl -->
  <div class="border-linie flex flex-wrap gap-1 border-b pb-3">
    {#each [{ k: "loeschen" as const, n: "Sicher löschen" }, { k: "aussenansicht" as const, n: "Außenansicht" }] as w (w.k)}
      <button
        class="rounded-md px-3 py-1.5 text-sm transition
               {werkzeug === w.k
          ? 'bg-schrift text-grund font-medium'
          : 'text-schrift-leise hover:bg-flaeche'}"
        onclick={() => (werkzeug = w.k)}
      >
        {w.n}
      </button>
    {/each}
  </div>

  {#if werkzeug === "loeschen"}
    <!-- =================================================================
         Sicheres Löschen
         ================================================================= -->
    <section class="space-y-4">
      {#if waehlen}
        <div class="flex flex-wrap items-center gap-2">
          <button
            class="bg-schrift text-grund rounded-md px-4 py-2 text-sm font-medium
                   disabled:cursor-not-allowed disabled:opacity-40"
            disabled={arbeitet}
            onclick={waehlen}
          >
            {arbeitet ? "Wird geprüft…" : "Dateien auswählen"}
          </button>
          {#if echt && leeren}
            <button
              class="border-linie text-schrift-leise hover:text-schrift rounded-md border px-3 py-1.5 text-sm"
              onclick={leeren}
            >
              Auswahl leeren
            </button>
          {/if}
        </div>
      {/if}

      {#if ergebnisse.length > 0}
        <!--
          Was tatsächlich geschah, Schritt für Schritt. Ein pauschales
          „Gelöscht“ wäre eine Behauptung über drei verschiedene Dinge, von
          denen jedes einzeln scheitern kann — Version 1 sagte genau dieses
          eine Wort.
        -->
        <div class="border-linie bg-flaeche space-y-2 rounded-lg border p-4">
          <h3 class="font-medium">
            {ergebnisse.filter((e) => e.entfernt).length} von {ergebnisse.length}
            {ergebnisse.length === 1 ? "Datei" : "Dateien"} entfernt
          </h3>
          <ul class="space-y-2 text-sm">
            {#each ergebnisse as e}
              <li class="border-linie rounded border p-2">
                <p class="font-mono text-xs break-all">{e.pfad}</p>
                <p class="text-schrift-leise mt-1 text-xs">
                  {e.ueberschrieben ? "überschrieben" : "nicht überschrieben"} ·
                  {e.umbenannt ? "umbenannt" : "nicht umbenannt"} ·
                  {e.entfernt ? "entfernt" : "nicht entfernt"}
                </p>
                {#if e.fehler}
                  <p class="text-fehler mt-1 text-xs">{e.fehler}</p>
                {/if}
              </li>
            {/each}
          </ul>
        </div>
      {/if}

      <div class="flex flex-wrap gap-1.5">
        {#each echt ? kandidaten : LOESCHFAELLE as f, i (f.pfad)}
          <button
            class="rounded-md px-3 py-1.5 text-xs transition
                   {fall === i
              ? 'border-schrift-leise bg-flaeche border'
              : 'border-linie text-schrift-leise border'}"
            onclick={() => (fall = i)}
          >
            {f.pfad.split(/[\\/]/).pop()}
          </button>
        {/each}
      </div>

      <header class="flex flex-wrap items-baseline justify-between gap-2">
        <h2 class="min-w-0 truncate font-mono text-sm">{fallDatei.pfad}</h2>
        <!--
          „Größe unbekannt“ statt „0 Bytes“. Eine Datei, die das Programm
          nicht ansehen kann, ist keine leere Datei — und hier steht sie
          auf dem Bildschirm, auf dem etwas unwiderruflich verschwindet.
        -->
        <p class="text-schrift-leise text-sm">
          {fallDatei.groesseBytes === null
            ? "Größe unbekannt"
            : groesse(fallDatei.groesseBytes)}
        </p>
      </header>

      <Zustandsmarke marke={markeFuerLoeschen(befund)} gross />

      <!--
        Die Vorbehalte einzeln und aufgeklappt. Sie sind die eigentliche
        Nachricht dieses Werkzeugs — nicht die Zahl der Durchgänge.
      -->
      {#if befund.vorbehalte.length > 0}
        <section class="space-y-2">
          <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
            Was Löschen hier nicht erreicht ({befund.vorbehalte.length})
          </h3>
          <ul class="space-y-2" data-pruefstelle="vorbehalte">
            {#each befund.vorbehalte as v}
              {@const t = vorbehaltText(v)}
              <li class="border-linie bg-flaeche rounded-lg border p-3">
                <p class="text-sm font-medium">{t.wort}</p>
                <p class="text-schrift-leise mt-1 text-sm">{t.satz}</p>
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      <!-- ===============================================================
           Die Durchgänge — und warum mehr nicht mehr hilft
           =============================================================== -->
      <section class="border-linie space-y-2 border-t pt-4">
        <label class="flex flex-wrap items-center gap-3 text-sm">
          <span class="text-schrift-leise">Überschreibdurchgänge</span>
          <input
            type="number"
            min="1"
            max="35"
            bind:value={durchgaenge}
            class="border-linie bg-grund w-20 rounded-md border px-3 py-1.5"
          />
        </label>

        {#if durchgaenge > 1}
          <!--
            Kein Verbot, aber auch kein Schweigen. Wer es trotzdem will,
            bekommt es — er soll nur nicht glauben, es nütze etwas.
          -->
          <Zustandsmarke
            marke={{
              zustand: "keineAussage",
              wort: `${durchgaenge} Durchgänge bringen keinen Zusatznutzen`,
              satz:
                "Einer genügt bei jedem Datenträger, der nach 2001 gebaut wurde. " +
                "Die verbreitete Annahme, 35 Durchgänge seien nötig, stammt aus " +
                "der Arbeit von Gutmann über MFM- und RLL-Kodierung der frühen " +
                "1990er Jahre und ist auf heutige Laufwerke nicht übertragbar. " +
                "Version 1 hatte drei voreingestellt und suggerierte damit einen " +
                "Nutzen, den es nicht gibt.",
            }}
          />
        {/if}

        <div class="space-y-3 pt-1">
          {#if loeschen}
            <!--
              Die Bestätigung steht VOR dem Knopf, nicht dahinter. Ein
              Rückfragefenster danach erzieht zum Wegklicken; ein Häkchen
              davor verlangt, dass jemand gelesen hat, was darüber steht.

              Und es hängt an der Auswahl: Kommt eine Datei dazu, ist es
              von selbst weg.
            -->
            <label class="flex cursor-pointer items-start gap-3 text-sm">
              <input
                type="checkbox"
                class="mt-1"
                checked={bestaetigt}
                onchange={() =>
                  (bestaetigtFuer = bestaetigt ? null : auswahlKennung)}
              />
              <span>
                Mir ist klar: <span class="font-medium">
                  {kandidaten.length}
                  {kandidaten.length === 1 ? "Datei ist" : "Dateien sind"}
                  danach fort</span
                >, und was oben steht, ist das, was Löschen hier
                <span class="font-medium">nicht</span> erreicht.
              </span>
            </label>
          {/if}

          <div class="flex flex-wrap items-center gap-3">
            <button
              class="border-fehler text-fehler hover:bg-fehler/10 rounded-md border px-5 py-2.5
                     text-sm font-medium disabled:cursor-not-allowed disabled:opacity-40"
              disabled={!loeschen || !bestaetigt || arbeitet}
              data-pruefstelle="endgueltig-loeschen"
              onclick={() => loeschen?.(durchgaenge)}
            >
              {arbeitet ? "Wird gelöscht…" : "Endgültig löschen"}
            </button>
            <span class="text-schrift-leise text-sm">
              Danach ist die Datei fort — hier gibt es kein Rückgängig.
            </span>
          </div>

          <!--
            Der Balken steht UNTER dem Knopf, dort wo eben noch der Satz
            über das fehlende Rückgängig stand. Wer hier wartet, sieht
            genau die Stelle an, an der er gerade geklickt hat.
          -->
          {#if fortschritt}
            <Fortschrittsbalken {fortschritt} />
          {/if}
        </div>
      </section>
    </section>
  {:else}
    <!-- =================================================================
         Außenansicht
         ================================================================= -->
    <section class="space-y-4">
      <div class="flex flex-wrap gap-1.5">
        {#each AUSSENANSICHTEN as a, i (a.fassung)}
          <button
            class="rounded-md border px-3 py-1.5 text-xs transition
                   {aussenFall === i
              ? 'border-schrift-leise bg-flaeche'
              : 'border-linie text-schrift-leise'}"
            onclick={() => (aussenFall = i)}
          >
            {a.fassung === "v1" ? "Eine Datei aus Version 1" : "Eine Datei aus Version 2"}
          </button>
        {/each}
      </div>

      <h2 class="text-xl font-semibold">Was ein Mitleser sieht</h2>
      <p class="text-sm">
        Ohne jeden Schlüssel — nur die Datei selbst. Nützlich, um zu prüfen,
        was eine verschlüsselte Sendung über sich preisgibt, bevor man sie
        aus der Hand gibt.
      </p>

      <dl class="border-linie bg-flaeche grid gap-4 rounded-lg border p-4 sm:grid-cols-3">
        <Bezugswert beschriftung="Fassung">{aussen.fassung}</Bezugswert>
        <Bezugswert beschriftung="Suite">{aussen.suite ?? "unbekannt"}</Bezugswert>
        <Bezugswert beschriftung="Kapseln">{aussen.kapseln ?? "unbekannt"}</Bezugswert>
      </dl>

      <Zustandsmarke
        marke={{
          zustand: aussen.offengelegt.length > 0 ? "warnung" : "bestaetigt",
          wort:
            aussen.offengelegt.length > 0
              ? "Der Kopf steht im Klartext"
              : "Nichts als die Kapselzahl",
          satz: aussenText(aussen),
        }}
        gross
      />

      {#if aussen.offengelegt.length > 0}
        <!--
          Die Sätze kommen aus dem Kern. Die Oberfläche zählt sie auf, statt
          sie zu deuten: Was ein Format preisgibt, hängt am Format, und
          feste Felder dafür wären beim nächsten schon zu eng.
        -->
        <div class="border-warnung-rand bg-warnung-grund space-y-1 rounded-lg border p-3">
          <p class="text-sm font-medium">Im Klartext lesbar</p>
          <ul class="space-y-0.5">
            {#each aussen.offengelegt as zeile, i (i)}
              <li class="text-bezug font-mono text-sm break-all">{zeile}</li>
            {/each}
          </ul>
        </div>
      {/if}

      <p class="text-schrift-leise text-sm">
        Auch bei Version 2 bleibt sichtbar, <span class="text-schrift">dass</span>
        Sie etwas verschickt haben und wie groß es ungefähr ist. Verschlüsselung
        verbirgt den Inhalt, nicht den Vorgang.
      </p>
    </section>
  {/if}
</article>
