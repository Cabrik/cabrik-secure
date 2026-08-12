<!--
  Ein Wert, den das Programm GELESEN hat — kein Urteil.
  Format, Größe, Zeitpunkt, Fingerprint, Fundstelle.

  Cyan, nach der Informationshierarchie des Glascockpits: Dort trägt Cyan
  die Bezugsdaten, an denen man sich orientiert, ohne dass sie eine Lage
  bewerten. Es ist zugleich die Farbe des Logos.
-->
<script lang="ts">
  interface Props {
    beschriftung: string;
    /** Für Fingerprints und Fundstellen: feste Zeichenbreite. */
    fest?: boolean;
    children?: import("svelte").Snippet;
  }
  let { beschriftung, fest = false, children }: Props = $props();
</script>

<div class="min-w-0">
  <dt class="text-schrift-leise text-xs">{beschriftung}</dt>
  <dd class="text-bezug text-sm break-words {fest ? 'font-mono' : ''}">
    {@render children?.()}
  </dd>
</div>
