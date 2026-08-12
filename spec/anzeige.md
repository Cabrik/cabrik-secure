# Anzeigevertrag

**Welche Zustände es gibt, und was jeder behaupten darf.**

Dieses Dokument steht vor den Wireframes, nicht danach. Es ist keine
Gestaltungsfrage.

---

## 1. Warum das eine Spezifikation ist

Der Kern gibt keine Wahrheiten zurück, sondern **Aussagen mit Reichweite**.
`StripResult::Complete` heißt „alle Metadatenträger, die dieses Programm für
dieses Format kennt". Es heißt nicht „metadatenfrei". Der Unterschied steht
im Modulkopf, in der Spezifikation und in der Ausgabe der CLI — überall dort
in Worten.

Eine Oberfläche verkürzt. Aus einem Absatz wird ein Häkchen, aus einer
Einschränkung eine Fußnote, und aus „alle bekannten" wird „alle". In genau
diesem Schritt geht die Ehrlichkeit verloren, die das Projekt von v1
unterscheidet — und zwar **still**, ohne dass jemand eine falsche Zeile
schreibt.

v1 hatte kein einziges Anzeigeproblem im Code. Es hatte eines im Verhalten:
Unbekannte Formate wurden stillschweigend kopiert und sahen danach genauso
aus wie bereinigte. Ein Häkchen für einen Fall, über den nichts bekannt war.

---

## 2. Die Fliegerei — und wo sie über drei Farben hinausgeht

Der Vergleich mit Cockpit-Anzeigen ist richtig gewählt. Er trägt aber weiter,
als „grün, gelb, rot" vermuten lässt, und die drei Zusätze sind hier die
eigentlich wichtigen.

### 2.1 Grün heißt nicht „sicher"

Ein grünes Instrument sagt: *dieses System meldet Normalbetrieb.* Es sagt
nicht, dass der Flug sicher ist. Ein Öldruckinstrument im grünen Bereich
macht keine Aussage über den Sprit.

Genauso hier: Grün bei den Metadaten heißt **„alle Träger entfernt, die
dieses Programm für dieses Format kennt"** — nicht „diese Datei ist
anonym". Die Beschriftung muss das sagen, nicht die Farbe.

### 2.2 Das Flaggensymbol: der vierte Zustand

Ein künstlicher Horizont, der seine Eingangsdaten verliert, zeigt eine
**Flagge** — nicht eine gerade Horizontlinie. Das ist die wichtigste
Entwurfsentscheidung der ganzen Instrumentierung: Die gefährlichste Anzeige
ist die, die etwas Plausibles zeigt, während sie nichts weiß.

`StripResult::Unknown` ist genau dieser Fall. Es ist **kein** Fehler — die
Datei ist in Ordnung, es ist nur ein Format, das wir nicht verstehen. Es ist
aber erst recht kein Grün.

Deshalb gibt es **vier** Zustände, nicht drei.

### 2.3 Farbe steht nie allein

Cockpit-Anzeigen verwenden Farbe zusammen mit Form, Lage und Beschriftung —
nie Farbe allein. Der Grund ist banal und zwingend: Rund **acht Prozent der
Männer** unterscheiden Rot und Grün schlecht oder gar nicht. Eine Oberfläche,
deren Aussage in der Farbe steckt, ist für sie unlesbar.

Für ein Werkzeug, dessen ganzer Zweck darin besteht, eine Einschätzung
mitzuteilen, ist das kein Randfall.

**Regel:** Jeder Zustand trägt Farbe **und** Zeichen **und** Wort. Die
Bedeutung steckt im Wort. Farbe und Zeichen sind Beschleuniger, nicht Träger.

---

## 3. Die vier Zustände

| Zustand | Farbe | Zeichen | Bedeutung |
|---|---|---|---|
| **Bestätigt** | grün | ✓ | Es wurde geprüft, und es traf zu |
| **Warnung** | gelb | ! | Es wurde geprüft, und etwas ist zu beachten |
| **Fehler** | rot | ✕ | Der Vorgang ist gescheitert |
| **Keine Aussage** | grau | ? | Es konnte nicht geprüft werden |

