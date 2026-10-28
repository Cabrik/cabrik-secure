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
  import type { Eingespielt, Entsperrbedarf, KdfStufe } from "../kern/typen";
  import { MINDESTLAENGE } from "../kern/typen";
  import { groesseGerichtet } from "../anzeige/zustand";
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

  /**
   * Beim Einspielen fällt „Optionen“ weg: Signierschlüssel und Ableitung
   * stehen in der Sicherung und lassen sich nicht wählen. Ein Schritt, in
   * dem es nichts zu entscheiden gibt, täuschte eine Entscheidung vor.
   */
  const SCHRITTE_SICHERUNG: { kennung: Schritt; name: string }[] = [
    { kennung: "willkommen", name: "Worum es geht" },
    { kennung: "passwort", name: "Sicherung" },
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
   * Neu anlegen, einen Schlüssel aus Version 1 übernehmen — oder eine
   * Sicherung einspielen.
   *
   * **Kein Nebenweg.** Wer die ausgelieferte v1 benutzt hat, kommt ohne
   * die Übernahme an nichts mehr heran, was an ihn gerichtet wurde. Wer
   * eine Sicherung hat und stattdessen neu anlegt, ebenso. Die Wahl steht
   * deshalb im ersten Schritt und nicht in einer Einstellung.
   */
  let modus = $state<"neu" | "v1" | "sicherung">("neu");

  const schritte = $derived(modus === "sicherung" ? SCHRITTE_SICHERUNG : SCHRITTE);

  /** Die gewählte Sicherung. Leer heißt: noch keine gewählt. */
  let sicherungQuelle = $state("");
  /**
   * Was das Öffnen der Sicherung kostet — `null`, solange keine gewählt
   * ist oder die gewählte keine Sicherung ist. Warum nicht, steht dann in
   * `identitaetsspeicher.fehler`, und zwar sofort nach der Wahl.
   */
  let sicherungBedarf = $state<Entsperrbedarf | null>(null);
  /** Die Antwort auf die Rückfrage bei ungewöhnlichen Werten. */
  let ungewoehnlichZugelassen = $state(false);
  /**
   * Das Passwort der Sicherung. **Kein neues:** Eingespielt wird dieselbe
   * Identität in derselben Hülle.
   */
  let sicherungPasswort = $state("");
  /** Was nach dem Einspielen dasteht — mit der Auskunft über die Kontakte. */
  let eingespielt = $state<Eingespielt | null>(null);

  async function sicherungSuchen() {
    const gewaehlt = await identitaetsspeicher.sicherungWaehlen();
    if (!gewaehlt) return;
    sicherungQuelle = gewaehlt;
    ungewoehnlichZugelassen = false;
    sicherungBedarf = await identitaetsspeicher.sicherungPruefen(gewaehlt);
  }

  const sicherungRueckfrage = $derived(
    sicherungBedarf !== null && sicherungBedarf.ungewoehnlich && !ungewoehnlichZugelassen,
  );
  const sicherungFertig = $derived(
    sicherungBedarf !== null &&
      sicherungBedarf.passt &&
      !sicherungRueckfrage &&
      sicherungPasswort.length > 0,
  );
  /** Die alte Schlüsseldatei. Leer heißt: noch keine gewählt. */
  let v1Quelle = $state("");
  /** Das Passwort der alten Datei — es öffnet nur sie, nichts sonst. */
  let v1AltesPasswort = $state("");

  /**
   * Die Mindestlänge — **aus dem Kern**, nicht hier festgelegt.
   *
   * Sie stand bis vor kurzem allein an dieser Stelle. Damit hatte der
   * Passwortwechsel keine, und die Kommandozeile auch nicht: dieselbe
   * Entscheidung, an einer Tür bewacht und an dreien nicht.
   *
   * Zwölf Zeichen sind keine magische Grenze. Sie sind die Stelle, ab der
   * ein reines Durchprobieren aller Zeichenfolgen bei dieser
   * Passwortableitung aussichtslos wird. Gegen das Raten aus einer Liste
   * hilft sie nicht — dagegen hilft nur, nichts Erratbares zu wählen.
   */

  const langGenug = $derived(passwort.length >= MINDESTLAENGE);
  const stimmtUeberein = $derived(
    wiederholung.length > 0 && passwort === wiederholung,
  );
  /**
   * Bei der Übernahme gehören Datei und altes Passwort dazu.
   *
   * Ohne diese Bedingung liefe jemand in die langsame Ableitung und
   * bekäme danach „Das Passwort des alten Schlüssels passt nicht“ — für
   * einen Fehler, den man ihm sofort hätte sagen können.
   */
  const v1Fertig = $derived(
    modus !== "v1" || (v1Quelle.trim().length > 0 && v1AltesPasswort.length > 0),
  );
  const passwortFertig = $derived(
    langGenug && stimmtUeberein && verstanden && v1Fertig,
  );

  /** Ob es aus dem zweiten Schritt weitergehen kann. */
  const schrittFertig = $derived(modus === "sicherung" ? sicherungFertig : passwortFertig);

  async function v1DateiSuchen() {
    const gewaehlt = await identitaetsspeicher.v1DateiWaehlen();
    if (gewaehlt) v1Quelle = gewaehlt;
  }

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

  /** Läuft gerade die Ableitung? Argon2 braucht spürbar Zeit. */
  let arbeitet = $state(false);

  async function weiter() {
    const i = schritte.findIndex((s) => s.kennung === schritt);
    if (i >= schritte.length - 1) return;
    const naechster = schritte[i + 1]!.kennung;

    // Der Übergang zum Abschluss ist der Vorgang selbst: Hier entsteht die
    // Identität, nicht schon vorher.
    if (naechster === "fertig" && modus === "sicherung") {
      if (arbeitet) return;
      arbeitet = true;
      // Erst leeren, dann ableiten -- wie beim Anlegen.
      const versuch = sicherungPasswort;
      sicherungPasswort = "";
      eingespielt = await identitaetsspeicher.sicherungEinspielen(
        sicherungQuelle,
        versuch,
        ungewoehnlichZugelassen,
      );
      angelegt = eingespielt?.identitaet ?? null;
      arbeitet = false;
      if (!angelegt) return;
    } else if (naechster === "fertig") {
      if (arbeitet) return;
      arbeitet = true;
      const versuch = passwort;
      // Erst leeren, dann ableiten. Dazwischen liegt rund eine halbe
      // Sekunde, und in der soll in keinem Feld ein Passwort stehen
      // (`spec/entsperrung.md` §5.1).
      const altes = v1AltesPasswort;
      passwort = "";
      wiederholung = "";
      v1AltesPasswort = "";
      angelegt =
        modus === "v1"
          ? await identitaetsspeicher.ausV1Uebernehmen(
              v1Quelle,
              altes,
              versuch,
              bezeichnung.trim() || null,
              kdf,
            )
          : await identitaetsspeicher.anlegen(
              bezeichnung.trim() || null,
              versuch,
              kdf,
              signieren,
            );
      arbeitet = false;
      // Bei einem Fehlschlag stehenbleiben. Zum Abschluss weiterzugehen,
      // ohne dass etwas entstanden ist, wäre die schlimmste Art zu lügen:
      // Der Nutzer glaubte, er hätte einen Schlüssel.
      if (!angelegt) return;
    }
    schritt = naechster;
  }
  function zurueck() {
    const i = schritte.findIndex((s) => s.kennung === schritt);
    if (i > 0) schritt = schritte[i - 1]!.kennung;
  }
</script>

<article class="space-y-6">
  <!-- Fortschritt: sichtbar, wie viel noch kommt. -->
  <ol class="flex flex-wrap gap-2 text-xs">
    {#each schritte as s, i (s.kennung)}
      {@const jetzt = s.kennung === schritt}
      {@const vorbei = schritte.findIndex((x) => x.kennung === schritt) > i}
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
      <h2 class="text-xl font-semibold">
        {modus === "sicherung"
          ? "Sie holen Ihren Schlüssel zurück"
          : "Sie erzeugen gleich einen Schlüssel"}
      </h2>
      <p class="text-sm leading-relaxed">
        CabrikSecure verschlüsselt Dateien und Nachrichten so, dass nur die
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

      <!-- Bei einer Sicherung steht die Bezeichnung in der Datei. Ein
           Feld, dessen Inhalt nirgends ankommt, wäre eine Attrappe. -->
      {#if modus !== "sicherung"}
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
      {/if}

      <!-- Übernahme und Sicherung stehen hier und nicht in einer
           Einstellung: Wer schon einen Schlüssel hat und einen neuen
           anlegt, kommt an nichts mehr heran, was an den alten gerichtet
           wurde. -->
      <div class="border-linie space-y-3 rounded-lg border border-dashed p-4">
        <p class="text-sm font-medium">Haben Sie schon einen Schlüssel?</p>
        <p class="text-schrift-leise text-sm leading-relaxed">
          Aus Version 1 oder als Sicherung von CabrikSecure? Dann holen Sie ihn
          hierher, statt einen neuen anzulegen. Andernfalls kommen Sie an
          nichts mehr heran, was an ihn gerichtet wurde — auch nicht an das,
          was noch unterwegs ist.
        </p>
        <!-- Gewählt sieht aus wie der gewählte Bereich und der laufende
             Schritt: hell hinterlegt. Bis zum 28.10.2026 stand hier
             `bg-flaeche` -- dieselbe Farbe wie die Karte darunter, also
             unsichtbar. Im ersten Durchlauf auf einem frischen Rechner
             fiel das sofort auf. -->
        <div class="flex flex-wrap gap-2">
          {#each [{ wert: "neu" as const, name: "Neuen Schlüssel anlegen" }, { wert: "v1" as const, name: "Schlüssel aus Version 1 übernehmen" }, { wert: "sicherung" as const, name: "Sicherung einspielen" }] as w (w.wert)}
            <button
              type="button"
              class="rounded-md border px-3 py-2 text-sm transition
                     {modus === w.wert
                ? 'border-schrift bg-schrift text-grund font-medium'
                : 'border-linie text-schrift-leise hover:bg-flaeche'}"
              aria-pressed={modus === w.wert}
              onclick={() => (modus = w.wert)}
            >
              {w.name}
            </button>
          {/each}
        </div>
        {#if modus === "neu"}
          <p class="text-sm leading-relaxed">
            Es entsteht ein neues Schlüsselpaar. Was an einen früheren
            Schlüssel gerichtet war, öffnet es nicht.
          </p>
        {:else if modus === "sicherung"}
          <p class="text-sm leading-relaxed">
            Sie haben unter „Identität“ mit „Schlüsseldatei sichern“ eine
            Kopie angelegt? Dann kommt hier <span class="font-medium">dieselbe
            Identität</span> zurück — mit demselben Fingerprint und dem
            Passwort, das beim Sichern galt. Ihre Kontakte müssen nichts neu
            prüfen.
          </p>
        {:else}
          <p class="text-sm leading-relaxed">
            <span class="font-medium">Nur einmal übernehmen.</span> Bei jeder
            Übernahme entsteht ein neuer Post-Quantum-Schlüssel — zweimal
            übernehmen ergäbe zwei Identitäten, die Sie für eine halten. Für
            einen zweiten Rechner nehmen Sie die <em>entstandene</em>
            Schlüsseldatei mit.
          </p>
        {/if}
      </div>
    </section>
  {:else if schritt === "passwort" && modus === "sicherung"}
    <!-- ================================================================= -->
    <section class="space-y-4">
      <h2 class="text-xl font-semibold">Die Sicherung und ihr Passwort</h2>

      <!-- Erst die Datei, dann das Passwort. Ist es die Kontaktdatei oder
           ein Schlüssel aus Version 1, steht das da, bevor jemand tippt. -->
      <div class="border-linie bg-flaeche space-y-3 rounded-lg border p-4">
        <p class="text-sm font-medium">Die gesicherte Schlüsseldatei</p>
        <div class="flex items-center gap-3">
          <button
            type="button"
            class="border-linie rounded-md border px-3 py-2 text-sm"
            onclick={sicherungSuchen}
          >
            Datei wählen
          </button>
          <span class="text-schrift-leise truncate text-sm" data-pruef="sicherung-quelle">
            {sicherungQuelle || "Noch keine gewählt"}
          </span>
        </div>
        <p class="text-schrift-leise text-xs leading-relaxed">
          Sie endet auf „.cabrik-key“. Liegt daneben eine Datei gleichen
          Namens mit der Endung „.cabrik-contacts“, kommen Ihre Kontakte mit —
          sofern sie zu diesem Schlüssel gehört.
        </p>
      </div>

      {#if sicherungBedarf && sicherungRueckfrage}
        <!-- Dieselbe Rückfrage wie auf dem Sperrbildschirm, und aus
             demselben Grund vor dem Passwortfeld: Wer erst nach dem Tippen
             gefragt wird, hat umsonst getippt. -->
        <div
          class="border-warnung-rand bg-warnung-grund space-y-3 rounded-md border px-3 py-3 text-sm"
          data-pruef="rueckfrage"
        >
          <p class="text-warnung font-medium">
            Diese Sicherung verlangt ungewöhnlich viel:
            {groesseGerichtet(sicherungBedarf.speicherBytes, "auf")} Arbeitsspeicher
            und {sicherungBedarf.durchgaenge} Durchgänge. Das ist mehr, als CabrikSecure
            selbst je für einen Schlüssel verwendet.
          </p>
          <p class="text-schrift-leise leading-relaxed">
            Haben Sie die Datei selbst so eingerichtet, können Sie fortfahren —
            das Öffnen dauert dann entsprechend länger. Wenn nicht, ist sie
            womöglich verändert worden: Dann schlägt es ohnehin fehl, nur eben
            erst nach der langen Rechnung.
          </p>
          <button
            type="button"
            onclick={() => (ungewoehnlichZugelassen = true)}
            class="border-linie bg-grund hover:border-schrift w-full rounded-md border
                   px-4 py-2 text-sm font-medium"
          >
            Trotzdem fortfahren
          </button>
        </div>
      {:else if sicherungBedarf && !sicherungBedarf.passt}
        <div class="space-y-2" data-pruef="zu-wenig-speicher">
          <p class="text-warnung text-sm leading-relaxed">
            Zum Öffnen braucht diese Sicherung
            {groesseGerichtet(sicherungBedarf.speicherBytes, "auf")} Arbeitsspeicher.
            {#if sicherungBedarf.grenzeBytes !== null}
              Auf diesem Rechner stehen dafür gerade
              {groesseGerichtet(sicherungBedarf.grenzeBytes, "ab")} zur Verfügung.
            {/if}
          </p>
          <p class="text-schrift-leise text-xs leading-relaxed">
            Andere Programme zu schließen kann helfen; danach die Datei noch
            einmal wählen.
          </p>
        </div>
      {:else if sicherungBedarf}
        <label class="block">
          <span class="text-schrift-leise mb-1 block text-sm">Passwort der Sicherung</span>
          <input
            type="password"
            class="border-linie bg-grund w-full rounded-md border px-3 py-2"
            bind:value={sicherungPasswort}
          />
        </label>
        <!-- Der Fall, an dem man sonst lange rätselt: Nach einer
             Passwortänderung öffnet sich eine ältere Sicherung weiter mit
             dem alten Passwort. -->
        <p class="text-schrift-leise text-xs leading-relaxed">
          Es ist das Passwort, das galt, als die Sicherung entstand. Haben Sie
          es seitdem geändert, öffnet sich eine ältere Sicherung weiter mit
          dem alten.
        </p>
      {/if}
    </section>
  {:else if schritt === "passwort"}
    <!-- ================================================================= -->
    <section class="space-y-4">
      <h2 class="text-xl font-semibold">
        {modus === "v1" ? "Die alte Datei und ein neues Passwort" : "Ein Passwort für den Schlüssel"}
      </h2>

      {#if modus === "v1"}
        <!-- Erst die alte Datei, dann das alte Passwort, dann das neue.
             Dieselbe Reihenfolge wie im Kern: Wer die falsche Datei
             erwischt hat, soll das erfahren, statt an einem Passwort zu
             zweifeln, das gar nicht schuld ist. -->
        <div class="border-linie bg-flaeche space-y-3 rounded-lg border p-4">
          <p class="text-sm font-medium">Der Schlüssel aus Version 1</p>
          <div class="flex items-center gap-3">
            <button
              type="button"
              class="border-linie rounded-md border px-3 py-2 text-sm"
              onclick={v1DateiSuchen}
            >
              Datei wählen
            </button>
            <span class="text-schrift-leise truncate text-sm" data-pruef="v1-quelle">
              {v1Quelle || "Noch keine gewählt"}
            </span>
          </div>
          <label class="block">
            <span class="text-schrift-leise mb-1 block text-sm">
              Passwort der alten Datei
            </span>
            <input
              type="password"
              class="border-linie bg-grund w-full rounded-md border px-3 py-2"
              bind:value={v1AltesPasswort}
            />
          </label>
          <p class="text-schrift-leise text-xs leading-relaxed">
            Es öffnet nur die alte Datei. Verschlossen wird die neue mit dem
            Passwort darunter — das alte weiterzuverwenden wäre bequem, hieße
            aber, eine womöglich alte und schwache Wahl mitzuschleppen.
          </p>
        </div>
      {/if}

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
        {#if modus === "v1"}
          <!-- Keine Wahl, sondern eine Auskunft: Ob der alte Schlüssel
               einen Signierteil hat, steht in der alten Datei und lässt
               sich nachträglich nicht ändern. Eine Schaltfläche hier
               täuschte eine Entscheidung vor, die es nicht gibt. -->
          <p class="border-linie bg-flaeche rounded-lg border p-3 text-sm">
            Wird aus dem alten Schlüssel übernommen. Hatte er keinen, bleibt
            es dabei — das lässt sich nachträglich nicht ändern.
          </p>
        {:else}
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
          wort: modus === "sicherung" ? "Sicherung eingespielt" : "Schlüssel erzeugt",
          satz: `Die Identität „${angelegt?.bezeichnung ?? "ohne Bezeichnung"}“ steht jetzt unter „Identität“ und ist einsatzbereit.`,
        }}
        gross
      />

      {#if eingespielt}
        <!--
          Die Kontakte, ausgesprochen. Mit ihnen kommt der Stand ihrer
          Prüfung herein — und das soll niemand übersehen, auch nicht den
          Fall, in dem eine Kontaktdatei dalag und NICHT genommen wurde.
        -->
        <div class="border-linie bg-flaeche space-y-2 rounded-lg border p-4 text-sm" data-pruef="kontakte">
          <p class="font-medium">Ihre Kontakte</p>
          {#if eingespielt.herkunft === "sicherung" || eingespielt.herkunft === "programmordner"}
            <p class="leading-relaxed">
              {eingespielt.kontakte}
              {eingespielt.kontakte === 1 ? "Kontakt" : "Kontakte"} übernommen,
              davon {eingespielt.verifiziert} verifiziert —
              {eingespielt.herkunft === "sicherung"
                ? "aus der Kontaktdatei neben der Sicherung."
                : "aus der Kontaktdatei, die noch auf diesem Rechner lag."}
              Sehen Sie unter „Kontakte“ nach, ob es die sind, die Sie
              erwarten.
            </p>
          {:else if eingespielt.herkunft === "unpassend"}
            <p class="leading-relaxed">
              Neben der Sicherung lag eine Kontaktdatei, aber sie gehört nicht
              zu diesem Schlüssel — oder sie ist beschädigt.
              <span class="font-medium">Übernommen wurde sie nicht.</span>
            </p>
          {:else}
            <p class="leading-relaxed">
              Neben der Sicherung lag keine Kontaktdatei. Nehmen Sie Ihre
              Kontakte neu auf und vergleichen Sie sie noch einmal.
            </p>
          {/if}
        </div>
      {/if}

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
            {#each angelegt.fingerprint.split(/[-\s]+/) as gruppe, i (i)}
              <span class="text-bezug font-mono tracking-wider">{gruppe}</span>
            {/each}
          </div>
        </div>
      {/if}

      <dl class="border-linie bg-flaeche grid gap-4 rounded-lg border p-4 sm:grid-cols-2">
        <Bezugswert beschriftung="Verschlüsselung">
          {angelegt?.hatPostQuantum === false
            ? "nur klassisch (X25519)"
            : "Post-Quantum-Hybrid (X-Wing)"}
        </Bezugswert>
        <Bezugswert beschriftung="Signierschlüssel">
          <!-- Aus der ENTSTANDENEN Identität, nicht aus dem Schalter
               darüber. Hier stand `signieren` -- also die Absicht statt
               des Ergebnisses. Bei einer Übernahme aus Version 1 ist das
               schlicht falsch: Ob ein Signierteil da ist, steht in der
               alten Datei und nicht in einem Kästchen dieser Oberfläche. -->
          {angelegt?.hatSignierschluessel ? "vorhanden" : "keiner"}
        </Bezugswert>
        <Bezugswert beschriftung="Passwortableitung">
          <!-- Aus der Datei, wie der Signierschlüssel darüber. Bei einer
               eingespielten Sicherung hat niemand eine Stufe gewählt; die
               Wahl dieses Bildschirms stünde dort auf „empfohlen“, ganz
               gleich, was die Sicherung führt. -->
          {angelegt?.kdf
            ? KDF_WAHL.find((w) => w.wert === angelegt!.kdf)!.wort
            : `Eigene Werte (${angelegt?.kdfSpeicherMib ?? 0} MiB)`}
        </Bezugswert>
        <Bezugswert beschriftung="Schlüsseldatei" fest>
          {angelegt?.pfad ?? ""}
        </Bezugswert>
      </dl>

      {#if modus === "sicherung"}
        <div class="border-linie bg-flaeche space-y-2 rounded-lg border p-4 text-sm">
          <p class="font-medium">Derselbe Fingerprint wie vorher</p>
          <p class="leading-relaxed">
            Vergleichen Sie ihn mit dem, den Ihre Kontakte von Ihnen kennen.
            Stimmt er nicht, ist das nicht Ihre Sicherung.
          </p>
        </div>
      {:else if modus === "v1"}
        <!-- Der Satz, den sonst niemand sagt. Ohne ihn hält jemand den
             geänderten Fingerprint für einen Angriff -- und liegt damit
             gar nicht so falsch, denn genau so sähe einer aus. -->
        <div class="border-linie bg-flaeche space-y-2 rounded-lg border p-4 text-sm">
          <p class="font-medium">Ihr Fingerprint hat sich geändert</p>
          <p class="leading-relaxed">
            Das ist richtig so: Die übernommene Identität hat einen neuen
            Post-Quantum-Schlüssel bekommen, und der geht in den Fingerprint
            ein. Ihre bisherigen Gegenüber sehen deshalb
            <span class="font-medium">„Geändert“</span> und müssen Sie
            einmalig neu verifizieren.
          </p>
          <p class="leading-relaxed">
            <span class="text-schrift-leise">Was bleibt:</span> Alles, was an
            den alten Schlüssel gerichtet wurde, lässt sich weiterhin öffnen.
            Es ist derselbe private Schlüssel in einer neuen Hülle.
          </p>
        </div>
      {/if}

      {#if modus !== "sicherung"}
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
      {/if}

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
        disabled={(schritt === "passwort" && !schrittFertig) || arbeitet}
        onclick={weiter}
        data-pruefstelle="weiter"
      >
        {#if arbeitet}
          {modus === "sicherung" ? "Sicherung wird geöffnet…" : "Schlüssel wird erzeugt…"}
        {:else if modus === "sicherung" && schritt === "passwort"}
          Sicherung einspielen
        {:else}
          <!-- „Erzeugen“ wäre bei einer Übernahme das falsche Wort: Es
               entsteht kein neuer Schlüssel, sondern derselbe private
               Schlüssel bekommt eine neue Hülle. Wer hier „erzeugen“ liest,
               fürchtet, sein alter sei weg. -->
          {schritt !== "wahl"
            ? "Weiter"
            : modus === "v1"
              ? "Schlüssel übernehmen"
              : "Schlüssel erzeugen"}
        {/if}
      </button>

      {#if identitaetsspeicher.fehler}
        <p
          class="border-fehler text-fehler flex w-full items-start gap-2 rounded-md border px-3 py-2 text-sm"
          role="alert"
        >
          <span aria-hidden="true">✕</span>
          <span>{identitaetsspeicher.fehler}</span>
        </p>
      {/if}

      {#if schritt === "passwort" && modus === "sicherung" && !sicherungFertig}
        <span class="text-schrift-leise text-sm">
          {#if !sicherungBedarf}
            Wählen Sie die Sicherung.
          {:else if sicherungRueckfrage}
            Bestätigen Sie oben, dass sie trotzdem geöffnet werden soll.
          {:else if !sicherungBedarf.passt}
            Für diese Sicherung reicht der Arbeitsspeicher gerade nicht.
          {:else}
            Geben Sie das Passwort der Sicherung ein.
          {/if}
        </span>
      {:else if schritt === "passwort" && modus !== "sicherung" && !passwortFertig}
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
