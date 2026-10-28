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

```
versiegeln_beginnen(suite, empfaenger, passwort, laenge, absender,
                    optionen, zufall)          → (Kopf, Versiegelung)
Versiegelung::weiter(klartext, aus)            — in beliebigen Stücken
Versiegelung::abschliessen(aus)                — Füllung, letzter Block, Trailer
```

**Kein `std::io` im Kern.** `cabrik-core` soll später per UniFFI nach
Swift und Kotlin und kennt deshalb keine Dateien (`lib.rs`, `stream.rs`).
Er bekommt Stücke und hängt Bytes an; das Lesen der Quelle und das
Schreiben des Ziels übernimmt die aufrufende Schicht (§4). *Nachtrag vom
29.10.2026:* Der Entwurf skizzierte hier `Read` und `Write` — das hätte
diesem Grundsatz widersprochen.

- `laenge` steht vorab fest — aus dem Dateisystem oder, nach einer
  Bereinigung, aus ihrem Ergebnis. Kommen weniger oder mehr Bytes,
  **MUSS** abgebrochen werden: Die Datei hat sich beim Lesen verändert,
  und ihr Header nennte eine falsche Länge.
- Im Speicher liegen ein Block Klartext, ein Block Ciphertext und der
  fortlaufende Hash für das Transkript (`envelope-v2.md` §9).
- **Ein Format-Schreiber, nicht zwei.** `seal` ist selbst nichts anderes
  als Anfang, Blöcke und Abschluss. Dass es dabei Byte für Byte geblieben
  ist, was es vorher schrieb, hält eine Rückschau fest: eine Prüfsumme über
  81 Envelopes aus dem Stand davor (`tests/formatfreeze.rs`). Dass die
  Stückelung nichts ändert, prüft `tests/stroemen.rs` mit Stücken von 1
  Byte bis 1 MiB.

### 3.2 Entschlüsseln in eine zurückgehaltene Ablage

```
oeffnen_beginnen(oeffner, anfang, signatur_verlangt)
                                     → (Oeffnung, verbraucht)
Oeffnung::weiter(daten, ablage)      — in beliebigen Stücken
Oeffnung::abschliessen(ablage)       → Bericht
```

`anfang` sind die ersten Bytes, höchstens `KOPF_HOECHSTENS` = 2 MiB. Mehr
braucht kein Kopf, den dieses Format hervorbringen kann: 255 Kapseln zu
höchstens 4096 Bytes, ein Header aus höchstens acht TLV-Feldern. Hier
laufen die Schritte 1 bis 7 aus `envelope-v2.md` §12, die Signaturpflicht
eingeschlossen — vor dem ersten Block.

`envelope-v2.md` §8.4 verlangt, dass die API die Zurückhaltung
**erzwingt**, statt sie zu beschreiben. Deshalb schreibt der Kern nicht in
ein beliebiges Ziel, sondern in eine Ablage mit drei Handgriffen:

```
schreiben(block)        — ungeprüfter Klartext, noch nicht freigegeben
freigeben(&Geprueft)    — nach Blöcken, Trailer, Signatur und Füllung
verwerfen()             — bei jedem Fehler, auch nach dem letzten Block
```

`freigeben` verlangt einen **Beleg**, und den stellt allein der Kern aus,
in `abschliessen`, wenn alles geprüft ist. Ein Aufrufer kann ungeprüften
Klartext damit nicht freigeben, auch nicht aus Versehen — er bekommt den
Beleg nie in die Hand. `verwerfen` ruft der Kern bei jedem Fehler, den er
sieht; bricht der Aufrufer selbst ab, behandelt eine Ablage, die ohne
Freigabe fallen gelassen wird, das wie verworfen.

**Fehler der Ablage** kommen getrennt von denen des Envelopes zurück
(`Oeffnungsfehler::Ablage`). Die Fehlerkodes sind festgeschrieben
(`test-vectors.md` §7), und eine volle Platte ist keine Eigenschaft der
Datei.

Zurück kommt der Bericht wie bei `open` — Dateiname, Art, Größe,
Zeitpunkt, Signaturlage —, aber ohne Klartext, dazu der **Fingerabdruck
des Envelopes**: das Transkript aus `envelope-v2.md` §9, auch ohne
Signatur gebildet. Es bindet Prolog, Header und jeden Block; zwei
Durchgänge über denselben Envelope ergeben denselben Wert, zwei
verschiedene Envelopes nie. Ihn braucht §6.

Auch hier ein Weg, nicht zwei: `open` ist Anfang, Blöcke und Abschluss in
eine Ablage im Speicher. Für jeden Fall **MUSS** die blockweise Öffnung
denselben Klartext oder denselben Fehlerkode liefern wie `open`; das Ziel
`envelope_bloecke` prüft es nachts an gestörten Envelopes.

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
fremdem Namen. Beim nächsten Ablegen **unter demselben Namen** räumt das
Programm sie weg und sagt es. *Nachtrag vom 29.10.2026:* Der Entwurf
sagte „in denselben Ordner“. Reste anderer Ziele bleiben aber liegen —
ob dort gerade ein anderer Vorgang schreibt, weiß die Ablage nicht, und
fremde Dateien zu löschen ist kein Aufräumen.

**Die Zwischendatei** (`cabrik_ablage::Zwischendatei`) gilt für alles, was
das Programm schreibt — Envelopes, Klartext, Schlüssel- und Kontaktdatei:

- Sie wird nur **neu** angelegt, nie über etwas Bestehendes. Bis Oktober
  2026 hieß sie `<ziel ohne Endung>.tmp` und wurde mit `File::create`
  angelegt; beim Speichern von `bericht.pdf` wäre eine `bericht.tmp` des
  Nutzers überschrieben und danach umbenannt worden.
- Vor dem Umbenennen wird sie auf die Platte gezwungen. Sonst stünde nach
  einem Stromausfall womöglich eine leere Datei unter dem Zielnamen — und
  wer einen USB-Stick gleich nach „fertig“ abzieht, hätte dasselbe.
- Klartext geht ohne Zwischenpuffer hinein; ein Puffer hielte eine Kopie,
  die sich nicht überschreiben ließe.
- Sie erfüllt die Ablage aus §3.2: umbenannt wird mit dem Beleg des Kerns,
  und nur als neue Datei.

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

- **Kern:** die Rückschau und die Stückelung (§3.1); abgeschnittene,
  verlängerte und während des Lesens veränderte Quellen; ein
  manipulierter Block oder Trailer lässt die Ablage verworfen und nichts
  freigegeben zurück; das Fuzz-Ziel `envelope_bloecke` (§3.2).
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

1. Kern: blockweise Versiegelung und die zurückgehaltene Ablage (§3) —
   erledigt am 29.10.2026
2. Ablage: Envelope und Klartext neben dem Ziel (§4) — erledigt am
   29.10.2026
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
