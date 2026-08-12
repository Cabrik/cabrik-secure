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
 * **Der vierte Fall ist der wichtige.** `unbekannt` heißt nicht „Fehler" und
 * schon gar nicht „sauber" — es heißt, dass über die Datei nichts gesagt
 * werden kann. Siehe `spec/anzeige.md` §2.2.
 */
export type Bereinigung =
  | { fall: "vollstaendig"; entfernt: Fund[]; format: string }
  | { fall: "teilweise"; entfernt: Fund[]; geblieben: Fund[]; grund: string; format: string }
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
  | { fall: "verifiziert"; fingerprint: string; name: string; verifiziertAm: number }
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
