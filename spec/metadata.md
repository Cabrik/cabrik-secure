# Cabrik Secure — Metadaten-Erkennung und -Bereinigung

**Status:** Entwurf · Phase 1, Dokument 6 von 7
**Setzt voraus:** `threat-model.md`, `envelope-v2.md`

---

## 1. Zwei getrennte Probleme

Es lohnt sich, sauber zu trennen, denn nur eines davon ist ein Dateiformat-Problem.

| | Problem | Lösung |
|---|---|---|
| **A** | Der **Envelope** verrät Dateiname, Größe, Zeitpunkt, Absender | vollständig gelöst in `envelope-v2.md` §3, §7 |
| **B** | Die **Nutzdatei** trägt eingebettete Metadaten (EXIF, Autor, GPS) | nur teilweise lösbar — dieses Dokument |

Problem A war in v1 gravierender als B und blieb unbemerkt: Der Dateiname
`Kuendigung_Arbeitgeber_vertraulich.pdf` stand im Klartext in der `.enc`-Datei,
lesbar ohne jeden Schlüssel. Kein noch so gründliches EXIF-Strippen hätte das
aufgewogen.

Problem B lässt sich **grundsätzlich nicht vollständig** lösen: Metadaten aus
einem Format zu entfernen, das man nicht versteht, ist unmöglich. Entscheidend
ist daher nicht die Abdeckung, sondern der ehrliche Umgang mit Lücken.

## 2. Der Kernfehler in v1

```python
else:
    import shutil
    shutil.copy2(path, out_path)
```

Für jedes nicht unterstützte Format kopiert v1 die Datei — und meldet keinen
Fehler. Der Nutzer klickt „Metadaten strippen", bekommt eine `.clean`-Datei und
schließt daraus, die Datei sei bereinigt.

`shutil.copy2` **erhält** zusätzlich Zugriffs- und Änderungszeit. Die einzige
Metadatenart, die auch bei unbekannten Formaten entfernbar gewesen wäre, wurde
also aktiv mitgenommen.

**Anforderung an v2:** Für ein Format, das nicht verstanden wird, wird
Sauberkeit **niemals** behauptet.

## 3. Fähigkeitsmodell

Jede Operation liefert einen von drei Zuständen — nie einen Wahrheitswert.

```
enum StripResult {
    Complete { removed: Vec<Finding> },
    Partial  { removed: Vec<Finding>, remaining: Vec<Finding>, reason: String },
    Unknown  { format_hint: Option<String> },
}
```

| Zustand | Bedeutung | Anzeige |
|---|---|---|
| `Complete` | Alle bekannten Metadatenträger des Formats behandelt | „Bereinigt" — grün |
| `Partial` | Bereinigt, aber Reste benannt | „Teilweise bereinigt: *…*" — Warnung |
| `Unknown` | Format nicht verstanden | „Unbekanntes Format — **keine Aussage möglich**" — neutral, **nie grün** |

`Complete` heißt „alle *bekannten* Träger" — nicht „garantiert metadatenfrei".
Die Oberfläche **MUSS** diesen Unterschied im Hilfetext benennen.

## 4. Formatabdeckung in 2.0

