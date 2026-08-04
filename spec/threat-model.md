# Cabrik Secure — Threat Model

**Status:** Entwurf · Phase 1, Dokument 1 von 7
**Gilt für:** v2.0

Dieses Dokument legt fest, wogegen Cabrik Secure schützt und wogegen
ausdrücklich nicht. Alle weiteren Spezifikationen leiten sich daraus ab.
Wo eine Schutzwirkung nicht erreichbar ist, wird sie hier benannt statt
im Produkt suggeriert.

---

## 1. Was Cabrik Secure ist

Ein Offline-Werkzeug, das Dateien und Textnachrichten in verschlüsselte
Envelopes verwandelt. Es transportiert nichts. Der Nutzer bringt den Envelope
selbst zum Empfänger — per E-Mail-Anhang, Messenger, USB-Stick, Cloud-Ablage.

Diese Abgrenzung ist grundlegend: Cabrik Secure schützt den **Inhalt** und
Aussagen über den **Absender**. Es schützt nicht die Tatsache, dass eine
Übertragung stattfand.

## 2. Schutzgüter

| # | Gut | Bedeutung |
|---|---|---|
| S1 | Vertraulichkeit des Inhalts | Ohne passenden Private Key ist der Klartext nicht zugänglich |
| S2 | Integrität des Inhalts | Jede Veränderung am Envelope wird beim Entschlüsseln erkannt |
| S3 | Absender-Authentizität | Der Empfänger kann feststellen, dass eine Nachricht von einem *verifizierten Kontakt* stammt |
| S4 | Absender-Anonymität | Wahlweise: der Envelope enthält nichts, was den Absender identifiziert |
| S5 | Vertraulichkeit ruhender Schlüssel | Ein gestohlenes Keyfile ist ohne Passwort wertlos |
| S6 | Metadaten-Hygiene der Nutzdaten | Eingebettete Metadaten in Anhängen werden erkannt und entfernbar gemacht |
| S7 | Metadaten-Hygiene des Envelopes | Der Envelope selbst verrät nichts über Inhalt, Absender oder Dateiname |

**S3 und S4 schließen sich pro Nachricht aus, aber nicht im Produkt.** Der
Nutzer wählt pro Nachricht. In v1 war diese Wahl kaputt: der Signaturschlüssel
stand im Klartext-Header, wodurch S3 automatisch S4 zerstörte. Siehe §6.1.

## 3. Angreifermodelle

### A1 — Passiver Beobachter des Envelopes

Sieht die `.enc`-Datei, hat keine Schlüssel. Typisch: Mail-Provider,
Cloud-Anbieter, Netzwerk-Mitschnitt, Fund auf einem USB-Stick.

**Abgewehrt:** S1, S2, S7. Aus dem Envelope ist weder Inhalt noch Dateiname,
Absender oder Klartextgröße ableitbar (letztere nur grob über die Envelope-Größe).

### A2 — Aktiver Manipulator

Verändert den Envelope unterwegs, tauscht Teile aus, spielt alte Envelopes
erneut ein.

**Abgewehrt:** S2. Manipulation führt zu einem Fehlschlag, nie zu falschem
Klartext. Auch das Abschneiden eines gestreamten Envelopes wird erkannt (§7.3).

**Nicht abgewehrt:** Erneutes Einspielen eines *unveränderten* alten Envelopes.
Cabrik Secure kennt keinen Sitzungszustand und kann Replay nicht verhindern.
Der Zeitstempel liegt im verschlüsselten Teil und wird dem Empfänger angezeigt —
die Bewertung bleibt beim Menschen.

### A3 — Identitätsfälscher

Versucht, eine Nachricht als jemand anderes erscheinen zu lassen.

**Abgewehrt:** S3 — **aber nur für verifizierte Kontakte.** Eine Signatur, deren
Schlüssel nicht im Trust Store steht, beweist nichts über die Person. Genau das
war der Konstruktionsfehler in v1 (§6.1). Das UI muss den Unterschied sichtbar
machen (§8).

### A4 — Angreifer mit dem Keyfile

Hat die Keyfile-Datei, nicht das Passwort. Typisch: gestohlener Laptop,
Backup in fremder Hand, beschlagnahmter Datenträger.

