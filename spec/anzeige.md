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

**Diese vier gelten für Befunde** — für Aussagen über eine Datei, einen
Absender, einen Kontakt. Sie gelten **nicht** für Zustände des Programms
selbst.

Der Unterschied ist kein Wortklauben. „Gesperrt“ trug im Sperrbildschirm
einmal das Zeichen `?`, weil es grau ist und grau hier „keine Aussage“
heißt. Gelesen wurde es, wie es dasteht: als sei etwas schiefgegangen.
Gesperrt zu sein ist aber ein **bekannter** Zustand — das Gegenteil von
„konnte nicht geprüft werden“.

Wo es keine Farbe zu beschleunigen gibt, trägt das Wort allein.

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
| **Magenta** | Was der **Nutzer verlangt** hat. Der Sollwert. | „Sie haben eine Signatur verlangt"; „Bleibt hier — wird nicht versendet" |

**Regel: Cyan und Magenta erscheinen nie in einer Zustandsmarke.** Dort
wären sie doch wieder Zustände, und die Ampel hätte sechs Lichter. Sie
gehören in die Angaben *um* die Marke herum.

Der Nutzen zeigt sich am Fall `unsigniert`: Dieselbe Lage ist neutral oder
ein Fehler, je nachdem, ob eine Signatur verlangt war. Bisher stand dieser
Unterschied nur im Satztext. Jetzt steht der Sollwert daneben — in der
Farbe, die genau das bedeutet.

Der zweite Fall ist die **gewählte Originalfassung**: Wer eine Datei
unverändert versendet, hat das eingestellt. Nicht grün (nichts wurde
bereinigt), nicht gelb (es ist kein Warnfall — es war die Absicht), nicht
grau (es fehlt keine Aussage).

Der dritte Fall ist die **vom Versand ausgenommene Datei**. Sie ist nicht
grün (nichts an ihr ist in Ordnung), nicht gelb (es ist kein Warnfall mehr)
und nicht grau (es fehlt keine Aussage). Sie ist eine **Einstellung**: Der
Nutzer hat entschieden, dass sie hierbleibt. Genau dafür gibt es Magenta.

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

### 4.1b Metadaten in **Empfangenem** (`Inspection`)

Dieselben Funde, andere Frage — und deshalb eine eigene Zuordnung.

§4.1 beschreibt, was ein Bereinigen **ergab**. Bei einer Datei, die gerade
ankommt, ist nichts bereinigt worden und soll auch nichts bereinigt werden:
Sie gehört jemand anderem, und wir geben sie unverändert weiter, wie wir sie
bekommen haben. Die Frage lautet nicht „was ist herausgegangen", sondern
**„was ist drin"**.

| Befund | Zustand | Beschriftung |
|---|---|---|
| verstanden, keine Funde | Bestätigt | „Nichts gefunden — in den bekannten Metadatenträgern von **Format**" |
| verstanden, Funde | Warnung | „**N** Funde" + die kritischen namentlich |
| nicht verstanden | **Keine Aussage** | wie §4.1 |
| nicht lesbar | Fehler | wie §4.1 |
| **Textnachricht** | *keine Anzeige* | „Eine Textnachricht trägt keine Dateimetadaten" |

Zwingend:

- **Funde sind hier Warnung, auch wenn keiner kritisch ist.** Beim Senden
  werden sie entfernt — die Meldung beschreibt einen erledigten Vorgang.
  Hier bleiben sie stehen. Ein Farbprofil grün zu nennen hieße „nichts
  drin" zu sagen, wo etwas drin ist.
- **Kritische Funde werden benannt, nicht nur gezählt.** Wer „3 Funde"
  liest, klappt die Liste vielleicht nicht auf. „Darunter eine Ortsangabe"
  liest er.
- **Die Fundliste steht offen.** Beim Senden ist sie eine Quittung und darf
  zu sein; hier ist sie das Einzige, was jemand **vor dem Speichern** noch
  ändern kann.
- **Der Satz nennt den Absender.** Was hier auftaucht, hat *er* über sich
  preisgegeben — ein Foto mit GPS-Angabe verrät, wo er stand. Diese
  Blickrichtung gibt es in §4.1 nicht und sie ist der eigentliche Nutzen
  der Anzeige.
- **„Nichts gefunden" ist nicht „keine Anzeige".** Eine leere Fundliste ist
  eine Aussage: Es wurde nachgesehen. Bei einer Textnachricht stellt sich
  die Frage gar nicht. Die beiden dürfen nie gleich aussehen.
- Die Datei wird **nicht** stillschweigend bereinigt. Wer sie weitergeben
  will, speichert sie und schickt sie über *Senden* — dort ist Bereinigen
  eine bewusste Handlung.

