<!--
  Der Bildschirm „Identität“.

  DIE WICHTIGSTE EIGENSCHAFT DIESES BILDSCHIRMS IST, WAS ER NICHT KANN.
  Es gibt keinen Knopf, der den privaten Schlüssel anzeigt, exportiert oder
  in die Zwischenablage legt — und der Typ `Identitaet` hat gar kein Feld
  dafür. Was nicht existiert, kann nicht versehentlich angezeigt werden
  (`spec/anzeige.md` §6).

  DREI SACHVERHALTE, DIE MAN AN KEINER ANDEREN STELLE ERFÄHRT:

  1. Die Bezeichnung ist nur lokal. Wer die Austausch-Nutzlast bekommt,
     vergibt den Namen selbst. Wer das nicht weiß, hält den Namen für etwas
     Zugesichertes — und genau darauf beruht die Hälfte aller
     Identitätstäuschungen.
  2. Ohne Signierschlüssel lässt sich nichts zuordnen, auch nichts Eigenes.
     Das ist ein legitimer Modus und wird deshalb neutral gezeigt, nicht
     als Warnung — dieselbe Regel wie bei `unsigniert` in §4.2.
  3. Für das Passwort gibt es keine Wiederherstellung. Nicht „schwierig“,
     sondern keine. Das gehört hierhin und nicht ins Kleingedruckte.
-->
<script lang="ts">
  import type { Identitaet, KdfStufe } from "../kern/typen";
  import Zustandsmarke from "../anzeige/Zustandsmarke.svelte";
  import Bezugswert from "../anzeige/Bezugswert.svelte";
  import { identitaetsspeicher } from "../kern/speicher.svelte";

  interface Props {
    identitaet: Identitaet;
    /** Wird nach dem Löschen gerufen, damit die Auswahl nachziehen kann. */
    geloescht?: () => void;
  }
  let { identitaet, geloescht }: Props = $props();

  /**
   * Für welche Identität die Löschabfrage offen ist — und was abgetippt wurde.
   *
   * Ein Häkchen wäre hier zu billig. Das Löschen ist der einzige Vorgang im
   * Programm, der Daten dauerhaft unlesbar macht; wer die Bezeichnung
   * abschreiben muss, hat sie zumindest gelesen.
   */
  let loeschFragtFuer = $state<string | null>(null);
  const loeschFragt = $derived(loeschFragtFuer === identitaet.fingerprint);

  let abschrift = $state("");
  const abschriftStimmt = $derived(abschrift.trim() === identitaet.bezeichnung);

  function loeschabfrage() {
    abschrift = "";
    loeschFragtFuer = identitaet.fingerprint;
  }

  function loeschen() {
    if (!abschriftStimmt) return;
    identitaetsspeicher.loeschen(identitaet.fingerprint);
    loeschFragtFuer = null;
    geloescht?.();
  }

  /**
   * Was die Stufen kosten — gemessen, nicht geschätzt.
   *
   * Die Zahlen stammen aus der Arbeit am Kern (Release-Bau). Eine
   * Oberfläche, die „stark“ schreibt, ohne den Preis zu nennen, lässt den
   * Nutzer eine Entscheidung treffen, deren Folgen er erst beim Entsperren
   * merkt.
   */
  const KDF_TEXT: Record<KdfStufe, { wort: string; satz: string }> = {
    min: {
      wort: "Minimum (64 MiB)",
      satz:
        "Die Untergrenze der Spezifikation. Nur für schwache Geräte gedacht — " +
        "sie macht das Durchprobieren von Passwörtern billiger.",
    },
    empfohlen: {
      wort: "Empfohlen (256 MiB)",
      satz:
        "Rund eine halbe Sekunde je Entsperrung auf einem üblichen Rechner. " +
        "Spürbar, aber erträglich.",
    },
    stark: {
      wort: "Stark (1 GiB)",
      satz:
        "Deutlich langsamer — auch bei jedem eigenen Entsperren, nicht nur " +
        "für einen Angreifer.",
    },
  };

  const gruppen = $derived(identitaet.fingerprint.trim().split(/\s+/));

  function datum(u: number): string {
    return new Date(u * 1000).toLocaleDateString("de-DE", {
      year: "numeric",
      month: "long",
      day: "numeric",
    });
  }

  let weitergeben = $state(false);
</script>

