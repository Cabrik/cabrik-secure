<!--
  Die Sanduhr — für das, was **dauert**.

  # Warum ohne Dreieck

  Warten ist kein Warnfall. Der erste Entwurf setzte die Sanduhr in das
  Warndreieck und erbte damit dessen Bedeutung: „etwas ist zu beachten“.
  Zu beachten ist hier aber nichts — es dauert nur, und zwar mit Absicht.

  # Wo sie hingehört

  Dorthin, wo echte Sekunden vergehen und der Nutzer sich sonst fragt, ob
  noch etwas geschieht. Die Passwortableitung ist der Fall: Argon2 braucht
  seine Zeit, und das ist keine Trägheit, sondern der Zweck.

  Nicht dagegen an „eine Datei wartet“ — das ist eine Auskunft über einen
  Zustand, kein Vorgang, der läuft. Ein Symbol, das sich um nichts dreht,
  lehrt, es zu übersehen.

  # Farbe und Strichstärke

  Wie beim Warndreieck: `currentColor`, damit sie dem Erscheinungsbild
  folgt. Der Einsatzort bestimmt den Ton — beim Ableiten ist es der leise,
  denn es ist keine Warnung.

  Die Linie ist dicker als in der Vorlage. Dort ist sie auf 160
  Bildpunkte gezeichnet; als Symbol neben Text steht sie auf 18.
-->
<script lang="ts">
  interface Props {
    /** Zusätzliche Klassen — für die Größe am Einsatzort. */
    klasse?: string;
  }
  let { klasse = "h-[1.1em] w-[1.1em]" }: Props = $props();
</script>

<svg
  viewBox="70 91 80 80"
  class="{klasse} shrink-0"
  fill="none"
  stroke="currentColor"
  aria-hidden="true"
>
  <!-- Sand in der oberen Kammer -->
  <path
    d="M90,100 L130,100
       C130,120 110,124.5 110,131
       C110,124.5 90,120 90,100 Z"
    fill="currentColor"
    stroke="none"
  />
  <!-- Das fallende Korn -->
  <circle cx="110" cy="143" r="3.8" fill="currentColor" stroke="none" />
  <!-- Der Hügel unten -->
  <path
    d="M96.5,164 L123.5,164
       C123.5,157 117.5,152.5 110,152.5
       C102.5,152.5 96.5,157 96.5,164 Z"
    fill="currentColor"
    stroke="none"
  />
  <!-- Die Kontur -->
  <path
    d="M88,98 L132,98
       C132,120 110,124.5 110,131
       C110,137.5 132,142 132,164
       L88,164
       C88,142 110,137.5 110,131
       C110,124.5 88,120 88,98 Z"
    stroke-width="9"
    stroke-linejoin="round"
    stroke-linecap="round"
  />
</svg>
