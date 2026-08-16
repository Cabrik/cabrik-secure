/**
 * Die Typen des Rust-Kerns, in TypeScript nachgebildet.
 *
 * # Warum sie hier so genau stehen
 *
 * Phase 3 baut gegen Mock-Daten, ohne jede Rust-Anbindung. Die Versuchung
 * wäre, sich dafür bequeme Typen auszudenken — ein `status: "ok" | "warn"`
 * etwa. Genau das wäre der Fehler: Die Oberfläche entstünde dann gegen eine
 * Wirklichkeit, die es nicht gibt, und in Phase 4 müsste alles noch einmal.
 *
 * Diese Datei bildet stattdessen die Aufzählungen aus `cabrik-core` und
 * `cabrik-metadata` **eins zu eins** ab. Sie ist damit zweierlei: die
 * Grundlage der Mock-Daten und der Entwurf des späteren Brückenvertrags.
 *
 * Wo etwas fehlt, fehlt es mit Absicht — siehe `spec/anzeige.md` §6:
 * Schlüsselmaterial kommt hier nie an.
 */

// ---------------------------------------------------------------------------
// Metadaten (cabrik-metadata::model)
// ---------------------------------------------------------------------------

/** Wie schwer ein einzelner Fund wiegt. */
import mindestlaenge from "./vertrag/mindestlaenge.json";

export type Schwere = "gering" | "beachtlich" | "kritisch";

/**
 * Art eines Fundes.
 *
 * Die Namen sind übersetzt, die Fälle nicht: Jeder entspricht genau einer
 * Variante von `FindingKind`.
 */
export type Fundart =
  | "ortsangabe"
  | "personenname"
  | "geraet"
  | "software"
  | "zeitangabe"
  | "organisation"
  | "vorschaubild"
  | "zugeschnittenes_bild"
  | "nachverfolgte_aenderung"
  | "farbprofil"
  | "kommentar"
  | "bearbeitungssitzung"
  | "dateiname"
  | "unbekannte_erweiterung"
  /**
   * Eine Art, die dieser Vertrag noch nicht kennt.
   *
   * `FindingKind` ist im Kern `#[non_exhaustive]`. Käme dort eine Art
   * hinzu, fiele sie in `cabrik-bruecke` auf diesen Wert — statt die
   * Oberfläche mit etwas zu treffen, das sie nicht einordnen kann.
   * Derselbe Gedanke wie beim vierten Anzeigezustand.
   */
  | "unbekannt";

/** Ein einzelner Fund in einer Datei. */
export interface Fund {
  art: Fundart;
  /** Wo er steckt, etwa `"Video:udta/©xyz"`. */
  ort: string;
  /** Der Wert, sofern anzeigbar. Bei Bildern und Anhängen steht hier die Größe. */
  wert: string | null;
  schwere: Schwere;
}

/**
 * Ergebnis des Bereinigens.
 *
 * **Der vierte Fall ist der wichtige.** `unbekannt` heißt nicht „Fehler“ und
 * schon gar nicht „sauber“ — es heißt, dass über die Datei nichts gesagt
 * werden kann. Siehe `spec/anzeige.md` §2.2.
 */
export type Bereinigung =
  | { fall: "vollstaendig"; entfernt: Fund[]; format: string }
  | {
      fall: "teilweise";
      entfernt: Fund[];
      geblieben: Fund[];
      grund: string;
      format: string;
    }
  | { fall: "unbekannt"; formathinweis: string | null }
  | { fall: "fehler"; grund: string };

/**
 * Was in einer **empfangenen** Datei steht.
 *
 * # Warum das nicht `Bereinigung` ist
 *
 * Weil `Bereinigung` beschreibt, was ein Bereinigen *ergab*. Bei einer Datei,
 * die gerade ankommt, ist nichts entfernt worden und soll auch nichts
 * entfernt werden — sie gehört jemand anderem. Die Frage lautet nicht „was
 * ist herausgegangen“, sondern „was ist drin“.
 *
 * # Wem die Auskunft nützt
 *
 * **Nicht nur dem Empfänger.** Was hier auftaucht, hat der *Absender* über
 * sich preisgegeben: Ein Foto mit GPS-Angabe verrät, wo er stand. Wer das
 * sieht, kann ihn warnen — und weiß, was er selbst weitergäbe.
 */
