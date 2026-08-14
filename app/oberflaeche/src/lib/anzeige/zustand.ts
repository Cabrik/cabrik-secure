/**
 * Die vier Anzeigezustände und ihre Zuordnung (`spec/anzeige.md`).
 *
 * # Warum das Funktionen sind und keine Absichtserklärung
 *
 * Ein Anzeigevertrag, der nur in einem Dokument steht, wird beim dritten
 * Bildschirm gebrochen — nicht aus Nachlässigkeit, sondern weil niemand
 * beim Schreiben eines Knopfes ein Kapitel nachschlägt.
 *
 * Deshalb steht die Zuordnung hier als reine Funktion mit Tests. Wer eine
 * Nachricht anders einordnen will, muss den Test ändern, und das fällt in
 * einer Durchsicht auf.
 */

import type {
  Absender,
  Bereinigung,
  Fund,
  Schwere,
  Sperrfrist,
} from "../kern/typen";
import { FRIST_SEKUNDEN } from "../kern/typen";

/**
 * Die vier Zustände.
 *
 * `keineAussage` ist der wichtigste: Er entspricht der Flagge am künstlichen
 * Horizont, die erscheint, wenn das Instrument seine Eingangsdaten verliert.
 * Er ist **kein** abgestuftes Gelb — Gelb heißt „ich weiß etwas“, Grau heißt
 * „ich weiß es nicht“.
 */
export type Zustand = "bestaetigt" | "warnung" | "fehler" | "keineAussage";

/** Was eine Anzeige braucht: Zustand, Zeichen, Wort. Farbe allein genügt nie. */
export interface Marke {
  zustand: Zustand;
  /** Die Kurzform. Trägt die Bedeutung, wenn die Farbe nicht ankommt. */
  wort: string;
  /** Der Satz darunter. Nennt, was tatsächlich getan oder festgestellt wurde. */
  satz: string;
}

/**
 * Zeichen je Zustand.
 *
 * Bewusst Text und kein Bildzeichen: Es überlebt jede Schriftart und jeden
 * Bildschirmleser, und es lässt sich vorlesen.
 */
export const ZEICHEN: Record<Zustand, string> = {
  bestaetigt: "✓",
  warnung: "!",
  fehler: "✕",
  keineAussage: "?",
};

// ---------------------------------------------------------------------------
// Metadaten
// ---------------------------------------------------------------------------

/**
 * Ordnet ein Bereinigungsergebnis ein.
 *
 * Zwei Regeln aus `spec/anzeige.md` §4.1 sind hier zwingend:
 *
 * 1. Bei `vollstaendig` wird **das Format genannt**. „Alle bekannten
 *    Metadaten entfernt“ ohne Bezug wäre eine stärkere Aussage, als der
 *    Kern deckt.
 * 2. Bei `unbekannt` darf nichts stehen, was nach Erfolg klingt.
 */
export function markeFuerBereinigung(b: Bereinigung): Marke {
  switch (b.fall) {
    case "vollstaendig":
      return {
        zustand: "bestaetigt",
        wort: "Bereinigt",
        satz: `Alle bekannten Metadaten entfernt (${b.format}).`,
      };
    case "teilweise":
      return {
        zustand: "warnung",
        wort: "Teilweise bereinigt",
        satz: b.grund,
      };
    case "unbekannt":
      return {
        zustand: "keineAussage",
        wort: "Keine Aussage",
        satz: b.formathinweis
          ? `Erkannt als ${b.formathinweis}, aber nicht verstanden — über den Inhalt lässt sich nichts sagen.`
          : "Format nicht verstanden — über den Inhalt lässt sich nichts sagen.",
      };
    case "fehler":
      return { zustand: "fehler", wort: "Fehler", satz: b.grund };
  }
}

// ---------------------------------------------------------------------------
// Absender
// ---------------------------------------------------------------------------

/**
 * Ordnet den Absender ein.
 *
 * `signaturVerlangt` bildet ab, was die CLI mit `--require-signature` tut:
 * **Dieselbe Lage wird anders bewertet, je nachdem was der Nutzer verlangt
 * hat** — nicht danach, was das Programm für richtig hält.
 *
 * Ohne diese Unterscheidung müsste man sich entscheiden, ob eine unsignierte
 * Nachricht gelb ist. Beide Antworten wären falsch: Für jemanden, der
 * anonyme Zusendungen erwartet, ist sie der Normalfall; für jemanden, der
 * eine Zusicherung braucht, ist sie unbrauchbar.
 */
