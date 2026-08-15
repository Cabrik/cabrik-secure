<!--
  Die Hülle des Prototyps.

  Sie ist bewusst eine Fallauswahl und kein Programmfenster: In Phase 3 geht
  es darum, JEDEN Zustand ansehen und beurteilen zu können — auch die, die
  im Alltag selten vorkommen und gerade deshalb schlecht gestaltet werden.
-->
<script lang="ts">
  import { FAELLE, STAPEL } from "./lib/kern/mock";
  import {
    identitaetsspeicher,
    kontaktspeicher,
    sendespeicher,
    sitzungsspeicher,
  } from "./lib/kern/speicher.svelte";
  import { imFenster } from "./lib/kern/tauri";
  import Sperrbildschirm from "./lib/bildschirme/Sperrbildschirm.svelte";
  import Sperrleiste from "./lib/anzeige/Sperrleiste.svelte";

  import Empfangen from "./lib/bildschirme/Empfangen.svelte";
  import Senden from "./lib/bildschirme/Senden.svelte";
  import Kontakte from "./lib/bildschirme/Kontakte.svelte";
  import Identitaet from "./lib/bildschirme/Identitaet.svelte";
  import Onboarding from "./lib/bildschirme/Onboarding.svelte";
  import Werkzeuge from "./lib/bildschirme/Werkzeuge.svelte";
  import { darstellung } from "./lib/anzeige/darstellung.svelte";

  // Einmal beim Start. Im Fenster kommt der Stand aus dem Kern, im Browser
  // aus den Beispieldaten -- die Bildschirme merken den Unterschied nicht.
  void kontaktspeicher.laden();

  /**
   * Takt und Tastaturbeobachtung, solange die Hülle steht.
   *
   * Der Rückgabewert meldet ab. Ohne ihn liefen nach einem Neuaufbau zwei
   * Takte, und in den Tests bliebe einer stehen und feuerte in eine
   * abgebaute Anwendung hinein.
   */
  $effect(() => sitzungsspeicher.beobachten());

  // Ziehen-und-Fallenlassen gilt fürs ganze Fenster, nicht für einen
  // Bildschirm: Wer Dateien hereinzieht, während er beim Empfangen ist,
  // meint trotzdem das Senden.
  $effect(() => sendespeicher.beobachten());

  /**
   * Fallengelassene Dateien führen zum Sendebildschirm.
   *
   * Ohne das verschwinden sie in einen Halter, den gerade niemand ansieht.
   * Von außen sieht das aus, als habe das Fenster sie nicht angenommen —
   * und genau das war der erste Eindruck beim ersten Versuch.
   *
   * Der Zähler und nicht die Liste ist der Auslöser: Wer eine Datei
   * hineinzieht, die schon drin ist, hat trotzdem gerade etwas getan.
   */
  let gesehenerWurf = 0;
  $effect(() => {
    if (sendespeicher.zuletztGefallen === gesehenerWurf) return;
    gesehenerWurf = sendespeicher.zuletztGefallen;
    if (gesehenerWurf === 0) return;
    bereich = "senden";
    stapelKennung = AUSWAHL;
  });

  /**
   * Wenn der Kern entsperrt, sind Kontakte und Identität erst jetzt lesbar.
   *
   * Vorher gibt der Befehl einen Fehler zurück -- der Aufruf beim Start
   * lief also gegen eine gesperrte Sitzung. Ohne dieses Nachladen bliebe
   * das Verzeichnis nach dem Entsperren leer, und der Nutzer sähe „keine
   * Kontakte“, wo drei stehen.
   */
  let warGesperrt = true;
  $effect(() => {
    const gesperrt = sitzungsspeicher.stand?.gesperrt ?? true;
    if (warGesperrt && !gesperrt) {
      void kontaktspeicher.laden();
      void identitaetsspeicher.laden();
    }
    // Beim Sperren vergessen, was offen war. Nicht bloß der Ordnung
    // halber: Sonst stünden Bezeichnung und Fingerprint noch da, während
    // der Schlüssel längst fort ist -- eine Anzeige, die etwas behauptet,
    // das nicht mehr gilt.
    if (!warGesperrt && gesperrt) identitaetsspeicher.vergiss();
    warGesperrt = gesperrt;
  });

  type Bereich =
    | "empfangen"
    | "senden"
    | "kontakte"
    | "identitaet"
    | "onboarding"
    | "werkzeuge";

  let bereich = $state<Bereich>("empfangen");
  let fallKennung = $state(FAELLE[0]!.kennung);
  /** Die Kennung der echten Auswahl — kein Beispielstapel trägt sie. */
  const AUSWAHL = "auswahl";
  let stapelKennung = $state(AUSWAHL);
  /**
   * Welche Identität gerade gezeigt wird — über den Fingerprint, nicht
   * über einen Index: Ein Index zeigte nach dem Löschen auf die falsche.
   */
  let identitaetFp = $state("");
  const identitaet = $derived(
    identitaetsspeicher.liste.find((i) => i.fingerprint === identitaetFp) ??
      identitaetsspeicher.liste[0],
  );

  const fall = $derived(FAELLE.find((f) => f.kennung === fallKennung) ?? FAELLE[0]!);
  /**
   * Was der Sendebildschirm zeigt: die echte Auswahl oder ein Beispiel.
   *
   * **Kein zusammengebautes Objekt mehr.** Vorher entstand hier bei jeder
   * Änderung ein neuer `Stapel`, und der Bildschirm bekam ihn als einen
   * Wert. Das war eine Schicht, die nichts beitrug außer einer weiteren
   * Stelle, an der etwas veralten kann. Jetzt hängt jeder Wert für sich
   * am Halter.
   */
  const beispiel = $derived(STAPEL.find((s) => s.kennung === stapelKennung));
  const sendedateien = $derived(beispiel ? beispiel.dateien : sendespeicher.dateien);
  const istAuswahl = $derived(beispiel === undefined);

  const stapelErklaerung = $derived(
    beispiel
      ? beispiel.worumEsGeht
      : sendespeicher.dateien.length === 0
        ? "Hier landen die Dateien, die Sie verschicken wollen. Beim Auswählen wird jede angesehen und gesagt, was beim Verschlüsseln aus ihren Metadaten wird — verändert wird dabei nichts."
        : "Was hier steht, kommt von Ihrer Platte. Jede Datei wurde angesehen; der Befund ist das, was beim Verschlüsseln tatsächlich geschieht, nicht eine Schätzung davon.",
  );

  const BEREICHE: { kennung: Bereich; name: string }[] = [
    { kennung: "empfangen", name: "Empfangen" },
    { kennung: "senden", name: "Senden" },
    { kennung: "kontakte", name: "Kontakte" },
    { kennung: "identitaet", name: "Identität" },
    { kennung: "werkzeuge", name: "Werkzeuge" },
    { kennung: "onboarding", name: "Einrichtung" },
  ];

  const ERLAEUTERUNG: Record<Exclude<Bereich, "empfangen" | "senden">, string> = {
    kontakte:
      "Beim Empfangen ist Vertrauen eine Anzeige. Hier ist es eine Handlung — " +
      "und deshalb ist „nicht verifiziert“ hier grau statt gelb: Als Eintrag im " +
      "Verzeichnis ist es erwartbar, denn so fängt jeder Kontakt an.",
    identitaet:
      "Der Bildschirm, dessen wichtigste Eigenschaft ist, was er nicht kann: " +
      "Es gibt keinen Knopf, der den privaten Schlüssel zeigt — und der Typ " +
      "dahinter hat gar kein Feld dafür.",
    werkzeuge:
      "Sicheres Löschen ist der Fall, an dem sich Ehrlichkeit entscheidet. " +
      "Version 1 hatte drei Überschreibdurchgänge voreingestellt und " +
      "suggerierte damit einen Nutzen, den es auf heutigen Datenträgern nicht " +
      "gibt.",
    onboarding:
      "Hier steht die unbequemste Entscheidung des Entwurfs: keine " +
      "Passwort-Stärkeanzeige. Ein Programm kann nicht wissen, ob ein Passwort " +
      "gut ist — es kennt die Liste nicht, in der es vielleicht steht.",
  };

  const erlaeuterung = $derived(
    bereich === "empfangen"
      ? fall.worumEsGeht
      : bereich === "senden"
        ? stapelErklaerung
        : ERLAEUTERUNG[bereich],
  );
