<!--
  Der Bildschirm, wenn Cabrik gar nicht erst starten konnte.

  # Warum es ihn gibt

  Weil das Fenster unter Windows mit `windows_subsystem = "windows"` läuft
  und deshalb KEINE KONSOLE hat. Ein `eprintln!` beim Start schrieb dort auf
  einen Ausgang, den es nicht gibt: Wer Cabrik doppelklickte und dessen
  Schlüsseldatei beschädigt war, sah gar nichts. Kein Fenster, keine
  Meldung, nur einen Prozess, der sofort wieder verschwand.

  Version 1 stürzte in dieser Lage mit einem Traceback ab. Das war schlecht,
  aber sichtbar. Stillschweigend nicht zu starten ist schlechter.

  # Warum kein Meldungsfenster

  Weil ein Meldungsfenster eine Sackgasse ist: „Fehler“, „OK“, weg. Hier
  steht der PFAD — bei einer Schlüsseldatei ist die Auskunft, welche Datei
  im Weg liegt, der Unterschied zwischen einem Rätsel und einer Aufgabe —
  und ein SCHRITT, kein Trost.

  # Warum rot und nicht grau

  Das ist der eine Fall, in dem etwas tatsächlich nicht stimmt. Grau hieße
  „ich weiß es nicht“; hier wissen wir es genau (`spec/anzeige.md` §3).
-->
<script lang="ts">
  import type { Startfehler } from "../kern/typen";

  interface Props {
    fehler: Startfehler;
  }
  let { fehler }: Props = $props();

  let kopiert = $state(false);

  function pfadKopieren() {
    if (!fehler.pfad) return;
    void navigator.clipboard?.writeText(fehler.pfad).then(() => {
      kopiert = true;
    });
  }
</script>

<div class="flex min-h-screen items-center justify-center p-6">
  <div class="w-full max-w-lg space-y-6">
    <div class="space-y-1">
      <p class="text-lg font-semibold tracking-tight">
        Cabrik<span class="text-bezug">Secure</span>
      </p>
      <!-- Zeichen und Wort, nicht nur die Farbe — wie überall. -->
      <p class="text-fehler flex items-center gap-2 text-sm">
        <span aria-hidden="true">✕</span>
        <span>Start nicht möglich</span>
      </p>
    </div>

    <div class="border-fehler bg-flaeche space-y-4 rounded-xl border p-6">
      <h1 class="text-xl font-semibold">Cabrik konnte nicht starten</h1>

      <p class="text-sm leading-relaxed" role="alert">{fehler.meldung}</p>

      {#if fehler.pfad}
        <!--
          Der Pfad in fester Breite und zum Kopieren. Ihn abzutippen ist bei
          einem Windows-Anwendungsdatenpfad eine Zumutung — und wer sich
          vertippt, sucht an der falschen Stelle.
        -->
        <div class="border-linie bg-grund space-y-2 rounded-md border p-3">
          <p class="text-schrift-leise text-xs">Betroffene Datei</p>
          <p class="text-bezug font-mono text-xs break-all">{fehler.pfad}</p>
          <button
            class="border-linie text-schrift-leise hover:text-schrift rounded-md
                   border px-3 py-1.5 text-xs"
            onclick={pfadKopieren}
          >
            {kopiert ? "Pfad kopiert" : "Pfad kopieren"}
          </button>
        </div>
      {/if}

      <div class="space-y-2">
        <p class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">
          Was Sie tun können
        </p>
        <p class="text-sm leading-relaxed">{fehler.rat}</p>
      </div>
    </div>

    <!--
      Der Satz, der die eigentliche Angst nimmt. Wer diesen Bildschirm sieht,
      denkt zuerst, seine Daten seien fort — und in diesem Zustand macht man
      Dinge, die es dann tatsächlich sind.
    -->
    <p class="text-schrift-leise text-center text-xs leading-relaxed">
      Verschlüsselte Dateien sind hiervon nicht betroffen. Sie liegen, wo sie
      lagen, und lassen sich mit derselben Schlüsseldatei und demselben
      Passwort weiter öffnen — auch von einer anderen Installation.
    </p>
  </div>
</div>
