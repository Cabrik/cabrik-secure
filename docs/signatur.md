# Code Signing — der Weg zur signierten Auslieferung

Dieses Dokument beantwortet eine einzige Frage: **Was muss geschehen, damit
ein Nutzer den Installer von Cabrik Secure ausführen kann, ohne von Windows
gewarnt zu werden — und in welcher Reihenfolge?**

Es steht getrennt vom [Fahrplan](ROADMAP.md), weil es Entscheidungen enthält,
die außerhalb des Quelltexts fallen: Rechtsform, Kosten, Fristen. Der
Fahrplan verweist unter 5.3 hierher.

Stand: 24. August 2026. Die Belege stehen am Ende.

---

## 1. Die unbequeme Wahrheit zuerst

**Eine Signatur beseitigt die SmartScreen-Warnung nicht.** Sie ist
notwendig, aber nicht hinreichend.

Microsofts eigene Darstellung, wörtlich:

| Weg | Beim ersten Download |
| --- | --- |
| Microsoft Store | ✅ nie eine Warnung — Microsoft signiert neu |
| Gültiges Zertifikat (OV/EV) | ⚠️ Warnung, bis Reputation entsteht; immerhin **mit** verifiziertem Namen |
| Keine Signatur | ⚠️ „Windows hat Ihren PC geschützt" |
| Selbstsigniert | ⚠️ dasselbe wie ohne Signatur |

Der Aufbau der Reputation dauert *„several weeks and hundreds of clean
installs from a wide audience"*. Es gibt für Endkunden **kein** Verfahren,
eine Datei zur Prüfung einzureichen; sie entsteht allein durch
Download-Volumen.

Was die Signatur konkret bringt:

- Der **Name des Herausgebers** steht in der Warnung, statt „Unbekannter
  Herausgeber". Bei einem Verschlüsselungsprogramm ist das der Unterschied
  zwischen „wer ist das?" und „das ist der, von dem ich es geladen habe".
- Die Reputation **überträgt sich auf neue Fassungen**, solange dieselbe
  Identität signiert. Unsignierte Dateien fangen bei jeder Version bei null
  an.
- **Smart App Control** unter Windows 11 blockiert unsignierte Dateien
  vollständig — dort ist die Signatur nicht Kosmetik, sondern
  Voraussetzung für die Ausführung überhaupt.

### Was sich geändert hat und in älteren Ratgebern noch falsch steht

**EV-Zertifikate bringen keine sofortige Reputation mehr.** Das war jahrelang
ihr Hauptargument und der Grund für den Aufpreis. Microsoft schreibt heute:

> „EV certificates no longer bypass SmartScreen. … Paying a premium for EV
> solely to avoid SmartScreen warnings is no longer justified."

Hintergrund: Zum August 2024 wurden die EV-Kennungen aus den Wurzeln des
Microsoft Trusted Root Program entfernt; seither werden alle
Code-Signing-Zertifikate gleich behandelt. Verkäufer von EV-Zertifikaten
behaupten teilweise weiterhin das Gegenteil — beim Prüfen dieses Dokuments
tat es SSL.coms eigene FAQ.

**Für uns heißt das: kein EV.** Es kostet mehr und leistet für unseren Zweck
nichts.

---

## 2. Die drei Wege

### Weg A — Azure Artifact Signing (früher Trusted Signing)

Microsofts eigener Dienst. ~10 $/Monat, kein Hardware-Token, fügt sich in
GitHub Actions ein. Technisch der bequemste Weg.

**Für uns zu.** Die Anleitung sagt wörtlich:

> „Public Trust certificates are available to organizations in the United
> States, Canada, the European Union … **Individual developers must be
> located in the United States or Canada.**"

Also: In der EU nur für **Organisationen**. Und dort verlangt die Prüfung
**drei Jahre nachweisbare Steuerhistorie**. Ein heute angemeldetes Gewerbe
erfüllt das im Jahr 2029.

Microsoft hat angekündigt, das Verfahren auf jüngere Organisationen
auszuweiten. Ein Datum dafür gibt es nicht.

### Weg B — Zertifikat einer klassischen Zertifizierungsstelle