### 4.2 Authentizität (`Authenticity`)

Sechs Zustände, und die Zuordnung ist hier am wenigsten offensichtlich.

| Zustand | Anzeige | Begründung |
|---|---|---|
| `SignedVerified` | **Bestätigt** | Der einzige Fall, der Grün verdient — so steht es im Code. Der **Weg** wird dabei genannt, siehe unten |
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

**Bei `SignedVerified` wird der Weg genannt** (`verified_via`), nicht nur der
Zeitpunkt. `spec/trust-store.md` §5 stellt fest, dass die Wege nicht
gleichwertig sind, und verlangt ausdrücklich, dass die schwächste Zeile der
Tabelle benannt wird: Ein Fingerprint, der über denselben Kanal kam wie die
Nachricht, beweist nichts.

| Weg | Satz | Vorbehalt |
|---|---|---|
| QR-Code | „Über QR-Code geprüft am …" | keiner — ein Angreifer hätte im Raum stehen müssen |
| Safety Number | „Safety Number verglichen am …" | keiner |
| Fingerprint | „Fingerprint abgeglichen am …" | **„derselbe Kanal, derselbe Angreifer"** |
| `None` | „Geprüft am …" | „Auf welchem Weg, ist nicht vermerkt" |

Der Zustand bleibt in **allen vier Fällen grün**. Das Programm hat nicht
darüber zu befinden, ob die Prüfung des Nutzers ihm gut genug war — es hat
zu sagen, *was* geprüft wurde. Dann kann er selbst urteilen.

Ein Hinweis, der bei jedem Weg erschiene, würde nicht gelesen. Er muss den
schwachen Fall vom starken unterscheiden, sonst ist er Dekoration — dafür
gibt es in `durchlauf.rs` eine ausdrückliche Gegenprobe.

**Das gilt einheitlich für Nachricht und Verzeichnis.** Anders als bei der
Farbe (§4.4) gibt es hier keinen Zusammenhang, in dem der Weg unwichtiger
wäre: Wer den Kontakt ansieht, will dasselbe wissen wie der, dem gerade eine
Nachricht davon zugestellt wurde.

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

### 4.3a Der Befund vor dem Senden

**Es wird alles gezeigt, was gefunden wurde — nicht nur, was übrig bleibt.**

Zeigt eine Oberfläche nur den Rest, sieht eine sauber bereinigte Datei aus
wie eine, in der nie etwas stand. Der Nutzer erfährt nie, dass sein Name,
die Seriennummer seiner Kamera und der Aufnahmeort darin waren — und lernt
dadurch auch nie, dass er das mit sich herumträgt. Der vollständige Befund
ist nicht nur Kontrolle, er ist die einzige Stelle, an der jemand das über
seine eigenen Dateien erfährt.

Je Fund steht deshalb dabei, **ob er die Datei verlässt**: „wird entfernt"
oder „bleibt". Die Schwere färbt den einzelnen Fund, nicht das Gesamturteil.

**Die Wahl der Fassung.** Zu jeder verstandenen Datei gehört die
Entscheidung, ob die bereinigte Fassung oder das Original hinausgeht.
Manchmal sind die Angaben der Zweck — eine Urheberangabe im Foto, ein
Bearbeitungsverlauf, der den Empfänger angeht. Wer das braucht und es nicht
bekommt, umgeht das Programm und schickt die Datei ungeprüft über einen
anderen Weg. Eine sichtbare, benannte Wahl ist besser als eine Umgehung.

Bei `unbekannt` gibt es **keine Wahl**: Ohne verstandenes Format existiert
keine bereinigte Fassung, und zwei anzubieten wäre eine Behauptung.

Eine gewählte Originalfassung hebt jede grüne Gesamtaussage auf. „Alles
bereinigt" über eine Datei, an der bewusst nichts geändert wurde, wäre
falsch.

### 4.3a-2 Frühere Fassungen und die Schalter des Kerns

**Frühere PDF-Fassungen sind kein Metadatum, sondern Inhalt.** Sie stehen
deshalb gesondert, nicht in der Fundliste.

PDFs werden inkrementell fortgeschrieben: Jede Bearbeitung hängt hinten an,
statt zu ersetzen. Wer Namen aus einem Dokument entfernt und speichert, hat
die vorige Fassung mitsamt den Namen weiterhin in der Datei. Ein Leser zeigt
sie nicht an. Ein Werkzeug schon.

Angezeigt wird deshalb nicht „wie sah Fassung 1 aus", sondern **was nur dort
steht** (`nur_hier`) — also der Text, den jemand herausgenommen hat und der
trotzdem mitfährt. Das ist die klassische Schwärzungspanne.

