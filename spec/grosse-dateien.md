# Cabrik Secure — Große Dateien

**Status:** Verbindlich · Phase 5, Dokument 10 — geschrieben vor der Umsetzung
**Setzt voraus:** `envelope-v2.md` (§8, §12), `metadata.md` (§4.2.9),
`shredding.md` (§3), `entsperrung.md`

---

## 1. Das Versprechen und der Stand

`spec/README.md` nennt für v2 einen Speicherbedarf von **konstant ~256 KiB**,
gleich wie groß die Datei ist. Das Format trägt das: Es verschlüsselt in
Blöcken von 64 KiB, die Länge steht vorab im Header und kommt aus einem
`stat()` (`envelope-v2.md` §8.2), und für das Entschlüsseln in eine Datei
ist die Regel schon festgelegt — erst daneben, nach der Prüfung umbenennen
(§8.4).

Die Umsetzung löst das Versprechen nicht ein:

| Vorgang | Was heute im Speicher liegt |
|---|---|
| Senden | die Datei, ihre bereinigte Fassung und der Envelope — **dreimal** |
| Prüfen vor dem Senden | jede Datei ganz, auch wenn nur ihr Kopf gebraucht wird |
| Empfangen | der Klartext, in der Sitzung, bis er abgelegt oder verworfen wird |

Bei einem Video von 3 GB sind das beim Senden um 9 GB. Dazu läuft
`envelope_oeffnen` ohne `async` — unter Windows also auf dem Faden, der
das Fenster zeichnet. Solange eine große Datei gelesen und entschlüsselt
wird, steht das Fenster still.

**Das Format ändert sich nicht.** Die Envelopes bleiben Byte für Byte
dieselben, die Testvektoren auch. Was 0.1.x geschrieben hat, liest die
neue Fassung, und umgekehrt.

## 2. Ziel und Grenze

| Vorgang | heute | danach |
|---|---|---|
| Senden ohne Bereinigung (Original gewählt, Format unbekannt) | 3× | konstant |
| Senden eines Videos (MP4/MOV, MKV/WebM, AVI) | 3× | konstant (§5) |
| Senden anderer Formate, solange der Speicher reicht | 3× | etwa 2× — Datei und bereinigte Fassung; der Envelope strömt |
| Senden anderer Formate, wenn er nicht reicht | 3× — Auslagern oder Absturz | das Angebot, unbereinigt zu senden (§7) |
| Prüfen vor dem Senden: Video, unbekanntes Format | 1× | konstant |
| Empfangen bis 64 MiB | 1× | 1× wie heute — mit Vorschau |
| Empfangen über 64 MiB | 1× | konstant, in zwei Durchgängen (§6) |

„Konstant“ heißt: einige Blöcke zu 64 KiB und das Gerüst aus §5 —
unabhängig von der Dateigröße.

Die Grenze von 64 MiB beim Empfangen trennt, was angesehen wird, von dem,
was abgelegt wird. Unterhalb bleibt alles, wie es ist: Text und Bilder
werden im Programm gezeigt, die Metadaten der empfangenen Datei genannt.

## 3. Kern: zwei neue Wege, dieselben Bytes

### 3.1 Verschlüsseln aus einer Quelle

Neben `seal` tritt ein Weg, der aus einer Quelle liest und in ein Ziel
schreibt:

```
seal_strom(suite, empfaenger, passwort, quelle: Read, laenge: u64,
           absender, optionen, zufall, ziel: Write, melden)
```

- `laenge` steht vorab fest — aus dem Dateisystem oder, nach einer
  Bereinigung, aus ihrem Ergebnis. Liefert die Quelle weniger oder mehr
  Bytes, **MUSS** abgebrochen werden: Die Datei hat sich beim Lesen
  verändert, und ihr Header nennte eine falsche Länge.
- Im Speicher liegen ein Block Klartext, ein Block Ciphertext und der
  fortlaufende Hash für das Transkript (`envelope-v2.md` §9).
- **Byte-Gleichheit ist Pflicht:** Bei gleichem Zufall **MUSS**
  `seal_strom` dieselben Bytes liefern wie `seal`. Geprüft über alle
  Verschlüsselungsvektoren (`test-vectors.md` §3). Damit gibt es keinen
  zweiten Format-Schreiber, nur einen zweiten Weg zum selben Ergebnis.

### 3.2 Entschlüsseln in eine zurückgehaltene Ablage

`envelope-v2.md` §8.4 verlangt, dass die API die Zurückhaltung
**erzwingt**, statt sie zu beschreiben. Deshalb schreibt der Kern nicht in
ein beliebiges `Write`, sondern in eine Ablage mit drei Handgriffen:

