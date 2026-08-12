/**
 * Beispielfälle für den Prototyp.
 *
 * # Warum sie echt aussehen
 *
 * Ein Prototyp mit „Lorem ipsum" und „Datei1.txt" prüft nichts. Die Fälle
 * hier stammen aus der wirklichen Arbeit am Kern: der Aufnahmeort in
 * `moov/udta/©xyz`, der Kodierername in den MP3-Tonrahmen, der
 * Kennzeichner, der die beiden Hälften eines Live Photo verknüpft, das
 * Hauptbild im `SubIFD` einer Rohdatei.
 *
 * Erst an solchen Fällen zeigt sich, ob eine Anzeige trägt. „Teilweise
 * bereinigt" mit einem erfundenen Grund liest sich immer gut.
 *
 * **Acht Fälle, und sie decken alle vier Zustände ab** — einzeln und in
 * jeder Kombination von Metadaten- und Absenderzustand, die vorkommen kann.
 */

import type { Geoeffnet } from "./typen";

export interface Fall {
  kennung: string;
  titel: string;
  /** Warum dieser Fall im Prototyp steht. */
  worumEsGeht: string;
  daten: Geoeffnet;
  /** Ob der Nutzer eine Signatur verlangt hatte. */
  signaturVerlangt?: boolean;
}

