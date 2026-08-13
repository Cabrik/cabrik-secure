<!--
  Der Bildschirm „Erste Einrichtung“.

  HIER STEHT DIE UNBEQUEMSTE ENTSCHEIDUNG DES GANZEN ENTWURFS:
  ES GIBT KEINE PASSWORT-STÄRKEANZEIGE.

  Jedes zweite Programm zeigt an dieser Stelle einen Balken, der von rot
  nach grün läuft. Er ist die bekannteste Lüge der Softwaregestaltung. Ein
  Programm kann nicht wissen, ob ein Passwort gut ist — es kennt die Liste
  nicht, in der es vielleicht steht. `Sommer2024!` erfüllt jede Regel über
  Länge, Groß- und Kleinschreibung, Ziffern und Sonderzeichen und steht
  trotzdem in jeder Angriffsliste. Der Balken zeigt Grün und meint Rot.

  Das ist exakt der Fall, für den es den vierten Anzeigezustand gibt
  (`spec/anzeige.md` §2.2): Grau, keine Aussage. Nicht „schwach“, nicht
  „stark“ — sondern der ehrliche Satz, dass dieses Programm es nicht
  beurteilen kann, und daneben ein Rat, der etwas taugt.

  DER ZWEITE SATZ, DER SONST NIE FÄLLT: Die Passwortableitung verteuert
  jeden Rateversuch. Sie macht ein erratbares Passwort nicht sicher. Wer das
  nicht sagt, verkauft Argon2 als Ersatz für ein gutes Passwort.
-->
<script lang="ts">
  import type { KdfStufe } from "../kern/typen";
  import Zustandsmarke from "../anzeige/Zustandsmarke.svelte";
  import Bezugswert from "../anzeige/Bezugswert.svelte";
  import Sollwert from "../anzeige/Sollwert.svelte";
  import { identitaetsspeicher } from "../kern/speicher.svelte";
  import type { Identitaet } from "../kern/typen";

  interface Props {
    /** Führt zur fertigen Identität — sonst müsste man sie selbst suchen. */
    ansehen?: (fingerprint: string) => void;
  }
  let { ansehen }: Props = $props();

  type Schritt = "willkommen" | "passwort" | "wahl" | "fertig";

  const SCHRITTE: { kennung: Schritt; name: string }[] = [
    { kennung: "willkommen", name: "Worum es geht" },
    { kennung: "passwort", name: "Passwort" },
    { kennung: "wahl", name: "Optionen" },
    { kennung: "fertig", name: "Fertig" },
  ];

  let schritt = $state<Schritt>("willkommen");

  let bezeichnung = $state("");
  let passwort = $state("");
  let wiederholung = $state("");
  let verstanden = $state(false);
  let signieren = $state(true);
  let kdf = $state<KdfStufe>("empfohlen");

  /**
   * Die Mindestlänge — mit Begründung, nicht als Schikane.
   *
   * Zwölf Zeichen sind keine magische Grenze. Sie sind die Stelle, ab der
   * ein reines Durchprobieren aller Zeichenfolgen bei dieser
   * Passwortableitung aussichtslos wird. Gegen das Raten aus einer Liste
   * hilft sie nicht — dagegen hilft nur, nichts Erratbares zu wählen.
   */
  const MINDESTLAENGE = 12;

  const langGenug = $derived(passwort.length >= MINDESTLAENGE);
  const stimmtUeberein = $derived(
    wiederholung.length > 0 && passwort === wiederholung,
  );
  const passwortFertig = $derived(langGenug && stimmtUeberein && verstanden);

  const KDF_WAHL: { wert: KdfStufe; wort: string; satz: string }[] = [
    {
      wert: "min",
      wort: "Minimum — 64 MiB",
      satz:
        "Untergrenze der Spezifikation. Nur für schwache Geräte. Macht das " +
        "Durchprobieren für einen Angreifer billiger.",
    },
    {
      wert: "empfohlen",
      wort: "Empfohlen — 256 MiB",
      satz: "Rund eine halbe Sekunde je Entsperrung. Spürbar, aber erträglich.",
    },
    {
      wert: "stark",
      wort: "Stark — 1 GiB",
      satz:
        "Deutlich langsamer, und zwar auch für Sie selbst: bei jedem " +
        "Entsperren, nicht nur beim Angreifer.",
    },
  ];

  /** Die tatsächlich angelegte Identität — erst nach dem letzten Schritt. */
  let angelegt = $state<Identitaet | null>(null);

  function weiter() {
    const i = SCHRITTE.findIndex((s) => s.kennung === schritt);
    if (i >= SCHRITTE.length - 1) return;

    // Der Übergang von der Auswahl zum Abschluss ist der Vorgang selbst:
    // Hier entsteht die Identität, nicht schon vorher.
    if (schritt === "wahl") {
      angelegt = identitaetsspeicher.anlegen(
        bezeichnung.trim() || "Ohne Bezeichnung",
        kdf,
        signieren,
      );
    }
    schritt = SCHRITTE[i + 1]!.kennung;
  }
  function zurueck() {
    const i = SCHRITTE.findIndex((s) => s.kennung === schritt);
    if (i > 0) schritt = SCHRITTE[i - 1]!.kennung;
  }