```
schreiben(block)   — ungeprüfter Klartext, noch nicht freigegeben
freigeben()        — als Schritt 11 von envelope-v2 §12, nach Signatur und Trust Store
verwerfen()        — bei jedem Fehler, auch nach dem letzten Block
```

`freigeben` ruft allein der Kern auf, und nur, wenn Trailer und Signatur
geprüft sind. Ein Aufrufer kann ungeprüften Klartext damit nicht
versehentlich als fertig behandeln.

Zurück kommt der Bericht wie bei `open` — Dateiname, Art, Größe,
Zeitpunkt, Signaturlage —, aber ohne Klartext, dazu der **Fingerabdruck
des Envelopes**: SHA-256 über alle gelesenen Bytes. Ihn braucht §6.

Byte-Gleichheit wie oben: Für jeden Vektor **MUSS** die freigegebene
Ablage genau den Klartext von `open` enthalten.

## 4. Ablage: was auf die Platte kommt

**Envelopes** entstehen wie heute neben ihrem Ziel und werden am Ende
umbenannt (`cabrik-ablage`: erst daneben, dann umbenennen) — nur strömend
statt aus einem Puffer. Unter dem endgültigen Namen steht nie ein halber
Envelope. Bricht etwas ab, wird die Zwischendatei gelöscht.

**Klartext beim Empfangen** entsteht neben dem Ort, den der Nutzer
gewählt hat, und nirgends sonst — nicht in `%TEMP%`, nicht im
Programmordner (`shredding.md` §3.2: Entschlüsseltes schreibt der Nutzer
bewusst, und nichts landet unbemerkt auf einem anderen Datenträger). Die
Zwischendatei trägt einen erkennbaren Namen
(`.<ziel>.cabrik-teil`). Schlägt die Prüfung fehl, wird sie gelöscht —
kein Teilergebnis.

**Nach einem Absturz** bleibt sie womöglich liegen und enthält dann
ungeprüften Klartext — an dem Ort, den der Nutzer gewählt hat, aber unter
fremdem Namen. Das Programm räumt sie beim nächsten Ablegen in denselben
Ordner weg und sagt es.

## 5. Bereinigen ohne die ganze Datei

### 5.1 Warum es bei Video geht

Die drei Video-Bereiniger verschieben kein Byte (`metadata.md` §4.2.9):
Sie ersetzen Blöcke durch Platzhalter gleicher Größe und leeren Felder an
fester Stelle. Die bereinigte Datei ist also die ursprüngliche mit
einigen **Flicken** — Stelle und neue Bytes. Die Dateilänge bleibt
gleich; die Bereiniger prüfen das selbst (`debug_assert`).

Mit einer Flickliste lässt sich strömen: die Datei der Reihe nach lesen,
an den Flickstellen die neuen Bytes einsetzen, verschlüsseln. Eine
bereinigte Fassung entsteht dabei weder im Speicher noch auf der Platte —
das Zwischenprodukt aus `shredding.md` §3.2 Punkt 2 gibt es bei Video
dann gar nicht mehr.

### 5.2 Woher die Flickliste kommt: das Gerüst

Die Bereiniger arbeiten auf `&[u8]`, also auf der ganzen Datei. Sie
umzuschreiben hieße, zu jedem Format eine zweite Antwort auf die Frage
„was ist hier Metadatum“ zu geben. Das ist dieselbe Falle, die
`datei_pruefen` vermeidet, indem es `strip` selbst laufen lässt: **Eine
Vorhersage, die nicht der Vorgang selbst ist, läuft ihm irgendwann
davon.**

Stattdessen bekommt der unveränderte Bereiniger eine kleine Datei:

1. **Gerüst bauen.** Die Datei der Reihe nach lesen und alles behalten
   außer dem Inhalt der Nutzdatenblöcke — `mdat` bei MP4/MOV, `Cluster`
   bei Matroska, `movi` bei AVI. Die Größenangaben der umschließenden
   Blöcke werden angepasst, damit das Gerüst eine gültige Datei ist. Wo
   jeder behaltene Abschnitt in der Originaldatei lag, wird notiert.
2. **Denselben `strip` laufen lassen** — auf dem Gerüst.
3. **Vergleichen.** Wo sich Gerüst und bereinigtes Gerüst unterscheiden,
   liegen die Flicken. Ihre Stellen werden in die der Originaldatei
   zurückgerechnet.

Auch `datei_pruefen` und die Inspektion laufen über das Gerüst; die Funde
sind damit dieselben wie beim Senden.

