# Cabrik Secure — Entsperrung und Sitzung

**Status:** Entwurf · Phase 4, Dokument 9
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
- **Auslagerung und Ruhezustand** können Speicherseiten auf die Platte
  schreiben, bevor wir sie überschreiben. Dagegen hilft, die eine Seite mit
  dem Passwortpuffer im Arbeitsspeicher festzunageln (`VirtualLock` unter
  Windows, `mlock` unter POSIX). Das ist umsetzbar und gehört zum nativen
  Fenster dazu.
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

## 9. Offene Punkte

- Ob der Ruhezustand des Rechners eine Sperre auslösen soll. Technisch
  feststellbar, aber die Ereignisse unterscheiden sich je Plattform — erst
  entscheiden, wenn es mehr als eine gibt.
- Ob die verbleibende Zeit sichtbar mitlaufen soll oder nur der Zustand.
  Ein Zähler, der ständig läuft, kann drängen; einer, der fehlt, überrascht.
- Ob das Sperren laufende Vorgänge abbricht oder abwartet. Beim
  Verschlüsseln einer großen Datei ist beides unangenehm.
