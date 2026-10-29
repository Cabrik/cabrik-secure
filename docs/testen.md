# CabrikSecure testen

Danke, dass Sie mittesten. CabrikSecure verschlüsselt Dateien und
Textnachrichten zwischen Menschen, die einander kennen — ohne Server und
ohne Konto.

Diese Seite brauchen Sie **vor** der Installation: welche Datei die
richtige ist, wie Sie prüfen, dass sie unverändert bei Ihnen angekommen
ist, und wie Sie ein Programm starten, für das noch kein Herausgeber
bürgt. Für alles danach hat das Programm eine **Hilfe** mit Handbuch und
Beispielen.

> **Eine frühe Testfassung.** Sie ist nicht signiert, und niemand hat sie
> unabhängig geprüft. Verschlüsseln Sie damit nichts, dessen Offenlegung
> Ihnen ernsthaft schadet. Und löschen Sie keine Originale, die Sie danach
> nur noch verschlüsselt hätten.

## Die Dateien

**Fassung 0.1.4** vom 29.10.2026, gebaut von GitHub aus dem Stand
[`ebed089`](https://github.com/Cabrik/cabrik-secure/commit/ebed08996ec8695037791ba921143e351dc039ee)
dieses Repositorys ([Protokoll](https://github.com/Cabrik/cabrik-secure/actions/runs/40797664393)).

| Ihr System | Ihre Datei |
|---|---|
| Windows 10 oder 11 | `CabrikSecure_0.1.4_x64-setup.exe` |
| Mac mit Apple-Chip (M1 und neuer) | `CabrikSecure_0.1.4_aarch64.dmg` |
| Debian, Ubuntu, Linux Mint | `CabrikSecure_0.1.4_amd64.deb` |
| andere Linux-Systeme | `CabrikSecure_0.1.4_amd64.AppImage` |

Die Datei bekommen Sie von der Person, die Sie zum Testen eingeladen hat.
Hier steht nur ihre Prüfsumme — mit Absicht, siehe unten.

Für Macs mit Intel-Prozessor gibt es diesmal keine Fassung. Unter Linux
braucht CabrikSecure ein aktuelles System: Ubuntu ab 24.04, Linux Mint ab 22,
Debian ab 13 oder Fedora ab 40 — allgemein eine glibc ab 2.39. Auf
älteren Systemen startet es nicht.

## 1. Prüfen, ob die Datei unverändert ist

Ihr Rechner kann bei einer unsignierten Datei nicht feststellen, ob sie
unterwegs verändert wurde — in einer Cloud-Ablage, einem Messenger, auf
einem USB-Stick. Die **Prüfsumme** kann es: Sie wird aus jedem einzelnen
Byte der Datei berechnet, und ändert sich eines, ändert sich die Summe.

Die richtige Summe steht hier, die Datei kommt auf einem anderen Weg. Wer
beides fälschen wollte, müsste beide Wege beherrschen.

Warnt schon der Browser, die Datei werde selten heruntergeladen, liegt
das an derselben fehlenden Signatur. Behalten Sie die Datei, in Edge über
„…“ und **Beibehalten**. Ob sie unverändert ist, zeigt dieser Schritt.

Kopieren Sie die Zeile für Ihre Datei, fügen Sie sie ein und drücken Sie
die Eingabetaste. Die Zeilen nehmen an, dass die Datei im Ordner
„Downloads“ liegt; sonst ändern Sie diesen Teil.

**Windows** — Windows-Taste drücken, `PowerShell` tippen, Eingabetaste,
dann:

```powershell
(Get-FileHash "$HOME\Downloads\CabrikSecure_0.1.4_x64-setup.exe").Hash -eq "5cc4b8fd29e89755af9e1dab49712a622fd5a1c04723aed2f9c34bcc04a3c9d1"
```

**macOS** — das Programm „Terminal“ öffnen (Programme → Dienstprogramme),
dann:

```sh
shasum -a 256 -c <<< "fd104048aeb732336cde92e86edc01b25502656500cb7287b039353d28f10ea3  $HOME/Downloads/CabrikSecure_0.1.4_aarch64.dmg"
```

**Linux** — im Terminal, je nach Datei:

```sh
sha256sum -c <<< "9774cb1ea645141c489f2d2201ab747eaabd90e77659ecdca8cab8d3bc3301e4  $HOME/Downloads/CabrikSecure_0.1.4_amd64.deb"
```

```sh
sha256sum -c <<< "9a5e4f6da69061b526dd74f357d384064e05eccfaf7926d3c556d6ea4f8d4925  $HOME/Downloads/CabrikSecure_0.1.4_amd64.AppImage"
```

**Die Antwort** muss unter Windows `True` lauten und unter macOS und Linux
mit `OK` enden.

- **Findet der Befehl die Datei nicht** — rote Schrift unter Windows, „No
  such file“ sonst —, stimmt der Ordner nicht, oder der Browser hat dem
  Namen etwas angehängt, etwa „ (1)“.
- **Lautet die Antwort `False` oder `FAILED`, obwohl die Datei gefunden
  wurde, öffnen Sie die Datei nicht.** Sie ist dann nicht die, die hier gebaut
  wurde. Löschen Sie sie und sagen Sie Bescheid.

## 2. Installieren und starten

### Windows

1. Doppelklick auf `CabrikSecure_0.1.4_x64-setup.exe`. Haben Sie auch
   `.msi`-Dateien bekommen, lassen Sie die liegen: Nur das Setup fragt beim
   Deinstallieren, ob Ihre Schlüssel bleiben sollen.
2. Windows meldet: „Der Computer wurde durch Windows geschützt“. Klicken Sie
   auf **Weitere Informationen** und dann auf **Trotzdem ausführen**. Als
   Herausgeber steht dort „Unbekannter Herausgeber“ — das stimmt: Für diese
   Fassung bürgt noch kein Zertifikat.
3. Das Setup installiert nur für Ihr Benutzerkonto und braucht keine
   Administratorrechte. Fehlt Windows die Webansicht von Microsoft
   (WebView2), lädt es sie nach; dafür braucht es dann einmal Internet.
4. Danach starten Sie CabrikSecure über das Startmenü.

Meldet CabrikSecure beim Start auf Englisch, es finde die „WebView2 Runtime“
nicht, fehlt die Webansicht doch. Der Link in der Meldung führt auf eine
Seite für Entwickler, auf der man leicht das falsche Paket erwischt.
Nehmen Sie stattdessen [Microsofts Seite für
Nutzer](https://developer.microsoft.com/microsoft-edge/webview2/consumer/),
installieren Sie von dort und starten Sie CabrikSecure noch einmal.

> **Windows 11 mit „Intelligenter App-Steuerung“** (englisch *Smart App
> Control*) lässt unsignierte Programme gar nicht zu; einen Knopf
> „Trotzdem ausführen“ gibt es dann nicht. Ob sie bei Ihnen an ist, sehen
> Sie unter **Windows-Sicherheit → App- & Browsersteuerung → Intelligente
> App-Steuerung**. Steht dort „Aus“ oder „Auswertung“, geht alles wie oben
> beschrieben. Steht dort „Ein“, schalten Sie sie dafür bitte **nicht** ab
> — je nach Windows-Stand lässt sie sich danach nur mit einer
> Neuinstallation wieder einschalten. Sagen Sie stattdessen Bescheid: Auf
> diesem Rechner geht es erst mit einer signierten Fassung.

### macOS

Auf einem echten Mac ist diese Fassung noch nicht gestartet worden — Sie
gehören zu den Ersten. Weicht etwas von dieser Beschreibung ab, ist genau
das eine wertvolle Rückmeldung.

1. Doppelklick auf `CabrikSecure_0.1.4_aarch64.dmg`, dann „CabrikSecure“
   in den Ordner „Programme“ ziehen.
2. CabrikSecure starten. macOS lehnt ab, weil Apple das Programm nicht auf
   Schadsoftware prüfen konnte. Schließen Sie die Meldung — **nicht** mit
   „In den Papierkorb legen“.
3. **Systemeinstellungen → Datenschutz & Sicherheit** öffnen und nach unten
   scrollen. Dort steht, dass „CabrikSecure“ blockiert wurde. Klicken Sie
   auf **Dennoch öffnen** und bestätigen Sie mit Ihrem Passwort oder Touch
   ID. Fragt macOS danach noch einmal, wieder mit **Dennoch öffnen**.
4. Ab jetzt startet es wie jedes andere Programm.

Heißt es stattdessen, „CabrikSecure“ sei **beschädigt**, liegt das nicht
an der Datei — sofern die Prüfsumme gestimmt hat —, sondern daran, wie sie
versiegelt ist. Sagen Sie bitte Bescheid. Weiter kommen Sie mit diesem
Befehl im Terminal:

```sh
xattr -dr com.apple.quarantine "/Applications/CabrikSecure.app"
```

### Linux

Auch unter Linux ist diese Fassung noch auf keinem echten Rechner
gestartet worden.

**Debian, Ubuntu, Linux Mint** — im Terminal:

```sh
sudo apt install "$HOME/Downloads/CabrikSecure_0.1.4_amd64.deb"
```

Eine Notiz am Ende über `_apt` und „Permission denied“ ist harmlos. Danach
steht „CabrikSecure“ im Anwendungsmenü.

**Andere Systeme** — das AppImage läuft ohne Installation. Einmal
ausführbar machen, dann starten:

```sh
chmod +x "$HOME/Downloads/CabrikSecure_0.1.4_amd64.AppImage"
"$HOME/Downloads/CabrikSecure_0.1.4_amd64.AppImage"
```

Meldet es, `libfuse.so.2` fehle, braucht es FUSE 2: unter Fedora
`sudo dnf install fuse-libs`, unter Arch `sudo pacman -S fuse2`, unter
Ubuntu `sudo apt install libfuse2t64`.

## 3. Der erste Start

CabrikSecure führt Sie zur **Einrichtung**: Sie legen Ihre Identität an — Ihr
Schlüsselpaar — und wählen ein Passwort. Haben Sie schon eine Sicherung
aus CabrikSecure, etwa von einem anderen Rechner, wählen Sie dort stattdessen
„Sicherung einspielen“.

- **Das Passwort lässt sich nicht zurücksetzen.** Vergessen Sie es, ist
  alles dauerhaft unlesbar, was an Sie verschlüsselt wurde. Auch wir
  kommen nicht heran.
- **Legen Sie danach eine Sicherung an:** unter „Identität“ mit
  „Schlüsseldatei sichern“, etwa auf einen USB-Stick. Ist „Kontakte
  mitsichern“ angehakt, entsteht daneben eine zweite Datei; bewahren Sie
  beide zusammen auf.
- **Die Hilfe** finden Sie oben links bei den anderen Bereichen. „Erste
  Schritte“ führt durch das Wichtigste, „Beispiele“ zeigt Lagen, die man
  selten von selbst herstellt.

CabrikSecure braucht kein Internet und kein Konto. Es schickt uns nichts, auch
keine Absturzberichte — was hakt, erfahren wir nur von Ihnen.

## Was Sie ausprobieren können

Am meisten hilft es, wenn Sie CabrikSecure benutzen, wie Sie es im Alltag
täten — am besten zu zweit, mit einem anderen Tester:

1. Kontakte austauschen und einmal am Telefon vergleichen.
2. Eine Datei verschlüsseln, auf Ihrem gewohnten Weg schicken und beim
   anderen öffnen lassen. Ein Foto vom Handy eignet sich gut: Lesen Sie,
   was CabrikSecure über seine Metadaten sagt.
3. Eine Textnachricht hin und zurück.
4. Sperren, entsperren, das Programm schließen und wieder öffnen.
5. „Sicher löschen“ an einer **Kopie** ausprobieren.
6. Sichern und wieder einspielen: unter „Identität“ sichern, die Identität
   löschen und die Sicherung in der Einrichtung einspielen. **Nur mit einer
   Identität, an der noch nichts hängt** — geht dabei etwas schief, ist
   alles fort, was an sie gerichtet ist.
7. Eine große Datei, etwa ein langes Video vom Handy, hin und zurück
   schicken. Über 64 MiB prüft CabrikSecure beim Öffnen alles, hält den
   Inhalt aber nicht fest; beim Speichern wird ein zweites Mal
   entschlüsselt. Dass das Speichern so lange dauert wie das Öffnen, ist
   also gewollt — dass der Rechner dabei langsam wird, nicht.

Zwei Grenzen dieser Fassung kennen wir schon: Große PDF-, Office- und
ZIP-Dateien und große Bilder liegen beim Bereinigen noch ganz im
Arbeitsspeicher — Videos und Dateien, deren Format CabrikSecure nicht
kennt, dagegen nicht mehr. Und QR-Codes lassen sich noch nicht abscannen.

## Rückmeldungen

Schreiben Sie der Person, die Sie eingeladen hat. Alles ist willkommen,
auch „das habe ich nicht verstanden“. Am meisten hilft:

- Ihr System, etwa „Windows 11“, „macOS 15“ oder „Ubuntu 24.04“
- die Fassung — sie steht im Namen der Datei, etwa „0.1.4“
- was Sie getan haben, was Sie erwartet haben und was stattdessen geschah
- ein Bildschirmfoto, wenn es etwas zeigt

**Schicken Sie nie Ihr Passwort oder Ihre Schlüsseldatei mit** — und
nichts, was privat bleiben soll.

## Eine neue Fassung

Kommt eine neue Fassung, stehen ihre Prüfsummen hier, mit einer höheren
Nummer. Installieren Sie sie einfach über die alte: unter Windows mit dem
neuen Setup, unter macOS durch Ersetzen in „Programme“, unter Linux wie
oben. Identität und Kontakte bleiben erhalten, mit demselben Passwort.

**Haben Sie eine Fassung vor 0.1.3, hieß sie noch „Cabrik Secure“** —
und das neue Setup erkennt sie nicht als Vorgängerin. Deinstallieren Sie
sie unter Windows deshalb zuerst (siehe unten), **ohne** den Haken bei
„Lösche die Anwendungsdaten“: Dann bleiben Identität und Kontakte. Danach
installieren Sie die neue Fassung. In dieser Reihenfolge, denn das Deinstallieren
trägt auch aus, welches Programm `.cabrik`-Dateien öffnet. Unter macOS
löschen Sie das alte „Cabrik Secure“ aus „Programme“.

## Deinstallieren

Identität und Kontakte liegen nicht beim Programm, sondern in Ihrem
Benutzerverzeichnis: unter Windows in `%APPDATA%\CabrikSecure`, unter macOS
und Linux in `~/.config/cabrik`.

- **Windows:** Einstellungen → Apps, bei CabrikSecure „Deinstallieren“.
  Das Setup bietet „Lösche die Anwendungsdaten“ an. Setzen Sie den Haken,
  fragt es danach ausdrücklich, ob auch Schlüssel und Kontakte gelöscht
  werden sollen; vorgewählt ist „Nein“.
- **macOS:** CabrikSecure aus „Programme“ in den Papierkorb ziehen.
- **Linux:** `sudo apt remove cabrik-secure`, beim AppImage die Datei
  löschen.

Unter macOS und Linux bleiben Identität und Kontakte liegen, bis Sie den
Ordner selbst löschen; im Finder kommen Sie mit ⇧⌘G und
`~/.config/cabrik` hin. **Bedenken Sie vorher:** Was an Sie verschlüsselt
wurde, lässt sich danach nicht mehr öffnen.

## Warum es keine signierte Fassung gibt

Eine Signatur setzt ein Zertifikat voraus, das auf einen Herausgeber
ausgestellt ist. Das kommt nach dieser Testphase. Bis dahin übernimmt die
Prüfsumme hier einen Teil dessen, was eine Signatur leisten würde: Sie
zeigt, dass Ihre Datei die ist, die hier gebaut wurde. Wer dafür bürgt,
zeigt sie nicht — das tut in dieser Testphase die Person, die Sie
eingeladen hat.