<article class="space-y-6">
  <header class="flex flex-wrap items-baseline justify-between gap-2">
    <div>
      <h2 class="text-xl font-semibold">{identitaet.bezeichnung}</h2>
      <p class="text-schrift-leise mt-0.5 text-sm">
        Erzeugt am {datum(identitaet.erzeugtAm)}
      </p>
    </div>
    <button
      class="bg-schrift text-grund rounded-md px-4 py-2 text-sm font-medium"
      onclick={() => (weitergeben = !weitergeben)}
    >
      {weitergeben ? "Schließen" : "Weitergeben"}
    </button>
  </header>

  <!--
    Der Satz, den man nirgendwo sonst erfährt. Er steht ganz oben, weil die
    Bezeichnung direkt darüber steht und ohne diesen Hinweis wie eine
    Zusicherung aussieht.
  -->
  <p class="border-linie text-schrift-leise rounded-lg border border-dashed px-4 py-3 text-sm">
    Diese Bezeichnung bleibt bei Ihnen. Wer Ihre Austausch-Nutzlast aufnimmt,
    vergibt den Namen selbst — <span class="text-schrift">ein Name ist nie
    eine Zusicherung</span>, weder Ihrer noch der von anderen. Nachprüfbar
    ist allein der Fingerprint darunter.
  </p>

  <!-- ===================================================================
       Der Fingerprint
       =================================================================== -->
  <section class="space-y-2">
    <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
      Fingerprint
    </h3>
    <div class="border-linie bg-flaeche rounded-lg border p-4">
      <div class="grid grid-cols-3 gap-x-6 gap-y-2 sm:grid-cols-5">
        {#each gruppen as gruppe, i (i)}
          <span class="text-bezug font-mono text-lg tracking-wider">{gruppe}</span>
        {/each}
      </div>
    </div>
    <p class="text-schrift-leise text-xs">
      Cyan, weil es ein gelesener Wert ist und kein Urteil. Er ist öffentlich —
      Sie dürfen ihn überall hinschreiben.
    </p>
  </section>

  <!-- ===================================================================
       Was diese Identität kann
       =================================================================== -->
  <section class="space-y-3">
    <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
      Eigenschaften
    </h3>

    <dl class="border-linie bg-flaeche grid gap-4 rounded-lg border p-4 sm:grid-cols-2">
      <Bezugswert beschriftung="Verschlüsselung">
        {identitaet.hatPostQuantum
          ? "Post-Quantum-Hybrid (X-Wing)"
          : "nur klassisch (X25519)"}
      </Bezugswert>
      <Bezugswert beschriftung="Passwortableitung">
        {KDF_TEXT[identitaet.kdf].wort}
      </Bezugswert>
      <Bezugswert beschriftung="Schlüsseldatei" fest>{identitaet.pfad}</Bezugswert>
    </dl>

    <p class="text-schrift-leise text-sm">{KDF_TEXT[identitaet.kdf].satz}</p>

    {#if !identitaet.hatPostQuantum}
      <Zustandsmarke
        marke={{
          zustand: "warnung",
          wort: "Kein Post-Quantum-Schlüssel",
          satz:
            "Diese Identität stammt aus Version 1. An Sie gerichtete Nachrichten " +
            "sind gegen einen künftigen Quantenrechner nicht geschützt. Eine neue " +
            "Identität behebt das — die alte bleibt daneben bestehen, damit ältere " +
            "Nachrichten weiter zu öffnen sind.",
        }}
      />
    {/if}

    <!--
      Ohne Signierschlüssel: neutral, nicht gelb. Dieselbe Regel wie bei
      `unsigniert` — anonymer Versand ist für manche Nutzer der wichtigste
      Modus überhaupt, und ihn zu warnen träfe ausgerechnet die, die ihn
      brauchen.
    -->
    {#if !identitaet.hatSignierschluessel}
      <Zustandsmarke
        marke={{
          zustand: "keineAussage",
          wort: "Ohne Signierschlüssel",
          satz:
            "Ihre Nachrichten sind niemandem zuzuordnen — auch Ihnen nicht. " +
            "Empfänger sehen „nicht signiert“. Das ist ein gewählter Modus, " +
            "kein Mangel.",
        }}
      />
    {/if}
  </section>

  <!-- ===================================================================
       Weitergeben
       =================================================================== -->
  {#if weitergeben}
    <section class="border-linie bg-flaeche space-y-3 rounded-lg border p-4">
      <h3 class="font-medium">Austausch-Nutzlast</h3>
      <p class="text-sm">
        Damit jemand Ihnen schreiben kann, braucht er Ihre öffentlichen
        Schlüssel. Die Nutzlast enthält
        <span class="text-schrift font-medium">ausschließlich öffentliche
        Angaben</span> — Sie dürfen sie per Mail, Messenger oder Aushang
        weitergeben.
      </p>
      <p class="text-schrift-leise text-sm">
        Der Weg, auf dem sie ankommt, entscheidet allerdings nichts über
        Echtheit. Wer sie erhält, sollte den Fingerprint über einen zweiten
        Weg abgleichen — sonst hat er nur die Zusicherung desselben Kanals,
        über den ein Angreifer sie ausgetauscht hätte.
      </p>
      <div class="flex flex-wrap gap-2 pt-1">
        <button class="border-linie hover:bg-grund rounded-md border px-4 py-2 text-sm">
          Als Datei speichern
        </button>
        <button class="border-linie hover:bg-grund rounded-md border px-4 py-2 text-sm">
          Als QR-Code zeigen
        </button>
      </div>
    </section>
  {/if}

  <!-- ===================================================================
       Sicherung und Passwort
       =================================================================== -->
  <section class="border-linie space-y-3 border-t pt-5">
    <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
      Sicherung
    </h3>

    <!--
      Der unbequemste Satz des Programms, und er gehört an die sichtbarste
      Stelle. „Schwierig wiederherzustellen“ wäre eine Lüge: Es gibt keinen
      Weg, und zwar auch für uns nicht.
    -->
    <Zustandsmarke
      marke={{
        zustand: "keineAussage",
        wort: "Es gibt keine Wiederherstellung",
        satz:
          "Ihr Passwort ist der einzige Weg zu dieser Identität. Es ist " +
          "nirgends hinterlegt, auch nicht bei uns. Ist es weg, sind alle an " +
          "Sie gerichteten Nachrichten dauerhaft unlesbar — das ist keine " +
          "Härte des Verfahrens, sondern sein Zweck.",
      }}
      gross
    />

    <p class="text-sm">
      Sichern Sie deshalb <span class="font-medium">beides getrennt</span>: die
      Schlüsseldatei an einen zweiten Ort, das Passwort dorthin, wo Sie es
      wiederfinden. Die Datei allein nützt niemandem etwas — sie ist mit Ihrem
      Passwort verschlüsselt.
    </p>

    <div class="flex flex-wrap gap-2">
      <button class="border-linie hover:bg-grund rounded-md border px-4 py-2 text-sm">
        Schlüsseldatei sichern
      </button>
      <button class="border-linie hover:bg-grund rounded-md border px-4 py-2 text-sm">
        Passwort ändern
      </button>
    </div>
    <p class="text-schrift-leise text-xs">
      Das Ändern des Passworts erzeugt keine neue Identität: Ihr Fingerprint
      bleibt derselbe, und alle Kontakte behalten ihre Verifikation.
    </p>
  </section>

  <!-- ===================================================================
       Löschen — der folgenschwerste Vorgang des Programms
       =================================================================== -->
  <section class="border-fehler/40 space-y-3 rounded-lg border border-dashed p-4">
    <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
      Identität löschen
    </h3>

    {#if !loeschFragt}
      <p class="text-schrift-leise text-sm">
        Entfernt den Schlüssel dauerhaft von diesem Rechner.
      </p>
      <button
        class="border-fehler text-fehler rounded-md border px-4 py-2 text-sm"
        onclick={loeschabfrage}
      >
        Identität löschen
      </button>
    {:else}
      <!--
        Die härteste Aussage des Programms, und sie steht als Fehler, nicht
        als Warnung: Es ist keine Lage, die man abwägen könnte, sondern
        eine, die sich nicht rückgängig machen lässt.
      -->
      <Zustandsmarke
        marke={{
          zustand: "fehler",
          wort: "Danach ist alles dauerhaft unlesbar",
          satz:
            "Jede Nachricht, die je an diesen Fingerprint verschlüsselt wurde, " +
            "lässt sich nie wieder öffnen — auch nicht von uns, auch nicht mit " +
            "Ihrem Passwort. Es gibt keine Sicherung beim Hersteller und keinen " +
            "Wiederherstellungsschlüssel.",
        }}
        gross
      />

      <div class="border-linie bg-flaeche space-y-2 rounded-lg border p-3 text-sm">
        <p class="font-medium">Was Sie vorher bedenken sollten</p>
        <p>
          Ihre Kontakte haben Ihren öffentlichen Schlüssel und
          <span class="text-schrift">verschlüsseln weiter an ihn</span>. Solche
          Nachrichten kommen an und lassen sich nicht mehr öffnen. Sagen Sie
          Bescheid, bevor Sie löschen.
        </p>
        <p class="text-schrift-leise">
          Wollen Sie nur zu einem neuen Schlüssel wechseln, löschen Sie diesen
          <span class="text-schrift">nicht</span>: Legen Sie eine zweite
          Identität an und lassen Sie die alte stehen, damit ältere Nachrichten
          lesbar bleiben. Genau dafür ist mehr als eine Identität vorgesehen.
        </p>
      </div>

      <!--
        Abschreiben statt Häkchen. Ein Häkchen erzieht zum Wegklicken, und
        das ist der eine Vorgang, bei dem Wegklicken nicht passieren darf.
      -->
      <label class="block">
        <span class="mb-1 block text-sm">
          Tippen Sie zur Bestätigung
          <span class="text-bezug font-mono">{identitaet.bezeichnung}</span>
        </span>
        <input
          class="border-linie bg-grund w-full rounded-md border px-3 py-2"
          bind:value={abschrift}
          placeholder={identitaet.bezeichnung}
        />
      </label>

      <div class="flex flex-wrap items-center gap-3">
        <button
          class="border-fehler text-fehler rounded-md border px-4 py-2 text-sm font-medium
                 disabled:cursor-not-allowed disabled:opacity-40"
          disabled={!abschriftStimmt}
          onclick={loeschen}
          data-pruefstelle="identitaet-loeschen"
        >
          Endgültig löschen
        </button>
        <button
          class="border-linie hover:bg-flaeche rounded-md border px-4 py-2 text-sm"
          onclick={() => (loeschFragtFuer = null)}
        >
          Abbrechen
        </button>
      </div>
    {/if}
  </section>
</article>