</script>

<!--
  Gesperrt ist ein eigener Bildschirm, kein Schleier über diesem hier.
  Ein halbdurchsichtiges Fenster darüber ließe Dateinamen, Kontaktnamen und
  Befunde stehen -- und behauptete damit, es sei noch etwas offen.

  `geladen` verhindert das Aufflackern: Vor der ersten Antwort ist `stand`
  noch `null`, und das hieße „keine Identität“. Jemandem, der längst eine
  hat, für einen Augenblick die Einrichtung zu zeigen, ist kein Schönheits-
  fehler, sondern eine Falschaussage.
-->
{#if sitzungsspeicher.geladen && sitzungsspeicher.stand?.gesperrt}
  <Sperrbildschirm />
{:else}
<div class="mx-auto flex min-h-screen max-w-7xl gap-8 p-6">
  <nav class="w-72 shrink-0">
    <div class="flex items-baseline justify-between px-3 pb-4">
      <p class="text-lg font-semibold tracking-tight">
        Cabrik<span class="text-bezug">Secure</span>
      </p>
      <button
        class="border-linie text-schrift-leise hover:text-schrift rounded border px-2 py-1 text-xs"
        onclick={() => darstellung.umschalten()}
        aria-label="Zwischen hellem und dunklem Modus wechseln"
      >
        {darstellung.modus === "dunkel" ? "hell" : "dunkel"}
      </button>
    </div>

    <!--
      Bereiche.

      `flex-wrap` ist hier nicht Kosmetik: Ohne den Umbruch laufen die
      Knöpfe aus der 18rem breiten Spalte heraus und verschwinden hinter
      dem Inhaltsbereich. Mit drei Bereichen fiel das nicht auf, mit sechs
      sofort.
    -->
    <div class="border-linie mb-4 flex flex-wrap gap-1 border-b px-1 pb-3">
      {#each BEREICHE as b (b.kennung)}
        <button
          class="rounded-md px-3 py-1.5 text-sm whitespace-nowrap transition
                 {bereich === b.kennung
            ? 'bg-schrift text-grund font-medium'
            : 'text-schrift-leise hover:bg-flaeche'}"
          onclick={() => (bereich = b.kennung)}
        >
          {b.name}
        </button>
      {/each}
    </div>

    {#if bereich === "empfangen"}
      <p class="text-schrift-leise px-3 pb-2 text-xs font-semibold tracking-wide uppercase">
        Beispielfälle
      </p>
      <div class="space-y-1">
        {#each FAELLE as f (f.kennung)}
          <button
            class="w-full rounded-md px-3 py-2 text-left text-sm transition
                   {fallKennung === f.kennung
              ? 'bg-schrift text-grund'
              : 'text-schrift hover:bg-flaeche'}"
            onclick={() => (fallKennung = f.kennung)}
          >
            {f.titel}
          </button>
        {/each}
      </div>
    {:else if bereich === "identitaet"}
      <p class="text-schrift-leise px-3 pb-2 text-xs font-semibold tracking-wide uppercase">
        {identitaetsspeicher.liste.length === 1 ? "Identität" : "Identitäten"}
      </p>
      <div class="space-y-1">
        {#each identitaetsspeicher.liste as i (i.fingerprint)}
          <button
            class="w-full rounded-md px-3 py-2 text-left text-sm transition
                   {identitaetFp === i.fingerprint
              ? 'bg-schrift text-grund'
              : 'text-schrift hover:bg-flaeche'}"
            onclick={() => (identitaetFp = i.fingerprint)}
          >
            {i.bezeichnung}
          </button>
        {/each}
        {#if identitaetsspeicher.liste.length === 0}
          <p class="text-schrift-leise px-3 text-sm">Keine mehr vorhanden.</p>
        {/if}
      </div>
    {:else if bereich === "senden"}
      <div class="space-y-1 pb-3">
        <button
          class="w-full rounded-md px-3 py-2 text-left text-sm transition
                 {stapelKennung === AUSWAHL
            ? 'bg-schrift text-grund'
            : 'text-schrift hover:bg-flaeche'}"
          onclick={() => (stapelKennung = AUSWAHL)}
        >
          Ausgewählte Dateien
          {#if sendespeicher.dateien.length > 0}
            <span class="text-xs opacity-70">({sendespeicher.dateien.length})</span>
          {/if}
        </button>
      </div>
      <p class="text-schrift-leise px-3 pb-2 text-xs font-semibold tracking-wide uppercase">
        Beispielstapel
      </p>
      <div class="space-y-1">
        {#each STAPEL as s (s.kennung)}
          <button
            class="w-full rounded-md px-3 py-2 text-left text-sm transition
                   {stapelKennung === s.kennung
              ? 'bg-schrift text-grund'
              : 'text-schrift hover:bg-flaeche'}"
            onclick={() => (stapelKennung = s.kennung)}
          >
            {s.titel}
          </button>
        {/each}
      </div>
    {/if}

    <!--
      Was hier steht, muss stimmen. „Prototyp mit Beispieldaten“ im Fenster
      anzuzeigen, wo es tatsächlich über den Kern geht, wäre die Sorte
      kleine Unwahrheit, die man später niemandem mehr erklärt.
    -->
    <p class="text-schrift-leise px-3 pt-6 text-xs leading-relaxed">
      {#if imFenster()}
        Kontakte kommen aus dem Kern. Alles andere ist noch Beispieldaten.
      {:else}
        Prototyp mit Beispieldaten, im Browser. Im Fenster gehen die Kontakte
        bereits über den Kern.
      {/if}
    </p>
    {#if identitaetsspeicher.fehler}
      <!--
        Ein Fehlschlag beim Abrufen der Identität gehört sichtbar dorthin,
        wo man ihn lesen kann. Vorher wurde er verschluckt, und die leere
        Liste sah aus wie „es gibt noch keine“ -- zwei ganz verschiedene
        Lagen mit derselben Anzeige.
      -->
      <p class="border-fehler text-fehler mx-3 mt-3 rounded-md border px-3 py-2 text-xs">
        {identitaetsspeicher.fehler}
      </p>
    {/if}
    {#if kontaktspeicher.fehler}
      <!--
        Ein Fehler aus dem Kern gehört sichtbar dorthin, wo man ihn lesen
        kann — nicht in eine Konsole, die im Fenster gar nicht aufgeht.
      -->
      <p class="border-fehler text-fehler mx-3 mt-3 rounded-md border px-3 py-2 text-xs">
        {kontaktspeicher.fehler}
      </p>
    {/if}

    <Sperrleiste />
  </nav>

  <main class="min-w-0 flex-1 space-y-4">
    {#if sendespeicher.ziehtDrueber}
      <!--
        Die Rückmeldung während des Ziehens. Sie ist kein Schmuck: Ohne
        sie sieht ein Fenster, das annimmt, genauso aus wie eines, das es
        nicht tut — und dann lässt niemand los.
      -->
      <p
        class="border-bezug text-bezug rounded-md border border-dashed px-4 py-3 text-sm"
        role="status"
      >
        Loslassen, um die Dateien anzusehen. Verändert wird dabei nichts.
      </p>
    {/if}
    {#if sendespeicher.versand}
      {@const v = sendespeicher.versand}
      {@const gelungen = v.dateien.filter((d) => d.ziel !== null)}
      {@const misslungen = v.dateien.filter((d) => d.ziel === null)}
      <div class="border-linie bg-flaeche space-y-3 rounded-xl border p-5">
        <div class="flex flex-wrap items-baseline justify-between gap-2">
          <h2 class="text-bestaetigt flex items-center gap-2 text-lg font-semibold">
            <span aria-hidden="true">✓</span>
            {gelungen.length}
            {gelungen.length === 1 ? "Datei" : "Dateien"} verschlüsselt
          </h2>
          <button
            class="border-linie hover:bg-grund rounded-md border px-3 py-1.5 text-sm"
            onclick={() => sendespeicher.versandSchliessen()}
          >
            Schließen
          </button>
        </div>

        <dl class="grid gap-3 text-sm sm:grid-cols-3">
          <div>
            <dt class="text-schrift-leise text-xs">Verfahren</dt>
            <dd class="text-bezug">{v.suite}</dd>
          </div>
          <div>
            <dt class="text-schrift-leise text-xs">Empfänger</dt>
            <dd class="text-bezug">{v.empfaenger.join(", ")}</dd>
          </div>
          <div>
            <dt class="text-schrift-leise text-xs">Signatur</dt>
            <!--
              „nicht signiert“ gehört gesagt, auch wenn es gewollt war:
              Eine Identität ohne Signierschlüssel kann nicht signieren,
              und wer das Häkchen gesetzt hat, glaubte sonst an eine
              Zusicherung, die es nicht gibt.
            -->
            <dd class="text-bezug">
              {v.signiert ? "mit Ihrer Identität signiert" : "nicht signiert"}
            </dd>
          </div>
        </dl>

        {#if gelungen.length > 0}
          <ul class="space-y-1">
            {#each gelungen as d}
              <li class="text-bezug font-mono text-xs break-all">{d.ziel}</li>
            {/each}
          </ul>
        {/if}

        {#if v.vorbehalte.length > 0}
          <div class="border-warnung-rand bg-warnung-grund space-y-1 rounded-md border px-3 py-2">
            {#each v.vorbehalte as vb}
              <p class="text-warnung flex items-start gap-2 text-sm">
                <span aria-hidden="true">!</span>
                <span>{vb}</span>
              </p>
            {/each}
          </div>
        {/if}

        {#if misslungen.length > 0}
          <div class="border-fehler space-y-1 rounded-md border px-3 py-2">
            <p class="text-fehler text-sm font-medium">
              {misslungen.length}
              {misslungen.length === 1 ? "Datei wurde" : "Dateien wurden"} nicht
              verschlüsselt
            </p>
            <ul class="space-y-1 text-xs">
              {#each misslungen as d}
                <li class="break-all">
                  <span class="font-mono">{d.quelle}</span> — {d.fehler}
                </li>
              {/each}
            </ul>
          </div>
        {/if}

        <!--
          Der Satz, der bei v1 fehlte. Die Klartextdatei liegt nach dem
          Verschlüsseln UNVERÄNDERT weiter da. Wer das nicht sagt, lässt
          jemanden glauben, er habe etwas geschützt — dabei liegt daneben
          eine zweite Kopie, und die erste ist offen wie zuvor.
        -->
        <p class="text-schrift-leise text-xs leading-relaxed">
          Die Ausgangsdateien liegen unverändert weiter da. Verschlüsseln hat
          eine zweite Datei danebengelegt, es hat die erste nicht ersetzt.
        </p>
      </div>
    {/if}
    {#if sendespeicher.gespeichert.length > 0}
      {@const gelungen = sendespeicher.gespeichert.filter((e) => e.ziel !== null)}
      {@const misslungen = sendespeicher.gespeichert.filter((e) => e.ziel === null)}
      <!--
        Wohin gespeichert wurde, gehört auf den Bildschirm.

        Eine Erfolgsmeldung ohne Pfad ist keine: Wer sie liest, weiß nicht,
        wo die Datei liegt, und sucht sie im Dateimanager. Und was NICHT
        geklappt hat, steht daneben statt unterzugehen — sonst zählt jemand
        vierzig Dateien ab und findet neununddreißig.
      -->
      <div class="border-linie bg-flaeche space-y-3 rounded-xl border p-5">
        <div class="flex flex-wrap items-baseline justify-between gap-2">
          <h2 class="text-lg font-semibold">
            {gelungen.length}
            {gelungen.length === 1 ? "Datei" : "Dateien"} bereinigt gespeichert
          </h2>
          <button
            class="border-linie hover:bg-grund rounded-md border px-3 py-1.5 text-sm"
            onclick={() => sendespeicher.ergebnisSchliessen()}
          >
            Schließen
          </button>
        </div>

        {#if gelungen.length > 0}
          <ul class="space-y-1 text-sm">
            {#each gelungen as e}
              <li class="text-bezug font-mono text-xs break-all">{e.ziel}</li>
            {/each}
          </ul>
        {/if}

        {#if misslungen.length > 0}
          <div class="border-warnung-rand bg-warnung-grund space-y-1 rounded-md border px-3 py-2">
            <p class="text-warnung flex items-center gap-2 text-sm font-medium">
              <span aria-hidden="true">!</span>
              {misslungen.length}
              {misslungen.length === 1 ? "Datei wurde" : "Dateien wurden"} nicht
              gespeichert
            </p>
            <ul class="space-y-1 text-xs">
              {#each misslungen as e}
                <li class="break-all">
                  <span class="font-mono">{e.quelle}</span> — {e.fehler}
                </li>
              {/each}
            </ul>
          </div>
        {/if}

        <!--
          Der Satz, der sonst nie fällt. Nach dem Speichern liegen ZWEI
          unverschlüsselte Fassungen auf der Platte — eine davon mit allem,
          was drinstand. Wer das verschweigt, lässt jemanden glauben, er
          habe etwas bereinigt.
        -->
        <p class="text-schrift-leise text-xs leading-relaxed">
          Die Ausgangsdateien liegen unverändert weiter da — mit allem, was
          in ihnen stand. Bereinigt wurde die neue Fassung daneben.
        </p>
      </div>
    {/if}
    {#if sendespeicher.fehler}
      <p class="border-fehler text-fehler rounded-md border px-4 py-3 text-sm" role="alert">
        {sendespeicher.fehler}
      </p>
    {/if}
    <p class="border-linie bg-flaeche text-schrift-leise rounded-md border px-4 py-3 text-sm">
      <span class="text-schrift font-medium">Worum es hier geht:</span>
      {erlaeuterung}
    </p>

    <!--
      Eine Fehlergrenze um den Inhalt.

      Bricht ein Bildschirm beim Zeichnen ab, bleibt sonst schlicht der
      alte stehen: Der Klick tut nichts, es erscheint nichts, und von außen
      ist das nicht von einem toten Knopf zu unterscheiden. Genau so sah
      ein gemeldeter Fehler aus.

      Ein Programm, das abstürzt, muss das sagen dürfen. Der Satz darunter
      ist unbequem — er ist trotzdem besser als ein Bildschirm, der so tut,
      als sei nichts gewesen.
    -->
    <svelte:boundary>
    <div class="border-linie bg-flaeche rounded-xl border p-6">
      {#if bereich === "empfangen"}
        <Empfangen {fall} />
      {:else if bereich === "senden"}
        <Senden
          dateien={sendedateien}
          kennung={stapelKennung}
          waehlen={istAuswahl ? () => void sendespeicher.waehlen() : undefined}
          leeren={istAuswahl && sendespeicher.dateien.length > 0
            ? () => sendespeicher.leeren()
            : undefined}
          arbeitet={sendespeicher.arbeitet}
          bereinigtSpeichern={istAuswahl
            ? (pfade) => void sendespeicher.bereinigtSpeichern(pfade)
            : undefined}
          verschluesselnEcht={istAuswahl
            ? (pfade, empfaenger, signieren, original) =>
                void sendespeicher.verschluesseln(
                  pfade,
                  empfaenger,
                  signieren,
                  original,
                )
            : undefined}
        />
      {:else if bereich === "kontakte"}
        <Kontakte />
      {:else if bereich === "identitaet"}
        {#if identitaet}
          <Identitaet
            {identitaet}
            geloescht={() => {
              identitaetFp = identitaetsspeicher.liste[0]?.fingerprint ?? "";
            }}
          />
        {:else}
          <!--
            Ohne Identität lässt sich weder öffnen noch senden. Das ist kein
            Fehler, sondern der Zustand vor der ersten Einrichtung — und der
            Weg dorthin gehört dazugesagt.
          -->
          <div class="space-y-3">
            <h2 class="text-xl font-semibold">Keine Identität vorhanden</h2>
            <p class="text-sm">
              Ohne Schlüssel lässt sich nichts entschlüsseln, was an Sie
              gerichtet ist, und nichts signieren. Legen Sie unter
              <span class="font-medium">Einrichtung</span> eine neue an.
            </p>
            <p class="text-schrift-leise text-sm">
              Eine neue Identität hat einen neuen Fingerprint. Alle Kontakte
              müssen ihn erneut erhalten — und was an den alten Schlüssel
              verschlüsselt wurde, bleibt zu.
            </p>
            <button
              class="bg-schrift text-grund rounded-md px-4 py-2 text-sm font-medium"
              onclick={() => (bereich = "onboarding")}
            >
              Zur Einrichtung
            </button>
          </div>
        {/if}
      {:else if bereich === "werkzeuge"}
        <Werkzeuge />
      {:else}
        <Onboarding
          ansehen={(fp) => {
            identitaetFp = fp;
            bereich = "identitaet";
          }}
        />
      {/if}
    </div>

    {#snippet failed(fehler, nochmal)}
      <div class="border-fehler bg-flaeche space-y-3 rounded-xl border p-6">
        <h2 class="text-fehler flex items-center gap-2 text-lg font-semibold">
          <span aria-hidden="true">✕</span>
          Dieser Bildschirm ließ sich nicht darstellen
        </h2>
        <p class="text-sm leading-relaxed">
          Das ist ein Fehler dieses Programms, nicht Ihrer. An Ihren Dateien
          wurde dabei nichts verändert — dieser Bildschirm zeigt nur an.
        </p>
        <p class="border-linie text-schrift-leise rounded border px-3 py-2 font-mono text-xs break-all">
          {fehler instanceof Error ? fehler.message : String(fehler)}
        </p>
        <button
          class="bg-schrift text-grund rounded-md px-4 py-2 text-sm font-medium"
          onclick={nochmal}
        >
          Noch einmal versuchen
        </button>
      </div>
    {/snippet}
    </svelte:boundary>
  </main>
</div>
{/if}