export type Metadatenbefund =
  /** Format verstanden. `funde` darf leer sein — das ist eine Aussage. */
  | { fall: "erkannt"; format: string; funde: Fund[] }
  /** Nicht verstanden. **Keine Aussage über den Inhalt.** */
  | { fall: "unbekannt"; formathinweis: string | null }
  /** Ließ sich nicht untersuchen. */
  | { fall: "fehler"; grund: string };

// ---------------------------------------------------------------------------
// Authentizität (cabrik-core::trust::Authenticity)
// ---------------------------------------------------------------------------

/**
 * Wer die Nachricht geschickt hat — und wie sicher das ist.
 *
 * Sechs Fälle. Ihre Zuordnung zu den vier Anzeigezuständen ist in
 * `spec/anzeige.md` §4.2 festgelegt und alles andere als offensichtlich.
 */
export type Absender =
  /** Nicht signiert. **Ein legitimer Modus, kein Mangel.** */
  | { fall: "unsigniert" }
  /** Gültige Signatur eines Schlüssels, den niemand kennt. */
  | { fall: "unbekannt"; signierschluessel: string }
  /** Bekannter Kontakt, aber **nie verifiziert**. */
  | { fall: "bekannt"; fingerprint: string; name: string }
  /** Verifizierter Kontakt. Der einzige Fall, der Grün verdient. */
  | {
      fall: "verifiziert";
      fingerprint: string;
      name: string;
      verifiziertAm: number;
      /**
       * Auf welchem Weg verifiziert wurde — entspricht `verified_via` im
       * Trust Store, dort ebenfalls optional.
       *
       * Die Wege sind **nicht gleichwertig**, und `spec/trust-store.md` §5
       * verlangt ausdrücklich, dass das benannt wird: Ein Fingerprint, der
       * über denselben Kanal kam wie die Nachricht, beweist nichts. Ohne
       * dieses Feld stünde bei jeder verifizierten Nachricht derselbe Satz,
       * und der schwächste Weg sähe aus wie der stärkste.
       *
       * `null` heißt „nicht vermerkt“ — bei aus v1 übernommenen Kontakten
       * der Normalfall, denn v1 kannte diese Unterscheidung nicht.
       */
      verifiziertUeber: Verifikationsweg | null;
    }
  /** Der Schlüssel ist nicht der aktuelle des Kontakts. */
  | {
      fall: "gewechselt";
      fingerprint: string;
      name: string;
      /** Ob der abgelöste Schlüssel damals verifiziert war — wiegt schwerer. */
      vorherVerifiziert: boolean;
    }
  /** Lokal als kompromittiert markiert. */
  | { fall: "widerrufen"; fingerprint: string; name: string };

// ---------------------------------------------------------------------------
// Das Ergebnis des Öffnens (cabrik-core::envelope::Opened)
// ---------------------------------------------------------------------------

/** Art der Nutzdaten. */
export type Inhaltsart = "text" | "datei";

/**
 * Was beim Öffnen herauskommt.
 *
 * **Ohne die Nutzdaten selbst.** Der Klartext bleibt in Rust; die Oberfläche
 * bekommt seine Größe und, bei einer Datei, den Zielpfad. Bei einer
 * Textnachricht ist der Text der Inhalt und wird durchgereicht — er ist
 * dann ohnehin das, was angezeigt werden soll.
 */
export interface Geoeffnet {
  art: Inhaltsart;
  /** Nur bei `art === "text"`. */
  text: string | null;
  /** Der mitgeschickte Dateiname, bereits auf Unbedenklichkeit geprüft. */
  dateiname: string | null;
  groesseBytes: number;
  /** Unix-Sekunden, sofern der Absender einen Zeitpunkt mitgeschickt hat. */
  zeitpunkt: number | null;
  absender: Absender;
  /**
   * Was in der Datei steht. `null` **nur** bei einer Textnachricht.
   *
   * `null` heißt „die Frage stellt sich nicht“, nicht „nichts gefunden“ —
   * dafür gibt es `{ fall: "erkannt", funde: [] }`.
   */
  metadaten: Metadatenbefund | null;
}

