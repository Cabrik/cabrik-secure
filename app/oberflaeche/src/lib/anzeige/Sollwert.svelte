<!--
  Etwas, das der NUTZER verlangt hat — nicht das Programm.

  Magenta, nach der Fliegerei: Dort ist Magenta der eingestellte Sollwert,
  die Höhe, auf die der Pilot den Autopiloten gesetzt hat. Nicht „schlimmer
  als grün", sondern eine andere Art von Angabe.

  Hier: „Sie haben eine Signatur verlangt." Erst dadurch wird aus einer
  unsignierten Nachricht ein Fehler statt einer neutralen Feststellung.
-->
<script lang="ts">
  interface Props { children?: import("svelte").Snippet }
  let { children }: Props = $props();
</script>

<p
  class="text-sollwert border-sollwert/40 inline-flex items-center gap-1.5 rounded border px-2 py-0.5 text-xs"
>
  <span aria-hidden="true">◆</span>
  {@render children?.()}
</p>