„Keine Aussage" ist neutral, nicht bedrohlich. Es ist die Flagge am
Instrument: eine ehrliche Auskunft über die eigene Reichweite.

**Grau ist nicht abgestuftes Gelb.** Gelb heißt „ich weiß etwas, und es ist
zu beachten". Grau heißt „ich weiß es nicht". Wer die beiden zusammenlegt,
verliert genau die Unterscheidung, um die es hier geht.

---

## 3a. Die zweite Achse: Cyan und Magenta

Im Glascockpit gibt es **zwei** Farbsysteme, nicht eines. Sie zu vermischen
ist der klassische Entwurfsfehler.

| Warnhierarchie | | Informationshierarchie | |
|---|---|---|---|
| Rot | Warnung, sofort handeln | Weiß | gegenwärtiger Zustand, Skalen |
| Gelb | Vorsicht, wahrnehmen | **Cyan** | Bezugsdaten, nicht aktiv |
| Grün | normal, eingerastet | **Magenta** | der **gewählte** Sollwert |

Magenta auf dem Fluglagenanzeiger ist nicht „schlimmer als grün". Es ist die
Höhe, die der Pilot am Autopiloten **eingestellt** hat — der Wert, an dem
gemessen wird. Cyan trägt Bezugsdaten, an denen man sich orientiert, ohne
dass sie eine Lage bewerten.

Beides als fünften und sechsten Zustand einzuführen zerrisse den Vertrag aus
§3. Als eigene Achse **darüber** stärkt es ihn, weil es zwei Dinge trennt,
die bisher beide grau waren:

| Farbe | Was sie trägt | Beispiele |
|---|---|---|
| **Cyan** | Werte, die das Programm **gelesen** hat. Kein Urteil. | Format, Größe, Zeitpunkt, Fingerprint, Fundstelle |
| **Magenta** | Was der **Nutzer verlangt** hat. Der Sollwert. | „Sie haben eine Signatur verlangt" |

**Regel: Cyan und Magenta erscheinen nie in einer Zustandsmarke.** Dort
wären sie doch wieder Zustände, und die Ampel hätte sechs Lichter. Sie
gehören in die Angaben *um* die Marke herum.

Der Nutzen zeigt sich am Fall `unsigniert`: Dieselbe Lage ist neutral oder
ein Fehler, je nachdem, ob eine Signatur verlangt war. Bisher stand dieser
Unterschied nur im Satztext. Jetzt steht der Sollwert daneben — in der
Farbe, die genau das bedeutet.

### Und das Logo

Die Wahl fiel nicht willkürlich auf Cyan: `icon.ico` besteht zu 45 % aus
Schwarz, zu 25 % aus Weiß und zu 10 % aus **`#00E8FF`**. Die Marke ist
bereits cyan auf schwarz. Der dunkle Modus nimmt das auf.

Eine Abweichung gibt es: Der Grund ist **nicht reines Schwarz**. Leuchtendes
Cyan auf reinem Schwarz erzeugt Halation — die Kante blüht aus, und Text
daneben wird anstrengend. Cockpitanzeigen sind aus demselben Grund
dunkelgrau. Ein Logo darf das, eine Textoberfläche nicht.

---

## 4. Zuordnung

### 4.1 Metadaten (`StripResult`)

| Ergebnis | Zustand | Beschriftung |
|---|---|---|
| `Complete { removed }` | Bestätigt | „Alle bekannten Metadaten entfernt (**Format**)" |
| `Partial { remaining, reason }` | Warnung | „Teilweise bereinigt — **Grund**" |
| `Unknown { format_hint }` | **Keine Aussage** | „Format nicht verstanden — keine Aussage über den Inhalt" |
| `Err(Malformed)` | Fehler | „Datei beschädigt: **Grund**" |

Zwingend:

- Bei **Bestätigt** wird das Format genannt. „Alle bekannten Metadaten
  entfernt" ohne Bezug ist eine stärkere Aussage, als der Kern deckt.
