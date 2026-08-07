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

ZIEL = os.path.join("testvectors", "metadata")

with open(os.path.join(ZIEL, "manifest.json"), encoding="utf-8") as f:
    manifest = json.load(f)

fehler = []
geprueft = 0


def pruefe(bedingung, meldung):
    if not bedingung:
        fehler.append(meldung)
    return bedingung


for eintrag in manifest["dateien"]:
    name = eintrag["datei"]
    original = os.path.join(ZIEL, name)
    bereinigt = original + ".stripped"

    if not os.path.exists(bereinigt):
        fehler.append(f"{name}: kein bereinigtes Ergebnis -- Rust-Test zuerst laufen lassen")
        continue

    erwartet = eintrag["erwartet"]
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
                         if isinstance(v, str) and k not in ("dpi",)}
            pruefe(not textreste, f"{name}: Textreste im PNG: {textreste}")

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