| Format | v1 | v2 | Behandlung |
|---|---|---|---|
| JPEG | teilweise | `Complete` | EXIF, IPTC, XMP, ICC, Kommentarsegmente |
| PNG | teilweise | `Complete` | `tEXt`, `iTXt`, `zTXt`, `eXIf`, `tIME`, Palette **erhalten** |
| TIFF | teilweise | `Complete` | Die Datei wird **neu gebaut**, siehe §4.2.7. Alle Metadatenmarken, Exif-/GPS-Unterverzeichnisse, `SubIFDs`, Vorschau-Verzeichnisse |
| WebP | teilweise | `Complete` | `EXIF`, `XMP `, `ICCP` Chunks — **und die Merkmalsbits in `VP8X`** |
| GIF | ✗ | `Complete` | Kommentar-Extensions und fremde Anwendungs-Extensions. `NETSCAPE` bleibt |
| BMP | ✗ | `Complete`/`Partial` | Anhängsel hinter den Bilddaten; verwiesenes Farbprofil als `Critical` |
| **HEIC/HEIF** | ✗ | `Complete` | EXIF- und XMP-Items im Meta-Box |
| **AVIF** | ✗ | `Complete` | dito |
| **SVG** | ✗ | `Partial` | `<metadata>`, `<title>`, `<desc>`, Editor-Namespaces. Bleibt `Partial`, weil SVG beliebiges XML und sogar Skripte tragen kann |
| PDF | teilweise | `Partial` | DocInfo **und** XMP (v1 nur DocInfo). Bleibt `Partial`: eingebettete Schriften, Anhänge, JavaScript, inkrementelle Änderungshistorie |
| **DOCX** | teilweise | `Complete`/`Partial` | `core.xml`, **`app.xml`**, **`custom.xml`**, **`customXml/`**, `settings.xml` (rsid), Vorschaubild, **Metadaten eingebetteter Bilder**. Kommentare und Revisionen werden gemeldet, nicht entfernt — siehe §4.2.1 |
| **XLSX** | ✗ | `Complete`/`Partial` | dito |
| **PPTX** | ✗ | `Complete`/`Partial` | dito |
| **ODT/ODS/ODP** | ✗ | `Complete`/`Partial` | `meta.xml`, **`settings.xml`** (Druckername!), Bearbeitungsdauer und -zyklen, Vorlagenpfad, `Thumbnails/`, eingebettete Bilder. Kommentare und Revisionen wie bei OOXML |
| **ZIP** | ✗ | `Partial` | Zeitstempel normalisiert, enthaltene Dateien bereinigt. **Die Eintragsnamen bleiben** — sie sind das Archiv |
| Alles andere | stille Kopie | **`Unknown`** | keine Aussage |

Fett = neu in 2.0.

### 4.1 Warum PDF `Partial` bleibt

PDF ist kein Dateiformat, sondern ein Container mit Objektgraph. Bereinigt
werden DocInfo und XMP; nicht sicher entfernbar sind eingebettete Schriften
(die Lizenz- und Herstellerangaben tragen), Anhänge, JavaScript und die
inkrementelle Änderungshistorie, in der frühere Fassungen vollständig erhalten
sein können.

Ein PDF „vollständig bereinigt" zu nennen wäre falsch. v2 nennt die Reste
konkret, statt sie zu verschweigen.

### 4.2 Warum OOXML in v1 unvollständig war

v1 setzte nur `core_properties` (`docProps/core.xml`). Unangetastet blieben:

- `docProps/app.xml` — Firmenname, Vorlage, Bearbeitungszeit, Seitenzahl
- `docProps/custom.xml` — beliebige benutzerdefinierte Felder
- `word/settings.xml` — `rsid`-Werte, die Bearbeitungssitzungen unterscheidbar
  machen
- Kommentare und nachverfolgte Änderungen im Dokumentkörper

Der Firmenname in `app.xml` ist in der Praxis oft die verräterischste Angabe
überhaupt.

### 4.2.1 Was beim Umsetzen dazukam

Die Liste oben stammte aus der Durchsicht von v1. Beim Prüfen **echter**
Word-Dateien kamen vier weitere Fundstellen hinzu, die kein Entwurf auf dem
Papier hergegeben hätte:

| Fundstelle | Warum sie zählt |
|---|---|
| `docProps/thumbnail.*` | Eine **zweite Kopie des Dokumentinhalts** als Bild. Word legt sie unaufgefordert an. |
| `customXml/itemProps*.xml` | Trägt eine **feste GUID**, in jedem aus derselben Vorlage erzeugten Dokument dieselbe. Sie verknüpft Dokumente über Empfänger hinweg, auch wenn sonst alles bereinigt wurde. |
| `docProps/core.xml` → `dc:description` | Die interne Notiz. In Word unter „Kommentare" geführt und beim Weitergeben regelmäßig vergessen. |
| `word/media/*` | Eingebettete Bilder bringen ihr **eigenes EXIF** mit, samt GPS. v1 kannte diesen Fall gar nicht. |

Eingebettete Bilder werden deshalb rekursiv durch dieselbe Bereinigung
geschickt wie eine einzelne Bilddatei. Der gemeldete Ort führt bis zum
Fundort: `OOXML:word/media/image1.jpg → EXIF:GPSInfo`.

### 4.2.2 Kommentare und Revisionen bleiben — und deshalb `Partial`

**Abweichung von Stand 3.** Dort stand für OOXML `Complete`, einschließlich
Kommentaren und nachverfolgten Änderungen. Beim Umsetzen zeigte sich, dass das
zwei verschiedene Dinge vermengt.

Eine nachverfolgte Löschung zu entfernen heißt, sie **anzunehmen oder zu
verwerfen** — beides verändert den sichtbaren Text. Das ist keine
Metadatenbereinigung mehr, sondern eine inhaltliche Entscheidung. Dieselbe
Trennlinie gilt bereits für zugeschnittene Bilder (§7.2); sie hier anders zu
ziehen wäre inkonsequent.

**Regel:** Kommentare, nachverfolgte Änderungen und zugeschnittene Bilder
werden als `Critical` gemeldet und **standardmäßig nicht** entfernt. Enthält
ein Dokument eines davon, ist das Ergebnis `Partial` mit benannten Resten. Ein
Dokument ohne sie erreicht `Complete`.

**Der Nutzer entscheidet danach.** Auf ausdrückliche Anweisung — nie
voreingestellt — lassen sich zwei der drei auflösen:

| Anweisung | Wirkung |
|---|---|
| Kommentare entfernen | Der Kommentarteil und die Marken im Dokumentkörper verschwinden. Der **sichtbare Text bleibt Zeichen für Zeichen erhalten.** |
| Änderungen annehmen | Wie „Alle Änderungen annehmen" in Word: Einfügungen bleiben, Löschungen verschwinden **samt ihrem Text**, Formatierungsvermerke entfallen. |

Der **Zuschnitt bleibt in jedem Fall**. Ihn zu beheben hieße, das Bild zu
dekodieren, zu beschneiden und neu zu kodieren — das verändert die Darstellung
und setzt einen Bild-Codec voraus, den ein Metadatenwerkzeug nicht mitbringen
sollte. Wer den weggeschnittenen Bereich wirklich loswerden will, schneidet
das Bild **vor** dem Einfügen zu.

Die Reihenfolge ist der Punkt: erst melden, dann fragen, dann eingreifen. Ein
Werkzeug, das ungefragt Text löscht, überrascht seinen Nutzer im schlechtesten
Moment.

Damit hängt das Ergebnis vom Inhalt ab, nicht vom Format — was der ehrlichere
Zuschnitt ist: Ein Dokument mit nachverfolgten Änderungen **ist** nicht
vollständig bereinigt, und das zu behaupten wäre genau der v1-Fehler.

### 4.2.3 Entfernte Teile brauchen entfernte Beziehungen

`_rels/.rels` und `word/_rels/document.xml.rels` verweisen auf jeden Teil.
Bleibt ein Verweis auf einen entfernten Teil stehen, beantwortet Word das
Öffnen mit einer **Reparaturabfrage** — die Datei sieht für den Nutzer kaputt
aus, obwohl sie es nicht ist.

Deshalb:

- Eigenschaftsteile (`core.xml`, `app.xml`, `custom.xml`) werden durch **leere
  Hüllen ersetzt**, nicht entfernt. Eine leere Hülle trägt dieselbe
  Information wie ein fehlender Teil — nämlich keine — und lässt alle
  Verweise heil.
