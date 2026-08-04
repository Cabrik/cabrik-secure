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
    Overwrite,        // HDD, klassisches Dateisystem — Überschreiben wirkt
    BestEffort,       // SSD/NVMe — Überschreiben ist nicht verlässlich
    Unsupported,      // Netzlaufwerk, schreibgeschützt, Cloud-Sync-Ordner
}
```

**Erkennung unter Windows:** `DeviceIoControl` mit
`IOCTL_STORAGE_QUERY_PROPERTY` und `StorageDeviceSeekPenaltyProperty` —
`IncursSeekPenalty = false` bedeutet SSD. Ergänzend
`StorageDeviceTrimProperty` für TRIM-Unterstützung.

**Zusätzlich erkannt und gemeldet:**

- Netzlaufwerke und Wechselmedien
- Ordner unter Synchronisation (OneDrive, Dropbox, Google Drive) — dort
  existieren mit hoher Wahrscheinlichkeit **Serverkopien**, die lokal nicht
  erreichbar sind
- Aktive Volume Shadow Copies auf dem Laufwerk

Der letzte Punkt ist wichtig: Eine Datei, die in einem Cloud-Ordner lag, ist
durch lokales Löschen praktisch nie beseitigt. v2 **MUSS** darauf hinweisen,
statt Erfolg zu melden.

## 5. Ablauf

```
1. Fähigkeit ermitteln (§4), Ergebnis dem Nutzer anzeigen
2. Exklusiven Zugriff sichern; scheitert das → Fehler, kein stiller Abbruch
3. Schreibschutz- und Nur-Lesen-Attribute entfernen
4. Ist die Datei kleiner als die MFT-Residenzgrenze (~700 Bytes):
   auf 4 KiB vergrößern, damit der Inhalt aus dem MFT-Eintrag ausgelagert wird
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

**Schritt 4** behandelt den MFT-Residenzfall — ohne ihn bleibt bei kleinen
Dateien der vollständige Inhalt im MFT-Eintrag stehen, egal wie oft
„überschrieben" wurde.

**Schritt 7** entfernt den Dateinamen aus dem MFT. Der Name allein kann
verräterisch genug sein — siehe das Beispiel aus `metadata.md` §1.

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

## 9. Offene Punkte

- Genaue MFT-Residenzgrenze prüfen — sie hängt von der MFT-Eintragsgröße und
  der Anzahl der Attribute ab, ~700 Bytes ist ein Richtwert
- Erkennung von Cloud-Sync-Ordnern: Reparse Points und die bekannten
  Anbieterpfade reichen vermutlich nicht für alle Fälle
- Verhalten bei Verzeichnissen (rekursiv?) — in v1 nicht vorhanden
- Ob auf macOS und Linux dieselbe Fähigkeitserkennung sinnvoll abbildbar ist
  (`rotational` in sysfs unter Linux; APFS ist grundsätzlich `BestEffort`)
