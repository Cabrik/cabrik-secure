<!--
  Der Bildschirm „Empfangen".

  Hier treffen alle vier Zustände zusammen — Absender und Metadaten
  unabhängig voneinander. Deshalb wurde er zuerst gebaut: Wer ihn ehrlich
  hinbekommt, hat die schwierigste Anzeige des Programms hinter sich.

  DIE ANORDNUNG IST EINE AUSSAGE. Ganz oben steht, was jemand nach dem
  Öffnen als Erstes wissen will: Von wem? Und: Was verrät die Datei? Der
  Inhalt kommt darunter. Umgekehrt gebaut läse niemand die Einschätzung.
-->
<script lang="ts">
  import type { Fall } from "../kern/mock";
  import { groesse, markeFuerAbsender, markeFuerBereinigung } from "../anzeige/zustand";
  import Zustandsmarke from "../anzeige/Zustandsmarke.svelte";
  import Fundliste from "../anzeige/Fundliste.svelte";
  import Bezugswert from "../anzeige/Bezugswert.svelte";
  import Sollwert from "../anzeige/Sollwert.svelte";

  interface Props { fall: Fall }
  let { fall }: Props = $props();

  const d = $derived(fall.daten);
  const absender = $derived(markeFuerAbsender(d.absender, fall.signaturVerlangt));
  const metadaten = $derived(d.metadaten ? markeFuerBereinigung(d.metadaten) : null);

  const zeitpunkt = $derived(
    d.zeitpunkt
      ? new Date(d.zeitpunkt * 1000).toLocaleString("de-DE", {
          dateStyle: "long",
          timeStyle: "short",
        })
      : null,
  );

  const fingerprint = $derived(
    "fingerprint" in d.absender ? d.absender.fingerprint : null,
  );
  const format = $derived(
    d.metadaten && "format" in d.metadaten ? d.metadaten.format : null,
  );
</script>

<article class="space-y-5">
  <header>
    <h2 class="text-xl font-semibold">{d.dateiname ?? "Textnachricht"}</h2>
  </header>

  <!--
    Zwei Einschätzungen nebeneinander, nicht zu einer verrechnet. Ein
    verifizierter Absender macht eine unbereinigte Datei nicht sauber, und
    umgekehrt. Sie zu einem Gesamturteil zusammenzuziehen wäre bequem und
    falsch.
  -->
  <section class="grid gap-3 md:grid-cols-2">
    <div class="space-y-2">
      <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">Absender</h3>
      <Zustandsmarke marke={absender} gross />
      {#if fall.signaturVerlangt}
        <Sollwert>Sie haben eine Signatur verlangt</Sollwert>
      {/if}
    </div>

    <div class="space-y-2">
      <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">Metadaten</h3>
      {#if metadaten}
        <Zustandsmarke marke={metadaten} gross />
      {:else}
        <div class="border-linie bg-flaeche text-schrift-leise rounded-lg border p-4 text-sm">
          Eine Textnachricht trägt keine Dateimetadaten.
        </div>
      {/if}
    </div>
  </section>

  <!--
    Die Bezugswerte: was das Programm gelesen hat. Cyan, weil es Tatsachen
    sind und keine Beurteilung — dieselbe Trennung wie im Cockpit.
  -->
  <section class="border-linie bg-flaeche rounded-lg border p-4">
    <dl class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
      <Bezugswert beschriftung="Größe">{groesse(d.groesseBytes)}</Bezugswert>
      {#if format}
        <Bezugswert beschriftung="Format">{format}</Bezugswert>
      {/if}
      {#if zeitpunkt}
        <Bezugswert beschriftung="Gesendet">{zeitpunkt}</Bezugswert>
      {/if}
      {#if fingerprint}
        <Bezugswert beschriftung="Fingerprint" fest>{fingerprint}</Bezugswert>
      {/if}
    </dl>
  </section>

  {#if d.metadaten}
    <section class="space-y-2">
      {#if d.metadaten.fall === "vollstaendig"}
        <Fundliste funde={d.metadaten.entfernt} ueberschrift="Entfernt" />
      {:else if d.metadaten.fall === "teilweise"}
        <!-- Geblieben zuerst und aufgeklappt: Das ist die Nachricht. -->
        <Fundliste funde={d.metadaten.geblieben} ueberschrift="Geblieben" offen />
        <Fundliste funde={d.metadaten.entfernt} ueberschrift="Entfernt" />
      {/if}
    </section>
  {/if}

  <section class="space-y-2">
    <h3 class="text-schrift-leise text-xs font-semibold tracking-wide uppercase">Inhalt</h3>
    {#if d.art === "text"}
      <p class="border-linie bg-flaeche rounded-lg border p-4 whitespace-pre-wrap">{d.text}</p>
    {:else}
      <div class="border-linie bg-flaeche flex flex-wrap items-center gap-3 rounded-lg border p-4">
        <span class="text-schrift-leise text-sm">Die Datei ist entschlüsselt und liegt bereit.</span>
        <button
          class="bg-schrift text-grund ml-auto rounded-md px-4 py-2 text-sm font-medium hover:opacity-85"
        >
          Speichern unter …
        </button>
      </div>
    {/if}
  </section>
</article>
