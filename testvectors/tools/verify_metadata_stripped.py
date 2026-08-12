"""Prueft die von Rust bereinigten Bilder unabhaengig nach.

Der Rust-Test prueft die Bytestruktur. Diese Pruefung oeffnet die Ergebnisse
mit Pillow und misst nach, was strukturelle Tests nicht sehen koennen:

  * Ist das Ergebnis ueberhaupt noch ein gueltiges Bild?
  * Sind die Pixel unveraendert?
  * Ist die Farbtabelle intakt?
  * Sind wirklich keine Metadaten mehr da?

Der v1-Palette-Bug erzeugte eine *gueltige* Datei mit *falschen Farben*.
Genau das faellt nur auf, wenn jemand das Bild wirklich oeffnet und die
Pixel vergleicht.

Voraussetzung: `cargo test -p cabrik-metadata --test fixtures` wurde
ausgefuehrt und hat die `*.stripped`-Dateien abgelegt.

Aufruf:
    python testvectors/tools/verify_metadata_stripped.py
"""
import json
import os
import sys

import piexif
from PIL import Image

# Ohne diese Anmeldung kann Pillow keine HEIC-Datei oeffnen -- und die
# Pruefung meldete "laesst sich nicht mehr oeffnen", obwohl die Datei in
# Ordnung war. Die unabhaengige HEIC-Pruefung lief damit ueberhaupt nie.
try:
    import pillow_heif

    pillow_heif.register_heif_opener()
except ImportError:
    pass

ZIEL = os.path.join("testvectors", "metadata")

with open(os.path.join(ZIEL, "manifest.json"), encoding="utf-8") as f:
    manifest = json.load(f)

fehler = []
geprueft = 0


def pruefe(bedingung, meldung):
    if not bedingung:
        fehler.append(meldung)
    return bedingung


# Modi, die KEIN Bild bezeichnen. Ton und Bewegtbild stehen im selben
# Manifest, werden aber von verify_medien_stripped.py geprueft -- mit
# ffmpeg und mutagen statt mit Pillow.
#
# Ohne diese Grenze scheiterte die Pruefung an der ersten MP3-Datei
# ("cannot identify image file"), und zwar erst, als Ton und Video ins
# Manifest kamen. Ein Pruefwerkzeug, das an neuen Eintraegen zerbricht,
# haette die CI beim ersten Lauf angehalten.
NICHT_BILD = {"ton", "video"}

# Formate, die Pillow nicht oeffnen kann. Sie werden von den Rust-Tests
# strukturell geprueft, PDF zusaetzlich mit pypdf -- hier waeren sie nur ein
# Fehlschlag ohne Aussage.
OHNE_PILLOW = {"SVG", "PDF"}

# Schluessel in `Image.info`, die zur STRUKTUR gehoeren und keine Angabe
# ueber Person, Ort oder Geraet machen. TIFF etwa fuehrt dort seine
# Kompressionsart -- sie zu melden waere ein Fehlalarm.
STRUKTUR_INFO = {"dpi", "compression", "version", "background", "duration",
                 "loop", "transparency", "icc_profile"}

for eintrag in manifest["dateien"]:
    name = eintrag["datei"]
    erwartet = eintrag["erwartet"]

    if erwartet.get("modus") in NICHT_BILD:
        continue
    if eintrag.get("format") in OHNE_PILLOW:
        continue

    original = os.path.join(ZIEL, name)
    bereinigt = original + ".stripped"

    if not os.path.exists(bereinigt):
        fehler.append(f"{name}: kein bereinigtes Ergebnis -- Rust-Test zuerst laufen lassen")
        continue
    print(f"  {name}")

    # --- Ist es noch ein gueltiges Bild? ------------------------------------
    try:
        with Image.open(bereinigt) as neu:
            neu.load()
            groesse, modus = neu.size, neu.mode
            neue_pixel = list(neu.convert("RGB").getdata())
            neue_palette = neu.getpalette()
    except Exception as e:                                   # noqa: BLE001
        fehler.append(f"{name}: laesst sich nicht mehr oeffnen: {e}")
        continue

    pruefe(list(groesse) == erwartet["groesse"],
           f"{name}: Groesse {groesse} statt {erwartet['groesse']}")
    pruefe(modus == erwartet["modus"],
           f"{name}: Modus {modus} statt {erwartet['modus']}")

    # --- Sind die Pixel unveraendert? ---------------------------------------
    with Image.open(original) as alt:
        alt.load()
        alte_pixel = list(alt.convert("RGB").getdata())
        alte_palette = alt.getpalette()

    pruefe(neue_pixel == alte_pixel,
           f"{name}: die Pixel haben sich veraendert -- genau der v1-Palette-Bug")

    # --- Farbtabelle -------------------------------------------------------
    if erwartet.get("hat_palette"):
        pruefe(neue_palette is not None, f"{name}: Farbtabelle fehlt")
        pruefe(neue_palette == alte_palette,
               f"{name}: die Farbtabelle wurde veraendert")

    # --- Sind die Metadaten wirklich weg? ----------------------------------
    if name.lower().endswith((".jpg", ".jpeg")):
        try:
            rest = piexif.load(bereinigt)
            leer = all(not rest.get(k) for k in ("0th", "Exif", "GPS", "1st")) \
                and not rest.get("thumbnail")
            pruefe(leer, f"{name}: es steht noch EXIF in der Datei: "
                         f"{[k for k, v in rest.items() if v]}")
        except Exception:                                    # noqa: BLE001
            pass  # kein EXIF lesbar = gewuenschtes Ergebnis
    else:
        with Image.open(bereinigt) as neu:
            textreste = {k: v for k, v in neu.info.items()
                         if isinstance(v, str) and k not in STRUKTUR_INFO}
            pruefe(not textreste, f"{name}: Textreste in der Datei: {textreste}")

    geprueft += 1

print()
if fehler:
    print(f"  {len(fehler)} Problem(e):")
    for f in fehler:
        print(f"    - {f}")
    sys.exit(1)

print(f"  {geprueft} Datei(en) unabhaengig geprueft:")
print("    gueltiges Bild, unveraenderte Pixel, Farbtabelle intakt,")
print("    keine Metadaten mehr.")