- Vorschaubild und `customXml/` werden **entfernt**, und ihre Beziehungen
  gleich mit.

### 4.2.4 ODF: zwei Angaben mehr, und eine Formatregel

ODF sammelt die Metadaten in `meta.xml` statt sie auf drei Teile zu verteilen.
Zwei Angaben gibt es so nur hier:

- `meta:editing-duration` — die **Gesamtbearbeitungszeit**, etwa `PT4H12M30S`.
  Wer ein Schreiben als schnell hingeworfen darstellen will, verrät damit vier
  Stunden Arbeit.
- `meta:editing-cycles` — wie oft gespeichert wurde.

Dazu nennt `meta:generator` nicht nur das Programm, sondern die
Betriebssystemvariante: `LibreOffice/7.4.2$Windows_X86_64`.

`settings.xml` ist der zweite Fundort und wird leicht übersehen: LibreOffice
legt dort Ansichtseinstellungen ab — darunter den **Namen des zuletzt
verwendeten Druckers**, oft in der Form `\\SERVER\Kanzlei-Drucker`. Der Teil
wird vollständig durch eine leere Hülle ersetzt.

**Formatregel.** Der Eintrag `mimetype` **MUSS** als erster und
**unkomprimiert** im Archiv stehen. Wird das verletzt, erkennen manche
Programme die Datei nicht mehr als ODF — sie sähe für den Nutzer kaputt aus,
obwohl jeder Teil für sich in Ordnung ist. Reihenfolge und
Kompressionsverfahren jedes Eintrags bleiben deshalb erhalten.

### 4.2.5 ZIP: warum es nie `Complete` wird

Bereinigt werden die **Zeitstempel** jedes Eintrags und die **Metadaten der
enthaltenen Dateien**. Nicht bereinigt werden können die **Eintragsnamen** —
sie sind das Archiv; sie zu entfernen hieße, es zu zerstören.

Das ist keine Einschränkung der Umsetzung, sondern eine des Formats, und sie
wird benannt statt verschwiegen. Namen mit Laufwerksbuchstaben oder
Benutzerverzeichnis (`Users/daniw/Desktop/…`) werden gesondert als `Critical`
gemeldet: Der Benutzername steht dann im Klartext im Archiv, ohne dass
irgendetwas entpackt werden müsste.

**Schachtelungsgrenze.** Ein ZIP in einem ZIP in einem ZIP hat kein
natürliches Ende; eine tausendfach geschachtelte Datei brächte ein Werkzeug
ohne Grenze zum Absturz. Enthaltene **bloße Archive** werden deshalb gemeldet,
nicht geöffnet.

Office-Dokumente sind davon ausgenommen, obwohl sie technisch ZIPs sind: Ihr
Aufbau ist bekannt, und ihre Bereinigung steigt nur noch in Bilder hinab und
endet dort. Ein Word-Dokument in einem Archiv wird also vollständig behandelt
— der häufigste Fall überhaupt.

### 4.2.6 Drei Bildformate, drei Fallen

**WebP.** Ein erweitertes WebP beginnt mit einem `VP8X`-Chunk, dessen erstes
Byte ankreuzt, welche optionalen Chunks folgen — darunter ICC, EXIF und XMP.
Wer die Chunks entfernt und die Ankreuzung stehen lässt, hinterlässt eine
Datei, die Metadaten **ankündigt**, die es nicht mehr gibt; strenge Leser
halten sie für beschädigt. Die Merkmalsbits **MÜSSEN** mitgelöscht werden —
aber nur die drei, nicht das Alpha-Merkmal daneben.