// ---------------------------------------------------------------------------
// Was ohne Schlüssel sichtbar ist (cabrik inspect)
// ---------------------------------------------------------------------------

/**
 * Was ein Mitleser **ohne** Schlüssel erkennen kann.
 *
 * **Die Liste ist frei, nicht aufgezählt.** Ein früherer Entwurf führte
 * hier feste Felder für Dateiname und Größe, weil das die Lecks von Version
 * 1 sind. Das war zu eng: Was ein Format preisgibt, hängt am Format, und
 * eine künftige Fassung leckte womöglich etwas anderes. Der Kern gibt
 * deshalb Sätze aus, keine Felder — und die Oberfläche zählt sie auf, statt
 * sie zu deuten.
 */
export interface Aussenansicht {
  /** Fassung des Formats, etwa `"v2"`. */
  fassung: string;
  /** Verfahren, sofern erkennbar. */
  suite: string | null;
  /** Zahl der Kapseln, sofern erkennbar. */
  kapseln: number | null;
  groesseBytes: number;
  /** Was ohne Schlüssel erkennbar ist. Leer heißt: nichts. */
  offengelegt: string[];
}

// ---------------------------------------------------------------------------
// Kontakte (cabrik-core::trust::Contact)
// ---------------------------------------------------------------------------

/** Wie das Vertrauen zustande kam. */
export type Verifikationsweg = "qr" | "safetyNumber" | "fingerprint";

/**
 * Vertrauenszustand eines Kontakts.
 *
 * Entspricht `TrustState`. **`gesehen` ist Trust on First Use**: Es erlaubt,
 * wiederkehrende Absender wiederzuerkennen, ohne Sicherheit vorzutäuschen.
 */
export type Vertrauen = "gesehen" | "verifiziert" | "gewechselt" | "widerrufen";

/** Ein Eintrag im Kontaktspeicher. */
export interface Kontakt {
  name: string;
  fingerprint: string;
  vertrauen: Vertrauen;
  /** Erstkontakt, Unix-Sekunden. */
  seit: number;
  verifiziertAm: number | null;
  verifiziertUeber: Verifikationsweg | null;
  notiz: string | null;
  /**
   * Ob der Kontakt einen Post-Quantum-Schlüssel führt.
   *
   * Fehlt bei aus v1 übernommenen Kontakten. Dann ist nur die klassische
   * Suite möglich — **und die Oberfläche muss das anzeigen**, sonst hält
   * jemand eine Nachricht für quantensicher, die es nicht ist.
   */
  hatPostQuantum: boolean;
  /**
   * Die Safety Number gegenüber der eigenen Identität.
   *
   * 60 Dezimalziffern in zwölf Gruppen zu fünf. Beide Seiten sehen
   * dieselbe — die Sortierung im Kern sorgt dafür, unabhängig davon, wer
   * fragt.
   */
  safetyNumber: string;
}

// ---------------------------------------------------------------------------
// Senden
// ---------------------------------------------------------------------------

/**
 * Eine Datei, die verschickt werden soll, samt Befund.
 *
 * **Der Pfad ist die Kennung, nicht der Name.** Namen wiederholen sich: Wer
 * aus zwei Ordnern je eine `Rechnung.pdf` auswählt, hätte sonst zwei
 * Dateien mit einer Kennung — und jede Ausnahme, jedes Häkchen und jede
 * Entscheidung über Metadaten träfe beide oder keine. Der Bildschirm
 * **zeigt** den Namen und **rechnet** mit dem Pfad.
 */
export interface Sendedatei {
  /** Wo sie liegt. Die Kennung dieser Datei. */
  pfad: string;
  /** Wie sie heißt — für die Anzeige. */
  name: string;
  groesseBytes: number;
  befund: Bereinigung;
  /**
   * Frühere Fassungen — nur bei PDF, sonst leer.
   *
   * Sie sind **kein Metadatum**, sondern Inhalt, der noch mitfährt. Deshalb
   * stehen sie im Befund gesondert und nicht in der Fundliste.
   */
  fassungen: Fassung[];
}

// ---------------------------------------------------------------------------
// Eigene Identität (cabrik-core::keyfile)
// ---------------------------------------------------------------------------