Certum (Polen, EU), SSL.com, Sectigo, GlobalSign. Für Einzelpersonen heißt
die Bauart **OV** beziehungsweise **IV** („Individual Validation").

- **Offen für Einzelpersonen in Deutschland** — das ist der entscheidende
  Unterschied zu Weg A
- Prüfung: **2–4 Werktage**, bei Certum automatisiert per Ausweis,
  Gesichtsabgleich und einer Rechnung, die die Anschrift belegt
- Seit dem 1. Juni 2023 muss der private Schlüssel auf **FIPS-140-2-Level-2**-
  Hardware liegen: USB-Token oder Cloud-HSM. Eine `.pfx`-Datei zum
  Herunterladen gibt es nicht mehr
- Cloud-Varianten (Certum Cloud, SSL.com eSigner) lassen sich in die CI
  einbinden; ein USB-Token nicht

**Wichtig für die Planung:** Seit Ende Februar 2026 gilt eine
Höchstlaufzeit von **459 Tagen** statt der früheren drei Jahre. Die Uhr
läuft ab Ausstellung, nicht ab erster Auslieferung.

### Weg C — Microsoft Store

Der einzige Weg **ohne** Warnung. Microsoft signiert die App neu, sie trägt
volle Reputation ab dem ersten Download.

Bei genauerem Hinsehen ist er stärker, als er zunächst klang:

- **Microsoft signiert für dich.** Wörtlich aus der Anleitung: *„The
  Microsoft Store will sign the MSIX for you, no need to sign before
  submission."* Auf diesem Weg wird **kein Zertifikat gekauft** — die
  100–250 € und die 459-Tage-Uhr entfallen ersatzlos
- **Die Anmeldung als Einzelentwickler ist seit September 2025
  kostenlos.** Die frühere einmalige Gebühr ist weg; an ihre Stelle tritt
  eine Identitätsprüfung mit Ausweis und Selfie — dasselbe Verfahren, das
  auch eine Zertifizierungsstelle verlangt
- **Tauri wird ausdrücklich unterstützt.** Microsoft dokumentiert die
  Paketierung mit der `winapp`-Befehlszeile für Tauri-Anwendungen, samt
  Beispielprojekt. Die Seite ist vom 19. August 2026, also aktuell
- **Kein Umbau am Programm.** `winapp init` legt ein
  `Package.appxmanifest` und die Symbole an, `winapp pack` schnürt die
  MSIX aus der fertigen `.exe`. Der Rust-Kern bleibt unangetastet

**Was dagegen steht — und das ist ernst zu nehmen:**

- **`broadFileSystemAccess` ist eine eingeschränkte Berechtigung.** Sie
  unterliegt einer gesonderten Prüfung, und Microsoft verlangt eine
  Begründung, warum die App sie braucht. Ein Verschlüsselungsprogramm
  braucht sie zwangsläufig: Es soll die Dateien des Nutzers verschlüsseln,
  nicht nur die in seinem eigenen Ordner
- **Das sichere Löschen könnte auffallen.** Ein Programm, das Dateien
  überschreibt, ist in einer Prüfung erklärungsbedürftig. Unsere Position
  ist gut — wir behaupten gerade **nicht** zu viel, sondern legen die
  Grenzen offen (`spec/shredding.md`) —, aber ein Prüfer muss das lesen
  wollen
- **Je Architektur ein Paket** (x64, ARM64)
- **Die Werkzeuge verlangen Windows 11**
- **Die Verschlüsselungs-Deklaration** wird dort verbindlich
  (`ITSAppUsesNonExemptEncryption`, ECCN 5D992) — sie steht unter 5.3
  ohnehin an

**Nicht entschieden.** Aber der Weg verdient eine ernsthafte Prüfung, bevor
Geld für ein Zertifikat ausgegeben wird: Er ist der einzige ohne Warnung,
und er ist der einzige ohne laufende Kosten.

**Denkbar ist auch beides.** Der Store für die Breite, ein signierter
Installer daneben für alle, die nicht über den Store installieren wollen —
bei einem Verschlüsselungsprogramm ist das ein realer Teil der
Zielgruppe. Dann braucht es doch ein Zertifikat, aber die Entscheidung
darüber fiele später und mit besserem Wissen.

---

## 3. Empfehlung

### Gewerbe: ja — aber nicht wegen der Signatur

Ein Zertifikat bekämst du auch als Privatperson (Weg B). Die Gründe für die
Anmeldung liegen woanders:

1. **Der Name auf dem Zertifikat ist der Name in der Warnung.** Als
   Privatperson steht dort dein bürgerlicher Name, nicht „Cabrik". Bei einem
   Produkt, das unter einem Namen auftreten soll, ist das ein Bruch.
2. **Die Identität lässt sich schlecht wechseln.** Microsoft rät
   ausdrücklich: *„Use a consistent signing identity — changing your signing
   certificate affects the publisher trust signal."* Wer erst privat
   signiert und später als Gewerbe, wirft die aufgebaute Reputation weg —
   also genau das, was Wochen und hunderte Installationen gekostet hat.
3. **Die Marke braucht einen Inhaber.** 5.3a steht ohnehin an.
4. **Kommerzieller Vertrieb braucht es ohnehin.** Es vorzuziehen kostet
   wenig; es nachzuholen kostet die Reputation aus Punkt 2.

Was es nicht ist: eine Voraussetzung für Azure. Die drei Jahre laufen so
oder so.

### Zertifikat: noch nicht kaufen

Das ist die Korrektur an meiner früheren Empfehlung. Sie beruhte auf Azures
Vorlauf von *Tagen bis Wochen* — und Azure steht uns nicht offen. Bei Weg B
sind es **2–4 Werktage**.

Dagegen steht die verkürzte Laufzeit: **459 Tage ab Ausstellung**. Ein heute
gekauftes Zertifikat ist zur Hälfte verbraucht, bevor es je etwas signiert
hat, das ein Nutzer herunterlädt.

**Kaufen, wenn ein auslieferbarer Stand existiert** — also wenn der
Installer auf einem frischen Windows geprüft ist und die Entscheidung
Store/kein Store gefallen ist.

### Reihenfolge

| # | Schritt | Wer | Dauer |
| --- | --- | --- | --- |
| 1 | Namensfrage endgültig klären (Marke, 5.3a) | du | — |
| 2 | Gewerbe anmelden | du | meist derselbe Tag |
| 3 | Store-Weg bewerten (Weg C gegen Weg B) | gemeinsam | — |
| 4 | Installer auf frischem Windows prüfen | du | ein Nachmittag |
| 5 | Bau auf Signieren vorbereiten | ich | — |
| 6 | Zertifikat kaufen und Prüfung durchlaufen | du | 2–4 Werktage |
| 7 | Signatur in die CI einhängen | ich | — |
| 8 | Reputation aufbauen | Zeit | Wochen |

Schritt 5 hängt an nichts und kann sofort geschehen: Der Bau bekommt eine
Stelle, an der signiert wird, die ohne Zertifikat nichts tut und den Bau
nicht anhält. Dann ist Schritt 7 ein Schalter statt eines Umbaus.

---

## 4. Was das kostet

| Posten | Betrag | Anmerkung |
| --- | --- | --- |
| Gewerbeanmeldung | ~20–65 € einmalig | je nach Gemeinde |
| OV/IV-Zertifikat | ~100–250 € pro Laufzeit | Certum am günstigsten, EU-Anbieter |
| Cloud-Signierung oder Token | teils enthalten | Cloud, wenn die CI signieren soll |
| Azure Artifact Signing | ~10 $/Monat | **erst ab 2029 erreichbar** |
| macOS-Notarisierung | 99 $/Jahr | erst, wenn macOS wirklich beliefert wird |

Die Preisspannen stammen von Händlerseiten und sind nicht geprüft; sie
dienen der Größenordnung.

---

## 5. Was offen ist und hier nicht behauptet wird

- **Ob ein deutsches Einzelunternehmen den Firmennamen ins Zertifikat
  bekommt** oder den bürgerlichen Namen des Inhabers. Ein Einzelunternehmen
  ist keine eigene juristische Person; die Zertifizierungsstellen handhaben
  das unterschiedlich. **Vor dem Kauf beim Anbieter erfragen** — davon
  hängt ab, was der Nutzer in der Warnung liest, und das ist der halbe Zweck
  der Übung.
- **Wann Microsoft Azure Artifact Signing für jüngere Organisationen
  öffnet.** Angekündigt, ohne Datum.
- **Ob der Store-Weg für dieses Produkt tragbar ist.** Nicht bewertet.
- **Die genauen Preise.** Siehe oben.

---

## Quellen

- [SmartScreen reputation for Windows app developers](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation) — Microsoft, Stand 17.08.2026. Quelle für die Tabelle in §1, für den Wegfall des EV-Vorteils und für „consistent signing identity"
- [Quickstart: Set up Artifact Signing](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart) — Microsoft, Stand 11.08.2026. Quelle für die Länderbeschränkung
- [Azure Artifact Signing — Eignung neuer Organisationen](https://learn.microsoft.com/en-us/answers/questions/5977141/azure-artifact-signing-trusted-signing-is-a-us-llc) — Microsoft Q&A. Quelle für die Drei-Jahres-Regel
- [Code Signing — required documents](https://support.certum.eu/en/code-signing-required-documents/) — Certum. Quelle für Prüfdauer und Unterlagen
- [Transition to shorter Code Signing certificate validity periods](https://www.certum.eu/en/news/shortening-code-signing-certificate-validity/) — Certum. Quelle für die 459 Tage
- [Which Code Signing Certificate do I Need? EV or OV?](https://www.ssl.com/faqs/which-code-signing-certificate-do-i-need-ev-ov/) — SSL.com. Genannt als **Gegenbeispiel**: behauptet weiterhin den EV-Vorteil
- [EV Certs do not grant immediate reputation anymore](https://www.todesktop.com/blog/posts/windows-apps-psa-ev-certs-do-not-grant-immediate-reputation-anymore) — ToDesktop. Bestätigt den Wegfall unabhängig
- [Using winapp CLI with Tauri](https://learn.microsoft.com/en-us/windows/apps/dev-tools/winapp-cli/guides/tauri) — Microsoft, Stand 19.08.2026. Quelle für die MSIX-Paketierung und für „The Microsoft Store will sign the MSIX for you"
- [Free developer registration for individual developers](https://blogs.windows.com/windowsdeveloper/2025/09/10/free-developer-registration-for-individual-developers-on-microsoft-store/) — Windows Developer Blog, 10.09.2025. Quelle für den Wegfall der Anmeldegebühr
- [broadFileSystemAccess — App Submission and Approval](https://learn.microsoft.com/en-us/answers/questions/672768/broadfilesystemaccess-app-submission-and-approval) — Microsoft Q&A. Quelle für die gesonderte Prüfung eingeschränkter Berechtigungen
