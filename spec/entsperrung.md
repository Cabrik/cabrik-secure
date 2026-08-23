# Cabrik Secure — Entsperrung und Sitzung

**Status:** Verbindlich · Phase 4, Dokument 9
**Setzt voraus:** `threat-model.md`, `keyfile-v2.md`, `trust-store.md`, `anzeige.md`

---

## 1. Die unbequeme Wahrheit

**Der Weg des Passworts von der Tastatur zum Kern lässt sich nicht
vollständig kontrollieren.** Das ist keine Schwäche dieser Umsetzung,
sondern folgt daraus, dass ein Mensch tippt und ein Betriebssystem die
Zeichen weiterreicht.

Was wir versprechen können, endet an einer klaren Grenze:

| Abschnitt | Wer kontrolliert ihn |
|---|---|
| Tastatur → Betriebssystem | Niemand außer dem Betriebssystem |
| Eingabefeld der Anwendung | **Wir** — je nach Bauart |
| Übergabe an den Kern | **Wir** |
| Verarbeitung im Kern | **Wir** |
| Auslagerung auf die Platte | **Wir**, in Grenzen (§5.3) |

Deshalb formuliert dieses Dokument keine Zusage über „das Passwort", sondern
über **unsere eigenen Kopien davon**. Alles andere wäre eine Behauptung über
fremde Software.

## 2. Was gehalten wird — und was nicht

### 2.1 Nicht das Passwort

Nach `keyfile::read` wird das Passwort nicht mehr gebraucht. Es wird
**nirgends** aufbewahrt: nicht in der Sitzung, nicht in einem
Zwischenspeicher, nicht in einer Einstellung.

Damit ist die Frage „wie lange halten wir das Passwort" nicht beantwortet,
sondern **weggefallen**. Version 1 hielt es dauerhaft im Klartext in ihrem
globalen Zustand — der schwerwiegendste Befund der Nachprüfung.

### 2.2 Sondern die entsperrte Identität

Gehalten wird:

- die `Identity` (X25519-Privatschlüssel, Ed25519-Seed, X-Wing-Seed),
- der daraus abgeleitete `ContactsKey`.

Beide sind `Zeroize + ZeroizeOnDrop`. **Sperren heißt fallenlassen**, und
das überschreibt den Speicher.

### 2.3 Ohne Identität keine Kontakte

`ContactsKey::derive` leitet aus `identity.enc_sk` ab. Der Kontaktspeicher
ist also ohne entsperrte Identität nicht lesbar — das ist keine
Entwurfsentscheidung dieses Dokuments, sondern folgt aus `trust-store.md`.

**Der Anfangszustand der Anwendung ist deshalb: gesperrt und leer.** Ein
Verzeichnis, das vor dem Entsperren Namen zeigt, wäre technisch unmöglich —
und wenn es sie zeigte, wären es erfundene.

## 3. Wie lange entsperrt bleibt

### 3.1 Sperre nach Untätigkeit

Die Grenze zählt ab der **letzten Handlung**, nicht ab dem Entsperren. Wer
eine Stunde am Stück arbeitet, wird nicht mitten hinein gesperrt.

### 3.2 Die Liste

| Wahl | Wofür |
|---|---|
| 1 Minute | Fremde Umgebung, Café, geteilter Arbeitsplatz |
| 5 Minuten | |
| **15 Minuten** | **Voreinstellung** |
| 30 Minuten | |
| 60 Minuten | |
| *Bis das Fenster geschlossen wird* | Ausdrücklich benannt, siehe unten |

**Eine Liste und keine freie Eingabe.** Freie Eingabe lädt zu „0" oder
„999999" ein — und das heißt „nie sperren", ohne dass jemand *entschieden*
hat, nie zu sperren. Jeder Eintrag einer Liste kann dagegen seinen Preis
danebenschreiben.

**Keine Werte über 60 Minuten.** Zwei oder vier Stunden sind keine eigene
Entscheidung, sondern dieselbe wie „bis ich das Fenster schließe" — nur als
Vorsicht verkleidet. Eine Liste, die Wahlmöglichkeiten führt, die sich nicht
wirklich unterscheiden, führt in die Irre.