/**
 * Stärke der Passwortableitung.
 *
 * **Die Zahlen dahinter stehen nicht hier**, sondern in
 * `cabrik_core::keyfile::KdfStufe`. Das ist Absicht: Gäbe es sie auch in
 * der Oberfläche, hätte dasselbe Wort zwei Auslegungen, und beim nächsten
 * Anheben der Empfehlung bliebe eine davon stehen.
 */
export type KdfStufe = "min" | "empfohlen" | "stark";

/**
 * Die eigene Identität.
 *
 * **Enthält kein Schlüsselmaterial und darf nie welches enthalten.** Die
 * Architekturregel für Phase 4 lautet: Das Frontend bekommt Handles, Status
 * und Fortschritt — nie Secrets. Dass dieser Typ gar kein Feld dafür hat,
 * ist die einfachste Art, das durchzusetzen: Was nicht existiert, kann
 * nicht versehentlich angezeigt werden.
 */
export interface Identitaet {
  /**
   * Die eigene Bezeichnung.
   *
   * **Nur lokal.** Wer die Austausch-Nutzlast bekommt, vergibt den Namen
   * selbst (`contacts add … --name`). Diese Bezeichnung wandert nicht mit.
   *
   * Sie steht im **verschlüsselten** Teil der Schlüsseldatei. Deshalb kann
   * der Sperrbildschirm gar nicht verraten, wessen Rechner das ist — das
   * ist keine Zurückhaltung der Anzeige, sondern eine Eigenschaft des
   * Formats (`spec/entsperrung.md` §4.1).
   */
  bezeichnung: string | null;
  fingerprint: string;
  /**
   * Achtstellige Kurzform — **ausschließlich zur Unterscheidung in Listen.**
   *
   * Sie umfasst 40 Bit und darf niemals Grundlage einer Verifikation sein.
   * Dafür gibt es den vollen Fingerprint und die Safety Number.
   */
  fingerprintKurz: string;
  erzeugtAm: number;
  /**
   * Welcher Stufe die Ableitung entspricht — falls einer.
   *
   * `null` heißt **nicht** „unbekannt“, sondern „zu keiner der drei Stufen
   * gehörend“: Die Kommandozeile lässt eigene Werte zu. Ein Etikett
   * danebenzusetzen, das ungefähr passt, wäre eine Falschaussage über die
   * Stärke — deshalb steht dann nur die Zahl.
   */
  kdf: KdfStufe | null;
  /** Der tatsächliche Speicherbedarf der Ableitung, in MiB. Steht immer da. */
  kdfSpeicherMib: number;
  /**
   * Ob ein Signierschlüssel vorhanden ist.
   *
   * Ohne ihn sind Nachrichten **nie** einem Absender zuzuordnen, auch nicht
   * dem eigenen. Das ist ein legitimer Modus, kein Mangel — und wird
   * deshalb neutral angezeigt, nicht als Warnung.
   */
  hatSignierschluessel: boolean;
  /** Ob ein Post-Quantum-Schlüssel geführt wird. Fehlt bei v1-Übernahmen. */
  hatPostQuantum: boolean;
  /** Wo die Datei liegt. Für die Sicherung, die der Nutzer selbst macht. */
  pfad: string;
}

// ---------------------------------------------------------------------------
// Sicheres Löschen (cabrik-shred)
// ---------------------------------------------------------------------------

/**
 * Was Überschreiben auf diesem Datenträger ausrichtet.
 *
 * `bestEffort` ist der **Normalfall** auf heutigen Systemen — SSD,
 * Copy-on-Write oder nicht feststellbar.
 */
export type Loeschfaehigkeit =
  "ueberschreiben" | "bestEffort" | "nichtMoeglich";

/**
 * Ein Vorbehalt beim Löschen.
 *
 * `kopienMoeglich` erscheint **immer**, außer es wurde positiv festgestellt,
 * dass es sich um ein einfaches lokales Volume handelt. Das ist ehrlicher
 * als eine Anbieterliste, die nie vollständig wird.
 */
export type Loeschvorbehalt =
  | { art: "cloudOrdner"; hinweis: string }
  | { art: "kopienMoeglich" }
  | { art: "wechselOderNetz" }
  | { art: "warSchreibgeschuetzt" }
  | { art: "zeitstempelBlieb" };