- Bei **Warnung** steht der Grund **im Text**, nicht in einem Hinweisfeld,
  das erst beim Zeigen erscheint. Wer die Warnung sieht, muss ohne weiteren
  Handgriff erfahren, was bleibt — etwa: der Kodierername steckt in den
  Tonrahmen, oder: das Bild liegt in einem `SubIFD` einer Rohdatei.
- Bei **Keine Aussage** darf nichts stehen, was nach Erfolg klingt. Die
  Formulierung nennt, was erkannt wurde („Photoshop-Dokument"), und stellt
  klar, dass darüber hinaus nichts gesagt werden kann.

Die Funde selbst tragen zusätzlich `Severity`. Sie färbt **den einzelnen
Fund**, nicht das Gesamturteil:

| `Severity` | Darstellung |
|---|---|
| `Critical` | hervorgehoben — GPS, Klarname, Gerätenummer, Zweitkopien |
| `Notable` | gewöhnlich |
| `Minor` | zurückhaltend, ausklappbar |

### 4.2 Authentizität (`Authenticity`)

Sechs Zustände, und die Zuordnung ist hier am wenigsten offensichtlich.

| Zustand | Anzeige | Begründung |
|---|---|---|
| `SignedVerified` | **Bestätigt** | Der einzige Fall, der Grün verdient — so steht es im Code |
| `SignedSeen` | Warnung | Bekannt, aber nie verifiziert. Der Name ist eine Behauptung des Speichers, keine Prüfung |
| `SignedUnknown` | **Keine Aussage** | Gültige Signatur eines fremden Schlüssels. Nichts ist falsch, es ist nur niemand zugeordnet |
| `SignedChanged` | Warnung | Deutlicher, wenn der abgelöste Schlüssel **verifiziert** war |
| `SignedRevoked` | **Fehler** | Der einzige Fall, in dem etwas aktiv nicht stimmt |
| `Unsigned` | **Keine Aussage** | Siehe unten |

**`Unsigned` ist kein Mangel.** Anonymer Versand ist ein legitimer Modus —
für manche Nutzer dieses Programms der wichtigste überhaupt. Ihn gelb zu
färben drängte jeden dazu, immer zu signieren, und träfe damit ausgerechnet
die, die es nicht dürfen. Die Anzeige lautet neutral: „Nicht signiert — sagt
nichts darüber, wer sie geschickt hat."

Wer eine Signatur **braucht**, verlangt sie ausdrücklich
(`--require-signature`). Dann ist ihr Fehlen ein **Fehler**, kein Hinweis.
Dieselbe Lage, zwei Bewertungen — abhängig davon, was der Nutzer verlangt
hat, nicht davon, was das Programm für richtig hält.

### 4.3 Sicheres Löschen (`ShredCapability`, `ShredOutcome`)

| Lage | Zustand | Beschriftung |
|---|---|---|
| `Overwrite`, gelöscht | Bestätigt | „Gelöscht und überschrieben" |
| `BestEffort`, gelöscht | Warnung | „Gelöscht — Überschreiben ist auf diesem Datenträger nicht verlässlich" |
| Löschen fehlgeschlagen | Fehler | Grund im Klartext |

`BestEffort` ist der **Normalfall** auf heutigen Systemen. Eine Oberfläche,
die deshalb dauernd gelb leuchtet, erzieht zum Wegsehen. Der Hinweis gehört
einmal deutlich an die Stelle, an der gelöscht wird — nicht als Dauerzustand
in die Kopfzeile.

### 4.4 Kontakte im Verzeichnis (`Vertrauen`)

| Vertrauen | Anzeige | Begründung |
|---|---|---|
| `verifiziert` | Bestätigt | Safety Number oder Fingerprint wurde verglichen |
| `gesehen` | **Keine Aussage** | Siehe unten — *nicht* Warnung |
| `gewechselt` | Warnung | Der Kontakt tritt mit einem anderen Schlüssel auf |
| `widerrufen` | Fehler | Als kompromittiert markiert |

**Die einzige Stelle, an der derselbe Sachverhalt zwei Farben bekommt.** Ein
nie verifizierter Kontakt ist im Verzeichnis **grau**, als Absender einer
Nachricht (§4.2, `SignedSeen`) **gelb**.

Das ist Absicht und kein Versehen. Im Verzeichnis ist „nicht verifiziert" der
**erwartbare** Zustand: So fängt jeder Kontakt an, und ein Verzeichnis, in dem
die Hälfte der Einträge gelb leuchtet, erzieht innerhalb einer Woche zum
Wegsehen — dann fällt auch das echte Gelb nicht mehr auf. Erst wenn eine
Nachricht ankommt und man sich auf den Namen verlassen soll, wird aus dem
fehlenden Vergleich eine Warnung.

Es ist dieselbe Denkfigur wie bei `Unsigned` in §4.2: **Die Bewertung hängt
davon ab, worum es gerade geht**, nicht allein vom Datenfeld. Und es ist die
Art Unterscheidung, die beim Umbauen still verschwindet — jemand
vereinheitlicht die Farbe, und niemandem fällt auf, was verloren ging.
Deshalb steht sie in `Kontakte.test.ts` als ausführbarer Test.

Der **Widerruf** verspricht nur, was er hält: „Wirkt nur bei Ihnen. Ein
Widerruf ohne Verteilweg erreicht niemanden sonst." Cabrik Secure hat keinen
Schlüsselserver; ein Widerruf ist ein Eintrag im eigenen Speicher und sonst
nichts.

---

## 5. Verbotene Formulierungen

Diese Wörter behaupten mehr, als der Kern deckt. Sie kommen in der
Oberfläche nicht vor:

| Nicht | Sondern |
|---|---|
| „sicher" | was tatsächlich getan wurde |
| „sauber", „bereinigt" ohne Bezug | „alle bekannten Metadaten entfernt (JPEG)" |
| „garantiert metadatenfrei" | — kommt gar nicht vor |
| „anonym" als Zusicherung | „nicht signiert" als Feststellung |
| „vollständig gelöscht" | „gelöscht und überschrieben" bzw. der Hinweis |
| „verifiziert" für bloß bekannte Kontakte | „bekannt, nicht verifiziert" |

---

## 6. Was die Oberfläche nie zu sehen bekommt

Vorbereitung für Phase 4, hier festgehalten, weil es den Entwurf der
Bildschirme bereits einschränkt.

**Regel (Fahrplan Phase 4):** Schlüsselmaterial bleibt in Rust. Die
Oberfläche erhält Handles, Zustände und Fortschritt — nie Geheimnisse.

Das heißt konkret:

| Nie über die Brücke | Stattdessen |
|---|---|
| Private Schlüssel, entsperrt oder nicht | ein Handle auf die entsperrte Identität |
| Passwörter | Eingabe wird unmittelbar an Rust gereicht, nicht zwischengespeichert |
| Der abgeleitete Schlüssel einer Sitzung | ein Sitzungshandle |
| Der Klartext einer großen Datei | Fortschritt und Zielpfad |

v1 hielt das Passwort dauerhaft im Klartext in einer globalen Variablen.
Das ist der Fehler, den diese Regel verhindert.

**Folge für den Entwurf:** Es kann keinen Bildschirm geben, der „den
Schlüssel anzeigt". Es kann einen geben, der den **Fingerprint** und die
**Safety Number** zeigt — beides ist öffentlich und zum Vorlesen gedacht.

---

## 7. Offen

- Ob „Keine Aussage" grau oder blau erscheint. Grau wirkt inaktiv, blau
  hinweisend. Zu entscheiden an einem Entwurf, nicht am Schreibtisch.
- Ob `Severity::Minor` überhaupt angezeigt wird oder nur in einer
  Detailansicht.
- Wie mehrere Funde derselben Art zusammengefasst werden, ohne die Zahl zu
  verschweigen.