**Abgewehrt:** S5, im Rahmen der Passwortstärke. Argon2id begrenzt die
Rate von Rateversuchen; ein schwaches Passwort bleibt schwach.

### A5 — Angreifer mit Zugriff auf das ausgeschaltete Gerät

Ausgebaute Festplatte, forensisches Image.

**Teilweise abgewehrt.** Keyfiles sind geschützt (S5). Entschlüsselte Dateien,
die der Nutzer selbst gespeichert hat, sind es nicht — das ist auch nicht
Aufgabe dieser Software. Reste früherer Klartext-Daten: siehe §7.4.

### A6 — Angreifer mit Zugriff auf das laufende, entsperrte Gerät

**Nicht abgewehrt. Grundsätzlich und dauerhaft nicht.** Wer Code im
Nutzerkontext ausführt, kann Tastatureingaben mitlesen, Prozessspeicher
auslesen und entschlüsselte Inhalte abgreifen, während sie geöffnet sind.
Keine Anwendung auf einem kompromittierten System kann das verhindern.

Cabrik Secure erschwert es lediglich: Schlüsselmaterial wird nach Gebrauch
zeroisiert, Klartext wird nicht unnötig auf Platte geschrieben, das Passwort
bleibt nicht dauerhaft im Speicher (anders als in v1).

### A7 — Verkehrsanalytiker

Beobachtet, *dass* und *wann* kommuniziert wird, und mit welchem Volumen.

**Nicht abgewehrt.** Cabrik Secure hat keinen Transportkanal und kann darüber
keine Aussage treffen. Die Envelope-Größe verrät die ungefähre Klartextgröße;
optionales Padding (§7.2) mildert das, beseitigt es nicht.

### A8 — Rechtlicher Zwang gegen den Nutzer

Herausgabeanordnung für Passwort oder Schlüssel.

**Nicht abgewehrt.** Es gibt keine Deniability-Funktion, keine versteckten
Volumes, keine Zwei-Passwort-Tricks. Wer das braucht, braucht ein anderes
Werkzeug. Diese Entscheidung ist bewusst: glaubhafte Abstreitbarkeit korrekt
zu bauen ist erheblich schwerer, als es aussieht, und eine schlecht gebaute
Variante ist gefährlicher als gar keine.

### A9 — Angreifer gegen die Auslieferungskette

Manipuliert das Installationspaket oder ein Update.

**Teilweise abgewehrt**, ab Phase 5: signierte Binaries, signierte Updates,
veröffentlichte Prüfsummen, quelloffener Krypto-Kern. Vor Phase 5 besteht
dieser Schutz nicht.

Dies ist der Hauptgrund gegen eine gehostete Web-App: dort liefert der Server
den Krypto-Code bei jedem Aufruf neu aus, und eine gezielte Manipulation
gegen einen einzelnen Nutzer wäre praktisch nicht nachweisbar.

## 4. Vertrauensannahmen

Cabrik Secure setzt voraus:

1. Das Gerät des Nutzers ist zum Zeitpunkt der Nutzung nicht kompromittiert (A6).
2. Der Nutzer verifiziert Kontakte über einen zweiten Kanal, bevor er ihnen
   Authentizität zuschreibt (§8).
3. Das Keyfile-Passwort ist ausreichend stark und nicht anderweitig bekannt.
4. Die verwendeten Primitiven (X25519, ChaCha20-Poly1305, Ed25519, Argon2id,
   SHA-256) sind sicher.
5. Der Zufallszahlengenerator des Betriebssystems ist brauchbar.

Fällt eine dieser Annahmen, fallen die zugehörigen Schutzwirkungen.

## 5. Ausdrücklich außerhalb des Schutzbereichs

- Kompromittiertes Endgerät (A6)
- Verkehrsanalyse (A7)
- Rechtlicher Zwang, glaubhafte Abstreitbarkeit (A8)
- Replay unveränderter Envelopes (A2)
- Schutz des Klartexts, nachdem der Nutzer ihn bewusst gespeichert hat
- Vollständige Metadaten-Entfernung aus Formaten, die das Programm nicht
  versteht (§7.5)