/**
 * Was sich über eine Datei sagen lässt, **bevor** gelöscht wird.
 *
 * Bewusst **ohne** Begründung im Klartext. Ein früherer Entwurf führte hier
 * ein Feld `grundlage` mit Sätzen wie „NTFS auf rotierender Platte, keine
 * Schattenkopien“. Der Kern liefert so etwas nicht, und es zu erfinden
 * hieße, der Oberfläche eine Gewissheit zu geben, die niemand geprüft hat.
 */
export interface Loeschbeurteilung {
  faehigkeit: Loeschfaehigkeit;
  vorbehalte: Loeschvorbehalt[];
}

/** Was tatsächlich geschehen ist. */
export interface Loeschergebnis {
  pfad: string;
  faehigkeit: Loeschfaehigkeit;
  /** Ob tatsächlich überschrieben wurde. */
  ueberschrieben: boolean;
  /** Ob der Name überschrieben wurde. */
  umbenannt: boolean;
  /** Ob der Verzeichniseintrag verschwunden ist. */
  entfernt: boolean;
  vorbehalte: Loeschvorbehalt[];
  /** Warum es fehlschlug, sofern es das tat. */
  fehler: string | null;
}

// ---------------------------------------------------------------------------
// Austausch-Nutzlast (spec/trust-store.md §5.1)
// ---------------------------------------------------------------------------

/**
 * Was beim Einlesen einer Austausch-Nutzlast herauskommt.
 *
 * **Ohne Namen.** Die Nutzlast trägt keinen — der Empfänger vergibt ihn
 * selbst. Ein Name, der mitgeliefert würde, sähe wie eine Angabe des
 * Absenders aus und wäre doch nur eine Behauptung.
 *
 * Der Fingerprint wird aus den Schlüsseln **neu berechnet**. Die in der
 * Nutzlast mitgeführten acht Byte sind ausdrücklich nur eine Prüfsumme
 * gegen Übertragungsfehler; ihnen zu vertrauen verbietet die Spezifikation.
 */
export type Nutzlastbefund =
  | {
      fall: "gelesen";
      /** Neu berechnet, nicht übernommen. */
      fingerprint: string;
      /** Ohne ihn kann der Kontakt empfangen, aber nie signieren. */
      hatSignierschluessel: boolean;
      /** Ohne ihn ist nur die klassische Suite möglich. */
      hatPostQuantum: boolean;
      /** Ob dieser Fingerprint oder Name bereits im Speicher steht. */
      schonBekannt: {
        name: string;
        /** `false` heißt: derselbe Kontakt, anderer Schlüssel. */
        gleicherSchluessel: boolean;
      } | null;
    }
  /** Prüfsumme passt nicht — ein Übertragungsfehler, kein Angriff. */
  | { fall: "beschaedigt"; grund: string }
  /** Kein Cabrik-Austauschformat. */
  | { fall: "unlesbar"; grund: string };

// ---------------------------------------------------------------------------
// Frühere PDF-Fassungen (cabrik-metadata::pdf::Fassung)
// ---------------------------------------------------------------------------

/**
 * Eine frühere Fassung eines PDF.
 *
 * PDFs werden **inkrementell** fortgeschrieben: Jede Bearbeitung hängt
 * hinten an, statt zu ersetzen. Die Datei enthält alle Fassungen; ein Leser
 * zeigt nur die letzte. Das ist die klassische Schwärzungspanne — ein
 * Dokument mit geschwärzten Namen, in dem die vorige Fassung mit den
 * lesbaren Namen vollständig weitersteckt.
 */
export interface Fassung {
  /** Zählung ab eins, älteste zuerst. */
  nummer: number;
  /** Länge der Datei bis zum Ende dieser Fassung. */
  bytes: number;
  seiten: number;
  /** Ob dies die Fassung ist, die ein Leser anzeigt. */
  wirdAngezeigt: boolean;
  /** Anfang des Textes, gekürzt. */
  auszug: string;
  /**
   * Zeilen, die es **nur hier** gibt — also später entfernt wurden.
   *
   * Das ist die eigentliche Auskunft: nicht „wie sah diese Fassung aus“,
   * sondern „was wurde herausgenommen und fährt trotzdem mit“.
   */
  nurHier: string[];
}