**GIF.** Anwendungs-Erweiterungen sehen alle gleich aus, sind es aber nicht:
`NETSCAPE` steuert die Wiederholung einer Animation und **MUSS bleiben**,
`XMP Data` ist ein vollständiger XMP-Block mit Verfassernamen und **MUSS
weg**. Wer alle entfernt, nimmt der Animation die Schleife; wer keine
entfernt, lässt den Namen stehen.

**BMP.** Trägt kaum Metadaten — und wird gerade deshalb geprüft statt
durchgewunken. Eine Datei ungeprüft weiterzureichen und als sauber zu melden
ist etwas anderes, als sie zu prüfen und nichts zu finden; der Unterschied ist
genau der v1-Fehler.

Zwei Dinge gibt es doch: ein **Anhängsel hinter den Bilddaten** (kein
Betrachter zeigt es an, mitverschickt wird es trotzdem) und ein **Farbprofil**.
Verweist der Header auf eine Profil*datei*, ist das ein Pfad — meist mit dem
Benutzernamen darin, deshalb `Critical`. Ein eingebettetes Profil bleibt
stehen: Es sitzt mitten in der Kopfstruktur, und es zu entfernen hieße,
sämtliche Größenangaben und Versätze neu zu berechnen. Für einen Fund der
Stufe „gering" ein schlechter Tausch, und das Ergebnis sagt es.

**Eine Falle in der Erkennung selbst.** Der erste Entwurf prüfte, ob die im
BMP-Kopf angegebene Dateigröße *genau* der Länge entspricht — und wies damit
ausgerechnet die Datei mit Anhängsel ab, also genau den Fall, für den das
Modul gebaut wurde. Erkannt wird deshalb an der Länge des Informationskopfs.
Der Fehler fiel erst an einer echten Pillow-Datei auf.

### 4.2.7 TIFF: warum die Datei neu gebaut werden muss

Bei PNG, JPEG und WebP liegen Metadaten in abgegrenzten Blöcken. Man entfernt
den Block, hängt die übrigen aneinander — die Bilddaten werden nie angefasst.

**TIFF funktioniert nicht so.** Eine TIFF-Datei ist ein Verzeichnis mit
Verweisen: Passt der Wert eines Eintrags nicht in vier Bytes, steht dort ein
**Versatz** in die Datei. Auch die Bilddaten hängen an solchen Versätzen
(`StripOffsets`, `TileOffsets`).

Daraus folgt zwingend: Einen Eintrag zu entfernen verschiebt alles
Nachfolgende. Die Datei **MUSS** vollständig neu geschrieben und jeder Versatz
neu vergeben werden. Wer das falsch macht, erzeugt eine Datei, die **keinen
Fehler meldet und trotzdem Müll anzeigt** — weil die Bilddaten an der falschen
Stelle gesucht werden. Das ist die gefährlichste Art von Fehler in diesem
ganzen Modul.

Die Bilddaten wandern dabei byteweise unverändert mit; neu berechnet werden
ausschließlich die Versätze. Die Byte-Reihenfolge der Eingabe (`II` oder `MM`)
bleibt erhalten.

**Seite oder Vorschaubild?** Ein TIFF kann mehrere Verzeichnisse enthalten.
Bei einem gescannten Dokument sind das **Seiten** und damit Inhalt; bei einer
Bilddatei ist das zweite Verzeichnis meist ein **Vorschaubild** und damit eine
zweite Kopie (§7.1). Die beiden sehen auf den ersten Blick gleich aus.

Unterschieden wird an `NewSubfileType` (Marke 254): Ist Bit 0 gesetzt, weist
sich das Verzeichnis selbst als verkleinerte Fassung aus und wird entfernt.
Sonst bleibt es. Eine Seite eines Scans zu verlieren wäre Datenverlust, ein
Vorschaubild zu behalten wäre ein Leck — die Marke ist die einzige verlässliche
Auskunft darüber, welcher Fall vorliegt.

Ebenso zweideutig ist `JPEGInterchangeFormat` (Marke 513): Führt dasselbe
Verzeichnis eigene Bilddaten, ist es ein Vorschaubild; sonst **ist** es das
Bild.

