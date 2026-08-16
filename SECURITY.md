# Sicherheitslücken melden

## Wohin

<!--
  NOCH EINZUTRAGEN, bevor dieses Repository öffentlich wird.

  Beides ist eine Entscheidung, die niemand außer dem Betreiber treffen
  kann, und beides falsch zu raten wäre schlimmer als eine Lücke:

  1. Eine Adresse, die tatsächlich gelesen wird. Eine eigene Domain wirkt
     seriöser als eine Freemail-Adresse — aber eine erfundene Adresse an
     einer Domain, die niemandem gehört, macht die Meldung unmöglich.
  2. Ob es einen Cabrik-Austauschschlüssel für verschlüsselte Meldungen
     gibt. Wenn ja, gehört seine Nutzlast hier hinein oder unter
     `docs/`, und der Fingerprint dazu.

  Solange hier ein Platzhalter steht, ist dieses Repository nicht
  veröffentlichungsreif.
-->

**⟨Meldeadresse eintragen⟩**

Bitte **kein** öffentliches Issue und keinen Pull Request. Wer eine Lücke
findet und keinen Kanal vorfindet, schreibt sie ins Netz oder gar nicht —
beides ist schlechter als eine Mail.

## Was hineingehört

- Was Sie gefunden haben und **welche Zusicherung dadurch bricht**. Die
  Zusicherungen stehen in `spec/threat-model.md`; ein Bezug darauf hilft
  mehr als eine Schwerebewertung.
- Wie es sich nachstellen lässt. Eine Datei, ein Ablauf, ein Testvektor.
- Welche Fassung, welches Betriebssystem.

Was **nicht** hineingehört: Ihr Passwort, Ihre Schlüsseldatei, echte
Envelopes mit fremdem Inhalt. Für eine Fehlersuche genügen immer erfundene
Daten — und wenn nicht, sagen wir das ausdrücklich dazu.

## Was Sie erwarten dürfen

| | |
|---|---|
| Eingangsbestätigung | innerhalb von **3 Werktagen** |
| Erste Einschätzung | innerhalb von **10 Werktagen** |
| Behebung und Veröffentlichung | nach Absprache, Richtwert **90 Tage** |

Wenn eine Frist reißt, sagen wir es, statt sie verstreichen zu lassen.

**Wir melden uns auch dann, wenn wir Ihren Fund nicht für eine Lücke
halten** — mit einer Begründung, der Sie widersprechen können. Ein
Schweigen wäre die schlechteste Antwort.

## Was als Lücke zählt

Maßgeblich ist `spec/threat-model.md`. Dort steht, wogegen Cabrik Secure
schützt **und wogegen ausdrücklich nicht**. Was dort als nicht abgedeckt
benannt ist, ist keine Lücke, sondern eine Grenze — aber melden Sie es
trotzdem, wenn Sie meinen, dass die Grenze falsch gezogen ist. Das ist eine
Diskussion, die wir führen wollen.

Ein paar Dinge, die erfahrungsgemäß gemeldet werden und **keine** Lücken
sind, weil sie so gewollt sind:

- **Ein Envelope verrät seine Größe.** Das steht im Threat Model und wird
  in der Oberfläche gezeigt. Bei Text wird gepolstert, bei Dateien nicht.
- **Eine unsignierte Nachricht nennt keinen Absender.** Anonymer Versand
  ist ein vorgesehener Modus, kein Mangel.
- **Ein nicht verstandenes Dateiformat wird nicht bereinigt.** Cabrik sagt
  dann „keine Aussage" statt Sauberkeit zu behaupten. Genau das ist der
  Punkt.
- **Ein vergessenes Passwort ist unwiederbringlich.** Es gibt keine
  Hintertür, und es wird nie eine geben.

## Was besonders interessiert

- Alles, wodurch Klartext oder Schlüsselmaterial die Rust-Schicht verlässt
- Alles, wodurch die Oberfläche etwas **behauptet**, was der Kern nicht
  deckt — eine grüne Marke, wo Grau richtig wäre, ist in diesem Programm
  ein Fehler ersten Ranges
- Metadaten, die beim Bereinigen übrig bleiben, ohne benannt zu werden
- Abweichungen der Umsetzung von den Spezifikationen unter `spec/`

## Umfang

Quelloffen und damit prüfbar sind:

    cabrik-core       Envelope, Keyfile, Kontaktspeicher, Fingerprints
    cabrik-metadata   Metadaten erkennen und entfernen
    cabrik-shred      sicheres Löschen
    cabrik-ablage     Dateiablage

Meldungen zu den übrigen Teilen sind trotzdem willkommen — sie lassen sich
nur nicht am Quelltext nachvollziehen.

## Kein Kopfgeld

Wir zahlen nichts für Funde. Das zu verschweigen und Hoffnungen zu wecken
wäre unfair. Wer möchte, wird in den Anmerkungen zur Fassung genannt, in
der die Lücke behoben ist — oder ausdrücklich nicht, wenn Ihnen das lieber
ist.