export function markeFuerAbsender(
  a: Absender,
  signaturVerlangt = false,
): Marke {
  switch (a.fall) {
    case "verifiziert":
      return {
        zustand: "bestaetigt",
        wort: a.name,
        satz:
          `${wegText(a.verifiziertUeber)} am ${datum(a.verifiziertAm)}. ` +
          "Die Signatur stammt vom Inhaber des geprüften Schlüssels." +
          wegVorbehalt(a.verifiziertUeber),
      };

    case "bekannt":
      return {
        zustand: "warnung",
        wort: `${a.name} — nicht verifiziert`,
        satz:
          "Der Name stammt aus Ihrem Kontaktspeicher, nicht aus einer Prüfung. " +
          "Vergleichen Sie die Safety Number, bevor Sie sich darauf verlassen.",
      };

    case "unbekannt":
      return {
        zustand: "keineAussage",
        wort: "Unbekannter Absender",
        satz:
          "Die Signatur ist gültig, aber der Schlüssel steht in keinem Ihrer Kontakte. " +
          "Das sagt nichts darüber, wer geschickt hat.",
      };

    case "gewechselt":
      return {
        zustand: "warnung",
        wort: `${a.name} — Schlüssel gewechselt`,
        satz: a.vorherVerifiziert
          ? "Der bisherige Schlüssel dieses Kontakts war von Ihnen verifiziert. Der neue ist es nicht. Fragen Sie auf einem anderen Weg nach, bevor Sie antworten."
          : "Dieser Kontakt benutzt einen anderen Schlüssel als bisher.",
      };

    case "widerrufen":
      return {
        zustand: "fehler",
        wort: `${a.name} — Schlüssel widerrufen`,
        satz:
          "Sie haben diesen Schlüssel als kompromittiert markiert. " +
          "Die Signatur ist gültig — das heißt hier gerade nichts Gutes.",
      };

    case "unsigniert":
      // Anonymer Versand ist ein legitimer Modus. Erst wenn der Nutzer eine
      // Signatur ausdrücklich verlangt hat, ist ihr Fehlen ein Fehler.
      return signaturVerlangt
        ? {
            zustand: "fehler",
            wort: "Nicht signiert",
            satz: "Sie haben eine Signatur verlangt. Diese Nachricht trägt keine.",
          }
        : {
            zustand: "keineAussage",
            wort: "Nicht signiert",
            satz: "Die Nachricht sagt nichts darüber, wer sie geschickt hat.",
          };
  }
}

// ---------------------------------------------------------------------------
// Einzelne Funde
// ---------------------------------------------------------------------------

/**
 * Wie ein einzelner Fund dargestellt wird.
 *
 * Die Schwere färbt **den Fund**, nicht das Gesamturteil. Ein einzelner
 * kritischer Fund in einer vollständig bereinigten Datei ist kein Grund für
 * eine Warnung — er ist ja weg.
 */
export const SCHWERE_RANG: Record<Schwere, number> = {
  kritisch: 0,
  beachtlich: 1,
  gering: 2,
};

/** Sortiert Funde: Schwerwiegendes zuerst, sonst nach Fundstelle. */
export function nachSchwere(funde: readonly Fund[]): Fund[] {
  return [...funde].sort(
    (a, b) =>
      SCHWERE_RANG[a.schwere] - SCHWERE_RANG[b.schwere] ||
      a.ort.localeCompare(b.ort),
  );
}

/** Menschenlesbarer Name einer Fundart. */
export const FUNDART_TEXT: Record<Fund["art"], string> = {
  ortsangabe: "Ortsangabe",
  personenname: "Personenname",
  geraet: "Gerät oder Seriennummer",
  software: "erzeugende Software",
  zeitangabe: "Zeitangabe",
  organisation: "Firmen- oder Organisationsname",
  vorschaubild: "eingebettetes Vorschaubild",
  zugeschnittenes_bild: "zugeschnittenes Bild",
  nachverfolgte_aenderung: "nachverfolgte Änderung",
  farbprofil: "Farbprofil",
  kommentar: "Kommentar",
  bearbeitungssitzung: "Bearbeitungssitzung",
  dateiname: "ursprünglicher Dateiname",
  unbekannte_erweiterung: "unbekannte Erweiterung",
  // Für eine Fundart, die der Kern kennt und dieser Vertrag noch nicht.
  // „Etwas Gefundenes“ ist ehrlicher als gar keine Zeile: Der Fund ist da,
  // nur sein Name fehlt.
  unbekannt: "unbenannte Fundart",
};

// ---------------------------------------------------------------------------
// Kleinkram
// ---------------------------------------------------------------------------

