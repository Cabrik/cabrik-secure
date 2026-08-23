<!--
  Die Sperre im entsperrten Zustand: Frist einstellen, sofort sperren, und
  die Warnstaffel kurz vor Ablauf (`spec/entsperrung.md` §3 und §9).

  # Warum hier kein dauerhafter Zähler steht

  Weil er die meiste Zeit belanglos ist und trotzdem drängt. Eine Uhr, die
  vierzehn Minuten lang herunterläuft, macht aus konzentrierter Arbeit ein
  Rennen — und beim fünften Mal sieht niemand mehr hin, auch nicht in der
  letzten Minute. Deshalb ist der Normalzustand: nichts.

  # Warum die Warnung gelb ist und nie rot

  Eine bevorstehende Sperre ist kein Vorfall. Sie ist das Programm, das tut,
  was eingestellt wurde. Rot wäre hier verbraucht — und würde beim nächsten
  echten Fehler weniger bedeuten.

  # Warum die Schwellen relativ sind

  „Zehn Minuten vorher“ ginge bei einer Frist von einer Minute nicht auf.
  Die Staffel misst deshalb in Anteilen der eingestellten Zeit; nur der
  Countdown der letzten dreißig Sekunden ist absolut, weil Sekunden am Ende
  Sekunden bleiben.
-->
<script lang="ts">
  import { sitzungsspeicher } from "../kern/speicher.svelte";
  import type { Sperrfrist } from "../kern/typen";
  import { FRIST_TEXT, restzeitText, warnstufe } from "./zustand";

  const stand = $derived(sitzungsspeicher.stand);

  /**
   * Der Hinweis, dass §3.4 auf diesem Rechner **nicht** voll greift.
   *
   * `null` heißt „noch nicht gefragt“ und zeigt nichts an. Der günstige
   * Fall zeigt ebenfalls nichts: Ein Programm, das seine funktionierenden
   * Schutzmaßnahmen aufzählt, erzieht dazu, den Kasten zu überlesen — und
   * dann fällt auch der Fall nicht auf, in dem etwas fehlt.
   */
  const schutzhinweis = $derived.by(() => {
    const s = sitzungsspeicher.ruheschutz;
    if (!s || s.art === "mitAufschub") return null;
    return s.art === "ohneAufschub"
      ? "Vor dem Ruhezustand wird gesperrt, aber Ihr System sagt dafür " +
          "keine Zeit zu. Im ungünstigsten Fall schläft es ein, bevor das " +
          "Überschreiben fertig ist."
      : `Vor dem Ruhezustand wird auf diesem Rechner nicht gesperrt: ${s.grund} ` +
          "Es gilt allein die Frist oben.";
  });
  const stufe = $derived(
    stand ? warnstufe(stand.restsekunden, stand.frist) : "keine",
  );

  let offen = $state(false);

  const FRISTEN: Sperrfrist[] = [
    "eineMinute",
    "fuenfMinuten",
    "fuenfzehnMinuten",
    "dreissigMinuten",
    "eineStunde",
    "bisZumSchliessen",
  ];
</script>

{#if stand && !stand.gesperrt}
  <div class="border-linie mt-6 space-y-2 border-t px-3 pt-4">
    {#if schutzhinweis}
      <!--
        GELB, nicht rot: Hier ist nichts gescheitert. Es ist eine
        Eigenschaft dieses Rechners, und der Nutzer kann sie meist nicht
        ändern — ihn mit Rot zu erschrecken, hieße ihn zu etwas zu drängen,
        das er gar nicht tun kann (`spec/anzeige.md`).

        Er steht ÜBER der Frist, weil er sie einschränkt: Wer liest
        „15 Minuten“ und darunter erst erfährt, dass beim Zuklappen nichts
        geschieht, hat die Zahl schon geglaubt.
      -->
      <p class="text-warnung text-xs leading-relaxed" data-pruef="ruheschutz">
        {schutzhinweis}
      </p>
    {/if}
    {#if stufe !== "keine" && stand.restsekunden !== null}
      <!--
        Drei Stufen, ein Element. Der Unterschied liegt in Größe und
        Ausführlichkeit, nicht in der Farbe: Wer die Abstufung nicht
        wahrnimmt, liest immer noch dieselbe Aussage.
      -->
      <p
        class="border-warnung-rand bg-warnung-grund text-warnung flex items-center gap-2
               rounded-md border px-2 py-1.5
               {stufe === 'countdown' ? 'text-sm font-medium' : 'text-xs'}"
        role={stufe === "countdown" ? "alert" : "status"}
      >
        <span aria-hidden="true">!</span>
        {#if stufe === "leise"}
          <span>Sperrt bald</span>
        {:else}
          <span>Sperrt {restzeitText(stand.restsekunden)}</span>
        {/if}
      </p>
    {/if}

    <div class="flex items-center gap-2">
      <button
        class="border-linie text-schrift-leise hover:text-schrift flex-1 rounded border
               px-2 py-1 text-xs"
        onclick={() => void sitzungsspeicher.sperren()}
      >
        Jetzt sperren
      </button>
      <button
        class="border-linie text-schrift-leise hover:text-schrift rounded border px-2 py-1 text-xs"
        aria-expanded={offen}
        onclick={() => (offen = !offen)}
      >
        Frist
      </button>
    </div>

    <!--
      Die eingestellte Frist ist ein Sollwert: etwas, das der Nutzer
      verlangt hat, kein Wert, den das Programm gelesen hat. Deshalb
      Magenta und nicht Cyan (`spec/anzeige.md` §3a).
    -->
    <p
      class="text-sollwert border-sollwert/40 inline-flex items-center gap-1.5 rounded
             border px-2 py-0.5 text-xs"
    >
      <span aria-hidden="true">◆</span>
      {FRIST_TEXT[stand.frist]}
    </p>

    {#if offen}
      <div class="space-y-1 pt-1">
        {#each FRISTEN as f (f)}
          <button
            class="w-full rounded px-2 py-1 text-left text-xs transition
                   {stand.frist === f
              ? 'text-sollwert border-sollwert/40 border'
              : 'text-schrift-leise hover:bg-flaeche border border-transparent'}"
            onclick={async () => {
              await sitzungsspeicher.fristSetzen(f);
              offen = false;
            }}
          >
            {FRIST_TEXT[f]}
          </button>
        {/each}
        <!--
          Die letzte Wahl braucht den Satz dazu. „Bis das Fenster
          geschlossen wird“ klingt harmlos und heißt: Wer den Rechner
          stehen lässt, lässt den Schlüssel offen liegen.
        -->
        <p class="text-schrift-leise px-2 pt-1 text-xs leading-relaxed">
          Ohne Frist bleibt der Schlüssel offen, solange das Fenster steht —
          auch wenn Sie den Raum verlassen.
        </p>
      </div>
    {/if}
  </div>
{/if}
