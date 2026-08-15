/**
 * Beispielfälle für den Prototyp.
 *
 * # Warum sie echt aussehen
 *
 * Ein Prototyp mit „Lorem ipsum“ und „Datei1.txt“ prüft nichts. Die Fälle
 * hier stammen aus der wirklichen Arbeit am Kern: der Aufnahmeort in
 * `moov/udta/©xyz`, der Kodierername in den MP3-Tonrahmen, der
 * Kennzeichner, der die beiden Hälften eines Live Photo verknüpft, das
 * Hauptbild im `SubIFD` einer Rohdatei.
 *
 * Erst an solchen Fällen zeigt sich, ob eine Anzeige trägt. „Teilweise
 * bereinigt“ mit einem erfundenen Grund liest sich immer gut.
 *
 * **Neun Fälle, und sie decken alle vier Zustände ab** — einzeln und in
 * jeder Kombination von Metadaten- und Absenderzustand, die vorkommen kann.
 */

import type {
  Aussenansicht,
  Fund,
  Geoeffnet,
  Identitaet,
  Kontakt,
  Loeschbeurteilung,
  Nutzlastbefund,
  Sendedatei,
} from "./typen";

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
    titel: "Verifizierter Absender, nichts in der Datei",
    worumEsGeht:
      "Der einzige Fall, in dem beides grün ist — und er sagt zugleich etwas über den Absender: Er hat die Datei bereinigt, bevor er sie schickte.",
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
      metadaten: { fall: "erkannt", format: "PDF", funde: [] },
    },
  },

  {
    kennung: "handyvideo",
    titel: "Handyvideo mit Aufnahmeort",
    worumEsGeht:
      "Der schwerwiegendste Fund des Programms — und hier steht er noch drin. Der Absender hat mitgeschickt, wo er stand: 46,948° Nord, 7,447° Ost, 561 Meter über dem Meer. Das ist die Berner Innenstadt, auf zwanzig Meter genau.",
    daten: {
      art: "datei",
      text: null,
      dateiname: "IMG_4711.MOV",
      groesseBytes: 24_117_248,
      zeitpunkt: 1_772_100_000,
      absender: { fall: "unsigniert" },
      metadaten: {
        fall: "erkannt",
        format: "QuickTime (MOV)",
        funde: [
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
        fall: "erkannt",
        format: "PDF",
        funde: [
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
    titel: "MP3 mit Interpret, Bild und Händlerkennung",
    worumEsGeht:
      "Drei kritische Funde in einer Datei, die harmlos aussieht. Die Händlerkennung im PRIV-Rahmen erkennt den ursprünglichen Käufer wieder — auch nach beliebig vielen Weitergaben.",
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
        fall: "erkannt",
        format: "MP3",
        funde: [
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
          {
            art: "software",
            ort: "MP3:Tonrahmen",
            wert: "der Name des Kodierers („LAME“) steht in den Zusatzdaten der Tonrahmen selbst",
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
      "Hersteller, GPS-Verzeichnis und zwei weitere Bildverzeichnisse. Wer eine Rohdatei weitergibt, gibt die volle Aufnahmesituation mit — hier steht sie aufgezählt da.",
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
        fall: "erkannt",
        format: "TIFF-Rohdatei (DNG, NEF, ARW, CR2)",
        funde: [
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
 * wirklich etwas zu sagen hast“: Beim ersten gibt es nichts zu sagen, beim
 * zweiten etwas, beim dritten viel — und der dritte muss trotzdem auf einen
 * Bildschirm passen.
 */
export interface Stapel {
  kennung: string;
  titel: string;
  worumEsGeht: string;
  dateien: Sendedatei[];
}

const OFFICE_STAPEL: Stapel = {
  kennung: "mit-verlauf",
  titel: "Ein Dokument mit Verlauf",
  worumEsGeht:
    "Anmerkungen und nachverfolgte Aenderungen sind kein Metadatum, sondern Inhalt. Sie zu entfernen ist deshalb eine Entscheidung mit Folgen -- und keine, die ein Programm wortlos treffen darf.",
  dateien: [
    {
      pfad: "C:\\Beispiele\\Vertragsentwurf.docx",
      name: "Vertragsentwurf.docx",
      groesseBytes: 412_672,
      fassungen: [],
      befund: {
        fall: "teilweise",
        format: "OOXML (Word)",
        grund:
          "Anmerkungen und nachverfolgte Aenderungen bleiben erhalten. Sie zu entfernen wuerde den Inhalt veraendern, und darueber entscheidet niemand ausser Ihnen.",
        entfernt: [
          {
            art: "personenname",
            ort: "OOXML:docProps/core.xml/creator",
            wert: "Dr. Anna Beispiel",
            schwere: "kritisch",
          },
          {
            art: "bearbeitungssitzung",
            ort: "OOXML:docProps/app.xml/TotalTime",
            wert: "482 Minuten Bearbeitungszeit",
            schwere: "beachtlich",
          },
        ],
        geblieben: [
          {
            art: "kommentar",
            ort: "OOXML:word/comments.xml",
            wert: '3 Anmerkungen, u. a. "Das koennen wir so nicht unterschreiben"',
            schwere: "kritisch",
          },
          {
            art: "nachverfolgte_aenderung",
            ort: "OOXML:word/document.xml/w:del",
            wert: "11 Loeschungen, deren Text noch enthalten ist",
            schwere: "kritisch",
          },
        ],
      },
    },
  ],
};

export const STAPEL: Stapel[] = [
  OFFICE_STAPEL,

  {
    kennung: "eine-saubere",
    titel: "Eine Datei, alles bereinigt",
    worumEsGeht:
      "Hier gibt es nichts zu entscheiden. Also wird auch nicht gefragt — die Vorschau bleibt zugeklappt.",
    dateien: [
      {
        pfad: "C:\\Beispiele\\Protokoll.pdf",
        name: "Protokoll.pdf",
        groesseBytes: 184_320,
        // Der Fall, um den es geht: Ein Dokument, aus dem jemand Namen
        // entfernt hat -- und die vorige Fassung steckt vollstaendig
        // weiter darin. Ein Leser zeigt sie nicht an, ein Werkzeug schon.
        fassungen: [
          {
            nummer: 1,
            bytes: 96_112,
            seiten: 4,
            wirdAngezeigt: false,
            auszug: "Vermerk zur Sitzung vom 14. Maerz, vertraulich.",
            nurHier: [
              "Hinweisgeber: Martin Kessler, Abteilung Einkauf",
              "Telefon privat: 0170 4432190",
            ],
          },
          {
            nummer: 2,
            bytes: 151_904,
            seiten: 4,
            wirdAngezeigt: false,
            auszug: "Vermerk zur Sitzung vom 14. Maerz, vertraulich.",
            nurHier: ["Die Angaben wurden vom Hinweisgeber selbst bestaetigt."],
          },
          {
            nummer: 3,
            bytes: 184_320,
            seiten: 4,
            wirdAngezeigt: true,
            auszug: "Vermerk zur Sitzung vom 14. Maerz, vertraulich.",
            nurHier: [],
          },
        ],
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
        pfad: "C:\\Beispiele\\Mitschnitt.mp3",
        name: "Mitschnitt.mp3",
        groesseBytes: 8_985_600,
        fassungen: [],
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
        pfad: `C:\\Beispiele\\Scan_${String(i + 1).padStart(3, "0")}.jpg`,
        name: `Scan_${String(i + 1).padStart(3, "0")}.jpg`,
        groesseBytes: 1_200_000 + i * 4096,
        fassungen: [],
        befund: bereinigt("JPEG", {
          art: "ortsangabe" as const,
          ort: "JPEG:GPS",
          wert: "Aufnahmeort entfernt",
          schwere: "kritisch" as const,
        }),
      })),
      {
        pfad: "C:\\Beispiele\\Uebersicht.psd",
        name: "Uebersicht.psd",
        groesseBytes: 47_185_920,
        fassungen: [],
        befund: {
          fall: "unbekannt",
          formathinweis: "Photoshop-Dokument (PSD)",
        },
      },
      {
        pfad: "C:\\Beispiele\\DSC_0042.NEF",
        name: "DSC_0042.NEF",
        groesseBytes: 31_457_280,
        fassungen: [],
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
        pfad: "C:\\Beispiele\\Interview.wav",
        name: "Interview.wav",
        groesseBytes: 82_774_016,
        fassungen: [],
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
        pfad: "C:\\Beispiele\\Notiz.txt.gpg",
        name: "Notiz.txt.gpg",
        groesseBytes: 2048,
        fassungen: [],
        befund: { fall: "fehler", grund: "Die Datei ließ sich nicht lesen." },
      },
    ],
  },
];

// ---------------------------------------------------------------------------
// Eigene Identität
// ---------------------------------------------------------------------------

export const IDENTITAET: Identitaet = {
  bezeichnung: "Arbeitsrechner",
  // Bindestriche und dreizehn Vierergruppen -- so, wie `display_full` im
  // Kern gruppiert. Die Attrappe hatte Leerzeichen und zehn Gruppen; das
  // sah plausibel aus und stimmte an keiner Stelle.
  fingerprint: "K7QM-2XVB-9HTN-4RDP-8CWJ-3FGY-6LZA-5NKE-1SUH-0MRB-4TVC-8XZQ-2JW0",
  // Acht Zeichen ohne Trenner, wie `Fingerprint::short`. Nur zum
  // Unterscheiden in Listen -- nie Grundlage einer Verifikation.
  fingerprintKurz: "K7QM2XVB",
  erzeugtAm: 1_762_400_000,
  kdf: "empfohlen",
  kdfSpeicherMib: 256,
  hatSignierschluessel: true,
  hatPostQuantum: true,
  pfad: "C:\\Users\\name\\AppData\\Roaming\\CabrikSecure\\identity.cabrik-key",
};

/**
 * Die aus v1 übernommene Identität — ohne Post-Quantum, ohne Signierung.
 *
 * Sie trägt **eigene** KDF-Werte: `kdf: null` heißt nicht „unbekannt“,
 * sondern „zu keiner der drei Stufen gehörend“. Der Fall kommt vor, weil
 * die Kommandozeile eigene Werte zulässt — und er ist der, an dem sich die
 * Anzeige entscheidet: Wer nur benannte Stufen baut, zeigt hier ein leeres
 * Feld, wo eine Zahl stehen müsste.
 */
export const IDENTITAET_V1: Identitaet = {
  bezeichnung: "Alter Schlüssel (v1)",
  fingerprint: "T4XW-8BQM-1JHC-7PVD-2RNG-9FKY-5ZLA-3SEU-6MTB-0WQJ-5HND-1CVP-9RG0",
  fingerprintKurz: "T4XW8BQM",
  erzeugtAm: 1_700_000_000,
  kdf: null,
  kdfSpeicherMib: 195,
  hatSignierschluessel: false,
  hatPostQuantum: false,
  pfad: "C:\\Users\\name\\Documents\\cabrik-v1.key",
};

// ---------------------------------------------------------------------------
// Sicheres Löschen
// ---------------------------------------------------------------------------

/**
 * Was der Bildschirm über eine zu löschende Datei zeigt.
 *
 * Pfad und Größe stammen vom Dateisystem, die Beurteilung vom Kern. Sie
 * stehen hier getrennt, weil sie es auch dort sind — `Assessment` kennt
 * keinen Pfad.
 */
export interface Loeschfall {
  pfad: string;
  groesseBytes: number;
  beurteilung: Loeschbeurteilung;
}

/**
 * Drei Datenträgerlagen — und die mittlere ist die häufigste.
 *
 * Der Prüfstein für `spec/anzeige.md` §4.3: `bestEffort` ist der Normalfall.
 * Eine Oberfläche, die deshalb dauernd gelb leuchtet, erzieht zum Wegsehen.
 */
export const LOESCHFAELLE: Loeschfall[] = [
  {
    pfad: "D:\\Archiv\\Protokoll-2019.pdf",
    groesseBytes: 2_411_724,
    beurteilung: {
      faehigkeit: "ueberschreiben",
      vorbehalte: [{ art: "warSchreibgeschuetzt" }],
    },
  },
  {
    pfad: "C:\\Users\\name\\Desktop\\Notizen.txt",
    groesseBytes: 4_096,
    beurteilung: {
      faehigkeit: "bestEffort",
      vorbehalte: [{ art: "kopienMoeglich" }],
    },
  },
  {
    pfad: "C:\\Users\\name\\OneDrive\\Vertraulich\\Liste.xlsx",
    groesseBytes: 88_064,
    beurteilung: {
      faehigkeit: "bestEffort",
      vorbehalte: [
        {
          art: "cloudOrdner",
          hinweis: "Ordnername „OneDrive“ und Reparse-Punkt",
        },
        { art: "kopienMoeglich" },
        { art: "zeitstempelBlieb" },
      ],
    },
  },
];

// ---------------------------------------------------------------------------
// Außenansicht
// ---------------------------------------------------------------------------

export const AUSSENANSICHTEN: Aussenansicht[] = [
  {
    fassung: "v2",
    suite: "Post-Quantum-Hybrid (0x0002)",
    kapseln: 3,
    groesseBytes: 190_112,
    offengelegt: [],
  },
  {
    fassung: "v1",
    suite: "klassisch (v1)",
    kapseln: 1,
    groesseBytes: 188_204,
    offengelegt: [
      "Dateiname: Kuendigung-Mueller.pdf",
      "Klartextgröße: 184320 Bytes",
      "Signierschlüssel des Absenders",
    ],
  },
];

// ---------------------------------------------------------------------------
// Austausch-Nutzlasten
// ---------------------------------------------------------------------------

/**
 * Fünf Nutzlasten, die je einen anderen Ausgang erzwingen.
 *
 * **Die Nutzlast wird hier nicht zerlegt.** Das Format zu lesen, die
 * Schlüssel zu prüfen und den Fingerprint neu zu berechnen gehört in den
 * Kern — in Phase 4 kommt der Befund von dort. Hier steht das Ergebnis
 * bereits fest, denn geprüft werden soll die Anzeige, nicht ein
 * nachgebauter Parser, den es später gar nicht gibt.
 */
export interface Nutzlastfall {
  kennung: string;
  titel: string;
  /** Gekürzt — die echte ist rund 2050 Zeichen lang. */
  text: string;
  befund: Nutzlastbefund;
}

const rumpf = (kopf: string) =>
  `cabrik:v2:${kopf}${"7QMB2XVN9HTK4RDP8CWJ3FGY6LZA5NKE1SUH0MRB".repeat(3)}`;

export const NUTZLASTEN: Nutzlastfall[] = [
  {
    kennung: "vollstaendig",
    titel: "Vollständig",
    text: rumpf("A"),
    befund: {
      fall: "gelesen",
      fingerprint: "R3PW 7KQN 2MHB 9XDT 5CVJ 1FGZ 8LYA 4NSE 6UHK 0BRM",
      hatSignierschluessel: true,
      hatPostQuantum: true,
      schonBekannt: null,
    },
  },
  {
    kennung: "ohne-pq",
    titel: "Ohne Post-Quantum-Schlüssel",
    text: rumpf("B"),
    befund: {
      fall: "gelesen",
      fingerprint: "H8ZC 4VTM 1NKQ 6PBD 3RWJ 9FGX 2LYA 7SEU 5MHT 0QRB",
      hatSignierschluessel: true,
      hatPostQuantum: false,
      schonBekannt: null,
    },
  },
  {
    kennung: "ohne-signatur",
    titel: "Ohne Signierschlüssel",
    text: rumpf("C"),
    befund: {
      fall: "gelesen",
      fingerprint: "N5TB 8WQK 3JHM 1PVD 7RCG 2FZY 9LXA 4SEU 6MKT 0DRB",
      hatSignierschluessel: false,
      hatPostQuantum: true,
      schonBekannt: null,
    },
  },
  {
    kennung: "schluesselwechsel",
    titel: "Bekannter Kontakt, anderer Schlüssel",
    text: rumpf("D"),
    befund: {
      fall: "gelesen",
      fingerprint: "Q2WM 6ZKB 4NHT 8PJD 1RVG 5FCY 3LXA 9SEU 7MKT 0BRN",
      hatSignierschluessel: true,
      hatPostQuantum: true,
      schonBekannt: { name: "Bert Muster", gleicherSchluessel: false },
    },
  },
  {
    kennung: "beschaedigt",
    titel: "Beim Kopieren abgeschnitten",
    text: "cabrik:v2:A7QMB2XVN9HTK4RDP8CWJ3FGY6LZA5NK",
    befund: {
      fall: "beschaedigt",
      grund:
        "Die Prüfsumme am Ende passt nicht zu den Schlüsseln davor. Das " +
        "deutet auf einen Übertragungsfehler — etwa einen Zeilenumbruch, den " +
        "ein Mailprogramm eingefügt hat.",
    },
  },
];
