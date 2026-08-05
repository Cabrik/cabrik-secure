# Cabrik Secure — Sicheres Löschen

**Status:** Entwurf · Phase 1, Dokument 7 von 7
**Setzt voraus:** `threat-model.md`, `envelope-v2.md`

---

## 1. Die unbequeme Wahrheit

**Auf einer SSD kann eine einzelne Datei nicht zuverlässig durch Überschreiben
gelöscht werden.** Das ist keine Implementierungsschwäche, sondern folgt aus
der Bauweise von Flash-Speicher.

| Ursache | Wirkung |
|---|---|
| **Wear-Leveling** | Der Flash-Translation-Layer schreibt jeden „Überschreibvorgang" auf eine **neue** physische Seite. Die alte bleibt lesbar, bis die Garbage Collection sie irgendwann löscht — Zeitpunkt unbestimmt, von außen weder steuerbar noch prüfbar |
| **Over-Provisioning** | 7–28 % des Flash sind für das Betriebssystem unsichtbar und können Kopien enthalten |
| **NTFS-Journal** | `$LogFile` und `$UsnJrnl` protokollieren Änderungen |
| **MFT-Residenz** | Dateien unter etwa 700 Bytes liegen **im MFT-Eintrag**. Das Überschreiben der „Datei" erreicht diese Kopie nicht |
| **Volume Shadow Copies** | Systemwiederherstellung kann vollständige frühere Fassungen halten |
| **Pagefile, Hibernation** | Auslagerungsdatei und `hiberfil.sys` können Klartext enthalten |
| **Suchindex, Thumbnails** | Windows Search und der Miniaturbild-Cache halten Inhalte und Vorschauen |

Dazu tritt: Selbst auf einer HDD bleibt der **Dateiname** im MFT stehen, auch
wenn der Inhalt überschrieben wurde.

## 2. Was v1 tat

```python
except Exception:
    pass
...
try:
    os.remove(path)
except Exception:
    pass
```

`secure_delete` verschluckt jeden Fehler. Die Einzeldatei-Funktion der
Oberfläche meldete anschließend „Gelöscht: …" — auch dann, wenn kein einziges
Byte überschrieben wurde, etwa weil die Datei schreibgeschützt oder von einem
anderen Prozess geöffnet war.

Das ist die schlechteste denkbare Eigenschaft für ein Sicherheitswerkzeug: Es
erzeugt Vertrauen, wo keines gerechtfertigt ist. Die Mehrfach-Variante prüfte
immerhin die Existenz — dass die Datei weg ist, bedeutet aber nicht, dass ihr
Inhalt weg ist.

## 3. Die eigentliche Lösung: gar nicht erst schreiben

Die Frage „wie lösche ich Klartext von der SSD" ist die falsche. Die richtige
lautet: **warum liegt dort überhaupt Klartext?**

### 3.1 Das Leck in v1

Beim Verschlüsseln mehrerer Anhänge schrieb v1 ein **unverschlüsseltes ZIP**
nach `tempfile.mkdtemp()`:

```python
temp_dir = tempfile.mkdtemp(prefix="cabrik_")
zip_path = os.path.join(temp_dir, "attachments.zip")
with zipfile.ZipFile(zip_path, "w", ...) as zf:
    ...
env_b64 = encrypt_file(pub, zip_path, ...)
```

Sämtliche Anhänge lagen damit vollständig im Klartext auf dem Datenträger,
bevor überhaupt verschlüsselt wurde. `shutil.rmtree` entfernt danach den
Verzeichniseintrag — die Daten bleiben physisch liegen, mit allen Folgen aus §1.

Wer Cabrik Secure benutzt, um Dateien vertraulich zu versenden, hat damit
unbeabsichtigt eine forensisch auffindbare Klartextkopie erzeugt.

### 3.2 Anforderung an v2

1. **Kein Klartext-Zwischenprodukt.** Mehrere Dateien werden über
   `archive_index` direkt in den verschlüsselten Stream geschrieben
   (`envelope-v2.md` §7.4). Ein ZIP entsteht nie.
2. **Bereinigte Dateien** aus dem Metadaten-Strip sind unvermeidlich Klartext.
   Sie werden im selben Verzeichnis wie das Original angelegt (nicht in `%TEMP%`),
   damit sie nicht unbemerkt auf einem anderen Datenträger landen, und nach dem
   Verschlüsseln mit dem Verfahren aus §5 entfernt.