// ---------------------------------------------------------------------------
// Formatabhängige Entscheidungen (cabrik metadata strip)
// ---------------------------------------------------------------------------

/**
 * Was beim Bereinigen zusätzlich geschehen soll.
 *
 * Entspricht eins zu eins den Schaltern von `cabrik metadata strip`. Es sind
 * **keine Schalter, sondern Zielkonflikte**: Jeder ist manchmal richtig und
 * manchmal fatal, und keiner darf wortlos voreingestellt werden.
 */
export interface Bereinigungswahl {
  /**
   * PDF: Welche Fassung eingeflacht wird, gezählt ab eins.
   *
   * `null` heißt: die zuletzt bearbeitete — also das, was ein Leser
   * anzeigt. Entspricht `--revision`.
   */
  fassung: number | null;
  /**
   * PDF: Die Änderungshistorie **nicht** entfernen.
   *
   * Für Fälle, in denen das Dokument nicht verändert werden darf —
   * Beweismittel, Archivierung. Frühere Fassungen bleiben dann
   * wiederherstellbar, mit allem, was aus ihnen entfernt wurde.
   * Entspricht `--keep-history` und schließt `fassung` aus.
   */
  historieBehalten: boolean;
  /**
   * Office: Anmerkungen entfernen.
   *
   * Betrifft **nur** die Anmerkungen — der Text bleibt Zeichen für Zeichen
   * erhalten. Entspricht `--remove-comments`.
   */
  kommentareEntfernen: boolean;
  /**
   * Office: Nachverfolgte Änderungen annehmen.
   *
   * Wie „Alle Änderungen annehmen“ in Word: Einfügungen bleiben,
   * Löschungen verschwinden **samt Text**. Das verändert den Inhalt und
   * ist deshalb nie voreingestellt. Entspricht `--accept-changes`.
   */
  aenderungenAnnehmen: boolean;
}

/** Die Voreinstellung: nichts, was den Inhalt verändert. */
export const WAHL_VOREINSTELLUNG: Bereinigungswahl = {
  fassung: null,
  historieBehalten: false,
  kommentareEntfernen: false,
  aenderungenAnnehmen: false,
};

// ---------------------------------------------------------------------------
// Sitzung (spec/entsperrung.md)
// ---------------------------------------------------------------------------

/**
 * Nach welcher Untätigkeit gesperrt wird.
 *
 * **Eine feste Liste und keine freie Zahl.** Freie Eingabe lädt zu „0“ oder
 * „999999“ ein — und das heißt „nie sperren“, ohne dass jemand
 * *entschieden* hat, nie zu sperren.
 *
 * **Keine Werte über 60 Minuten.** Zwei oder vier Stunden sind keine eigene
 * Entscheidung, sondern dieselbe wie `bisZumSchliessen` — nur als Vorsicht
 * verkleidet.
 */
export type Sperrfrist =
  | "eineMinute"
  | "fuenfMinuten"
  | "fuenfzehnMinuten"
  | "dreissigMinuten"
  | "eineStunde"
  | "bisZumSchliessen";

/**
 * Wie es um die Sitzung steht.
 *
 * **Kein Schlüsselmaterial, keine Bezeichnung der Identität.** Wer auf
 * einen gesperrten Bildschirm sieht, soll nicht erfahren, wessen Rechner
 * das ist.
 */
export interface Sitzungsstand {
  gesperrt: boolean;
  frist: Sperrfrist;
  /**
   * Sekunden bis zur Sperre.
   *
   * `null`, wenn gesperrt ist oder keine Frist läuft. Die Warnstufen leitet
   * die Oberfläche daraus ab — die Schwellen sind eine Anzeigefrage
   * (`spec/entsperrung.md` §9) und stehen deshalb hier, nicht im Kern.
   */
  restsekunden: number | null;
}

/**
 * Wie viele Sekunden Untätigkeit eine Frist erlaubt.
 *
 * Spiegelt `Sperrfrist::sekunden` in `cabrik-bruecke`. Hier und nicht in
 * der Anzeigeschicht, weil es keine Anzeigefrage ist: Die Zahl steht im
 * Kern, die Oberfläche liest sie nur ab. Die **Schwellen** der Warnstaffel
 * sind eine Anzeigefrage und stehen dort.
 *
 * `null` heißt: keine Frist. Nicht „unendlich lang“, sondern „es wird
 * nicht nach Zeit gesperrt“ — der Unterschied zählt, weil kein Rechnen
 * damit richtig wäre.
 */
