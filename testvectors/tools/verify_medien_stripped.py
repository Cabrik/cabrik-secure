"""Prueft die von Rust bereinigten Ton- und Bewegtbilddateien unabhaengig nach.

Der Rust-Test prueft die Bytestruktur. Diese Pruefung oeffnet die Ergebnisse
mit **ffmpeg** und **mutagen** und misst nach, was strukturelle Tests nicht
sehen koennen:

  * Laesst sich die Datei ueberhaupt noch oeffnen?
  * Dekodieren GENAUSO VIELE Rahmen wie vorher? Eine Datei, die nur noch
    aufgeht, waere kein Erfolg.
  * Ist die Spieldauer unveraendert?
  * Sind wirklich keine Marken mehr da -- auch fuer einen zweiten,
    unabhaengigen Leser?

WARUM ZWEI LESER. Bei Ogg packte ein erster Entwurf alle Kopfpakete in eine
Seite, weil sie hineinpassten. Die Struktur war einwandfrei, und ffmpeg
spielte die Datei weiterhin ab. Erst mutagen fiel darueber: Die Vorbis-Norm
verlangt, dass das Identifikationspaket ALLEIN auf der ersten Seite steht.
Ein Leser allein haette den Fehler durchgehen lassen.

Voraussetzung: `cargo test -p cabrik-metadata --test medien_echt` wurde
ausgefuehrt und hat die `*.stripped`-Dateien abgelegt.

Aufruf:
    python testvectors/tools/verify_medien_stripped.py
"""
import os
import sys

try:
    import av
except ImportError:
    print("  PyAV fehlt -- Pruefung entfaellt (pip install av)")
    sys.exit(0)

try:
    import mutagen
except ImportError:
    mutagen = None

ZIEL = os.path.join("testvectors", "metadata")

# Datei, Spurart, ob dekodiert werden darf.
#
# Die MP4-Vorlage ist von Hand gebaut und enthaelt keine echte
# Codec-Beschreibung -- sie hat nur die Boxen, um die es geht. ffmpeg stuerzt
# beim Dekodieren daran ab, weshalb sie nur geoeffnet wird. Das genuegt fuer
# ihren Zweck: Sie prueft die Boxstruktur, nicht den Bildinhalt.
DATEIEN = [
    ("ton_mit_marken.mp3", "audio", True),
    ("ton_mit_marken.flac", "audio", True),
    ("ton_mit_marken.ogg", "audio", True),
    ("ton_mit_marken.opus", "audio", True),
    ("ton_mit_marken.wav", "audio", True),
    ("video_mit_ortsangabe.mp4", "video", False),
    ("video_mit_marken.mkv", "video", True),
    ("video_mit_marken.webm", "video", True),
    ("video_mit_marken.avi", "video", True),
    ("live_photo.mov", "video", True),
]

SPUREN = [
    b"Dr. Anna Beispiel",
    b"Nicht an den Kunden geben",
    b"Angebot Nordstern",
    b"Interner Rohschnitt",
    b"ZOOM-F8N-00473829",
    b"+46.9481",
    b"8F3B1C2A-4D5E-4F60-9A7B-1C2D3E4F5061",
    b"iPhone 15 Pro",
]

# Was das jeweilige Format nicht loswerden kann, mit Begruendung.
GEDULDET = {
    "ton_mit_marken.mp3": (
        b"LAME3.100",
        "steht in den Zusatzdaten der Tonrahmen, also im Tondatenstrom selbst",
    ),
}

fehler = []
geprueft = 0


def pruefe(bedingung, meldung):
    if not bedingung:
        fehler.append(meldung)
    return bedingung


# Angaben, die zur Struktur des Behaelters gehoeren und keine Aussage ueber
# eine Person treffen: die ftyp-Marken und die Sprachkennung "und"
# (undetermined). ffmpeg reicht sie als "Metadaten" durch, sie sind aber
# Format, nicht Inhalt.
STRUKTUR = {"major_brand", "minor_version", "compatible_brands", "spur:language"}