**Die vier Schalter aus `cabrik metadata strip` sind keine Schalter, sondern
Zielkonflikte.** Jeder ist manchmal richtig und manchmal fatal:

| Schalter | Voreinstellung | Was gesagt werden muss |
|---|---|---|
| `--revision N` | angezeigte Fassung | Spätere Bearbeitungen gehen verloren |
| `--keep-history` | aus | Frühere Fassungen bleiben wiederherstellbar, **samt allem, was aus ihnen entfernt wurde** |
| `--remove-comments` | aus | Betrifft nur Anmerkungen; der Text bleibt Zeichen für Zeichen |
| `--accept-changes` | aus | **Verändert den Inhalt** — Löschungen verschwinden samt Text |

Zwingend:

- **Kein Schalter, der den Inhalt verändert, ist voreingestellt.**
- Ein Schalter wird **nur angeboten, wo er etwas bewirkt**. Ein Häkchen ohne
  Wirkung ist eine Behauptung über die Datei.
- Die Folge einer Wahl steht **sofort** daneben, nicht erst im Ergebnis.
- `--keep-history` hebt jede grüne Gesamtaussage auf: „Vollständig bereinigt"
  wäre falsch, wenn frühere Fassungen in der Datei bleiben.
- Eine getroffene Wahl erscheint auch in der **Übersicht**, nicht nur im
  Befund. Wer sie dort nicht wiederfindet, müsste jede Datei einzeln
  aufmachen — bei vierzig Dateien tut das niemand.

### 4.3b Löschen der Ausgangsdateien

**Eine Entscheidung, zwei Zeitpunkte.** Die Wahl fällt *vor* dem
Verschlüsseln, ausgeführt wird sie *danach*.

Niemand löscht ein Original, bevor er weiß, dass die verschlüsselte Datei
existiert — deshalb nicht sofort. Und niemand entscheidet das gut, wenn der
Vorgang gelaufen ist und der Blick zur nächsten Aufgabe wandert — deshalb
nicht erst danach.

Der Vorbehalt aus §4.3 (`BestEffort` ist der Normalfall) gehört **an die
Stelle, an der gewählt wird**. Sonst verspricht das Häkchen mehr, als es
hält.

Wurde nicht gelöscht, steht im Ergebnis der Satz, den
Verschlüsselungswerkzeuge gern weglassen: **Die Ausgangsdateien liegen
unverschlüsselt weiter da.** Verschlüsseln legt eine zweite Datei daneben,
es ersetzt die erste nicht.

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

### 4.5 Fortschritt bei Stapeln

Fünf Vorgänge arbeiten eine Liste von Dateien ab: prüfen, bereinigt
speichern, verschlüsseln, Löschen beurteilen, löschen. Bei vierzig Dateien
dauert jeder davon spürbar.

**Cyan, nicht grün oder gelb.** Ein Fortschritt bewertet nichts — er ist ein
gelesener Wert wie Format, Größe und Fingerprint (§3a). Ihn in einen der vier
Zustände zu stecken hieße, „es läuft" zu einer Aussage über die Lage zu
machen.

Zwingend:

- **Die Zahlen stehen da, nicht nur der Balken.** Ein Balken allein ist Farbe
  allein (§2.3). „3 von 40" trägt dieselbe Auskunft ohne ihn
- **Der Dateiname steht dabei.** „3 von 40" sagt nicht, ob es hakt oder
  läuft. Bleibt eine Minute lang derselbe Name stehen, weiß man wenigstens,
  **welche** Datei aufhält — und dass es nicht das Programm ist
- **Es steht dabei, was geschieht.** „Wird geprüft" und „Wird gelöscht"
  dürfen nicht gleich aussehen: Das eine ist folgenlos, das andere
  unwiderruflich
- **Verlaufsform, nicht Befehlsform.** „Wird gelöscht" beschreibt, was
  gerade geschieht; „Löschen" läse sich wie ein Knopf, den man noch drücken
  könnte
- **Gezählt werden die fertigen, nicht die begonnenen.** Bei der ersten
  Datei steht der Balken auf null, und das stimmt: Es ist noch nichts
  fertig
- **Der Balken verschwindet am Ende** — auch nach einem Fehlschlag. Ein
  Balken, der bei „39 von 40" stehen bleibt, behauptet Arbeit, die längst
  getan ist

Und eine Regel, die nicht die Anzeige betrifft, aber sie unmöglich macht,
wenn man sie bricht: **Ein Stapelbefehl läuft neben dem Hauptfaden.** Unter
Windows zeichnet der Hauptfaden das Fenster; ein Befehl, der ihn belegt,
friert die Anzeige ein, und keine Meldung käme durch — sie würde zugestellt,
wenn schon alles fertig ist.

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