Der letzte Eintrag heißt deshalb, was er tut. Er wird **magenta**
dargestellt (`anzeige.md` §3a): ein eingestellter Sollwert des Nutzers, kein
Zustand des Programms. Daneben steht, was er bedeutet — dass ein offener,
unbeaufsichtigter Rechner dann offen bleibt.

Damit ist auch die Frage nach einem Höchstwert beantwortet: **Das Maximum
ist das Schließen des Fensters**, und das sperrt immer.

### 3.3 Was nicht sperrt

**Der Wechsel zu einem anderen Fenster.** Wer die Safety Number am Telefon
vorliest, wechselt ständig weg. Eine Sperre, die dabei zuschlägt, wird nach
dem dritten Mal abgeschaltet — und dann schützt sie gar nicht mehr.

### 3.4 Was zusätzlich sperrt

- Ein Knopf **„Jetzt sperren"**, jederzeit erreichbar.
- Das Schließen des Fensters.
- **Der Wechsel in Bereitschaft oder Ruhezustand.** Dieser Punkt kam
  nachträglich dazu, und zwar aus §5.3: Das Ruhezustandsabbild ist eine
  Kopie des Arbeitsspeichers. Wer entsperrt in den Ruhezustand geht, hat
  sein Passwort auf der Platte. Vorher zu sperren ist das einzige Mittel,
  das dagegen hilft — Festnageln hilft ausdrücklich **nicht**.

**Und die Grenze dieses Punktes.** Alle drei Systeme melden den
bevorstehenden Wechsel (`WM_POWERBROADCAST` unter Windows, `PrepareForSleep`
über logind unter Linux, `NSWorkspaceWillSleepNotification` unter macOS).
Was keines davon zusagt, ist **genug Zeit danach**: Die Meldung kommt kurz
vorher, und wer sie zu lange aufhält, wird übergangen. Überschreiben ist
schnell, aber eine Zusage ist es nicht.

**Gewarnt werden und noch dazu kommen sind zweierlei.** Eine Meldung ohne
Aufschub nützt wenig — das Überschreiben liefe gegen ein System, das schon
wegdämmert. Windows wartet auf die Rückkehr des Rückrufs; unter Linux muss
der Aufschub eigens erbeten werden (`Inhibit` im Modus `delay`), und das
kann eine Polkit-Regel verweigern. Das Programm **weiß**, welcher der
beiden Fälle vorliegt, und behauptet nicht den einen, wenn der andere
gilt.

Es gilt deshalb als **Verbesserung des Regelfalls, nicht als Zusage**. Ein
abrupter Stromausfall, ein erzwungener Ruhezustand bei leerem Akku oder ein
zugeklappter Deckel im ungünstigsten Augenblick bleiben Fälle, in denen das
Passwort im Abbild landet.

**Und wo es heute schon gilt.**

| System | Weg | Stand |
|---|---|---|
| Windows | `PowerRegisterSuspendResumeNotification` | umgesetzt |
| Linux | `PrepareForSleep` von logind, mit Verzögerungssperre | umgesetzt, **die Meldung selbst nie beobachtet** |
| macOS | `IORegisterForSystemPower` | umgesetzt, **die Meldung selbst nie beobachtet** |

Auf beiden Läufern ist die Anmeldung geglückt und protokolliert: unter
Linux `angemeldet, Aufschub: ja`, unter macOS `IOKit hat angenommen`
(24.08.2026). Was auf keinem von beiden geprüft werden kann, bleibt
dasselbe: dass die Meldung im Betrieb ankommt. Dafür müsste ein Rechner
tatsächlich einschlafen.

Der Unterschied zwischen den ersten beiden Zeilen ist keine Förmlichkeit.
Unter Windows ist der ganze Weg auf einem laufenden System durchgegangen.
Unter Linux ist mehr als das Übersetzen belegt: Bei jedem Lauf der
Fortlaufprüfung wird die Anmeldung gegen das logind des Läufers versucht,
und das Protokoll sagt seit dem 21.08.2026 `angemeldet`, **Aufschub: ja**.
Verbindung zum Systembus, Anmeldung bei logind, das Abonnement auf
`PrepareForSleep` und die Verzögerungssperre sind also auf einem laufenden
System durchgegangen — nicht bloß übersetzt worden.