**BigTIFF** (Kennzahl 43 statt 42) hat 64-Bit-Versätze und einen anderen
Aufbau. Es wird erkannt und **abgelehnt**, nicht halb verstanden — eine Datei
falsch zu behandeln wäre schlimmer, als sie ehrlich unbehandelt zu lassen.

### 4.3 Der Palette-Bug aus v1

```python
im2 = Image.new(mode, size)
im2.putdata(data)
```

Bei Palette-PNGs (Mode `P`) erzeugt das ein Bild **ohne Palette**. Die
Indexwerte werden übernommen, die Farbtabelle nicht — das Ergebnis hat
falsche Farben.

v2 behandelt PNG auf Chunk-Ebene: Metadaten-Chunks werden entfernt, alle
übrigen (`PLTE`, `tRNS`, `IDAT`, …) bleiben unverändert. Das ist zugleich
verlustfrei — v1 kodierte das Bild neu.

## 5. Zeitstempel

Dateisystem-Zeitstempel sind Metadaten und **MÜSSEN** behandelt werden.

| | Verhalten |
|---|---|
| v1 | `shutil.copy2` — Zeitstempel wurden **erhalten** |
| v2 | Bereinigte Ausgabedateien erhalten den aktuellen Zeitpunkt |

Zusätzlich **MUSS** der Zeitstempel innerhalb von Containern normalisiert
werden — ZIP-Einträge, OOXML- und ODF-Bestandteile tragen eigene Zeiten.
Normalisiert wird auf einen festen Wert (`1980-01-01T00:00:00Z`, der ZIP-Epoche),
nicht auf die aktuelle Zeit, damit zwei Bereinigungen derselben Datei
identische Ergebnisse liefern.

## 6. Inspektion

Die Anzeige vorhandener Metadaten ist eigenständig nützlich — oft will der
Nutzer nur *wissen*, was drinsteht.

Rückgabe je Fund:

```
struct Finding {
    kind: FindingKind,      // GPS, Author, Device, Software, Timestamp, ...
    location: String,       // "docProps/app.xml:Company"
    value: String,          // gekürzt auf 200 Zeichen
    severity: Severity,     // Critical | Notable | Minor
}
```

| Schwere | Beispiele |
|---|---|
| `Critical` | **Eingebettete Vorschaubilder (§7.1)**, **zugeschnittene Bilder in Office-Dokumenten (§7.2)**, GPS-Koordinaten, Klarname, Seriennummer des Geräts, Firmenname |
| `Notable` | Kameramodell, Software, Bearbeitungszeit, Vorlagenname |
| `Minor` | Farbprofil, Auflösung, Orientierung |

v1 gab die rohen EXIF-Tag-Nummern aus (`0th:271`, `GPS:2`). Für den Nutzer ist
das unlesbar. v2 löst die gängigen Tags in Klartext auf und stuft sie ein.

## 7. Zweitkopien des Inhalts

Die beiden folgenden Fälle sind keine Metadaten im engeren Sinn, sondern
**zusätzliche Kopien des Bildinhalts** — teils in einem Zustand, den der Nutzer
gerade beseitigen wollte. Deshalb sind sie `Critical`, während ein Kameramodell
nur `Notable` ist.

### 7.1 Eingebettete Vorschaubilder

Bilddateien enthalten eine verkleinerte Vorschau. Viele Programme aktualisieren
beim Zuschneiden **das Hauptbild, aber nicht die Vorschau**. Wer ein Foto
beschneidet, um ein Gesicht, ein Kennzeichen oder ein Dokument im Hintergrund
zu entfernen, trägt das Entfernte in der Vorschau weiter mit sich.