- Garantierte Nichtwiederherstellbarkeit gelöschter Dateien auf SSDs (§7.4)
- Absender-Anonymität gegenüber dem *Empfänger*, wenn signiert wurde —
  das ist der Zweck der Signatur

## 6. Konsequenzen für das Format

### 6.1 Der Header darf nichts verraten

Aus einem v1-Envelope liest jeder ohne Schlüssel: Dateiname, exakte
Klartextgröße, Empfänger-Fingerprint, Zeitstempel, verwendetes Programm — und
bei signierten Nachrichten den **persistenten Signatur-Public-Key des
Absenders**.

Damit ist S4 in v1 unvereinbar mit S3: Wer signiert, um authentisch zu sein,
gibt seine dauerhafte Identität an jeden preis, der die Datei in die Hände
bekommt. Der ephemere Schlüsselaustausch macht den Absender unsichtbar, der
Header hebt das unmittelbar wieder auf.

**Anforderung an v2:** Im unverschlüsselten Teil steht ausschließlich, was zum
Entschlüsseln zwingend erforderlich ist — Formatkennung, Version, Ciphersuite,
KEM-Kapseln, Nonce-Basis. Alles andere, insbesondere Absenderidentität,
Signatur, Dateiname, Zeitstempel und Klartextgröße, liegt im verschlüsselten
Teil.

### 6.2 Empfänger dürfen nicht verknüpfbar sein

Der `recipient_fp` in v1 erlaubt es, gesammelte Envelopes demselben Empfänger
zuzuordnen, ohne einen einzigen davon zu entschlüsseln.

**Anforderung an v2:** Kein Empfänger-Identifikator im Klartext. Bei mehreren
Empfängern probiert der Leser die Kapseln durch (Trial Decryption). Die Anzahl
der Empfänger bleibt sichtbar und wird über Dummy-Kapseln optional verschleiert.

### 6.3 Das Programm darf sich nicht ausweisen

`"branding": "Cabrik Secure"` im Klartext. In einem Umfeld, in dem schon der
Besitz von Verschlüsselungssoftware belastend ist, ist das ein reales Risiko.

**Anforderung an v2:** Die Formatkennung ist ein kurzer, neutraler Magic-Wert.
Produktname und Version stehen im verschlüsselten Teil oder nirgends.

### 6.4 Authentizität muss vom Trust Store abhängen

**Anforderung an v2:** Die Bibliothek gibt niemals ein einfaches
`signature_valid: true` zurück, sondern einen dreiwertigen Zustand, der die
Kenntnis des Schlüssels einschließt (§8). Ein Aufrufer soll den Unterschied
nicht versehentlich einebnen können.

## 7. Konsequenzen für die Implementierung

### 7.1 Klartext gehört nicht auf die Platte

v1 packt mehrere Anhänge in ein **unverschlüsseltes** ZIP unter
`tempfile.mkdtemp()` und verschlüsselt erst danach. Das Klartext-ZIP wird zwar
gelöscht, liegt aber vorher vollständig auf dem Datenträger — mit allen
Konsequenzen aus §7.4.

**Anforderung:** Mehrere Dateien werden im Stream verpackt und verschlüsselt.
Wo eine temporäre Ablage unumgänglich ist, geschieht sie in einem Container,
dessen Schlüssel nur im RAM existiert und danach zeroisiert wird.

### 7.2 Größe verrät Inhalt

Die Envelope-Größe verrät die ungefähre Klartextgröße. Bei kurzen Nachrichten
aus einem bekannten Satz kann das genügen.

**Anforderung:** Optionales Padding auf Größenklassen, per Voreinstellung
aktiv für Textnachrichten.

### 7.3 Streaming darf nicht abschneidbar sein

Wer einen gestreamten Envelope nach einem beliebigen Chunk abschneidet, darf
keinen gültigen Teilklartext erhalten.

**Anforderung:** Jeder Chunk trägt seine Position im AAD, der letzte ist
explizit als solcher markiert. Ein abgeschnittener Envelope schlägt fehl.

### 7.4 Sicheres Löschen ist auf SSDs nicht garantierbar