3. **Entschlüsselte Ausgaben** schreibt der Nutzer bewusst — sie zu schützen
   ist nicht Aufgabe dieser Software (Threat Model §5).
4. **Schlüsselmaterial** wird zeroisiert (`zeroize`), auch in Zwischenpuffern.

**Das ist Crypto-Shredding, und es funktioniert wirklich:** Was nie im Klartext
geschrieben wurde, muss nicht gelöscht werden. Der Schlüssel existiert nur im
RAM; ist er weg, sind die Daten unlesbar — unabhängig von jeder Flash-Physik.

## 4. Fähigkeitsauskunft statt Versprechen

Vor dem Löschen ermittelt v2, was auf dem konkreten Datenträger erreichbar ist.

```
enum ShredCapability {
    Overwrite,        // Überschreiben wirkt tatsächlich
    BestEffort,       // Überschreiben ist nicht verlässlich
    Unsupported,      // Netzlaufwerk, schreibgeschützt
}
```

### 4.1 Das Dateisystem zählt mehr als die Hardware

Der wichtigste Punkt, und derjenige, der am häufigsten übersehen wird:
**Copy-on-Write-Dateisysteme überschreiben grundsätzlich nie an Ort und
Stelle** — unabhängig davon, ob darunter eine SSD oder eine rotierende Platte
liegt. Jeder Schreibvorgang landet in neuen Blöcken; die alten bleiben, bis
sie freigegeben werden. Bei aktiven Snapshots bleiben sie sogar dauerhaft.

Betroffen sind **btrfs, ZFS und APFS**. Auf einem ZFS-Pool auf HDDs ist
Überschreiben also ebenso wirkungslos wie auf einer NVMe.

`Overwrite` wird daher **nur** zurückgegeben, wenn alle drei Bedingungen
erfüllt sind:

1. Dateisystem ist **nicht** Copy-on-Write (NTFS, ext4, exFAT, HFS+, XFS)
2. Datenträger ist **rotierend**
3. Keine Snapshots oder Schattenkopien auf dem Volume erkannt

In allen anderen Fällen: `BestEffort`.

### 4.2 Erkennung je Plattform

| Plattform | Datenträgertyp | Dateisystem |
|---|---|---|
| **Windows** | `DeviceIoControl` + `IOCTL_STORAGE_QUERY_PROPERTY`, `StorageDeviceSeekPenaltyProperty` — `IncursSeekPenalty = false` bedeutet SSD | `GetVolumeInformation` |
| **Linux** | `/sys/block/<dev>/queue/rotational`; Zuordnung Datei → Gerät über `stat().st_dev` → `/sys/dev/block/MAJ:MIN`, bei LVM und Software-RAID über `slaves/` weiterverfolgen | `statfs().f_type` |
| **macOS** | praktisch immer SSD | praktisch immer APFS |

**macOS ist faktisch immer `BestEffort`.** APFS ist Copy-on-Write und die
Hardware seit Jahren durchweg Flash. Das ist keine Nachlässigkeit, sondern
dieselbe Erkenntnis, aus der Apple **„Sicheres Leeren des Papierkorbs" in
OS X 10.11 entfernt hat** — mit der ausdrücklichen Begründung, die Funktion
könne auf modernen Laufwerken nicht halten, was sie verspricht. Dieses
Präzedenz gehört in die Dokumentation: Wenn Apple das Feature aus seinem
eigenen System nimmt, ist es kein Versäumnis, es hier nicht zu versprechen.

### 4.3 Kopien außerhalb des Zugriffs

Cloud-Synchronisation, Backups und Schattenkopien erzeugen Kopien, die lokales
Löschen nicht erreicht. Der Versuch, sie vollständig zu **erkennen**, ist
aussichtslos — die Anbieterliste wäre nie vollständig, und Backup-Lösungen sind
beliebig.

**Deshalb wird die Frage umgedreht.** Statt zu beweisen „hier gibt es Kopien",
gilt: Die Warnung erscheint **immer**, außer es wurde positiv festgestellt, dass
es sich um ein einfaches lokales Volume ohne erkennbare Synchronisation handelt.

Das ist zugleich ehrlicher und einfacher als eine Anbieterliste.

Zusätzlich **positiv erkannt** und dann verschärft gewarnt:

- `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS`, `FILE_ATTRIBUTE_RECALL_ON_OPEN`,
  `FILE_ATTRIBUTE_OFFLINE` — gesetzt von allen Anbietern, die die Windows
  Cloud Filter API nutzen (OneDrive, Dropbox, Google Drive)
- Cloud-Reparse-Tags (`IO_REPARSE_TAG_CLOUD` und Varianten)
- Umgebungsvariablen `%OneDrive%`, `%OneDriveCommercial%` und die bekannten
  Anbieterpfade
- Aktive Volume Shadow Copies auf dem Volume
- Netzlaufwerke und Wechselmedien

## 5. Ablauf

```
1. Fähigkeit ermitteln (§4), Ergebnis dem Nutzer anzeigen
2. Exklusiven Zugriff sichern; scheitert das → Fehler, kein stiller Abbruch
3. Schreibschutz- und Nur-Lesen-Attribute entfernen
4. Ist die Datei kleiner als **8 KiB**: auf 8 KiB vergrößern, damit der Inhalt
   aus dem MFT-Eintrag ausgelagert wird (§5.1)
5. Inhalt überschreiben:
     Durchgang 1..n: Zufallsbytes
     Abschluss:      Nullbytes
   nach jedem Durchgang flush + fsync
6. Datei auf 0 Bytes kürzen
7. Dateinamen 3× in gleichlange Zufallsnamen umbenennen
8. Zeitstempel auf einen festen Wert setzen
9. Löschen
10. Ergebnis prüfen und ehrlich zurückgeben (§6)
```

**Schritt 7** entfernt den Dateinamen aus dem MFT. Der Name allein kann
verräterisch genug sein — siehe das Beispiel aus `metadata.md` §1.

### 5.1 Warum pauschal 8 KiB statt exakter Residenzgrenze

Die tatsächliche Grenze hängt von der MFT-Eintragsgröße (meist 1024 Bytes) und
der Zahl der Attribute ab und liegt typischerweise zwischen 700 und 900 Bytes.
Sie ließe sich über `FSCTL_GET_NTFS_VOLUME_DATA` exakt bestimmen —
`BytesPerFileRecordSegment` liefert den Wert.

Das wird **bewusst nicht** getan. Der Aufruf braucht ein Volume-Handle, das je
nach Konfiguration erhöhte Rechte erfordert, und liefert am Ende nur eine
Schwelle, unter der man ohnehin vergrößern würde.

Jede Datei unter 8 KiB pauschal zu vergrößern ist drei Zeilen Code, braucht
keine Sonderrechte, funktioniert auf jedem Dateisystem und liegt immer über
jeder denkbaren Residenzgrenze. Die Kosten sind wenige Kilobyte
Schreibvorgang — für einen Vorgang, der ohnehin mehrfach überschreibt,
bedeutungslos.

Die einfache Lösung ist hier zugleich die robustere.

### 5.2 Verzeichnisse

Rekursives Löschen wird **unterstützt** — ohne es würden Nutzer Dateien einzeln
auswählen und dabei welche übersehen, was schlechter ist als eine gut
abgesicherte rekursive Funktion.

Weil ein Fehlgriff unwiderruflich ist, gelten harte Leitplanken:

1. **Vorschau vor der Ausführung:** vollständiger Pfad, Anzahl der Dateien,
   Gesamtgröße.
2. **Bestätigung durch Eintippen des Verzeichnisnamens.** Ein Klick auf „OK"
   genügt bei einer unumkehrbaren Aktion nicht.
3. **Kategorische Verweigerung** bei: Laufwerkswurzeln, Benutzerprofil-Wurzel,
   `Windows`, `Program Files`, `/`, `/home`, `/usr`, `/etc` und Verzeichnissen,
   die ein `.git` enthalten.
4. **Symlinks und Junctions werden niemals verfolgt.** Ein Link im Baum darf
   nicht dazu führen, dass außerhalb gelöscht wird. Erkannt über
   `FILE_ATTRIBUTE_REPARSE_POINT` beziehungsweise `symlink_metadata`.
5. Verzeichniseinträge werden von innen nach außen entfernt.
6. **Verzeichnisnamen werden ebenfalls umbenannt**, bevor sie entfernt werden —
   auch sie stehen sonst im MFT.
7. Ein Fehler bei einer Datei bricht den Vorgang **nicht** ab, wird aber
   einzeln im Ergebnis geführt.