Aber: **Dass die Meldung ankommt und daraufhin gesperrt wird, hat nie
jemand gesehen.** Dafür müsste ein Rechner tatsächlich einschlafen, und
dieses Projekt hat keinen Linux-Rechner, nur einen Läufer.

Das steht hier und nicht nur im Quelltext, weil eine Zusage, die auf
einem System eingelöst und auf dem nächsten nur wahrscheinlich ist, ohne
diesen Satz eine Unwahrheit wäre.

**Woher die Zahlen für macOS stammen.** `IORegisterForSystemPower`
unterscheidet seine Meldungen über Konstanten aus Apples Kopfdateien, und
dieses Projekt hat keinen Mac. Sie aus dem Gedächtnis hinzuschreiben
hieße, eine Zusage auf einen geratenen Zahlenwert zu stellen — meldet er
nie, merkt es niemand; meldet er beim falschen Anlass, sperrt das Programm
mitten im Arbeiten.

Stattdessen liest sie der macOS-Läufer der Fortlaufprüfung dort vor, wo
sie verbindlich stehen, und **vergleicht sie bei jedem Lauf** mit denen im
Quelltext. Verschiebt Apple eine, wird dieser Lauf rot — und nicht
irgendwann jemandes Mac beim Zuklappen.

Ein Punkt dabei ist keine Feinheit: `kIOMessageCanSystemSleep` ist eine
**Frage** und keine Ankündigung — das System fragt, ob es in den
Leerlaufschlaf darf. Wer darauf sperrte, sperrte bei jeder Kaffeepause,
ohne dass der Rechner je einschläft.

Das Programm weiß den Stand zur Laufzeit: Die Anmeldung liefert einen
Fehler statt eines stillen Erfolgs, und der wird festgehalten. Angezeigt
wird er noch nicht — das folgt, sobald macOS steht, damit die Anzeige
nicht wochenlang einen Dauerhinweis zeigt, der danach nie wieder
erscheint.

Der Angreifer dahinter ist **A5** des Bedrohungsmodells — der mit der
ausgebauten Platte, nicht der mit dem laufenden Gerät. Das ist der Grund,
warum dieser Punkt überhaupt zählt: A5 gilt als teilweise abgewehrt, *weil*
Keyfiles durch das Passwort geschützt sind. Liegt das Passwort im
Ruhezustandsabbild derselben Platte, ist dieser Schutz für dieses Gerät
gegenstandslos. Dieser Punkt verkleinert das Fenster, er schließt es nicht.

## 4. Was die Oberfläche zeigt

### 4.1 Gesperrt ist ein Bildschirm, kein Hinweisfenster

Die Anwendung startet dort, wenn eine Schlüsseldatei existiert — und in der
Einrichtung, wenn nicht.

Nach `anzeige.md` §3 ist **gesperrt** der Zustand *Keine Aussage*: Es ist
der normale Ruhezustand, kein Fehler. Ein **falsches Passwort** ist der
Zustand *Fehler*.

### 4.2 Kein Zähler verbleibender Versuche

Es gibt **keine Begrenzung der Versuche**, und die Oberfläche zeigt auch
keine an.

Die Passwortableitung ist die Begrenzung: rund 0,4 Sekunden je Versuch bei
256 MiB (gemessen, Release-Bau). Wer die Schlüsseldatei hat, probiert
ohnehin offline und ohne diese Oberfläche — eine Sperre nach *n* Versuchen
hielte ihn nicht auf und schlösse vor allem den rechtmäßigen Nutzer aus.

### 4.3 Kein Hinweis darauf, *wie* falsch

Nicht „fast richtig", nicht die Länge, nicht die Zahl der übereinstimmenden
Zeichen. Die Meldung lautet, dass es nicht passte — mehr nicht.

### 4.4 Der Satz, der wiederholt gehört

Beim Entsperren steht derselbe Satz wie in der Einrichtung: **Wenn dieses
Passwort weg ist, ist alles weg.** Ihn zu wiederholen ist keine Redundanz,
sondern die Erinnerung an dem Punkt, an dem man sie braucht.

## 5. Der Weg des Passworts

### 5.1 Heute: durch die Webansicht

Zwischen Eingabefeld und Kern entstehen drei Kopien:

1. die JavaScript-Zeichenkette des Eingabefelds,
2. der JSON-Puffer der Tauri-Übergabe,
3. der `String`, den Rust daraus baut.