**Wann der Weg nicht gilt.** Weist der Bereiniger das Gerüst zurück, oder
fällt eine Flickstelle in eine Größenangabe, die beim Bauen angepasst
wurde, wird das Gerüst für diese Datei verworfen. Dann gilt §7 — als
gewöhnliche Datei, nicht als Fehler.

### 5.3 Verworfen

- **Ein Speicherabbild** (`mmap`): Die Bereiniger blieben unverändert,
  und das Betriebssystem läse nur, was berührt wird. Aber es braucht
  `unsafe`, und Fehler beim Lesen kommen nicht als Fehler an: Zieht jemand
  den USB-Stick mitten im Verschlüsseln, oder kürzt unter Linux und macOS
  ein anderes Programm die Datei, stürzt das Programm ab, statt es zu
  sagen.
- **Eigene Kopfleser je Format:** die zweite Antwort aus §5.2.

### 5.4 Pflichtprüfung

Für jede Testdatei und jeden Eintrag des Fuzz-Korpus der drei Formate
**MUSS** gelten: Die Flickliste aus dem Gerüst, auf die Originaldatei
angewandt, ergibt **Byte für Byte** das, was `strip` auf der ganzen Datei
liefert — oder der Gerüst-Weg lehnt ab. Ein eigenes Fuzz-Ziel prüft
dasselbe an zufälligen Eingaben.

Weitere Formate, deren Bereiniger kein Byte verschiebt — WAV mit `JUNK`,
FLAC mit `PADDING` —, kommen später auf demselben Weg dazu. Zuerst die
drei, bei denen große Dateien die Regel sind.

## 6. Empfangen großer Dateien

Heute sind Öffnen und Ablegen zwei Entscheidungen: Wer eine Nachricht von
einem Unbekannten öffnet, erfährt erst, von wem sie ist, und entscheidet
dann, ob sie auf die Platte soll. Der Klartext wartet derweil in der
Sitzung. Bei einer großen Datei ist genau das Warten im Speicher das
Problem.

**Zwei Durchgänge**, ab 64 MiB — entschieden am 28.10.2026.

1. **Beim Öffnen** wird ganz entschlüsselt und geprüft, der Klartext aber
   verworfen, Block für Block. Es bleiben der Bericht und der
   Fingerabdruck des Envelopes (§3.2).
2. **Beim Ablegen** wird ein zweites Mal entschlüsselt, diesmal in die
   zurückgehaltene Ablage am gewählten Ort. Freigegeben wird nur, wenn der
   Fingerabdruck derselbe ist wie beim ersten Durchgang — sonst hat sich
   die Datei dazwischen verändert, und es wird verworfen.

Der Preis: Die Datei wird zweimal gelesen und entschlüsselt. Auf einer SSD
sind das einige Sekunden je Gigabyte und Durchgang; gemessen wird es beim
Bauen.

**Verworfen:** bei großen Dateien zuerst den Ort wählen lassen, dann in
einem Durchgang entschlüsseln. Das spart den zweiten Durchgang, kehrt
aber die Reihenfolge um — man legt ab, bevor man weiß, von wem es kommt.

**Metadaten der empfangenen Datei.** Bei Video entstehen sie im ersten
Durchgang aus dem Gerüst (§5.2). Bei anderen Formaten über 64 MiB sagt
der Bildschirm, dass sie nicht angesehen wurden — ausdrücklich, nicht
durch Weglassen.

`envelope_oeffnen` wird `async`. Das Fenster darf nicht stehen, gleich
wie groß die Datei ist.

## 7. Was nicht stückweise geht

PDF, Office, ODF, ZIP und die Bildformate bauen beim Bereinigen um und
verschieben Bytes. Dafür liegt die Datei weiter ganz im Speicher, samt
bereinigter Fassung — der Envelope aber nicht mehr (§3.1).

**So weit der Arbeitsspeicher reicht** — entschieden am 28.10.2026, statt
einer festen Grenze. Gemessen wird wie bei der Passwortableitung
(`keyfile-v2.md` §4.2): **drei Viertel des gerade freien Speichers**, mit
`cabrik_speicher::freier_arbeitsspeicher`. Als Bedarf gilt das
**Dreifache** der Datei — sie selbst, die bereinigte Fassung und was der
Bereiniger unterwegs braucht, bei ZIP und Office die ausgepackten Teile.
Der Speichertest (§9) misst die Spitze je Format; liegt sie darüber, wird
der Faktor angehoben, nicht die Grenze.