</script>

<article class="space-y-6">
  <!-- Fortschritt: sichtbar, wie viel noch kommt. -->
  <ol class="flex flex-wrap gap-2 text-xs">
    {#each SCHRITTE as s, i (s.kennung)}
      {@const jetzt = s.kennung === schritt}
      {@const vorbei = SCHRITTE.findIndex((x) => x.kennung === schritt) > i}
      <li
        class="rounded-md px-2.5 py-1
               {jetzt
          ? 'bg-schrift text-grund font-medium'
          : vorbei
            ? 'text-bestaetigt'
            : 'text-schrift-leise'}"
      >
        {vorbei ? "✓" : i + 1}. {s.name}
      </li>
    {/each}
  </ol>

  {#if schritt === "willkommen"}
    <!-- ================================================================= -->
    <section class="space-y-4">
      <h2 class="text-xl font-semibold">Sie erzeugen gleich einen Schlüssel</h2>
      <p class="text-sm leading-relaxed">
        Cabrik Secure verschlüsselt Dateien und Nachrichten so, dass nur die
        Empfänger sie öffnen können. Dafür brauchen Sie ein Schlüsselpaar: Der
        öffentliche Teil geht an andere, der private bleibt bei Ihnen, durch
        Ihr Passwort geschützt.
      </p>

      <div class="border-linie bg-flaeche space-y-3 rounded-lg border p-4">
        <p class="text-sm">
          <span class="font-medium">Was dieses Programm für Sie tut:</span> Es
          hält den privaten Schlüssel verschlossen, prüft Signaturen und
          entfernt Metadaten aus Dateien, die Sie versenden.
        </p>
        <p class="text-sm">
          <span class="font-medium">Was es nicht tut:</span> Es macht Sie nicht
          anonym. Wer den Datenverkehr beobachtet, sieht weiterhin, dass Sie
          etwas verschicken und an wen — nur nicht, was.
        </p>
      </div>

      <label class="flex items-center gap-3 text-sm">
        <span class="text-schrift-leise w-40 shrink-0">Bezeichnung</span>
        <input
          class="border-linie bg-grund flex-1 rounded-md border px-3 py-2"
          bind:value={bezeichnung}
          placeholder="Arbeitsrechner"
        />
      </label>
      <p class="text-schrift-leise text-xs">
        Nur für Sie. Wer Ihre Schlüssel aufnimmt, vergibt den Namen selbst —
        diese Bezeichnung wandert nicht mit.
      </p>
    </section>
  {:else if schritt === "passwort"}
    <!-- ================================================================= -->
    <section class="space-y-4">
      <h2 class="text-xl font-semibold">Ein Passwort für den Schlüssel</h2>

      <div class="space-y-3">
        <label class="block">
          <span class="text-schrift-leise mb-1 block text-sm">Passwort</span>
          <input
            type="password"
            class="border-linie bg-grund w-full rounded-md border px-3 py-2"
            bind:value={passwort}
          />
        </label>
        <label class="block">
          <span class="text-schrift-leise mb-1 block text-sm">Wiederholen</span>
          <input
            type="password"
            class="border-linie bg-grund w-full rounded-md border px-3 py-2"
            bind:value={wiederholung}
          />
        </label>
      </div>

      <!--
        Die einzigen zwei Dinge, die das Programm tatsächlich WEISS: wie
        lang es ist und ob beide Eingaben gleich sind. Beides steht als
        Tatsache da, nicht als Urteil — deshalb cyan, nicht grün.
      -->
      <dl class="border-linie bg-flaeche grid gap-4 rounded-lg border p-4 sm:grid-cols-2">
        <Bezugswert beschriftung="Länge">
          {passwort.length}
          {passwort.length === 1 ? "Zeichen" : "Zeichen"}
          {#if !langGenug}
            <span class="text-schrift-leise">— mindestens {MINDESTLAENGE}</span>
          {/if}
        </Bezugswert>
        <Bezugswert beschriftung="Wiederholung">
          {wiederholung.length === 0
            ? "noch nicht eingegeben"
            : stimmtUeberein
              ? "stimmt überein"
              : "stimmt nicht überein"}
        </Bezugswert>
      </dl>

      <!--
        Kein Balken. Der ehrliche Satz an seiner Stelle.
      -->
      <Zustandsmarke
        marke={{
          zustand: "keineAussage",
          wort: "Wie gut Ihr Passwort ist, kann dieses Programm nicht sagen",
          satz:
            "Es kennt die Listen nicht, aus denen geraten wird. „Sommer2024!“ " +
            "erfüllt jede Regel über Länge, Zeichenarten und Sonderzeichen und " +
            "steht trotzdem in jeder dieser Listen. Ein Balken, der hier Grün " +
            "zeigte, wäre geraten.",
        }}
        gross
      />

      <div class="border-linie bg-flaeche space-y-2 rounded-lg border p-4 text-sm">
        <p class="font-medium">Was stattdessen hilft</p>
        <p>
          Vier bis sechs <span class="font-medium">zufällig gewählte</span>
          Wörter, die keinen Satz ergeben — etwa gewürfelt oder aus einer Liste
          gezogen. Das lässt sich merken und ist nicht erratbar. Ausgedachte
          Wortfolgen sind es sehr wohl: Menschen wählen die immer gleichen.
        </p>
        <p class="text-schrift-leise">
          Die Passwortableitung im nächsten Schritt verteuert jeden einzelnen
          Rateversuch erheblich. Sie macht ein erratbares Passwort
          <span class="text-schrift">nicht</span> sicher — sie verschafft einem
          guten nur zusätzlichen Vorsprung.
        </p>
      </div>

      <!--
        Erscheint genau einmal im Leben dieser Installation. Deshalb ist es
        hier vertretbar — anders als eine Bestätigung, die bei jedem Vorgang
        kommt und dadurch zum Wegklicken erzieht.
      -->
      <label
        class="border-warnung-rand bg-warnung-grund flex cursor-pointer items-start gap-3 rounded-lg border p-3"
      >
        <input type="checkbox" bind:checked={verstanden} class="mt-1" />
        <span class="text-sm">
          Mir ist klar: <span class="font-medium">Vergesse ich dieses Passwort,
          ist alles dauerhaft unlesbar.</span> Es gibt keine Wiederherstellung,
          auch nicht durch den Hersteller.
        </span>
      </label>
    </section>
  {:else if schritt === "wahl"}
    <!-- ================================================================= -->
    <section class="space-y-5">
      <h2 class="text-xl font-semibold">Zwei Entscheidungen</h2>

      <div class="space-y-2">
        <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
          Signierschlüssel
        </h3>
        <label
          class="border-linie bg-flaeche flex cursor-pointer items-start gap-3 rounded-lg border p-3"
        >
          <input type="checkbox" bind:checked={signieren} class="mt-1" />
          <span class="text-sm">
            <span class="block font-medium">Signierschlüssel anlegen</span>
            <span class="text-schrift-leise block">
              {signieren
                ? "Empfänger können prüfen, dass Nachrichten von Ihnen stammen."
                : "Ohne ihn sind Ihre Nachrichten niemandem zuzuordnen — auch Ihnen nicht. Für Zuträger und Hinweisgeber ist das der richtige Modus."}
            </span>
          </span>
        </label>
        {#if !signieren}
          <Sollwert>Sie verzichten auf Zuordenbarkeit</Sollwert>
        {/if}
      </div>

      <div class="space-y-2">
        <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
          Passwortableitung
        </h3>
        {#each KDF_WAHL as w (w.wert)}
          <label
            class="flex cursor-pointer items-start gap-3 rounded-lg border p-3
                   {kdf === w.wert ? 'border-schrift-leise bg-flaeche' : 'border-linie'}"
          >
            <input type="radio" value={w.wert} bind:group={kdf} class="mt-1" />
            <span class="text-sm">
              <span class="block font-medium">{w.wort}</span>
              <span class="text-schrift-leise block">{w.satz}</span>
            </span>
          </label>
        {/each}
        <p class="text-schrift-leise text-xs">
          Die Angaben sind gemessen, nicht geschätzt — auf einem üblichen
          Rechner mit fertig gebautem Programm.
        </p>
      </div>
    </section>
  {:else}
    <!-- ================================================================= -->
    <section class="space-y-4">
      <Zustandsmarke
        marke={{
          zustand: "bestaetigt",
          wort: "Schlüssel erzeugt",
          satz: `Die Identität „${angelegt?.bezeichnung ?? "ohne Bezeichnung"}“ steht jetzt unter „Identität“ und ist einsatzbereit.`,
        }}
        gross
      />

      {#if angelegt}
        <!--
          Der Fingerprint gehört hierher, nicht erst auf den nächsten
          Bildschirm: Er ist das Einzige an dieser Identität, das jemand
          anders je nachprüfen kann.
        -->
        <div class="border-linie bg-flaeche space-y-2 rounded-lg border p-4">
          <p class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
            Ihr Fingerprint
          </p>
          <div class="grid grid-cols-3 gap-x-6 gap-y-2 sm:grid-cols-5">
            {#each angelegt.fingerprint.split(" ") as gruppe, i (i)}
              <span class="text-bezug font-mono tracking-wider">{gruppe}</span>
            {/each}
          </div>
        </div>
      {/if}

      <dl class="border-linie bg-flaeche grid gap-4 rounded-lg border p-4 sm:grid-cols-2">
        <Bezugswert beschriftung="Verschlüsselung">
          Post-Quantum-Hybrid (X-Wing)
        </Bezugswert>
        <Bezugswert beschriftung="Signierschlüssel">
          {signieren ? "vorhanden" : "keiner"}
        </Bezugswert>
        <Bezugswert beschriftung="Passwortableitung">
          {KDF_WAHL.find((w) => w.wert === kdf)!.wort}
        </Bezugswert>
        <Bezugswert beschriftung="Schlüsseldatei" fest>
          {angelegt?.pfad ?? ""}
        </Bezugswert>
      </dl>

      <div class="border-linie bg-flaeche space-y-2 rounded-lg border p-4 text-sm">
        <p class="font-medium">Zwei Dinge jetzt gleich</p>
        <p>
          <span class="text-schrift-leise">1.</span> Sichern Sie die
          Schlüsseldatei an einen zweiten Ort. Sie ist mit Ihrem Passwort
          verschlüsselt und nützt niemandem etwas, der sie findet.
        </p>
        <p>
          <span class="text-schrift-leise">2.</span> Geben Sie Ihre
          Austausch-Nutzlast weiter, damit man Ihnen schreiben kann. Sie
          enthält ausschließlich öffentliche Angaben.
        </p>
      </div>

      {#if angelegt && ansehen}
        <button
          class="bg-schrift text-grund rounded-md px-5 py-2.5 text-sm font-medium"
          onclick={() => ansehen(angelegt!.fingerprint)}
        >
          Zur Identität
        </button>
      {/if}
    </section>
  {/if}

  <!-- ===================================================================
       Navigation
       =================================================================== -->
  <div class="border-linie flex flex-wrap items-center gap-3 border-t pt-4">
    {#if schritt !== "willkommen" && schritt !== "fertig"}
      <button
        class="border-linie hover:bg-flaeche rounded-md border px-4 py-2 text-sm"
        onclick={zurueck}
      >
        Zurück
      </button>
    {/if}

    {#if schritt !== "fertig"}
      <button
        class="bg-schrift text-grund rounded-md px-5 py-2.5 text-sm font-medium
               disabled:cursor-not-allowed disabled:opacity-40"
        disabled={schritt === "passwort" && !passwortFertig}
        onclick={weiter}
        data-pruefstelle="weiter"
      >
        {schritt === "wahl" ? "Schlüssel erzeugen" : "Weiter"}
      </button>

      {#if schritt === "passwort" && !passwortFertig}
        <span class="text-schrift-leise text-sm">
          {#if !langGenug}
            Noch {MINDESTLAENGE - passwort.length} Zeichen.
          {:else if !stimmtUeberein}
            Die Wiederholung stimmt noch nicht überein.
          {:else}
            Bestätigen Sie oben, dass es keine Wiederherstellung gibt.
          {/if}
        </span>
      {/if}
    {/if}
  </div>
</article>