Der bekannteste Fall ist der von Cat Schwartz (2003): ein für einen Blog
zugeschnittenes Porträt, dessen EXIF-Thumbnail die unbeschnittene Aufnahme
enthielt. Für die Zielgruppe dieses Programms ist die ernstere Variante die
Schwärzung von Dokumentfotos.

| Ort | Inhalt |
|---|---|
| EXIF-Thumbnail (JPEG, TIFF) | verkleinerte Fassung, oft unbeschnitten |
| HEIC/HEIF | mehrere Bild-Items, teils Fassungen vor der Bearbeitung |
| RAW/DNG | eingebettete JPEG-Vorschau in **voller Auflösung** |
| PDF | Seitenminiaturen |
| OOXML | `docProps/thumbnail.jpeg` — Vorschau der ersten Seite |

**Regel:** Eingebettete Vorschaubilder **MÜSSEN** als `Critical` gemeldet und
beim Strippen **immer** entfernt werden. Ein Vorschaubild zu erhalten ist nie
im Interesse des Nutzers.

### 7.2 Zugeschnittene Bilder in Office-Dokumenten

Der in der Praxis häufigste Fall — und der am wenigsten bekannte.

**Wer ein Bild in Word oder PowerPoint einfügt und dort zuschneidet, verschickt
das vollständige Original.** Der Zuschnitt ist lediglich ein Anzeigerechteck in
der XML-Beschreibung; die Bilddatei unter `word/media/` bleibt unverändert.
Empfänger können den Zuschnitt mit zwei Klicks rückgängig machen.

**Regel:** Der Fall **MUSS** erkannt und als `Critical` gemeldet werden — mit
Angabe, welches Bild betroffen ist.

Das Entfernen erfolgt jedoch **nicht** automatisch: Den weggeschnittenen Bereich
tatsächlich zu beseitigen bedeutet, das Bild neu zu kodieren und im Dokument zu
ersetzen. Das verändert das Dokument sichtbar und kann die Darstellung
beeinflussen. Es bleibt eine ausdrückliche, gesondert bestätigte Aktion.

Das ist die konsequente Anwendung des Grundsatzes aus §3: melden, was ist —
und den Eingriff dem Nutzer überlassen, wenn er über bloßes Löschen von
Metadaten hinausgeht.

### 7.3 Unbekannte Erweiterungspunkte

PNG kennt beliebige Chunk-Typen, OOXML beliebige Teile, PDF beliebige Objekte.
Ein unbekannter Chunk kann Metadaten enthalten — oder für die Darstellung
notwendig sein.

**Regel:** Unbekannte, nicht-kritische Erweiterungen werden **entfernt und
namentlich gemeldet**. Das Ergebnis bleibt `Complete`.

Begründung: Der Nutzer will eine bereinigte Datei, nicht eine Warnung über
etwas, das niemand einordnen kann. Entfernen ist die sichere Richtung — eine
entfernte Erweiterung kostet schlimmstenfalls ein Darstellungsdetail, eine
verbliebene kann eine Identität preisgeben. Die namentliche Meldung sorgt
dafür, dass der Nutzer den Verlust bemerkt, falls er zählt.

Bei PNG betrifft das alle Chunks außer den für die Darstellung notwendigen
(`IHDR`, `PLTE`, `IDAT`, `IEND`, `tRNS`, `gAMA`, `cHRM`, `sRGB`).

### 7.4 SVG

SVG wird **bereinigt**, das Ergebnis bleibt aber **immer `Partial`** — nie
`Complete`. SVG ist beliebiges XML mit unbegrenzten Erweiterungsmöglichkeiten;
eine Vollständigkeitszusage wäre nicht haltbar.

Entfernt wird über eine **Allowlist** (nur bekannte Elemente und Attribute
bleiben stehen):

