<!--
  Der Sperrbildschirm (`spec/entsperrung.md` §4).

  # Warum das ein Bildschirm ist und kein Fenster darüber

  Ein Dialog liegt über etwas. Damit sagt er: „Dahinter geht es weiter“ —
  und die Dateinamen, Kontaktnamen und Befunde stehen weiter da, nur
  ausgegraut. Genau das darf hier nicht sein. Gesperrt heißt: Es ist nichts
  offen. Was der Bildschirm zeigt, muss dem entsprechen.

  # Warum er grau ist und nicht rot

  „Gesperrt“ ist **keineAussage**, nicht `fehler` (`spec/anzeige.md`). Es
  ist der Normalzustand eines Programms, das seine Arbeit getan hat — kein
  Vorfall. Rot wird es erst, wenn ein Passwort nicht passt, und dann auch
  nur diese eine Zeile.

  # Was hier absichtlich fehlt

  - **Wessen Rechner das ist.** Kein Name, keine Bezeichnung, kein
    Fingerprint. Wer auf einen fremden gesperrten Bildschirm sieht, soll
    daraus nichts erfahren.
  - **Ein Versuchszähler.** „3. Versuch“ hilft niemandem, der sein Passwort
    weiß, und ist ein Geschenk an jeden, der es nicht weiß.
  - **Ein Hinweis, wie falsch das Passwort war.** Es gibt genau eine
    Meldung, und sie kommt wörtlich aus dem Kern.
-->
<script lang="ts">
  import { sitzungsspeicher } from "../kern/speicher.svelte";

  /**
   * Das Eingabefeld — der einzige Ort, an dem das Passwort steht.
   *
   * Es wird nach dem Aufruf sofort geleert, **auch bei Fehlschlag**
   * (`spec/entsperrung.md` §5.1). Ein stehengebliebenes Feld ist ein
   * Passwort im Speicher der Webansicht, unabhängig vom Ergebnis.
   *
   * Was das nicht heilt, steht in §5.2: Der Weg von der Tastatur bis
   * hierher führt durch das Betriebssystem und die Webansicht, und keine
   * dieser Stationen können wir überschreiben. Deshalb kommt in Phase 5
   * ein natives Fenster an diese Stelle.
   */
  let passwort = $state("");
  let sichtbar = $state(false);

  const arbeitet = $derived(sitzungsspeicher.arbeitet);

  async function entsperren(e: SubmitEvent) {
    e.preventDefault();
    if (!passwort || arbeitet) return;
    const versuch = passwort;
    // Erst leeren, dann senden: Zwischen beiden liegt bei Argon2 rund eine
    // halbe Sekunde, und in der soll nichts im Feld stehen.
    passwort = "";
    sichtbar = false;
    await sitzungsspeicher.entsperren(versuch);
  }
</script>

<div class="flex min-h-screen items-center justify-center p-6">
  <div class="w-full max-w-md space-y-6">
    <div class="space-y-1">
      <p class="text-lg font-semibold tracking-tight">
        Cabrik<span class="text-bezug">Secure</span>
      </p>
      <!--
        Zeichen und Wort, nicht nur die Farbe. Das ist die Grundregel des
        Anzeigevertrags und gilt hier wie überall.
      -->
      <p class="text-schrift-leise flex items-center gap-2 text-sm">
        <span aria-hidden="true">?</span>
        <span>Gesperrt</span>
      </p>
    </div>

    <div class="border-linie bg-flaeche space-y-4 rounded-xl border p-6">
      <div class="space-y-2">
        <h1 class="text-xl font-semibold">Passwort eingeben</h1>
        <p class="text-schrift-leise text-sm leading-relaxed">
          Ihr Schlüssel liegt verschlüsselt auf diesem Rechner. Ohne das
          Passwort lässt er sich nicht öffnen — auch nicht von uns.
        </p>
      </div>

      <form class="space-y-3" onsubmit={entsperren}>
        <div class="flex gap-2">
          <!--
            `autocomplete="off"` und kein `name`: Es gibt hier nichts, was
            ein Passwortspeicher sinnvoll merken könnte, und ein Vorschlag
            aus dem Browser wäre eine Kopie mehr.
          -->
          {#if sichtbar}
            <input
              type="text"
              bind:value={passwort}
              disabled={arbeitet}
              autocomplete="off"
              spellcheck="false"
              placeholder="Passwort"
              aria-label="Passwort"
              class="border-linie bg-grund focus:border-bezug min-w-0 flex-1 rounded-md
                     border px-3 py-2 font-mono text-sm outline-none"
            />
          {:else}
            <input
              type="password"
              bind:value={passwort}
              disabled={arbeitet}
              autocomplete="off"
              placeholder="Passwort"
              aria-label="Passwort"
              class="border-linie bg-grund focus:border-bezug min-w-0 flex-1 rounded-md
                     border px-3 py-2 font-mono text-sm outline-none"
            />
          {/if}
          <!--
            Anzeigen ist eine bewusste Handlung und kein Verstoß: Wer allein
            im Raum sitzt und sich vertippt hat, braucht sie. Sie fällt beim
            Absenden von selbst zurück.
          -->
          <button
            type="button"
            class="border-linie text-schrift-leise hover:text-schrift shrink-0 rounded-md
                   border px-3 py-2 text-xs"
            onclick={() => (sichtbar = !sichtbar)}
          >
            {sichtbar ? "verbergen" : "anzeigen"}
          </button>
        </div>

        <button
          type="submit"
          disabled={!passwort || arbeitet}
          class="bg-schrift text-grund w-full rounded-md px-4 py-2 text-sm font-medium
                 disabled:cursor-not-allowed disabled:opacity-40"
        >
          {arbeitet ? "Schlüssel wird abgeleitet…" : "Entsperren"}
        </button>
      </form>

      {#if arbeitet}
        <!--
          Argon2 braucht auf Absicht Zeit. Ohne diesen Satz wirkt die
          Verzögerung wie ein Hänger, und jemand drückt ein zweites Mal.
        -->
        <p class="text-schrift-leise text-xs leading-relaxed">
          Das dauert einen Moment. Die Ableitung ist absichtlich langsam —
          das ist es, was ein geratenes Passwort teuer macht.
        </p>
      {/if}

      {#if sitzungsspeicher.fehler}
        <!--
          Die einzige rote Stelle. Der Satz kommt aus dem Kern und sagt
          nicht, wie falsch das Passwort war (§4.3).
        -->
        <p
          class="border-fehler text-fehler flex items-start gap-2 rounded-md border px-3 py-2 text-sm"
          role="alert"
        >
          <span aria-hidden="true">✕</span>
          <span>{sitzungsspeicher.fehler}</span>
        </p>
      {/if}
    </div>

    <!--
      Derselbe Satz wie bei der Einrichtung, wörtlich. Wer ihn dort gelesen
      und für Beiwerk gehalten hat, liest ihn hier in dem Augenblick, in dem
      er zählt.
    -->
    <p class="text-schrift-leise text-center text-xs leading-relaxed">
      Wenn dieses Passwort weg ist, ist alles weg. Es gibt keine Sicherung
      beim Hersteller und keinen Weg zurück.
    </p>
  </div>
</div>
