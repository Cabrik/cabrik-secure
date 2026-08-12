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
  | "unbekannte_erweiterung";

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
  /** Das Ergebnis der Metadatenprüfung der entpackten Datei. */
  metadaten: Bereinigung | null;
}

// ---------------------------------------------------------------------------
// Was ohne Schlüssel sichtbar ist (cabrik inspect)
// ---------------------------------------------------------------------------

/**
 * Was ein Mitleser **ohne** Schlüssel erkennen kann.
 *
 * Bei v2 ist das nur die Zahl der Kapseln. Bei einer Datei aus Version 1 ist
 * es erheblich mehr — deren Kopf steht im Klartext.
 */
export interface Aussenansicht {
  fassung: 1 | 2;
  suite: string;
  kapseln: number;
  /** Nur bei v1: Der Dateiname stand dort im Klartext. */
  klartextDateiname: string | null;
  klartextGroesse: number | null;
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

/** Eine Datei, die verschickt werden soll, samt Befund. */
export interface Sendedatei {
  name: string;
  groesseBytes: number;
  befund: Bereinigung;
}

// ---------------------------------------------------------------------------
// Eigene Identität (cabrik-core::keyfile)
// ---------------------------------------------------------------------------

/**
 * Stärke der Passwortableitung.
 *
 * Entspricht `KdfStufe` in der CLI. Die Zahlen sind gemessen, nicht
 * geschätzt — sie stehen in `docs/ROADMAP.md` unter Phase 2.
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
   */
  bezeichnung: string;
  fingerprint: string;
  /** Kurzform für die beiläufige Anzeige. */
  fingerprintKurz: string;
  erzeugtAm: number;
  kdf: KdfStufe;
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

/** Was das Programm über eine zu löschende Datei feststellen konnte. */
export interface Loeschbefund {
  pfad: string;
  groesseBytes: number;
  faehigkeit: Loeschfaehigkeit;
  vorbehalte: Loeschvorbehalt[];
  /** Woran die Einschätzung hängt — damit sie nachprüfbar ist. */
  grundlage: string;
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