**Gemessen wird zweimal:** beim Prüfen vor dem Senden, damit der
Bildschirm es sagen kann, und unmittelbar bevor die Datei bereinigt wird
— dazwischen können andere Programme Speicher belegt haben. Reicht er dann
nicht mehr, wird diese Datei **nicht** gesendet, auch nicht unbereinigt,
und der Bericht sagt es. Unbereinigt geht nur, wer es gewählt hat. In
einem Stapel wird Datei für Datei gemessen; was die vorige belegt hat, ist
dann wieder frei.

**Reicht der Speicher nicht,** sagt es der Prüfbildschirm **vor** dem
Senden und bietet an, das Original unbereinigt zu senden — dieselbe Wahl,
die es heute schon je Datei gibt. Wer das nicht will, nimmt die Datei aus
der Auswahl. **Verworfen:** solche Dateien abzulehnen. Ein großes Archiv
ließe sich dann gar nicht senden, obwohl sein Inhalt verschlüsselt
ankäme.

**Ist der freie Speicher nicht zu ermitteln,** gilt er bis 1 GiB Dateigröße
als ausreichend und darüber als zu knapp. Anders als bei der Ableitung
lässt sich hier nicht „fehlbar anlegen“: Die Bereiniger belegen ihren
Speicher selbst, und ein misslungenes Anlegen beendet das Programm.

**Ein Vorbehalt, der heute schon gilt.** Je voller der Speicher, desto
eher lagert das Betriebssystem aus — und eine ausgelagerte bereinigte
Fassung liegt im Klartext in der Auslagerungsdatei (`entsperrung.md`
§5.3). Das trifft heute jede gesendete Datei. Mit diesem Dokument entfällt
es für Video und unbekannte Formate; für die übrigen bleibt es.

Für Dateien unbekannten Formats ändert sich nichts an der Aussage: Das
Programm kann sie nicht prüfen, der Inhalt ist trotzdem verschlüsselt
(`metadata.md` §8). Sie strömen ohne Grenze.

## 8. Fortschritt

Lesen, Verschlüsseln und Schreiben werden ein Durchgang. Der Balken zählt
dann einen Schritt „Verschlüsseln“ in Bytes. Das Gerüst zu bauen liest die
Datei nur bis zu den Nutzdaten und springt darüber; es ist gegenüber dem
Verschlüsseln kurz.

## 9. Prüfung

- **Kern:** Byte-Gleichheit beider Wege (§3); abgeschnittene,
  verlängerte und während des Lesens veränderte Quellen; ein
  manipulierter Trailer lässt die Ablage verworfen und nichts
  freigegeben zurück.
- **Bereinigung:** §5.4, an Testdateien und im Fuzzing.
- **Speicher:** Ein Test verschlüsselt und entschlüsselt eine erzeugte
  Datei von 1 GiB und prüft die Spitze des Arbeitsspeichers gegen eine
  feste Schranke — unter Windows `PeakWorkingSetSize`, unter Linux
  `VmHWM`, unter macOS `ru_maxrss`. Ohne ihn wäre „konstant“ eine
  Behauptung.
- **Einmal von Hand:** ein echtes Video über 3 GB, gesendet und
  empfangen, auf einem Rechner mit wenig Speicher.

## 10. Entschiedene Punkte

Beide am 28.10.2026:

1. **Empfangen über 64 MiB** (§6): zwei Durchgänge. Erst sehen, von wem
   es kommt, dann ablegen — auch wenn es einen Durchgang mehr kostet.
2. **Bereinigen im Speicher** (§7): so weit der freie Arbeitsspeicher
   reicht, gemessen wie bei der Passwortableitung, statt einer festen
   Grenze. Reicht er nicht, bietet der Prüfbildschirm an, unbereinigt zu
   senden; abgelehnt wird nicht.

## 11. Reihenfolge der Umsetzung

Jeder Schritt ist für sich nützlich und wird für sich geprüft:

1. Kern: `seal_strom` und die zurückgehaltene Ablage (§3)
2. Ablage: Envelope und Klartext neben dem Ziel (§4)
3. Senden ohne Bereinigung und Empfangen (§6) strömen — damit sind
   unbekannte Formate und gewählte Originale schon konstant
4. Das Gerüst für MP4/MOV, Matroska, AVI (§5)
5. Die Speichergrenze fürs Bereinigen und der Prüfbildschirm (§7)
6. Der Speichertest (§9) — als Erstes geschrieben, als Letztes grün

## 12. Was sich nicht ändert

- das Format, die Testvektoren, alte Envelopes
- Pipe-Eingaben der CLI: Sie puffern weiter im Speicher, höchstens
  256 MiB (`envelope-v2.md` §8.2)
- dass Bereinigen eine bewusste Entscheidung bleibt (`metadata.md` §8)