export const FAELLE: Fall[] = [
  {
    kennung: "alles-gut",
    titel: "Verifizierter Absender, vollständig bereinigt",
    worumEsGeht: "Der einzige Fall, in dem beides grün ist.",
    daten: {
      art: "datei",
      text: null,
      dateiname: "Protokoll.pdf",
      groesseBytes: 184_320,
      zeitpunkt: 1_772_000_000,
      absender: {
        fall: "verifiziert",
        fingerprint: "8F3B 1C2A 4D5E 4F60 9A7B",
        name: "Dr. Anna Beispiel",
        verifiziertAm: 1_770_000_000,
      },
      metadaten: {
        fall: "vollstaendig",
        format: "PDF",
        entfernt: [
          { art: "personenname", ort: "PDF:DocInfo/Author", wert: "Dr. Anna Beispiel", schwere: "kritisch" },
          { art: "software", ort: "PDF:DocInfo/Producer", wert: "Bearbeitungsprogramm 3.1", schwere: "beachtlich" },
          { art: "zeitangabe", ort: "PDF:DocInfo/CreationDate", wert: "2026-03-01 09:12:00", schwere: "beachtlich" },
        ],
      },
    },
  },

  {
    kennung: "handyvideo",
    titel: "Handyvideo mit Aufnahmeort",
    worumEsGeht:
      "Der schwerwiegendste Fund des ganzen Programms — und er ist entfernt. Grün, obwohl der Inhalt heikel war.",
    daten: {
      art: "datei",
      text: null,
      dateiname: "IMG_4711.MOV",
      groesseBytes: 24_117_248,
      zeitpunkt: 1_772_100_000,
      absender: { fall: "unsigniert" },
      metadaten: {
        fall: "vollstaendig",
        format: "QuickTime (MOV)",
        entfernt: [
          {
            art: "ortsangabe",
            ort: "Video:com.apple.quicktime.location.ISO6709",
            wert: "Aufnahmeort: +46.9481+007.4474+561.000/",
            schwere: "kritisch",
          },
          {
            art: "geraet",
            ort: "Video:com.apple.quicktime.content.identifier",
            wert: "Kennzeichner — verknüpft die beiden Hälften eines Live Photo: 8F3B1C2A-4D5E-4F60-9A7B-1C2D3E4F5061",
            schwere: "kritisch",
          },
          { art: "geraet", ort: "Video:com.apple.quicktime.model", wert: "Gerät: iPhone 15 Pro", schwere: "beachtlich" },
          { art: "zeitangabe", ort: "Video:mvhd", wert: "Erstellungs- und Änderungszeitpunkt, auf die Sekunde genau", schwere: "beachtlich" },
        ],
      },
    },
  },

  {
    kennung: "nicht-verifiziert",
    titel: "Bekannter, aber nie verifizierter Kontakt",
    worumEsGeht:
      "Der Name steht im Kontaktspeicher — das ist eine Behauptung, keine Prüfung. Gelb.",
    daten: {
      art: "text",
      text: "Die Unterlagen liegen ab morgen im vereinbarten Fach. Melde dich nicht über den üblichen Weg.",
      dateiname: null,
      groesseBytes: 98,
      zeitpunkt: 1_772_200_000,
      absender: { fall: "bekannt", fingerprint: "C9KY J9RH P88Z 1BQ4 M76W", name: "Bert Muster" },
      metadaten: null,
    },
  },

  {
    kennung: "mp3-rest",
    titel: "MP3 — der Kodierername bleibt im Tonstrom",
    worumEsGeht:
      "Teilweise bereinigt, mit einem Grund, der stimmt: Ihn zu entfernen hieße, den Ton neu zu berechnen.",
    daten: {
      art: "datei",
      text: null,
      dateiname: "Mitschnitt.mp3",
      groesseBytes: 8_985_600,
      zeitpunkt: null,
      absender: { fall: "unbekannt", signierschluessel: "29WN 92PP 1JH8 7P1M 10C5" },
      metadaten: {
        fall: "teilweise",
        format: "MP3",
        grund:
          "Der Name des Kodierers steckt in den Zusatzdaten der Tonrahmen; er ließe sich nur durch Neuberechnen des Tons entfernen.",
        entfernt: [
          { art: "personenname", ort: "MP3:ID3v2/TPE1", wert: "Interpret oder Verfasser: Dr. Anna Beispiel", schwere: "kritisch" },
          { art: "vorschaubild", ort: "MP3:ID3v2/APIC", wert: "eingebettetes Bild — es trägt eigene Metadaten (2040 Bytes)", schwere: "kritisch" },
          { art: "geraet", ort: "MP3:ID3v2/PRIV", wert: "Kennung — damit erkennt ein Händler seinen Käufer wieder: WM/UniqueFileIdentifier", schwere: "kritisch" },
        ],
        geblieben: [
          {
            art: "software",
            ort: "MP3:Tonrahmen",
            wert: 'der Name des Kodierers („LAME") steht in den Zusatzdaten der Tonrahmen selbst',
            schwere: "beachtlich",
          },
        ],
      },
    },
  },

  {
    kennung: "rohdatei",
    titel: "Rohdatei aus einer Kamera",
    worumEsGeht:
      "Erkannt und unangetastet gelassen. Sie umzuschreiben hieße, ihr Hauptbild für ein Vorschaubild zu halten.",
    daten: {
      art: "datei",
      text: null,
      dateiname: "DSC_0042.NEF",
      groesseBytes: 31_457_280,
      zeitpunkt: 1_771_900_000,
      absender: {
        fall: "verifiziert",
        fingerprint: "DVKQ G1JC 05M3 MKPN 825Q",
        name: "Cora Steinbach",
        verifiziertAm: 1_765_000_000,
      },
      metadaten: {
        fall: "teilweise",
        format: "TIFF-Rohdatei (DNG, NEF, ARW, CR2)",
        grund:
          "Das erste Verzeichnis ist nur eine Vorschau, das Hauptbild liegt in einem SubIFD. Wer die Aufnahme weitergeben will, exportiert sie als JPEG — das Ergebnis wird dann vollständig bereinigt.",
        entfernt: [],
        geblieben: [
          { art: "geraet", ort: "TIFF:Make", wert: "NIKON CORPORATION", schwere: "beachtlich" },
          { art: "ortsangabe", ort: "TIFF:GPS-IFD", wert: "GPS-Verzeichnis vorhanden", schwere: "kritisch" },
          { art: "vorschaubild", ort: "TIFF:SubIFDs", wert: "2 weitere Verzeichnisse", schwere: "kritisch" },
        ],
      },
    },
  },

  {
    kennung: "unbekanntes-format",
    titel: "Format nicht verstanden",
    worumEsGeht:
      "Die Flagge am Instrument. Kein Fehler, kein Grün — das Programm weiß es schlicht nicht.",
    daten: {
      art: "datei",
      text: null,
      dateiname: "Entwurf.psd",
      groesseBytes: 47_185_920,
      zeitpunkt: null,
      absender: { fall: "unsigniert" },
      metadaten: { fall: "unbekannt", formathinweis: "Photoshop-Dokument (PSD)" },
    },
  },

  {
    kennung: "schluessel-gewechselt",
    titel: "Schlüssel gewechselt — vorher verifiziert",
    worumEsGeht:
      "Der Fall, der in einer Ampel verlorenginge: gelb, aber mit deutlich anderem Gewicht als ein gewöhnlicher Hinweis.",
    daten: {
      art: "text",
      text: "Treffpunkt geändert. Details wie besprochen.",
      dateiname: null,
      groesseBytes: 44,
      zeitpunkt: 1_772_300_000,
      absender: {
        fall: "gewechselt",
        fingerprint: "W9VZ KAZQ 3QNH HBM3 6AQ6",
        name: "Dr. Anna Beispiel",
        vorherVerifiziert: true,
      },
      metadaten: null,
    },
  },

  {
    kennung: "widerrufen",
    titel: "Schlüssel widerrufen",
    worumEsGeht:
      "Der einzige Absenderfall, in dem etwas aktiv nicht stimmt. Die Signatur ist gültig — das heißt hier gerade nichts Gutes.",
    daten: {
      art: "text",
      text: "Bitte schick mir die Datei noch einmal, ich komme nicht mehr an die alte.",
      dateiname: null,
      groesseBytes: 71,
      zeitpunkt: 1_772_400_000,
      absender: { fall: "widerrufen", fingerprint: "KMPS 4270 RACX 7CYN 4XRB", name: "Bert Muster" },
      metadaten: null,
    },
  },
];
