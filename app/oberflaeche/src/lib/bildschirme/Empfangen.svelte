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
</script>

<article class="space-y-5">
  <header class="flex flex-wrap items-baseline justify-between gap-2">
    <h2 class="text-xl font-semibold">
      {d.dateiname ?? "Textnachricht"}
    </h2>
    <p class="text-sm text-slate-500">
      {groesse(d.groesseBytes)}{#if zeitpunkt} · {zeitpunkt}{/if}
    </p>
  </header>

  <!--
    Zwei Einschätzungen nebeneinander, nicht zu einer verrechnet. Ein
    verifizierter Absender macht eine unbereinigte Datei nicht sauber, und
    umgekehrt. Sie zu einem Gesamturteil zusammenzuziehen wäre bequem und
    falsch.
  -->
  <section class="grid gap-3 md:grid-cols-2">
    <div class="space-y-2">
      <h3 class="text-xs font-semibold tracking-wide text-slate-500 uppercase">Absender</h3>
      <Zustandsmarke marke={absender} gross />
    </div>

    <div class="space-y-2">
      <h3 class="text-xs font-semibold tracking-wide text-slate-500 uppercase">Metadaten</h3>
      {#if metadaten}
        <Zustandsmarke marke={metadaten} gross />
      {:else}
        <div class="rounded-lg border border-slate-200 bg-white p-4 text-sm text-slate-500">
          Eine Textnachricht trägt keine Dateimetadaten.
        </div>
      {/if}
    </div>
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
    <h3 class="text-xs font-semibold tracking-wide text-slate-500 uppercase">Inhalt</h3>
    {#if d.art === "text"}
      <p class="rounded-lg border border-slate-200 bg-white p-4 whitespace-pre-wrap">{d.text}</p>
    {:else}
      <div class="flex flex-wrap items-center gap-3 rounded-lg border border-slate-200 bg-white p-4">
        <span class="text-sm text-slate-600">
          Die Datei ist entschlüsselt und liegt bereit.
        </span>
        <button
          class="ml-auto rounded-md bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-700"
        >
          Speichern unter …
        </button>
      </div>
    {/if}
  </section>
</article>
