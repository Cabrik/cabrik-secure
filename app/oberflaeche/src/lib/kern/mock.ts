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
 * **Neun Fälle, und sie decken alle vier Zustände ab** — einzeln und in
 * jeder Kombination von Metadaten- und Absenderzustand, die vorkommen kann.
 */

import type { Fund, Geoeffnet, Kontakt, Sendedatei } from "./typen";

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
        verifiziertUeber: "safetyNumber",
      },
      metadaten: {
        fall: "vollstaendig",
        format: "PDF",
        entfernt: [
          {
            art: "personenname",
            ort: "PDF:DocInfo/Author",
            wert: "Dr. Anna Beispiel",
            schwere: "kritisch",
          },
          {
            art: "software",
            ort: "PDF:DocInfo/Producer",
            wert: "Bearbeitungsprogramm 3.1",
            schwere: "beachtlich",
          },
          {
            art: "zeitangabe",
            ort: "PDF:DocInfo/CreationDate",
            wert: "2026-03-01 09:12:00",
            schwere: "beachtlich",
          },
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
          {
            art: "geraet",
            ort: "Video:com.apple.quicktime.model",
            wert: "Gerät: iPhone 15 Pro",
            schwere: "beachtlich",
          },
          {
            art: "zeitangabe",
            ort: "Video:mvhd",
            wert: "Erstellungs- und Änderungszeitpunkt, auf die Sekunde genau",
            schwere: "beachtlich",
          },
        ],
      },
    },
  },

  {
    kennung: "signatur-verlangt",
    titel: "Signatur verlangt, aber keine da",
    worumEsGeht:
      "Dieselbe Lage wie beim Handyvideo — nicht signiert. Hier ist sie ein Fehler, weil Sie eine Signatur verlangt hatten. Der Sollwert steht in Magenta daneben.",
    signaturVerlangt: true,
    daten: {
      art: "datei",
      text: null,
      dateiname: "Zeugenaussage.pdf",
      groesseBytes: 412_672,
      zeitpunkt: 1_772_500_000,
      absender: { fall: "unsigniert" },
      metadaten: {
        fall: "vollstaendig",
        format: "PDF",
        entfernt: [
          {
            art: "software",
            ort: "PDF:DocInfo/Producer",
            wert: "Bearbeitungsprogramm 3.1",
            schwere: "beachtlich",
          },
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
      absender: {
        fall: "bekannt",
        fingerprint: "C9KY J9RH P88Z 1BQ4 M76W",
        name: "Bert Muster",
      },
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
      absender: {
        fall: "unbekannt",
        signierschluessel: "29WN 92PP 1JH8 7P1M 10C5",
      },
      metadaten: {
        fall: "teilweise",
        format: "MP3",
        grund:
          "Der Name des Kodierers steckt in den Zusatzdaten der Tonrahmen; er ließe sich nur durch Neuberechnen des Tons entfernen.",
        entfernt: [
          {
            art: "personenname",
            ort: "MP3:ID3v2/TPE1",
            wert: "Interpret oder Verfasser: Dr. Anna Beispiel",
            schwere: "kritisch",
          },
          {
            art: "vorschaubild",
            ort: "MP3:ID3v2/APIC",
            wert: "eingebettetes Bild — es trägt eigene Metadaten (2040 Bytes)",
            schwere: "kritisch",
          },
          {
            art: "geraet",
            ort: "MP3:ID3v2/PRIV",
            wert: "Kennung — damit erkennt ein Händler seinen Käufer wieder: WM/UniqueFileIdentifier",
            schwere: "kritisch",
          },
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
        // Der schwaechste Weg. spec/trust-store.md §5 verlangt, dass die
        // Oberflaeche den Vorbehalt benennt -- hier ist der Fall dazu.
        verifiziertUeber: "fingerprint",
      },
      metadaten: {
        fall: "teilweise",
        format: "TIFF-Rohdatei (DNG, NEF, ARW, CR2)",
        grund:
          "Das erste Verzeichnis ist nur eine Vorschau, das Hauptbild liegt in einem SubIFD. Wer die Aufnahme weitergeben will, exportiert sie als JPEG — das Ergebnis wird dann vollständig bereinigt.",
        entfernt: [],
        geblieben: [
          {
            art: "geraet",
            ort: "TIFF:Make",
            wert: "NIKON CORPORATION",
            schwere: "beachtlich",
          },
          {
            art: "ortsangabe",
            ort: "TIFF:GPS-IFD",
            wert: "GPS-Verzeichnis vorhanden",
            schwere: "kritisch",
          },
          {
            art: "vorschaubild",
            ort: "TIFF:SubIFDs",
            wert: "2 weitere Verzeichnisse",
            schwere: "kritisch",
          },
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
      metadaten: {
        fall: "unbekannt",
        formathinweis: "Photoshop-Dokument (PSD)",
      },
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
      absender: {
        fall: "widerrufen",
        fingerprint: "KMPS 4270 RACX 7CYN 4XRB",
        name: "Bert Muster",
      },
      metadaten: null,
    },
  },
];

// ---------------------------------------------------------------------------
// Kontakte
// ---------------------------------------------------------------------------

/**
 * Der Kontaktspeicher, in allen vier Vertrauenszuständen.
 *
 * Zwei Feinheiten stecken darin, die man beim Entwerfen leicht übersieht:
 *
 * 1. **Ein Kontakt ohne Post-Quantum-Schlüssel.** So sehen aus v1
 *    übernommene Kontakte aus. An sie lässt sich nur klassisch verschlüsseln,
 *    und das muss dastehen — sonst hält jemand eine Nachricht für
 *    quantensicher, die es nicht ist.
 * 2. **Ein Kontakt ohne Signierschlüssel** wäre eine Anonymitätsidentität.
 *    Sie kann empfangen, aber nicht signieren.
 */
export const KONTAKTE: Kontakt[] = [
  {
    name: "Dr. Anna Beispiel",
    fingerprint: "8F3B 1C2A 4D5E 4F60 9A7B 1C2D 3E4F 5061 8F3B 1C2A",
    vertrauen: "verifiziert",
    seit: 1_762_000_000,
    verifiziertAm: 1_770_000_000,
    verifiziertUeber: "safetyNumber",
    notiz: "Redaktion, Durchwahl 214",
    hatPostQuantum: true,
    safetyNumber:
      "38472 91053 66218 40397 15884 72609 " +
      "31745 08862 59413 77120 46538 90271",
  },
  {
    name: "Bert Muster",
    fingerprint: "C9KY J9RH P88Z 1BQ4 M76W DRS6 KMPS 4270 RACX 7CYN",
    vertrauen: "gesehen",
    seit: 1_771_500_000,
    verifiziertAm: null,
    verifiziertUeber: null,
    notiz: null,
    hatPostQuantum: true,
    safetyNumber:
      "50219 83746 12905 64831 27590 41368 " +
      "79024 15683 30947 82516 60473 19258",
  },
  {
    name: "Cora Steinbach",
    fingerprint: "DVKQ G1JC 05M3 MKPN 825Q WY9R 31XP RMH6 ARJ3 5AZJ",
    vertrauen: "gewechselt",
    seit: 1_755_000_000,
    verifiziertAm: 1_758_000_000,
    verifiziertUeber: "qr",
    notiz: "Neues Telefon seit März?",
    hatPostQuantum: true,
    safetyNumber:
      "61930 27584 40176 89352 13847 96205 " +
      "58619 30274 71508 42963 25180 73649",
  },
  {
    name: "Archiv (aus Version 1)",
    fingerprint: "W9VZ KAZQ 3QNH HBM3 6AQ6 KMS4 Q19E GZFJ P2EJ 216J",
    vertrauen: "verifiziert",
    seit: 1_700_000_000,
    verifiziertAm: 1_701_000_000,
    verifiziertUeber: "fingerprint",
    notiz: "Alter Schlüssel, noch aus v1 übernommen",
    // Der interessante Fall: kein Post-Quantum-Schlüssel.
    hatPostQuantum: false,
    safetyNumber:
      "10495 62837 74019 25683 90142 57368 " +
      "84027 19536 63805 47291 38650 20174",
  },
  {
    name: "Unbekannter Zuträger",
    fingerprint: "29WN 92PP 1JH8 7P1M 10C5 3C8T DS7D KKRQ 7TD9 QZBW",
    vertrauen: "widerrufen",
    seit: 1_768_000_000,
    verifiziertAm: null,
    verifiziertUeber: null,
    notiz: "Schlüssel nach dem Vorfall im Februar gesperrt",
    hatPostQuantum: true,
    safetyNumber:
      "72581 04963 18247 50396 82714 60539 " +
      "27418 95062 34871 60293 45786 11930",
  },
];

// ---------------------------------------------------------------------------
// Sendestapel
// ---------------------------------------------------------------------------

const bereinigt = (
  format: string,
  ...entfernt: Fund[]
): Sendedatei["befund"] => ({
  fall: "vollstaendig",
  format,
  entfernt,
});

/**
 * Drei Stapel, die je eine andere Entscheidung erzwingen.
 *
 * Sie sind der eigentliche Prüfstein für die Regel „stör nur, wenn du
 * wirklich etwas zu sagen hast": Beim ersten gibt es nichts zu sagen, beim
 * zweiten etwas, beim dritten viel — und der dritte muss trotzdem auf einen
 * Bildschirm passen.
 */
export interface Stapel {
  kennung: string;
  titel: string;
  worumEsGeht: string;
  dateien: Sendedatei[];
}

export const STAPEL: Stapel[] = [
  {
    kennung: "eine-saubere",
    titel: "Eine Datei, alles bereinigt",
    worumEsGeht:
      "Hier gibt es nichts zu entscheiden. Also wird auch nicht gefragt — die Vorschau bleibt zugeklappt.",
    dateien: [
      {
        name: "Protokoll.pdf",
        groesseBytes: 184_320,
        befund: bereinigt(
          "PDF",
          {
            art: "personenname",
            ort: "PDF:DocInfo/Author",
            wert: "Dr. Anna Beispiel",
            schwere: "kritisch",
          },
          {
            art: "software",
            ort: "PDF:DocInfo/Producer",
            wert: "Bearbeitungsprogramm 3.1",
            schwere: "beachtlich",
          },
        ),
      },
    ],
  },

  {
    kennung: "eine-mit-rest",
    titel: "Eine Datei mit Rest",
    worumEsGeht:
      "Jetzt gibt es etwas zu sagen — und deshalb muss es gesagt werden, bevor verschlüsselt wird.",
    dateien: [
      {
        name: "Mitschnitt.mp3",
        groesseBytes: 8_985_600,
        befund: {
          fall: "teilweise",
          format: "MP3",
          grund:
            "Der Name des Kodierers steckt in den Zusatzdaten der Tonrahmen; er ließe sich nur durch Neuberechnen des Tons entfernen.",
          entfernt: [
            {
              art: "personenname",
              ort: "MP3:ID3v2/TPE1",
              wert: "Dr. Anna Beispiel",
              schwere: "kritisch",
            },
          ],
          geblieben: [
            {
              art: "software",
              ort: "MP3:Tonrahmen",
              wert: 'der Name des Kodierers ("LAME") steht in den Zusatzdaten der Tonrahmen selbst',
              schwere: "beachtlich",
            },
          ],
        },
      },
    ],
  },

  {
    kennung: "grosser-stapel",
    titel: "41 Dateien — der Prüfstein",
    worumEsGeht:
      "38 sind vollständig bereinigt und stehen in einer aufklappbaren Zeile — auch die WAV-Datei, aus der ein Name und eine Gerätekennung entfernt wurden: Was weg ist, ist keine Entscheidung mehr. Die drei, bei denen etwas offenbleibt, stehen einzeln. Ein Bildschirm statt einundvierzig, ohne dass jemand wegsehen muss.",
    dateien: [
      ...Array.from({ length: 37 }, (_, i) => ({
        name: `Scan_${String(i + 1).padStart(3, "0")}.jpg`,
        groesseBytes: 1_200_000 + i * 4096,
        befund: bereinigt("JPEG", {
          art: "ortsangabe" as const,
          ort: "JPEG:GPS",
          wert: "Aufnahmeort entfernt",
          schwere: "kritisch" as const,
        }),
      })),
      {
        name: "Uebersicht.psd",
        groesseBytes: 47_185_920,
        befund: {
          fall: "unbekannt",
          formathinweis: "Photoshop-Dokument (PSD)",
        },
      },
      {
        name: "DSC_0042.NEF",
        groesseBytes: 31_457_280,
        befund: {
          fall: "teilweise",
          format: "TIFF-Rohdatei (DNG, NEF, ARW, CR2)",
          grund:
            "Das erste Verzeichnis ist nur eine Vorschau, das Hauptbild liegt in einem SubIFD. Wer die Aufnahme weitergeben will, exportiert sie als JPEG.",
          entfernt: [],
          geblieben: [
            {
              art: "ortsangabe",
              ort: "TIFF:GPS-IFD",
              wert: "GPS-Verzeichnis vorhanden",
              schwere: "kritisch",
            },
          ],
        },
      },
      {
        name: "Interview.wav",
        groesseBytes: 82_774_016,
        befund: bereinigt(
          "WAV",
          {
            art: "personenname",
            ort: "WAV:bext/Aufnehmender",
            wert: "Dr. Anna Beispiel",
            schwere: "kritisch",
          },
          {
            art: "geraet",
            ort: "WAV:bext/Gerätekennung",
            wert: "ZOOM-F8N-00473829",
            schwere: "kritisch",
          },
        ),
      },
      {
        name: "Notiz.txt.gpg",
        groesseBytes: 2048,
        befund: { fall: "fehler", grund: "Die Datei ließ sich nicht lesen." },
      },
    ],
  },
];