export const FRIST_SEKUNDEN: Record<Sperrfrist, number | null> = {
  eineMinute: 60,
  fuenfMinuten: 300,
  fuenfzehnMinuten: 900,
  dreissigMinuten: 1800,
  eineStunde: 3600,
  bisZumSchliessen: null,
};

/**
 * Was beim Ziehen über dem Fenster passiert.
 *
 * **Drei Fälle und nicht nur der letzte.** Ein Fenster, das erst beim
 * Loslassen reagiert, sieht bis dahin aus wie eines, das nichts annimmt —
 * und dann lässt niemand los. Die Rückmeldung vorher ist der eigentliche
 * Zweck; das Fallenlassen ist nur der Abschluss.
 */
export type Ziehereignis =
  | { art: "drueber" }
  | { art: "weg" }
  | { art: "fallen"; pfade: string[] };

/**
 * Was beim Speichern einer bereinigten Datei herauskam.
 *
 * **Je Datei einer.** Ein Stapel aus vierzig soll nicht an einer scheitern,
 * die gerade in Benutzung ist — und was nicht geklappt hat, muss benannt
 * werden, statt in einer Erfolgsmeldung unterzugehen.
 */
export interface Speicherergebnis {
  /** Die Ausgangsdatei — ihr Pfad ist die Kennung. */
  quelle: string;
  /** Wohin geschrieben wurde. `null`, wenn nichts geschrieben wurde. */
  ziel: string | null;
  /** Was das Bereinigen ergab. */
  befund: Bereinigung;
  /**
   * Warum nichts geschrieben wurde. `null` heißt: Es hat geklappt.
   *
   * Getrennt vom Befund, weil es zwei verschiedene Dinge sind: Der Befund
   * sagt, was in der Datei stand; dies sagt, warum sie nicht abgelegt
   * werden konnte.
   */
  fehler: string | null;
}

/** Was beim Verschlüsseln einer Datei herauskam. */
export interface Versandergebnis {
  /** Die Ausgangsdatei — ihr Pfad ist die Kennung. */
  quelle: string;
  /** Wohin geschrieben wurde. `null`, wenn nichts geschrieben wurde. */
  ziel: string | null;
  /** Größe des Envelopes in Bytes. */
  bytes: number;
  /** Was das Bereinigen ergab. `null`, wenn das Original hinausging. */
  befund: Bereinigung | null;
  /** Warum nichts geschrieben wurde. `null` heißt: Es hat geklappt. */
  fehler: string | null;
}

/**
 * Was für den ganzen Versand gilt.
 *
 * Getrennt von den einzelnen Ergebnissen, weil es sich auf den Vorgang
 * bezieht und nicht auf eine Datei.
 */
export interface Versandbericht {
  /** Das benutzte Verfahren, in Worten. */
  suite: string;
  /**
   * Ob signiert wurde.
   *
   * **Kann `false` sein, obwohl es gewollt war**: Eine Identität ohne
   * Signierschlüssel kann nicht signieren. Das gehört gesagt, statt
   * stillschweigend zu unterbleiben.
   */
  signiert: boolean;
  /** Die Namen der Empfänger, in der Reihenfolge der Kapseln. */
  empfaenger: string[];
  /** Vorbehalte, die vor dem Senden zu lesen sind. */
  vorbehalte: string[];
  /** Was mit den einzelnen Dateien geschah. */
  dateien: Versandergebnis[];
}

/**
 * Die Mindestlänge eines Passworts, in Zeichen.
 *
 * **Aus dem Kern, nicht abgeschrieben.** Die Zahl stand bis vor kurzem
 * allein im Einrichtungsbildschirm — der Passwortwechsel kannte sie nicht,
 * und die Kommandozeile auch nicht. Jetzt steht sie in
 * `cabrik_core::keyfile::MIN_PASSWORT_ZEICHEN` und kommt über das
 * Prüfmuster hierher: Wer sie dort anhebt, hebt sie überall an.
 *
 * Sie ist keine Stärkeanzeige. Die Länge ist das eine, was ein Programm
 * über ein Passwort **wissen** kann; alles andere wäre ein Urteil, das
 * niemand fällen kann, der die Liste nicht kennt, in der es vielleicht
 * steht.
 */