**Nur die dritte lässt sich überschreiben.** JavaScript-Zeichenketten sind
unveränderlich und werden eingesammelt, wann der Speicherbereiniger es für
richtig hält; auf den Übergabepuffer haben wir keinen Zugriff.

Was trotzdem gilt:

- Das Eingabefeld wird **unmittelbar nach dem Aufruf geleert**.
- Das Passwort landet in **keinem** Zustand, der den Aufruf überlebt.
- Es erscheint **nie** in einer Fehlermeldung und **nie** in einer Ausgabe.
- Es geht **nie** als Kommandozeilenargument (gilt seit Phase 2).

Das ist erheblich besser als Version 1. Es ist nicht dasselbe wie „sicher",
und dieses Dokument nennt es auch nicht so.

### 5.2 Das Ziel: ein natives Eingabefenster

**Der einzige echte Ausweg.** Ein Eingabefeld außerhalb der Webansicht gibt
die Zeichen unmittelbar in Speicher, den wir besitzen — die Kopien 1 und 2
entfallen ersatzlos.

Es ist plattformabhängig und nicht nebenbei zu haben; unter Windows käme
vermutlich `unsafe` ins Spiel, was `#![forbid(unsafe_code)]` widerspricht
und eine eigene, gekapselte Crate verlangte.

**Entwurfsauflage für heute:** Die Entsperrung wird so gebaut, dass die
Webansicht **ein** Aufrufer ist und nicht **der** Aufrufer. Der Kern nimmt
ein `Zeroizing<String>` entgegen und weiß nicht, woher es kommt. Ein
natives Fenster später auszutauschen berührt dann eine Datei.

### 5.3 Was auch dann bleibt

Ehrlichkeit über die Grenze:

- **Die Tastatureingabe des Betriebssystems** liegt außerhalb jeder
  Anwendung. Gegen einen Tastaturmitschnitt hilft kein Eingabefeld.
- **Auslagerung** kann Speicherseiten auf die Platte schreiben, bevor wir
  sie überschreiben. Dagegen hilft, die eine Seite mit dem Passwortpuffer im
  Arbeitsspeicher festzunageln (`VirtualLock` unter Windows, `mlock` unter
  POSIX). Das ist umsetzbar und gehört zum nativen Fenster dazu.
- **Der Ruhezustand nicht.** Diese Zeile stand hier zuerst zusammen mit der
  vorigen, als hülfe dasselbe Mittel gegen beides. Es ist umgekehrt: Das
  Ruhezustandsabbild ist eine Kopie des *physischen* Arbeitsspeichers, und
  Festnageln garantiert gerade, dass die Seite darin liegt. Wer sein Gerät
  in den Ruhezustand schickt, während das Programm entsperrt ist, hat das
  Passwort auf der Platte — festgenagelt oder nicht. Dagegen hilft nur, die
  Kopie vorher zu überschreiben, und genau deshalb gibt es die Frist aus §3.
- **Absturzabbilder** ebenso wenig. Ein Abbild enthält den Speicher des
  Prozesses; eine festgenagelte Seite ist davon nicht ausgenommen. Beide
  Systeme kennen einen Weg, einen Bereich davon auszunehmen. Ob wir ihn
  gehen, ist offen — hier steht vorerst nur, dass er nötig wäre.
- **Einfügen aus der Zwischenablage** legt das Passwort dort ab, wo jedes
  Programm es lesen kann. Die Oberfläche sagt das, verbietet es aber nicht:
  Wer einen Passwortverwalter benutzt, macht es richtig, nicht falsch.

## 6. Der Schlüsselbund des Betriebssystems

**Zurückgestellt.** Er würde die entsperrte Identität über einen Neustart
hinweg verfügbar machen, ohne dass jemand das Passwort tippt.

Der Grund für die Zurückstellung ist keine Bequemlichkeitsfrage: Er
verschiebt die Vertrauensgrenze. Was im Schlüsselbund liegt, bekommt jeder,
der Code als dieser Benutzer ausführen kann — **ohne** das Passwort. Für ein
Werkzeug, dessen Bedrohungsmodell den Zugriff auf den Rechner enthält
(`threat-model.md`), gibt das genau den Schutz auf, für den das Passwort da
ist.