function datum(unixSekunden: number): string {
  return new Date(unixSekunden * 1000).toLocaleDateString("de-DE", {
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}

/** Größenangabe, die sich vorlesen lässt. */
export function groesse(bytes: number): string {
  if (bytes < 1024) return `${bytes} Bytes`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

// ---------------------------------------------------------------------------
// Kontakte
// ---------------------------------------------------------------------------

/**
 * Ordnet einen Kontakt ein.
 *
 * Dieselben vier Zustände, aber ein anderer Blickwinkel als bei
 * [`markeFuerAbsender`]: Dort geht es um **eine Nachricht**, hier um den
 * **Eintrag im Verzeichnis**. Ein nie verifizierter Kontakt ist als Eintrag
 * kein Warnfall — er ist erwartbar, denn so fängt jeder an. Erst wenn eine
 * Nachricht von ihm kommt und man sich auf den Namen verlassen soll, wird
 * daraus eine Warnung.
 */
export function markeFuerKontakt(k: import("../kern/typen").Kontakt): Marke {
  switch (k.vertrauen) {
    case "verifiziert":
      return {
        zustand: "bestaetigt",
        wort: "Verifiziert",
        satz:
          `${wegText(k.verifiziertUeber)} am ${
            k.verifiziertAm ? datum(k.verifiziertAm) : "unbekanntem Datum"
          }.` + wegVorbehalt(k.verifiziertUeber),
      };
    case "gesehen":
      return {
        zustand: "keineAussage",
        wort: "Nicht verifiziert",
        satz:
          "Dieser Kontakt ist bekannt, aber nie geprüft worden. So fängt jeder an — " +
          "verlassen Sie sich erst darauf, wenn Sie die Safety Number verglichen haben.",
      };
    case "gewechselt":
      return {
        zustand: "warnung",
        wort: "Schlüssel gewechselt",
        satz:
          "Dieser Kontakt tritt mit einem anderen Schlüssel auf als bisher. " +
          "Das kann ein neues Gerät sein — oder jemand anders.",
      };
    case "widerrufen":
      return {
        zustand: "fehler",
        wort: "Widerrufen",
        satz: "Sie haben diesen Schlüssel als kompromittiert markiert.",
      };
  }
}

/**
 * Wie der Verifikationsweg benannt wird.
 *
 * Die Wege sind **nicht gleichwertig** (`spec/trust-store.md` §5), und die
 * Spezifikation verlangt, dass die Oberfläche das benennt. Der Zustand
 * bleibt trotzdem in allen Fällen grün: Das Programm hat nicht darüber zu
 * befinden, ob die Prüfung des Nutzers ihm gut genug war. Es hat zu sagen,
 * **was** geprüft wurde — dann kann er selbst urteilen.
 */
function wegText(w: import("../kern/typen").Verifikationsweg | null): string {
  switch (w) {
    case "qr":
      return "Über QR-Code geprüft";
    case "safetyNumber":
      return "Safety Number verglichen";
    case "fingerprint":
      return "Fingerprint abgeglichen";
    default:
      return "Geprüft";
  }
}

/**
 * Der Vorbehalt zum jeweiligen Weg — oder nichts.
 *
 * `spec/trust-store.md` §5 nennt einen Fall ausdrücklich: „Fingerprint per
 * Messenger senden — gering. Derselbe Kanal, derselbe Angreifer.“ Der Store
 * kann nicht unterscheiden, ob der Fingerprint abgetippt oder aus derselben
 * Unterhaltung kopiert wurde. Also wird der Vorbehalt genannt, statt ihn zu
 * unterschlagen.
 */
function wegVorbehalt(
  w: import("../kern/typen").Verifikationsweg | null,
): string {
  switch (w) {
    case "qr":
      // Der stärkste Weg: Ein Angreifer müsste im Raum gestanden haben.
      return "";
    case "safetyNumber":
      return "";
    case "fingerprint":
      return (
        " Das trägt nur, wenn der Fingerprint über einen anderen Weg kam als " +
        "die Nachricht selbst — derselbe Kanal, derselbe Angreifer."
      );
    default:
      return " Auf welchem Weg, ist nicht vermerkt.";
  }
}

// ---------------------------------------------------------------------------
// Stapel
// ---------------------------------------------------------------------------

/** Wie viele Dateien in welchem Zustand sind. */
export interface Stapelbefund {
  vollstaendig: number;
  /**
   * Die vollständig bereinigten Dateien.
   *
   * Sie werden auf eine Zeile zusammengefasst — aber nicht weggeworfen.
   * „Nicht stören“ heißt nicht „nicht nachsehen können“: Wer wissen will,
   * was aus ihnen entfernt wurde, muss es finden können.
   */
  sauber: import("../kern/typen").Sendedatei[];
  /** Die Dateien, zu denen es etwas zu sagen gibt — einzeln. */
  auffaellig: import("../kern/typen").Sendedatei[];
  gesamt: number;
}

/**
 * Fasst einen Stapel zusammen.
 *
 * # Warum nicht überspringen
 *
 * Bei vielen Dateien liegt die Versuchung nahe, die Vorschau abschaltbar zu
 * machen. Sie ist aber genau dort am **wichtigsten**: Wer vierzig Dateien
 * schickt und drei davon sind nur teilweise bereinigt, übersieht beim
 * Überspringen genau die drei, auf die es ankommt.
 *
 * Die Regel „stör nur, wenn du wirklich etwas zu sagen hast“ gilt trotzdem —
 * eine Ebene höher: Das Unauffällige wird zu **einer Zeile** zusammengefasst,
 * das Auffällige einzeln genannt. Ein Bildschirm statt vierzig, ohne dass
 * jemand absichtlich wegsehen muss.
 */
export function fasseStapel(
  dateien: readonly import("../kern/typen").Sendedatei[],
): Stapelbefund {
  const auffaellig = dateien.filter((d) => d.befund.fall !== "vollstaendig");
  const sauber = dateien.filter((d) => d.befund.fall === "vollstaendig");
  return {
    vollstaendig: sauber.length,
    sauber,
    auffaellig,
    gesamt: dateien.length,
  };
}

/**
 * Ob der Nutzer vor dem Verschlüsseln etwas entscheiden muss.
 *
 * Genau dann, wenn mindestens eine Datei nicht vollständig bereinigt werden
 * konnte. Sind alle grün, gibt es nichts zu sagen — und dann wird auch nicht
 * gestört.
 */
export function brauchtEntscheidung(befund: Stapelbefund): boolean {
  return befund.auffaellig.length > 0;
}

// ---------------------------------------------------------------------------
// Die Sperre (spec/entsperrung.md §9)
// ---------------------------------------------------------------------------

/** Wie die Frist heißt, wenn man sie auswählt. */
export const FRIST_TEXT: Record<Sperrfrist, string> = {
  eineMinute: "Nach 1 Minute",
  fuenfMinuten: "Nach 5 Minuten",
  fuenfzehnMinuten: "Nach 15 Minuten",
  dreissigMinuten: "Nach 30 Minuten",
  eineStunde: "Nach 1 Stunde",
  bisZumSchliessen: "Bis das Fenster geschlossen wird",
};

/**
 * Wie dringend die bevorstehende Sperre ist.
 *
 * `keine` heißt: nichts sagen. Das ist der Normalfall und der wichtigste
 * Wert — ein dauerhaft laufender Zähler drängt und ist die meiste Zeit
 * belanglos.
 */
export type Warnstufe = "keine" | "leise" | "deutlich" | "countdown";

/**
 * Die Staffel aus `spec/entsperrung.md` §9.
 *
 * **Relativ zur eingestellten Zeit, nicht absolut.** Feste Werte wie „zehn
 * Minuten vorher“ gingen bei einer Einstellung von einer Minute nicht auf.
 *
 * Bei 15 Minuten ergibt das: leise nach 10 Minuten Untätigkeit, deutlich
 * nach 12½, Countdown in der letzten halben Minute.
 *
 * Bei einer Minute schluckt der Countdown die beiden anderen Stufen — und
 * das ist richtig: Ein Hinweis bei 40 Sekunden Restzeit wäre Lärm.
 */
export function warnstufe(
  restsekunden: number | null,
  frist: Sperrfrist,
): Warnstufe {
  const gesamt = FRIST_SEKUNDEN[frist];
  if (gesamt === null || restsekunden === null) return "keine";
  if (restsekunden <= 30) return "countdown";
  if (restsekunden <= gesamt / 6) return "deutlich";
  if (restsekunden <= gesamt / 3) return "leise";
  return "keine";
}

/**
 * Wie die Restzeit dasteht.
 *
 * Unter einer Minute sekundengenau, darüber gerundet: „noch 4 Minuten“ ist
 * die Angabe, die jemand braucht — „noch 3:47“ liest sich wie eine Frist,
 * die man einhalten muss.
 */
export function restzeitText(sekunden: number): string {
  if (sekunden <= 60) {
    return `noch ${sekunden} ${sekunden === 1 ? "Sekunde" : "Sekunden"}`;
  }
  const minuten = Math.ceil(sekunden / 60);
  return `noch ${minuten} ${minuten === 1 ? "Minute" : "Minuten"}`;
}