| Was | Warum |
|---|---|
| `<metadata>`, `<title>`, `<desc>` | klassische Metadaten, oft mit Editor- und Autorenangaben |
| Editor-Namespaces (`inkscape:`, `sodipodi:`, `adobe:`) | Bearbeitungsspuren, Dateipfade, Ebenennamen |
| `<script>`, alle `on*`-Attribute | ausführbarer Code beim Öffnen im Browser des Empfängers |
| `<foreignObject>` | eingebettetes HTML, beliebig erweiterbar |
| **Externe Referenzen** (`xlink:href`, `href`, `url()` auf fremde Hosts) | siehe unten |

**Externe Referenzen sind der unterschätzte Teil.** Ein `xlink:href` auf eine
fremde URL wird zum Zählpixel: Sobald der Empfänger die Datei öffnet, meldet
sein Rechner Zeitpunkt und IP-Adresse an einen Dritten. Bei einem Werkzeug für
vertrauliche Kommunikation ist das der schwerwiegendste Einzelfund in einer
SVG-Datei.

**Eingebettete Rasterbilder** als `data:`-URI werden **rekursiv** behandelt —
sie tragen eigenes EXIF, einschließlich GPS und Vorschaubildern nach §7.1.

Die Allowlist ist der einzig vertretbare Ansatz: Eine Blockliste übersieht
zwangsläufig, was sie nicht kennt, und SVG entwickelt sich weiter.

## 8. Zusammenspiel mit dem Envelope

Die Bereinigung ist **optional** und ändert die Nutzdatei. Der Envelope schützt
den Inhalt ohnehin — Metadaten in der Datei sind erst dann ein Problem, wenn
der **Empfänger** sie sehen soll oder die Datei später weitergereicht wird.

Daraus folgt für die Oberfläche:

- Beim Anhängen **MUSS** inspiziert und bei `Critical`-Funden gewarnt werden,
  ohne ungefragt zu verändern.
- Das Strippen **MUSS** eine bewusste Entscheidung bleiben.
- Bei `Unknown` **MUSS** klar werden: Cabrik Secure kann hier nichts prüfen —
  der Inhalt ist trotzdem verschlüsselt.

Der letzte Punkt verhindert den naheliegenden Fehlschluss, ein unbereinigter
Anhang sei unsicher übertragen.

## 9. Nicht in 2.0

| Format | Grund |
|---|---|
| MP4, MOV, MKV | ISO-BMFF-Atome und Matroska-Tags sind eigener Aufwand; Kamera-Metadaten liegen an mehreren Stellen |
| MP3, FLAC, OGG | ID3v1/v2, Vorbis Comments |
| Office-Altformate `.doc`, `.xls`, `.ppt` | OLE-Compound-Format, deutlich aufwendiger als OOXML |
| RAW-Bildformate | herstellerspezifisch, teils undokumentiert |
| CAD, GIS | Nischenformate mit hohem Aufwand |

Alle liefern `Unknown` und damit **keine** Sauberkeitsaussage. Das ist das
korrekte Verhalten, kein Mangel.

## 10. Entschiedene Punkte

| Frage | Entscheidung |
|---|---|
| Unbekannte Erweiterungspunkte | Entfernen, namentlich melden, Ergebnis bleibt `Complete` (§7.3) |
| Eingebettete Vorschaubilder | `Critical`, immer entfernen (§7.1) |
| Zugeschnittene Office-Bilder | `Critical`, melden — Entfernen nur auf ausdrückliche Bestätigung (§7.2) |
| SVG | Bereinigen per Allowlist, Ergebnis immer `Partial` (§7.4) |

## 11. Offene Punkte

- Ob die PNG-Allowlist der notwendigen Chunks vollständig ist — `iCCP` und
  `sBIT` sind Grenzfälle zwischen Darstellung und Metadaten
- Ob bei RAW/DNG überhaupt bereinigt werden sollte, oder ob die Umwandlung in
  ein anderes Format der ehrlichere Rat ist
- Wie zugeschnittene Bilder in ODF-Dokumenten erkannt werden — das Datenmodell
  unterscheidet sich von OOXML