Kommt er später, dann:

- **ausdrücklich wählbar**, nie voreingestellt,
- mit einem Satz daneben, der benennt, was aufgegeben wird,
- und niemals für die Schlüsseldatei selbst, sondern höchstens für den
  abgeleiteten Sitzungsschlüssel.

## 7. Mehrere Identitäten

Der Speicher kann mehrere führen (`keyfile-v2.md`): eine aus Version 1
übernommene neben einer neuen, oder eine namentliche neben einer anonymen.

- Entsperrt wird **eine** Identität, nicht „der Speicher".
- Welche zuletzt benutzt wurde, ist **kein Geheimnis** und darf als
  Einstellung gemerkt werden — sie ist die Voreinstellung der Auswahl.
- Ein Wechsel sperrt die vorherige. Zwei gleichzeitig entsperrte Identitäten
  wären zwei gleichzeitig offene Türen, und der Nutzen wäre gering.

## 8. Getroffene Entscheidungen

| Frage | Entscheidung |
|---|---|
| Passwort halten? | **Nein.** Die Sitzung hat kein Feld dafür |
| Was wird gehalten? | `Identity` und `ContactsKey`, beide `ZeroizeOnDrop` |
| Sperre nach Untätigkeit | **Ja**, ab der letzten Handlung |
| Voreinstellung | **15 Minuten** |
| Auswahl | Feste Liste, 1/5/15/30/60 Minuten + „bis zum Schließen" |
| Werte über 60 Minuten | **Nein** — nicht von „bis zum Schließen" zu unterscheiden |
| Sperre bei Fensterwechsel | **Nein** — sie würde abgeschaltet |
| Versuchszähler | **Nein** — die Passwortableitung ist die Begrenzung |
| Hinweis auf die Art des Fehlers | **Nein** |
| Schlüsselbund | **Zurückgestellt**; später nur ausdrücklich wählbar |
| Natives Eingabefenster | **Ziel für Phase 5**; heute so gebaut, dass es austauschbar bleibt |
| Laufender Zähler | **Nein** — er drängt und ist meist belanglos |
| Vorwarnung | **Ja**, dreistufig und **relativ** zur eingestellten Zeit |
| Farbe der Vorwarnung | **Gelb.** Rot hieße „gescheitert", und hier scheitert nichts |
| Sperre bricht Vorgänge ab | **Kann sie nicht.** Die Messung ruht stattdessen |

## 9. Wie die Sperre sich ankündigt

**Kein dauerhaft laufender Zähler.** Er drängt, und er ist die meiste Zeit
belanglos — dieselbe Regel wie überall: stör nur, wenn du wirklich etwas zu
sagen hast.

Stattdessen eine Staffel, die **mit der eingestellten Zeit skaliert**. Feste
Werte wie „zehn Minuten vorher" gingen bei einer Einstellung von einer
Minute nicht auf:

| Wann | Was |
|---|---|
| Nach ⅔ der Zeit | Ein kleines Zeichen, unaufdringlich |
| Nach ⁵⁄₆ der Zeit | Deutlicher, mit der verbleibenden Zeit im Klartext |
| Letzte 30 Sekunden | Ein Countdown, der herunterzählt |

Bei 15 Minuten heißt das: Hinweis nach 10 Minuten Untätigkeit, deutlicher
nach 12½, Countdown in der letzten halben Minute.

### 9.1 Gelb, nicht rot

Eine bevorstehende Sperre ist **kein Fehler**. Nach `anzeige.md` §3 heißt
Rot „der Vorgang ist gescheitert" — hier ist nichts gescheitert, es
geschieht genau das, was der Nutzer eingestellt hat.

Die Staffel ist deshalb **Warnung (gelb)**: „Es wurde geprüft, und etwas ist
zu beachten." Der Countdown in der letzten halben Minute darf deutlicher
sein, bleibt aber gelb.

### 9.2 Was als Tätigkeit zählt

Das ist die Stelle, an der die Staffel überhaupt erst Sinn ergibt.

| Zählt | Zählt nicht |
|---|---|
| Tastatureingabe | Bloße Mausbewegung |
| Klick | Ein Fenster im Vordergrund |
| Scrollen | |

