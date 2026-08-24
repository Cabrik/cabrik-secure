<!--
  Das Warndreieck — für **echte** Warnungen.

  # Wofür es da ist und wofür nicht

  `spec/anzeige.md` §5: gelb und `!` heißen „Es wurde geprüft, und etwas
  ist zu beachten“. Nicht mehr und nicht weniger.

  Es gehört deshalb NICHT an Zustände des Programms. „Gesperrt“ trug
  einmal ein `?` und wäre um ein Haar zu einem Warndreieck geworden —
  beides falsch, aus demselben Grund: Gesperrt zu sein ist der erwünschte
  Normalzustand, keine Beobachtung, die jemandem zu denken geben soll.

  Und es hätte einen Preis über den einen Bildschirm hinaus: Wer das
  Dreieck bei jedem Entsperren sieht, liest es nach zwei Wochen nicht
  mehr. Wenn dann ein echtes auftaucht, trägt es nichts.

  # Warum `currentColor`

  Weil es zwei Erscheinungsbilder gibt. Im dunklen ist die Warnfarbe ein
  helles Gelb, im hellen ein dunkles Bernstein — sonst wäre sie auf Weiß
  nicht lesbar. Eine fest eingebaute Farbe stimmte in genau einem der
  beiden.

  # Warum die Linie dicker ist als in der Vorlage

  Die Vorlage ist auf 220 Bildpunkte gezeichnet, wo eine Haarlinie
  elegant wirkt. Als Symbol neben Text steht sie auf 18 — dort wäre
  dieselbe Strichstärke unter einem halben Bildpunkt und verschwände.
  Die Form bleibt, die Linie wächst mit.

  # Warum der Rahmen enger ist als in der Vorlage

  Weil die Zeichnung nur etwa **64 %** ihres `viewBox` füllt — der Rest
  ist eingebauter Rand. Bei einer Grafik, die für sich steht, fällt das
  nicht auf; als Symbol neben Text schon: Es wirkt rund ein Drittel zu
  klein, und zwar überall, nicht nur an einer Stelle.

  Der Rahmen ist deshalb auf die Zeichnung zugeschnitten (mit etwas Luft,
  damit die Linie nicht am Rand klebt). Das ist die richtige Stelle für
  die Behebung: Wer stattdessen am Einsatzort die Größe hochdrehte,
  müsste es an jedem Einsatzort wieder tun — und der nächste vergäße es.
-->
<script lang="ts">
  interface Props {
    /** Zusätzliche Klassen — für die Größe am Einsatzort. */
    klasse?: string;
  }
  let { klasse = "h-[1.1em] w-[1.1em]" }: Props = $props();
</script>

<!--
  `aria-hidden`, weil das Wort danebensteht. Der Anzeigevertrag verlangt
  Zeichen UND Wort; wer vorgelesen bekommt, hört das Wort, und ein
  zusätzliches „Warndreieck“ wäre Wiederholung.
-->
<svg
  viewBox="26 38 168 168"
  class="{klasse} shrink-0"
  fill="none"
  stroke="currentColor"
  aria-hidden="true"
>
  <path
    d="M132.15,76.06
       L171.65,140.37
       A26,26 0 0 1 149.49,180
       L70.51,180
       A26,26 0 0 1 48.35,140.37
       L87.85,76.06
       A26,26 0 0 1 132.15,76.06 Z"
    stroke-width="15"
    stroke-linejoin="round"
  />
  <path d="M105.2,96 L114.8,96 L113.3,138 L106.7,138 Z" fill="currentColor" stroke="none" />
  <circle cx="110" cy="156" r="8.5" fill="currentColor" stroke="none" />
</svg>
