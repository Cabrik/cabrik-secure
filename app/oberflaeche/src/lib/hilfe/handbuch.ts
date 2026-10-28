/**
 * Das Handbuch — für jemanden, der CabrikSecure zum ersten Mal öffnet.
 *
 * # Warum es im Programm steht und nicht daneben
 *
 * Entschieden am 27.10.2026: auf allen drei Systemen gleich, ohne
 * Startmenüeintrag oder PDF, das je System anders eingebunden werden
 * müsste — und vor allem immer zur installierten Fassung passend. Eine
 * Datei neben dem Programm veraltet still, sobald sich ein Knopf ändert.
 *
 * Nicht hier: wie man ein unsigniertes Programm installiert. Das braucht
 * man VOR der Installation, also gehört es auf die Seite für Tester.
 *
 * # Wie die Texte geschrieben sind
 *
 * Mit den Wörtern, die auf den Knöpfen stehen — wer „Weitergeben“ liest,
 * soll den Knopf „Weitergeben“ finden. Und nach `spec/anzeige.md` §5: kein
 * „sicher“ als Zusage, kein „anonym“, kein „vollständig gelöscht“. Ein
 * Handbuch, das mehr verspricht als der Bildschirm, macht die Ehrlichkeit
 * des Bildschirms wertlos. `handbuch.test.ts` prüft das.
 *
 * `**so**` wird fett gesetzt. Mehr Auszeichnung gibt es nicht: Die Texte
 * werden als Text eingesetzt, nie als HTML.
 */
import type { Beispielwahl } from "./beispiele";

/** Wohin ein Verweis führt. */
export type Ziel =
  | {
      art: "bereich";
      bereich: "empfangen" | "senden" | "kontakte" | "identitaet" | "werkzeuge" | "onboarding";
    }
  | { art: "kapitel"; kapitel: string }
  | { art: "beispiel"; beispiel: Beispielwahl };

export type Baustein =
  | { art: "absatz"; text: string }
  | { art: "liste"; punkte: string[] }
  | { art: "schritte"; punkte: string[] }
  | { art: "hinweis"; text: string }
  | { art: "verweis"; text: string; ziel: Ziel };

export interface Kapitel {
  kennung: string;
  titel: string;
  inhalt: Baustein[];
}