**Warum Scrollen zählt und Mausbewegung nicht:** Wer einen Metadatenbefund
liest, klickt minutenlang nicht — und genau den soll die Vorwarnung
schützen. Wer nicht am Rechner sitzt, scrollt aber auch nicht. Bloße
Mausbewegung dagegen entsteht durch Erschütterung, ein Haustier oder ein
anderes Programm; sie als Tätigkeit zu werten hieße, die Sperre nie greifen
zu lassen.

### 9.3 Warum Tätigkeit einen eigenen Weg braucht

Die Befehle allein reichen als Lebenszeichen nicht. Wer zehn Minuten an
einer Nachricht schreibt oder einen langen Befund liest, löst in dieser Zeit
**keinen einzigen** aus — und säße plötzlich vor dem Sperrbildschirm,
obwohl er die ganze Zeit da war.

Deshalb gibt es `taetigkeit`: einen Befehl, der nichts zurückgibt und nur
die Messung neu beginnen lässt. Die Oberfläche ruft ihn **gedrosselt** auf,
höchstens alle fünf Sekunden. Ungedrosselt liefe bei jedem Tastendruck ein
Aufruf über die Brücke; fünf Sekunden Ungenauigkeit sind bei einer Frist von
Minuten belanglos.

Zwei Eigenschaften machen ihn ungefährlich:

1. **Er prüft zuerst die Frist.** Eine Meldung, die nach Ablauf eintrifft,
   weckt nichts auf — sonst käme ein Tastendruck einer Entsperrung ohne
   Passwort gleich.
2. **Im gesperrten Zustand bleibt er folgenlos.** Sonst hielte Tippen auf
   dem Sperrbildschirm die Frist offen, obwohl niemand angemeldet ist.

Das Nachfragen nach dem Stand zählt ausdrücklich **nicht** als Tätigkeit.
Die Oberfläche fragt im Sekundentakt, damit der Countdown stimmt; würde das
die Messung zurücksetzen, hielte die Sitzung sich durch das Anzeigen ihrer
eigenen Restzeit selbst offen.

## 10. Laufende Vorgänge

**Die Sperre kann einen laufenden Vorgang nicht unterbrechen.** Das ist
keine Entwurfsentscheidung, sondern eine Eigenschaft des Kerns:
`envelope::seal` ist **ein** Aufruf, der die Identität für seine ganze Dauer
als Referenz hält. Während er läuft, gibt es keinen Zeitpunkt, an dem etwas
anderes zugreifen könnte.

Daraus folgt die Regel, und sie ist einfacher als jede Schätzung:

> **Die Zeitmessung ruht, solange ein Vorgang läuft.** Sie beginnt von vorn,
> wenn er fertig ist.

Damit entfällt der Fall, der zunächst Sorge machte — ein Vorgang, der länger
dauert als die eingestellte Sperre, und eine Sperre, die mitten hinein
zuschlägt. Er kann nicht eintreten.

Es entfällt damit auch alles, was ihn hätte behandeln müssen: eine Schätzung
der Dauer, eine Vorwarnung darüber, und ein Häkchen „für diesen Vorgang
einmalig warten". Jedes davon wäre eine Entscheidung gewesen, die der Nutzer
hätte treffen müssen, ohne sie treffen zu können — wer weiß vorher, ob eine
Datei zwölf oder zwanzig Minuten braucht.

**Zu prüfen, sobald der Kern strömend verschlüsselt.** `envelope-v2.md`
sieht Chunk-Streaming vor. Wenn ein Vorgang dadurch in Abschnitte zerfällt,
zwischen denen etwas anderes laufen kann, stellt sich die Frage neu — und
die richtige Antwort wäre dann vermutlich, dass der Vorgang beim Start
festhält, was er braucht, und die Sperre ihn nichts angeht.

## 11. Offene Punkte

- Ob der Ruhezustand des Rechners eine Sperre auslösen soll. Technisch
  feststellbar, aber die Ereignisse unterscheiden sich je Plattform — erst
  entscheiden, wenn es mehr als eine gibt.
- Ob der Sperrbildschirm zeigt, **was** offen war, als gesperrt wurde
  („Sie waren bei Senden"). Bequem beim Zurückkommen; verrät aber jedem, der
  auf den Bildschirm sieht, woran gearbeitet wurde.
