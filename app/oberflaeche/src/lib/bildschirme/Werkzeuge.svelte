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
  import type { Aussenansicht, Loeschbefund, Loeschvorbehalt } from "../kern/typen";
  import { LOESCHFAELLE, AUSSENANSICHTEN } from "../kern/mock";
  import { groesse } from "../anzeige/zustand";
  import Zustandsmarke from "../anzeige/Zustandsmarke.svelte";
  import Bezugswert from "../anzeige/Bezugswert.svelte";

  type Werkzeug = "loeschen" | "aussenansicht";

  let werkzeug = $state<Werkzeug>("loeschen");
  let fall = $state(0);

  const befund = $derived(LOESCHFAELLE[fall]!);

  /**
   * Die Marke zum Löschbefund.
   *
   * `bestEffort` ist gelb, aber der Satz nennt den Grund — sonst liest man
   * es als Fehler des Programms statt als Eigenschaft des Datenträgers.
   */
  function markeFuerLoeschen(b: Loeschbefund) {
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
    return a.fassung === 1
      ? "Version 1 schrieb den Kopf im Klartext. Wer die Datei abfängt, liest den Dateinamen und die Größe mit — ohne jeden Schlüssel."
      : "Sichtbar ist nur, dass es sich um einen Envelope handelt, und wie viele Kapseln er trägt. Namen, Größe und Absender stecken im verschlüsselten Teil.";
  }
</script>

<article class="space-y-5">
  <!-- Werkzeugwahl -->
  <div class="border-linie flex gap-1 border-b pb-3">
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
      <div class="flex flex-wrap gap-1.5">
        {#each LOESCHFAELLE as f, i (f.pfad)}
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
        <h2 class="min-w-0 truncate font-mono text-sm">{befund.pfad}</h2>
        <p class="text-schrift-leise text-sm">{groesse(befund.groesseBytes)}</p>
      </header>

      <Zustandsmarke marke={markeFuerLoeschen(befund)} gross />

      <p class="text-schrift-leise text-sm">
        <span class="text-schrift">Woran das hängt:</span>
        {befund.grundlage}
      </p>

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
            {#each befund.vorbehalte as v (v.art)}
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

        <div class="flex flex-wrap items-center gap-3 pt-1">
          <button
            class="border-fehler text-fehler hover:bg-fehler/10 rounded-md border px-5 py-2.5 text-sm font-medium"
          >
            Endgültig löschen
          </button>
          <span class="text-schrift-leise text-sm">
            Danach ist die Datei fort — hier gibt es kein Rückgängig.
          </span>
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
            {a.fassung === 1 ? "Eine Datei aus Version 1" : "Eine Datei aus Version 2"}
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
        <Bezugswert beschriftung="Fassung">Version {aussen.fassung}</Bezugswert>
        <Bezugswert beschriftung="Suite">{aussen.suite}</Bezugswert>
        <Bezugswert beschriftung="Kapseln">{aussen.kapseln}</Bezugswert>
      </dl>

      <Zustandsmarke
        marke={{
          zustand: aussen.fassung === 1 ? "warnung" : "bestaetigt",
          wort:
            aussen.fassung === 1
              ? "Der Kopf steht im Klartext"
              : "Nichts als die Kapselzahl",
          satz: aussenText(aussen),
        }}
        gross
      />

      {#if aussen.klartextDateiname}
        <div class="border-warnung-rand bg-warnung-grund space-y-1 rounded-lg border p-3">
          <p class="text-sm font-medium">Im Klartext lesbar</p>
          <p class="text-bezug font-mono text-sm">{aussen.klartextDateiname}</p>
          {#if aussen.klartextGroesse}
            <p class="text-bezug font-mono text-sm">
              {groesse(aussen.klartextGroesse)}
            </p>
          {/if}
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