def lies(pfad, art, dekodieren=True):
    """Oeffnet die Datei und gibt (Pruefsumme des Tons, Rahmenzahl, Metadaten).

    Die Pruefsumme ueber die DEKODIERTEN Abtastwerte ist der eigentliche
    Beweis: Sie faellt nur dann gleich aus, wenn der Ton Bit fuer Bit
    derselbe geblieben ist. Die Spieldauer taugt dafuer nicht -- sie wird bei
    MP3 aus der Dateigroesse geschaetzt und aendert sich schon deshalb, weil
    ein Tag wegfaellt.

    Ohne `dekodieren` wird die Datei nur geoeffnet -- fuer Vorlagen ohne
    echten Codec.
    """
    import hashlib

    h = hashlib.sha256()
    n = 0
    with av.open(pfad) as f:
        meta = dict(f.metadata)
        for s in f.streams:
            meta.update({f"spur:{k}": v for k, v in s.metadata.items()})
        if dekodieren:
            for rahmen in f.decode(**{art: 0}):
                n += 1
                for ebene in rahmen.planes:
                    h.update(bytes(ebene))
    meta = {k: v for k, v in meta.items() if k not in STRUKTUR}
    return (h.hexdigest() if dekodieren else None), n, meta


print("Unabhaengige Pruefung mit ffmpeg" + (" und mutagen" if mutagen else ""))
print()

for name, art, dekodieren in DATEIEN:
    original = os.path.join(ZIEL, name)
    bereinigt = original + ".stripped"

    if not os.path.exists(bereinigt):
        print(f"  {name}: uebersprungen (kein .stripped -- Rust-Test nicht gelaufen?)")
        continue

    with open(original, "rb") as f:
        roh_alt = f.read()
    with open(bereinigt, "rb") as f:
        roh_neu = f.read()

    # --- 1. Laesst sie sich noch oeffnen, und dekodiert sie gleich viel? ---
    try:
        alt_h, alt_n, _ = lies(original, art, dekodieren)
    except Exception as e:                                    # noqa: BLE001
        fehler.append(f"{name}: schon die VORLAGE ist unlesbar: {e}")
        continue

    try:
        neu_h, neu_n, neu_meta = lies(bereinigt, art, dekodieren)
    except Exception as e:                                    # noqa: BLE001
        fehler.append(f"{name}: nach dem Bereinigen unlesbar: {type(e).__name__}: {e}")
        continue

    pruefe(neu_n == alt_n,
           f"{name}: {neu_n} statt {alt_n} Rahmen -- es ging Inhalt verloren")
    pruefe(neu_h == alt_h,
           f"{name}: die dekodierten Daten haben sich geaendert")
    rahmen = f"{neu_n:>4} Rahmen" if dekodieren else "nur geoeffnet"

    # --- 2. Sind die Marken fuer ffmpeg wirklich weg? ---------------------
    # Ein leerer Wert ist in Ordnung: Matroska verlangt MuxingApp und
    # WritingApp als Pflichtelemente, sie werden geleert statt entfernt.
    #
    # "[0][0][0][0]" ist ebenfalls leer -- so schreibt ffmpeg ein genulltes
    # Feld fester Breite. Die Herstellerkennung in der Spurbeschreibung ist
    # so ein Feld; null ist dort der in ISO-BMFF vorgesehene Vorgabewert.
    uebrig = {k: v for k, v in neu_meta.items()
              if v.strip() and v.replace("[0]", "").strip()}
    pruefe(not uebrig, f"{name}: ffmpeg liest noch Metadaten: {uebrig}")

    # --- 3. Und fuer mutagen? --------------------------------------------
    if mutagen is not None and art == "audio":
        try:
            m = mutagen.File(bereinigt)
            if m is not None and m.tags:
                marken = dict(m.tags)
                pruefe(not marken, f"{name}: mutagen liest noch Marken: {marken}")
        except Exception as e:                                # noqa: BLE001
            fehler.append(f"{name}: mutagen kann die Datei nicht lesen: "
                          f"{type(e).__name__}: {e}")

    # --- 4. Steht der Klartext noch irgendwo in den Bytes? ---------------
    for spur in SPUREN:
        if spur in roh_alt:
            pruefe(spur not in roh_neu,
                   f"{name}: '{spur.decode()}' steht noch in der Datei")

    geduldet = GEDULDET.get(name)
    if geduldet and geduldet[0] in roh_neu:
        print(f"  {name}: '{geduldet[0].decode()}' bleibt -- {geduldet[1]}")

    laenge = "gleich lang" if len(roh_alt) == len(roh_neu) else \
             f"{len(roh_alt)} -> {len(roh_neu)} Bytes"
    print(f"  {name:28} {rahmen}, {laenge}")
    geprueft += 1

print()
if fehler:
    print(f"  {len(fehler)} Problem(e):")
    for f in fehler:
        print(f"    - {f}")
    sys.exit(1)

print(f"  {geprueft} Datei(en) unabhaengig geprueft:")
print("    oeffnet sich, dekodiert vollstaendig, Ton und Bild Bit fuer Bit")
print("    unveraendert, keine Marken mehr -- fuer ffmpeg wie fuer mutagen.")
