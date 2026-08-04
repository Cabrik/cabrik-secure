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
| TIFF | teilweise | `Complete` | EXIF-IFDs, XMP |
| WebP | teilweise | `Complete` | `EXIF`, `XMP `, `ICCP` Chunks |
| GIF | ✗ | `Complete` | Kommentar- und Anwendungs-Extensions |
| BMP | ✗ | `Complete` | trägt praktisch keine Metadaten |
| **HEIC/HEIF** | ✗ | `Complete` | EXIF- und XMP-Items im Meta-Box |
| **AVIF** | ✗ | `Complete` | dito |
| **SVG** | ✗ | `Partial` | `<metadata>`, `<title>`, `<desc>`, Editor-Namespaces. Bleibt `Partial`, weil SVG beliebiges XML und sogar Skripte tragen kann |
| PDF | teilweise | `Partial` | DocInfo **und** XMP (v1 nur DocInfo). Bleibt `Partial`: eingebettete Schriften, Anhänge, JavaScript, inkrementelle Änderungshistorie |
| **DOCX** | teilweise | `Complete` | `core.xml`, **`app.xml`**, **`custom.xml`**, `settings.xml` (rsid), Kommentare, Revisionen |
| **XLSX** | ✗ | `Complete` | dito |
| **PPTX** | ✗ | `Complete` | dito |
| **ODT/ODS/ODP** | ✗ | `Complete` | `meta.xml`, Bearbeitungszyklen |
| **ZIP** | ✗ | `Partial` | Einträge tragen Zeitstempel und teils Pfade |
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
| `Critical` | GPS-Koordinaten, Klarname, Seriennummer des Geräts, Firmenname |
| `Notable` | Kameramodell, Software, Bearbeitungszeit, Vorlagenname |
| `Minor` | Farbprofil, Auflösung, Orientierung |

v1 gab die rohen EXIF-Tag-Nummern aus (`0th:271`, `GPS:2`). Für den Nutzer ist
das unlesbar. v2 löst die gängigen Tags in Klartext auf und stuft sie ein.

## 7. Zusammenspiel mit dem Envelope

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

## 8. Nicht in 2.0

| Format | Grund |
|---|---|
| MP4, MOV, MKV | ISO-BMFF-Atome und Matroska-Tags sind eigener Aufwand; Kamera-Metadaten liegen an mehreren Stellen |
| MP3, FLAC, OGG | ID3v1/v2, Vorbis Comments |
| Office-Altformate `.doc`, `.xls`, `.ppt` | OLE-Compound-Format, deutlich aufwendiger als OOXML |
| RAW-Bildformate | herstellerspezifisch, teils undokumentiert |
| CAD, GIS | Nischenformate mit hohem Aufwand |

Alle liefern `Unknown` und damit **keine** Sauberkeitsaussage. Das ist das
korrekte Verhalten, kein Mangel.

## 9. Offene Punkte

- Ob `Complete` bei Formaten mit theoretisch unbegrenzten Erweiterungspunkten
  (PNG-Chunks unbekannten Typs) überhaupt vergeben werden darf, oder ob
  unbekannte Chunks zu `Partial` führen sollten — Neigung: unbekannte Chunks
  werden entfernt und namentlich gemeldet, Ergebnis bleibt `Complete`
- Umgang mit eingebetteten Miniaturbildern, die eine unbeschnittene Fassung
  des Bildes enthalten können — vermutlich `Critical`
- Ob SVG wegen möglicher Skripte grundsätzlich abgelehnt statt bereinigt werden
  sollte