**Zur Anzahl der Durchgänge:** Ein Durchgang genügt bei jedem Datenträger, der
nach 2001 gebaut wurde. Die verbreitete Annahme, 35 Durchgänge (Gutmann) seien
nötig, bezieht sich auf MFM- und RLL-Kodierung der frühen 1990er und ist auf
heutige Laufwerke nicht übertragbar. Voreinstellung: **1**, einstellbar bis 7.
v1 hatte 3 voreingestellt und suggerierte damit einen Nutzen, den zusätzliche
Durchgänge nicht haben.

## 6. Ehrliches Ergebnis

```
struct ShredOutcome {
    path: PathBuf,
    capability: ShredCapability,
    overwritten: bool,        // wurde tatsächlich geschrieben?
    renamed: bool,
    removed: bool,            // Verzeichniseintrag weg?
    warnings: Vec<Warning>,
    error: Option<ShredError>,
}
```

Die Oberfläche **MUSS** unterscheiden zwischen:

| Meldung | Bedingung |
|---|---|
| „Überschrieben und gelöscht" | `Overwrite`, alle Schritte erfolgreich |
| „Gelöscht — Überschreiben auf SSD nicht verlässlich" | `BestEffort` |
| „Gelöscht, aber Kopien wahrscheinlich vorhanden" | Cloud-Ordner oder Shadow Copies erkannt |
| „Fehlgeschlagen: *Grund*" | jeder Fehler |

Ein pauschales „Gelöscht" wie in v1 **DARF NICHT** mehr vorkommen.

## 7. Was v2 nicht tut

| Verfahren | Grund |
|---|---|
| ATA Secure Erase, NVMe Sanitize | wirkt nur laufwerksweit und löscht alles. Gehört in ein Systemwerkzeug, nicht hierher |
| TRIM erzwingen | nicht zuverlässig steuerbar; ob und wann das Laufwerk tatsächlich löscht, bleibt offen |
| Freien Speicherplatz überschreiben | auf SSDs wirkungslos, auf HDDs sehr langsam, und die eigentliche Antwort ist §3 |
| Shadow Copies löschen | erfordert Administratorrechte und greift systemweit ein |
| Pagefile oder Hibernation bereinigen | dito |

Zu allen diesen Punkten **SOLLTE** die Dokumentation auf die Bordmittel des
Betriebssystems verweisen, statt sie halbherzig nachzubauen.

## 8. Was in der Oberfläche stehen muss

Der Text beim Aufruf des Werkzeugs, sinngemäß verbindlich:

> Sicheres Löschen kann auf SSDs und NVMe-Laufwerken **nicht garantiert**
> werden. Durch Wear-Leveling bleiben frühere Fassungen physisch erhalten,
> auch wenn sie überschrieben wurden.
>
> Verlässlich ist nur, was nie unverschlüsselt gespeichert wurde. Cabrik
> Secure schreibt beim Verschlüsseln keine Klartext-Zwischendateien.

Das ist keine Beschwichtigung, sondern die Anleitung zum richtigen Gebrauch:
Wer Vertraulichkeit braucht, verschlüsselt von Anfang an — statt sich darauf zu
verlassen, Klartext hinterher beseitigen zu können.

## 9. Entschiedene Punkte

| Frage | Entscheidung |
|---|---|
| MFT-Residenzgrenze | Pauschal jede Datei unter 8 KiB vergrößern, keine Ermittlung der exakten Schwelle (§5.1) |
| Cloud-Erkennung | Frage umgedreht: Warnung immer, außer ein einfaches lokales Volume wurde positiv festgestellt (§4.3) |
| Verzeichnisse | Rekursiv, mit Vorschau, Eintippen des Namens, Sperrliste und ohne Linkverfolgung (§5.2) |
| macOS / Linux | Dateisystem vor Hardware. CoW-Dateisysteme sind immer `BestEffort`; macOS damit faktisch durchgängig (§4.1, §4.2) |

## 10. Offene Punkte

- Ob die Sperrliste aus §5.2 Punkt 3 konfigurierbar sein sollte oder besser
  fest bleibt
- Wie mit Alternate Data Streams unter NTFS umzugehen ist — sie werden beim
  Überschreiben der Hauptdatei nicht erfasst
- Ob bei erkannten Schattenkopien aktiv angeboten werden sollte, die
  Systemwiederherstellung zu prüfen, oder ob das zu weit in die
  Systemverwaltung greift