Wear-Leveling schreibt jeden Überschreibvorgang auf eine neue physische Seite;
die alte bleibt bis zur Garbage Collection lesbar. Dazu Over-Provisioning
(7–28 % unsichtbar), NTFS-Journal, Volume Shadow Copies, Pagefile,
Hibernation-Datei, Suchindex. Dateien unter etwa 700 Bytes liegen resident im
MFT-Eintrag und werden über die Datei gar nicht erreicht.

**Anforderung:** Nicht behaupten, was nicht geht. Das Werkzeug meldet, welche
Garantie auf dem konkreten Datenträger erreichbar ist. Die eigentliche Lösung
ist §7.1 — was nie im Klartext geschrieben wurde, muss nicht gelöscht werden.
Details in `shredding.md`.

### 7.5 Unbekannte Formate dürfen nicht als sauber gelten

v1 kopiert Formate, die es nicht kennt, unverändert durch und meldet keinen
Fehler. Der Nutzer darf daraus schließen, die Datei sei bereinigt worden.

**Anforderung:** Dreiwertiges Fähigkeitsmodell — vollständig bereinigt,
teilweise bereinigt mit Benennung des Rests, oder unbekanntes Format ohne
jede Aussage. Details in `metadata.md`.

### 7.6 Geheimnisse gehören nicht ins Frontend

**Anforderung:** Schlüsselmaterial, Passwörter und entschlüsselter Klartext
verlassen den Rust-Kern nicht als Kopie in die Webview, soweit vermeidbar.
Das Frontend erhält Handles, Statuswerte und Fortschritt.

## 8. Die drei Authentizitätszustände

Verbindlich für Bibliothek und Oberfläche. Die Bibliothek liefert diesen
Zustand, das UI stellt ihn dar — er wird nirgendwo auf einen Wahrheitswert
reduziert.

| Zustand | Bedeutung | Darstellung |
|---|---|---|
| **Verifiziert** | Signatur gültig, Schlüssel gehört zu einem im Trust Store als verifiziert markierten Kontakt | „Signiert von **Alice** ✓" — grün, mit Namen |
| **Unbekannt** | Signatur gültig, Schlüssel steht nicht im Trust Store oder ist unverifiziert | „Signiert von unbekanntem Schlüssel `K7QF…`" — neutral, **kein grüner Haken** |
| **Nicht signiert** | Keine Signatur vorhanden (anonymer Versand) | „Nicht signiert — Absender unbestimmt" — neutral |

Eine gültige Signatur eines unbekannten Schlüssels ist **keine** Authentizität.
Sie besagt nur, dass derselbe Schlüssel die Nachricht erzeugt hat — welcher
Schlüssel das ist, bleibt offen. v1 zeigte hier `signature_valid: true` und
legte damit einen Trugschluss nahe.

## 9. Zielgruppen und ihre Erwartungen

| Gruppe | Erwartung | Bedienbar? |
|---|---|---|
| Journalisten, Anwälte, Ärzte, Betriebsräte | Vertraulicher Dateiaustausch mit bekannten Gegenübern | ja — Kernzielgruppe |
| Unternehmen mit Compliance-Anforderungen | Nachvollziehbare Verschlüsselung ohne Cloud-Abhängigkeit | ja |
| Technisch versierte Privatnutzer | Ersatz für PGP-Dateiverschlüsselung | ja |
| Whistleblower unter staatlicher Beobachtung | Schutz auch bei Gerätezugriff und Verkehrsanalyse | **nein** — siehe A6, A7, A8 |

Die letzte Zeile ist wichtig: Cabrik Secure darf nicht den Eindruck erwecken,
gegen einen Angreifer zu schützen, der das Gerät kontrolliert oder den
Datenverkehr beobachtet. Die Dokumentation muss das ausdrücklich benennen.

## 10. Offene Punkte

- Post-Quantum: X25519 ist gegen künftige Quantencomputer nicht sicher
  („harvest now, decrypt later"). Eine Hybrid-Variante (X25519 + ML-KEM)
  ist für 2.0 nicht vorgesehen, aber die Ciphersuite-Verhandlung muss sie
  **nachrüstbar** halten. Zu klären in `envelope-v2.md`.
- Key-Rotation und Widerruf kompromittierter Schlüssel: nicht in 2.0,
  aber das Trust-Store-Format muss es später aufnehmen können.
- Größenklassen für Padding (§7.2): konkrete Werte in `envelope-v2.md`.