export const MINDESTLAENGE: number = mindestlaenge;

/**
 * Eine Datei, die gelöscht werden soll — samt Beurteilung.
 *
 * **Die Beurteilung steht vor der Tat.** Wer erst löscht und dann erfährt,
 * dass Überschreiben auf diesem Datenträger nichts ausrichtet, kann nichts
 * mehr entscheiden.
 */
export interface Loeschkandidat {
  /** Wo sie liegt. Die Kennung dieser Datei. */
  pfad: string;
  /** Wie sie heißt — für die Anzeige. */
  name: string;
  /** Wie groß sie ist. */
  groesseBytes: number;
  /** Was auf diesem Datenträger erreichbar ist. */
  beurteilung: Loeschbeurteilung;
}

/**
 * Wie weit ein Stapel ist.
 *
 * # Warum das vom Kern kommt und nicht geschätzt wird
 *
 * Weil nur er weiß, wo er steht. Eine Oberfläche, die aus der Zahl der
 * Dateien und einer angenommenen Dauer einen Balken rechnet, zeigt eine
 * Erfindung — und sie liegt genau dann daneben, wenn es darauf ankommt: bei
 * der einen 2-GB-Datei zwischen neununddreißig Fotos.
 */
export interface Fortschritt {
  /** Wie viele **fertig** sind — die gerade laufende nicht mitgezählt. */
  erledigt: number;
  /** Wie viele es insgesamt sind. */
  gesamt: number;
  /**
   * Die Datei, die **gerade** bearbeitet wird — nicht die zuletzt fertige.
   *
   * „3 von 40“ allein sagt nicht, ob es hakt oder läuft. Steht eine Minute
   * lang derselbe Name da, weiß man wenigstens, **welche** Datei aufhält.
   */
  laeuft: string;
}

/**
 * Nimmt Fortschrittsmeldungen entgegen.
 *
 * **Pflicht bei jedem Stapelbefehl, nicht wahlweise.** Ein Aufruf, der sie
 * weglassen darf, wird sie irgendwo weglassen — und ein Bildschirm ohne
 * Fortschritt ist von einem hängenden nicht zu unterscheiden. Wer wirklich
 * nichts anzeigen will, übergibt `() => {}` und hat es dann entschieden.
 */
export type Fortschrittsmelder = (f: Fortschritt) => void;

/**
 * Welcher Stapel gerade läuft.
 *
 * # Warum das nicht der Kern mitschickt
 *
 * Weil der Kern nicht weiß, wie es heißen soll. Er zählt Dateien; ob daraus
 * „Wird geprüft“ oder „Wird gelöscht“ wird, ist eine Frage der Anzeige und
 * gehört dorthin, wo `spec/anzeige.md` gilt.
 *
 * # Warum es überhaupt gebraucht wird
 *
 * Weil ein Balken ohne Bezeichnung bei allen fünf Stapeln derselbe wäre.
 * Beim **Löschen** ist das keine Kleinigkeit: Der Vorgang ist
 * unwiderruflich, und wer ihn mit dem Prüfen verwechselt, wartet gelassen
 * auf etwas anderes, als gerade geschieht.
 */
export type Stapelart =
  | "pruefen"
  | "speichern"
  | "verschluesseln"
  | "beurteilen"
  | "loeschen";

/** Ein Fortschritt samt der Auskunft, wozu er gehört. */
export type Stapelstand = Fortschritt & { art: Stapelart };

/**
 * Ein QR-Code als Zeichenweg.
 *
 * **Ein Pfad und kein Bild:** So nimmt er die Farbe des Textes an, in dem
 * er steht, und sieht im dunklen Modus richtig aus. Und er ist klein — ein
 * Code mit 141 Modulen Kantenlänge hat rund zwanzigtausend Felder; als
 * Liste von Wahrheitswerten wären das hunderte Kilobyte.
 */
export interface QrCode {
  /** Kantenlänge in Modulen — zugleich die Größe des Koordinatensystems. */
  groesse: number;
  /** Die dunklen Felder als SVG-Pfad. */
  pfad: string;
}