export const KAPITEL: Kapitel[] = [
  {
    kennung: "start",
    titel: "Erste Schritte",
    inhalt: [
      {
        art: "absatz",
        text:
          "CabrikSecure verschlüsselt Dateien und Textnachrichten so, dass nur die " +
          "Menschen sie öffnen können, für die Sie sie bestimmt haben. Es verschickt " +
          "selbst nichts: Die verschlüsselte Datei bringen Sie auf Ihrem gewohnten Weg " +
          "zum Empfänger — als E-Mail-Anhang, über einen Messenger, auf einem USB-Stick.",
      },
      {
        art: "schritte",
        punkte: [
          "**Identität anlegen** — unter „Einrichtung“. Dabei entsteht Ihr " +
            "Schlüsselpaar, verschlossen mit einem Passwort, das Sie selbst wählen.",
          "**Kontakte austauschen** — Sie geben Ihre Austausch-Nutzlast weiter " +
            "(unter „Identität“) und nehmen die Ihrer Gegenüber auf (unter „Kontakte“).",
          "**Vergleichen** — einmal je Kontakt, am Telefon oder persönlich. Erst " +
            "danach gilt ein Kontakt als verifiziert.",
          "**Senden und Empfangen** — Dateien unter „Senden“ verschlüsseln, " +
            "empfangene unter „Empfangen“ öffnen.",
        ],
      },
      { art: "verweis", text: "Zur Einrichtung", ziel: { art: "bereich", bereich: "onboarding" } },
      {
        art: "hinweis",
        text:
          "Wie das aussieht, zeigen die Beispiele — mit erfundenen Personen und " +
          "Dateien. Auf Ihrem Rechner geschieht dabei nichts.",
      },
      { art: "verweis", text: "Beispiele ansehen", ziel: { art: "kapitel", kapitel: "beispiele" } },
    ],
  },

  {
    kennung: "identitaet",
    titel: "Identität und Passwort",
    inhalt: [
      {
        art: "absatz",
        text:
          "Ihre Identität ist ein Schlüsselpaar. Den **öffentlichen** Teil geben Sie " +
          "weiter, damit andere Ihnen verschlüsselt schreiben können. Der andere Teil " +
          "bleibt auf Ihrem Rechner, verschlossen mit Ihrem Passwort. CabrikSecure zeigt ihn " +
          "nie an — auch Ihnen nicht.",
      },
      {
        art: "absatz",
        text:
          "Erkennbar ist eine Identität an ihrem **Fingerprint**, einer Folge von " +
          "Ziffern und Buchstaben, die aus dem öffentlichen Schlüssel berechnet wird. " +
          "Er ist öffentlich; Sie dürfen ihn überall hinschreiben.",
      },
      {
        art: "hinweis",
        text:
          "**Vergessen Sie das Passwort, ist alles dauerhaft unlesbar,** was an diese " +
          "Identität verschlüsselt wurde. Es gibt keine Wiederherstellung — auch " +
          "nicht durch den Hersteller.",
      },
      {
        art: "absatz",
        text:
          "**Sicherung.** Unter „Identität“ legt „Schlüsseldatei sichern“ eine Kopie an " +
          "einem zweiten Ort ab, etwa auf einem USB-Stick — auf Wunsch mit Ihren " +
          "Kontakten in einer zweiten Datei daneben. Beide sind verschlüsselt und " +
          "nützen niemandem, der Ihr Passwort nicht kennt. Zurück holen Sie die " +
          "Sicherung in der Einrichtung mit „Sicherung einspielen“: dieselbe " +
          "Identität, derselbe Fingerprint. Nach einer Passwortänderung öffnet sich " +
          "eine alte Sicherung weiter mit dem alten Passwort — legen Sie dann eine " +
          "neue an.",
      },
      {
        art: "absatz",
        text:
          "**Signierschlüssel.** Bei der Einrichtung entscheiden Sie, ob Ihre " +
          "Identität einen bekommt. Mit ihm können Empfänger prüfen, dass eine " +
          "Nachricht von Ihnen stammt. Ohne ihn ist keine Ihrer Nachrichten Ihnen " +
          "zuzuordnen — es kann dann aber auch niemand prüfen, dass sie von Ihnen kommt.",
      },
      {
        art: "absatz",
        text:
          "**Version 1.** Haben Sie CabrikSecure schon in Version 1 benutzt, " +
          "übernehmen Sie in der Einrichtung Ihren alten Schlüssel, statt einen neuen " +
          "anzulegen. Sonst kommen Sie an nichts mehr heran, was an den alten " +
          "gerichtet war.",
      },
      { art: "verweis", text: "Zur Identität", ziel: { art: "bereich", bereich: "identitaet" } },
    ],
  },

  {
    kennung: "kontakte",
    titel: "Kontakte austauschen und vergleichen",
    inhalt: [
      {
        art: "absatz",
        text:
          "Um jemandem zu schreiben, brauchen Sie seinen öffentlichen Schlüssel. Er " +
          "kommt als **Austausch-Nutzlast**: eine Zeichenfolge, die Ihr Gegenüber unter " +
          "„Identität“ mit „Weitergeben“ erzeugt — zum Kopieren, als Datei oder als " +
          "QR-Code. Sie enthält nur öffentliche Angaben.",
      },
      {
        art: "schritte",
        punkte: [
          "Ihr Gegenüber schickt Ihnen seine Austausch-Nutzlast, Sie ihm Ihre.",
          "Unter „Kontakte“ öffnet „+ Kontakt aufnehmen“ ein Feld. Fügen Sie die " +
            "Nutzlast ein oder laden Sie sie aus einer Datei, und geben Sie dem " +
            "Kontakt einen Namen. Den Namen vergeben Sie selbst — er steht nicht in " +
            "der Nutzlast.",
          "Dann **vergleichen** Sie: „Jetzt vergleichen“ zeigt eine Zahlenfolge. " +
            "Rufen Sie Ihr Gegenüber an — auf einem Weg, den Sie nicht über CabrikSecure " +
            "hergestellt haben — und lesen Sie sich die Ziffern gegenseitig vor. " +
            "Stimmen sie überein, klicken Sie „Sie stimmen überein“.",
        ],
      },
      {
        art: "hinweis",
        text:
          "**Warum der Vergleich nötig ist:** Wer den Weg kontrolliert, auf dem die " +
          "Nutzlast kommt, könnte sie gegen seine eigene austauschen. Der Vergleich " +
          "über einen zweiten Weg deckt das auf. Ohne ihn steht ein Kontakt auf " +
          "„bekannt, nicht verifiziert“, und sein Name ist nur ein Eintrag in Ihrem " +
          "eigenen Verzeichnis.",
      },
      {
        art: "absatz",
        text:
          "**Ein Kontakt hat plötzlich einen anderen Schlüssel?** Dann zeigt CabrikSecure " +
          "das an. Vergleichen Sie erneut, bevor Sie ihm wieder schreiben.",
      },
      {
        art: "absatz",
        text:
          "**Als kompromittiert markieren** — wenn Sie wissen, dass der Schlüssel " +
          "eines Kontakts in fremde Hände geraten ist. Das wirkt nur bei Ihnen: " +
          "Nachrichten mit diesem Schlüssel gelten danach als Fehler, auch wenn ihre " +
          "Signatur gültig ist.",
      },
      { art: "verweis", text: "Zu den Kontakten", ziel: { art: "bereich", bereich: "kontakte" } },
    ],
  },

  {
    kennung: "senden",
    titel: "Dateien und Texte verschlüsseln",
    inhalt: [
      {
        art: "schritte",
        punkte: [
          "Ziehen Sie Dateien in das Fenster, oder wählen Sie sie unter „Senden“ " +
            "mit „Dateien auswählen“.",
          "CabrikSecure sieht sich jede Datei an und sagt, was beim Verschlüsseln aus ihren " +
            "**Metadaten** wird — Angaben wie Aufnahmeort, Autorenname, Kameramodell " +
            "oder Bearbeitungszeit, die in vielen Dateien unsichtbar mitreisen.",
          "Wählen Sie die Empfänger. Nur sie können die Datei öffnen.",
          "„Verschlüsseln“. Neben jeder Datei entsteht eine zweite mit der Endung " +
            "**.cabrik** — die schicken Sie weg.",
        ],
      },
      {
        art: "hinweis",
        text:
          "**Die Ausgangsdatei bleibt unverändert liegen.** Verschlüsseln legt eine " +
          "zweite Datei daneben; es ersetzt die erste nicht. Wer sie danach nicht " +
          "mehr braucht, setzt den Haken „Nach dem Verschlüsseln sicher löschen“ — " +
          "und liest, was Löschen auf diesem Datenträger erreicht.",
      },
      {
        art: "absatz",
        text:
          "**Signieren.** Hat Ihre Identität einen Signierschlüssel, setzt " +
          "„Mit meiner Identität signieren“ Ihre Signatur dazu. Der Empfänger sieht dann, " +
          "dass die Nachricht von Ihnen kommt — sofern er Sie verifiziert hat. Eine " +
          "Nachricht ohne Signatur ist keinem Absender zuzuordnen; das ist eine " +
          "Feststellung, keine Zusage, dass niemand von dem Versand erfährt.",
      },
      {
        art: "absatz",
        text:
          "**Was CabrikSecure nicht von sich aus entfernt:** Anmerkungen und nachverfolgte " +
          "Änderungen in einem Word-Dokument etwa sind kein Beiwerk, sondern Inhalt. " +
          "Darüber entscheiden Sie — CabrikSecure fragt.",
      },
      {
        art: "absatz",
        text:
          "**Textnachricht.** Nicht jeder Weg nimmt Dateien an. Unter „Senden“ → " +
          "„Textnachricht“ entsteht ein Textblock zum Einfügen in eine E-Mail oder " +
          "einen Chat. Seine Rahmenzeilen nennen das Programm; die Länge Ihrer " +
          "Nachricht ist darin verschleiert.",
      },
      {
        art: "verweis",
        text: "Beispiel: ein Dokument mit Verlauf",
        ziel: { art: "beispiel", beispiel: { art: "senden", kennung: "mit-verlauf" } },
      },
    ],
  },

  {
    kennung: "empfangen",
    titel: "Nachrichten öffnen",
    inhalt: [
      {
        art: "absatz",
        text:
          "Eine .cabrik-Datei öffnen Sie per Doppelklick oder unter „Empfangen“ mit " +
          "„Datei öffnen …“. Einen Textblock fügen Sie unter " +
          "„Oder einen Text einfügen“ ein.",
      },
      { art: "absatz", text: "Nach dem Öffnen stehen oben zwei Einschätzungen nebeneinander:" },
      {
        art: "liste",
        punkte: [
          "**Absender** — von wem die Nachricht stammt, gemessen an Ihrem " +
            "Kontaktverzeichnis: verifiziert, bekannt aber nicht verifiziert, " +
            "unbekannt oder nicht signiert.",
          "**Metadaten** — was die Datei über sich verrät. Diese Angaben hat der " +
            "Absender mitgeschickt, vermutlich ohne es zu wissen. CabrikSecure lässt die " +
            "Datei, wie sie ankam.",
        ],
      },
      {
        art: "absatz",
        text:
          "Die beiden werden nicht verrechnet: Ein verifizierter Absender macht eine " +
          "Datei voller Metadaten nicht unbedenklich, und umgekehrt.",
      },
      {
        art: "absatz",
        text:
          "Entschlüsselt liegt die Datei zunächst nur im Programm. Erst " +
          "„Speichern unter …“ schreibt sie dorthin, wo Sie sie haben wollen.",
      },
      {
        art: "absatz",
        text:
          "**„Ich verlange eine Signatur“** — setzen Sie den Haken, wenn Sie von " +
          "einem Absender eine Signatur erwarten. Dann ist eine Nachricht ohne " +
          "Signatur ein Fehler statt einer neutralen Feststellung.",
      },
      {
        art: "verweis",
        text: "So sieht eine verifizierte Nachricht aus",
        ziel: { art: "beispiel", beispiel: { art: "empfangen", kennung: "alles-gut" } },
      },
    ],
  },

  {
    kennung: "farben",
    titel: "Was die Farben bedeuten",
    inhalt: [
      {
        art: "liste",
        punkte: [
          "**✓ Grün — bestätigt.** Es wurde geprüft, und es traf zu.",
          "**! Gelb — Warnung.** Es wurde geprüft, und etwas ist zu beachten.",
          "**✕ Rot — Fehler.** Der Vorgang ist gescheitert, oder etwas stimmt nicht.",
          "**? Grau — keine Aussage.** Es ließ sich nicht prüfen. Das ist keine " +
            "Warnung, sondern eine ehrliche Auskunft über die Reichweite des Programms.",
        ],
      },
      { art: "absatz", text: "Daneben zwei Farben, die nichts bewerten:" },
      {
        art: "liste",
        punkte: [
          "**Cyan** — Werte, die CabrikSecure gelesen hat: Größe, Format, Zeitpunkt, " +
            "Fingerprint.",
          "**Magenta** — was **Sie** eingestellt haben, etwa " +
            "„Sie haben eine Signatur verlangt“ oder die Frist bis zur Sperre.",
        ],
      },
      {
        art: "hinweis",
        text:
          "Grün heißt nie, dass alles in Ordnung ist — nur, dass diese eine Prüfung " +
          "gut ausging.",
      },
    ],
  },

  {
    kennung: "loeschen",
    titel: "Sicher löschen",
    inhalt: [
      {
        art: "absatz",
        text:
          "Unter „Werkzeuge“ überschreibt CabrikSecure Dateien, bevor es sie entfernt. Ob " +
          "das wirkt, hängt vom Datenträger ab — und CabrikSecure sagt es bei jeder Datei " +
          "dazu, bevor Sie „Endgültig löschen“ klicken.",
      },
      {
        art: "absatz",
        text:
          "Auf SSDs, in Cloud-Ordnern, auf Netzlaufwerken und bei Schattenkopien " +
          "bleiben alte Stände oft an anderer Stelle stehen. Dort ist Überschreiben " +
          "nicht verlässlich — das ist der Normalfall und kein Fehler des Programms.",
      },
      {
        art: "absatz",
        text:
          "Ein Durchgang genügt. Mehr Durchgänge bringen auf heutigen Datenträgern " +
          "keinen Zusatznutzen.",
      },
      {
        art: "hinweis",
        text:
          "Was nie unverschlüsselt auf der Platte lag, muss niemand löschen. Wer " +
          "das für alles will, verschlüsselt den ganzen Datenträger — mit BitLocker, " +
          "FileVault oder LUKS.",
      },
      {
        art: "verweis",
        text: "Beispiele: Netzlaufwerk, Cloud-Ordner, schreibgeschützt",
        ziel: { art: "beispiel", beispiel: { art: "werkzeuge", werkzeug: "loeschen" } },
      },
    ],
  },

  {
    kennung: "sperren",
    titel: "Sperren und Entsperren",
    inhalt: [
      {
        art: "absatz",
        text:
          "Solange CabrikSecure entsperrt ist, liegt Ihr Schlüssel offen im Arbeitsspeicher. " +
          "Deshalb sperrt es nach einer Frist von selbst — einstellbar links unten " +
          "unter „Frist“. „Jetzt sperren“ sperrt sofort.",
      },
      {
        art: "absatz",
        text:
          "Vor dem Ruhezustand sperrt CabrikSecure ebenfalls, wo das Betriebssystem es " +
          "zulässt. Geht das nicht, steht es links unten dabei.",
      },
      {
        art: "absatz",
        text:
          "Beim Entsperren rechnet CabrikSecure absichtlich eine Weile. Diese Verzögerung " +
          "ist es, die das Raten eines Passworts teuer macht.",
      },
      {
        art: "absatz",
        text:
          "Braucht ein Schlüssel mehr Arbeitsspeicher, als gerade frei ist, sagt " +
          "CabrikSecure das, bevor es nach dem Passwort fragt. Andere Programme zu " +
          "schließen kann helfen — am Passwort liegt es dann nicht.",
      },
      {
        art: "absatz",
        text:
          "Das Passwort ändern Sie unter „Identität“. Ihr Fingerprint bleibt " +
          "derselbe, und Ihre Kontakte merken nichts davon.",
      },
    ],
  },

  {
    kennung: "grenzen",
    titel: "Was CabrikSecure nicht kann",
    inhalt: [
      {
        art: "absatz",
        text:
          "CabrikSecure schützt den **Inhalt** Ihrer Dateien und sagt Ihnen, **von wem** " +
          "eine Nachricht stammt. Es schützt nicht:",
      },
      {
        art: "liste",
        punkte: [
          "**einen Rechner, auf dem fremde Software läuft.** Wer Ihren entsperrten " +
            "Rechner kontrolliert, kann mitlesen, was Sie tippen und öffnen. Dagegen " +
            "hilft kein Programm.",
          "**die Tatsache, dass Sie kommunizieren.** Wer den Datenverkehr beobachtet, " +
            "sieht, dass Sie etwas verschicken, wann und an wen, und ungefähr wie " +
            "groß — nur nicht, was.",
          "**vor rechtlichem Zwang.** Es gibt kein zweites Passwort und keinen " +
            "versteckten Bereich. Was Sie stattdessen tun können, steht unter " +
            "„Auf Reisen und bei Kontrollen“.",
          "**Dateien, die Sie entschlüsselt gespeichert haben.** Was auf der Platte " +
            "liegt, ist so geschützt wie Ihr Rechner.",
          "**Metadaten in Formaten, die CabrikSecure nicht kennt.** Dann sagt es das — es " +
            "behauptet nicht, die Datei sei bereinigt.",
          "**das Löschen auf SSDs.** Siehe „Sicher löschen“.",
          "**ein vergessenes Passwort.** Es gibt keine Hintertür, auch nicht für den " +
            "Hersteller.",
        ],
      },
      {
        art: "hinweis",
        text:
          "Wer unter staatlicher Beobachtung steht und weder sein Gerät noch seinen " +
          "Datenverkehr schützen kann, braucht mehr als dieses Programm.",
      },
      {
        art: "verweis",
        text: "Auf Reisen und bei Kontrollen",
        ziel: { art: "kapitel", kapitel: "reisen" },
      },
    ],
  },

  {
    kennung: "reisen",
    titel: "Auf Reisen und bei Kontrollen",
    inhalt: [
      {
        art: "absatz",
        text:
          "An einer Grenze oder bei einer Kontrolle kann jemand verlangen, dass Sie " +
          "Ihren Rechner entsperren oder Ihr Passwort nennen. **Davor schützt CabrikSecure " +
          "nicht.** Es gibt kein zweites Passwort und keinen versteckten Bereich: " +
          "So etwas so zu bauen, dass es einer Prüfung standhält, ist schwerer, als " +
          "es aussieht — und eine halbe Lösung verleitet dazu, sich auf sie zu " +
          "verlassen.",
      },
      {
        art: "absatz",
        text:
          "Was CabrikSecure kann: Ihren Schlüssel so sichern, dass er nicht auf dem Gerät " +
          "liegen muss, das Sie mitnehmen. Was nicht auf dem Gerät ist, kann dort " +
          "niemand öffnen — auch Sie nicht, solange Sie unterwegs sind.",
      },
      {
        art: "schritte",
        punkte: [
          "**Sichern** — unter „Identität“ mit „Schlüsseldatei sichern“, die " +
            "Kontakte mit. Bringen Sie beide Dateien an einen Ort, den Sie nicht " +
            "mitnehmen: zu Hause, zu einer Person, der Sie vertrauen.",
          "**Prüfen**, dass die Sicherung aufgeht — wenn Sie können, auf einem " +
            "zweiten Rechner mit „Sicherung einspielen“. Eine Sicherung, die nie " +
            "jemand geöffnet hat, ist eine Hoffnung.",
          "**Löschen** — unter „Identität“ mit „Identität löschen“. Schlüssel " +
            "und Kontakte sind dann nicht mehr auf diesem Rechner.",
          "**Nach der Rückkehr** — in der Einrichtung „Sicherung einspielen“. Es " +
            "kommt dieselbe Identität zurück, mit demselben Fingerprint; Ihre " +
            "Kontakte müssen nichts neu prüfen.",
        ],
      },
      {
        art: "hinweis",
        text:
          "**Geht die Sicherung verloren, ist alles verloren,** was an diese " +
          "Identität verschlüsselt wurde — auch, was während der Reise ankommt. " +
          "Löschen Sie deshalb erst, wenn die Sicherung an ihrem Ort liegt und " +
          "aufgeht.",
      },
      {
        art: "absatz",
        text: "**Was das nicht leistet:**",
      },
      {
        art: "liste",
        punkte: [
          "**Es verbirgt nicht, dass Sie CabrikSecure benutzen.** Das Programm bleibt " +
            "installiert, und verschlüsselte Dateien sehen aus wie das, was sie sind.",
          "**Es schützt keine Dateien, die Sie entschlüsselt gespeichert haben.** " +
            "Die liegen weiter auf der Platte. Siehe „Sicher löschen“ — auf SSDs " +
            "ist auch das nicht verlässlich.",
          "**Auch die Sicherung kann verlangt werden,** wenn bekannt ist, wo sie " +
            "liegt. Ohne Ihr Passwort öffnet sie niemand; mit ihm alles darin, " +
            "auch die Liste Ihrer Kontakte.",
          "**Ein Schlüssel in fremden Händen bleibt dort.** Haben Sie Ihr Passwort " +
            "herausgegeben, spielen Sie keine Sicherung dieser Identität mehr ein. " +
            "Legen Sie eine neue an, und bitten Sie Ihre Kontakte, die alte zu " +
            "widerrufen.",
        ],
      },
      {
        art: "hinweis",
        text:
          "Ob Sie ein Passwort nennen müssen, bestimmt das Recht des Landes, in dem " +
          "Sie kontrolliert werden, und es ist von Land zu Land verschieden. Dieses " +
          "Handbuch kann das nicht beantworten. Falsche Angaben gegenüber Behörden " +
          "können strafbar sein — auch dort, wo Verschlüsseln erlaubt ist.",
      },
      { art: "verweis", text: "Zur Identität", ziel: { art: "bereich", bereich: "identitaet" } },
    ],
  },

  {
    kennung: "neu",
    titel: "Neu anfangen und deinstallieren",
    inhalt: [
      {
        art: "absatz",
        text:
          "Ihre Identität und Ihre Kontakte liegen nicht im Programmordner, sondern " +
          "in Ihrem Benutzerverzeichnis:",
      },
      {
        art: "liste",
        punkte: [
          "**Windows:** %APPDATA%\\CabrikSecure",
          "**macOS und Linux:** ~/.config/cabrik",
        ],
      },
      {
        art: "absatz",
        text:
          "Deshalb überstehen sie ein Update und eine Neuinstallation — mit " +
          "demselben Passwort wie zuvor. Das ist Absicht: Ein Update darf Ihren " +
          "Schlüssel nie verwerfen, sonst wäre alles unlesbar, was an Sie " +
          "verschlüsselt wurde.",
      },
      {
        art: "absatz",
        text:
          "**Deinstallieren unter Windows.** Das Setup bietet " +
          "„Lösche die Anwendungsdaten“ an. Setzen Sie den Haken, fragt es " +
          "danach ausdrücklich, ob auch Schlüssel und Kontakte gelöscht werden " +
          "sollen — vorgewählt ist „Nein“. Wer CabrikSecure mit dem MSI-Paket " +
          "installiert hat, wird nicht gefragt; dort bleibt beides liegen.",
      },
      {
        art: "absatz",
        text:
          "**macOS und Linux** haben keinen Deinstaller, der fragt. Wer dort " +
          "alles entfernen will, löscht nach dem Programm den Ordner oben von Hand.",
      },
      {
        art: "absatz",
        text:
          "**Eine Identität entfernen** — unter „Identität“ mit „Identität löschen“. " +
          "Bedenken Sie vorher: Ihre Kontakte verschlüsseln weiter an diesen " +
          "Schlüssel, und was danach ankommt, lässt sich nicht mehr öffnen. Sagen " +
          "Sie Bescheid, bevor Sie löschen.",
      },
      {
        art: "absatz",
        text:
          "**Auf einem neuen Rechner** installieren Sie CabrikSecure und wählen in der " +
          "Einrichtung „Sicherung einspielen“. Ohne Sicherung gibt es keinen Weg " +
          "dorthin: Ihre Identität liegt nur in der Schlüsseldatei.",
      },
      {
        art: "hinweis",
        text:
          "CabrikSecure hält **eine Identität je Rechner.** Wollen Sie zu einem neuen " +
          "Schlüssel wechseln, sichern Sie den alten vorher: Ältere Nachrichten " +
          "öffnen sich nur mit ihm, und dafür müssten Sie die Sicherung später an " +
          "Stelle des neuen wieder einspielen.",
      },
    ],
  },

  {
    kennung: "beispiele",
    titel: "Beispiele",
    inhalt: [
      {
        art: "absatz",
        text:
          "Die Beispiele zeigen Lagen, die man selten von selbst herstellt — mit " +
          "erfundenen Personen, Dateien und Pfaden. Nichts davon liegt auf Ihrem " +
          "Rechner, und nichts lässt sich darin wirklich speichern, verschlüsseln " +
          "oder löschen.",
      },
    ],
  },
];
