"""Erzeugt echte Bilddateien mit echten Metadaten als Testvorlagen.

Die Modultests in cabrik-metadata bauen JPEGs und PNGs von Hand. Das prueft
die Struktur, aber nicht, ob das Ergebnis noch ein gueltiges Bild ist -- und
genau das war der v1-Palette-Bug: gueltige Datei, falsche Farben.

Hier entstehen deshalb Dateien mit Pillow und piexif, die anschliessend von
verify_metadata_stripped.py wieder geoeffnet und geprueft werden.

Aufruf:
    python testvectors/tools/gen_metadata_fixtures.py
"""
import json
import os

import piexif
from PIL import Image

ZIEL = os.path.join("testvectors", "metadata")
os.makedirs(ZIEL, exist_ok=True)

manifest = []


def gps_ifd():
    """Koordinaten des Bundeshauses in Bern -- Ortsangabe als Testfall."""
    return {
        piexif.GPSIFD.GPSLatitudeRef: b"N",
        piexif.GPSIFD.GPSLatitude: ((46, 1), (56, 1), (48, 1)),
        piexif.GPSIFD.GPSLongitudeRef: b"E",
        piexif.GPSIFD.GPSLongitude: ((7, 1), (26, 1), (39, 1)),
    }


# --- JPEG mit EXIF, GPS und eingebettetem Vorschaubild -----------------------

bild = Image.new("RGB", (64, 48))
for x in range(64):
    for y in range(48):
        bild.putpixel((x, y), (x * 4 % 256, y * 5 % 256, (x + y) % 256))

vorschau = bild.resize((16, 12))
vorschau_pfad = os.path.join(ZIEL, "_thumb.jpg")
vorschau.save(vorschau_pfad, "JPEG")
with open(vorschau_pfad, "rb") as f:
    vorschau_bytes = f.read()
os.remove(vorschau_pfad)

exif = {
    "0th": {
        piexif.ImageIFD.Make: b"Canon",
        piexif.ImageIFD.Model: b"EOS 5D Mark IV",
        piexif.ImageIFD.Software: b"Cabrik Testaufbau",
        piexif.ImageIFD.Artist: b"Max Mustermann",
        piexif.ImageIFD.DateTime: b"2026:03:14 15:09:26",
    },
    "Exif": {piexif.ExifIFD.DateTimeOriginal: b"2026:03:14 15:09:26"},
    "GPS": gps_ifd(),
    "1st": {},
    "thumbnail": vorschau_bytes,
}

jpeg_pfad = os.path.join(ZIEL, "foto_mit_exif.jpg")
bild.save(jpeg_pfad, "JPEG", exif=piexif.dump(exif), quality=90)
manifest.append({
    "datei": "foto_mit_exif.jpg",
    "format": "JPEG",
    "beschreibung": "Foto mit EXIF, GPS und eingebettetem Vorschaubild",
    "erwartet": {
        "hat_gps": True,
        "hat_vorschaubild": True,
        "groesse": list(bild.size),
        "modus": bild.mode,
    },
})

# --- JPEG ohne Metadaten ----------------------------------------------------

sauber_pfad = os.path.join(ZIEL, "foto_ohne_exif.jpg")
bild.save(sauber_pfad, "JPEG", quality=90)
manifest.append({
    "datei": "foto_ohne_exif.jpg",
    "format": "JPEG",
    "beschreibung": "Foto ohne Metadaten -- darf nicht veraendert werden",
    "erwartet": {"hat_gps": False, "hat_vorschaubild": False,
                 "groesse": list(bild.size), "modus": bild.mode},
})

# --- Palette-PNG: der v1-Bug ------------------------------------------------

palette_bild = Image.new("P", (32, 32))
# Eindeutige Palette, damit ein Verlust sofort auffaellt.
palette = []
for i in range(256):
    palette += [i, (255 - i), (i * 7) % 256]
palette_bild.putpalette(palette)
for x in range(32):
    for y in range(32):
        palette_bild.putpixel((x, y), (x * 8 + y) % 256)

png_pfad = os.path.join(ZIEL, "palette_mit_text.png")
palette_bild.save(png_pfad, "PNG", pnginfo=None)

# Text-Chunks nachtraegen -- Pillow schreibt sie ueber PngInfo.
from PIL import PngImagePlugin  # noqa: E402

info = PngImagePlugin.PngInfo()
info.add_text("Author", "Max Mustermann")
info.add_text("Software", "Cabrik Testaufbau")
info.add_text("Comment", "vertraulich")
palette_bild.save(png_pfad, "PNG", pnginfo=info)

manifest.append({
    "datei": "palette_mit_text.png",
    "format": "PNG",
    "beschreibung": "Palette-PNG mit Text-Chunks -- der v1-Palette-Bug",
    "erwartet": {
        "hat_gps": False,
        "hat_vorschaubild": False,
        "groesse": list(palette_bild.size),
        "modus": "P",
        "hat_palette": True,
    },
})

# --- Truecolor-PNG ohne Metadaten -------------------------------------------

rgb_png = os.path.join(ZIEL, "bild_ohne_text.png")
bild.save(rgb_png, "PNG")
manifest.append({
    "datei": "bild_ohne_text.png",
    "format": "PNG",
    "beschreibung": "PNG ohne Metadaten",
    "erwartet": {"hat_gps": False, "hat_vorschaubild": False,
                 "groesse": list(bild.size), "modus": "RGB"},
})

# ---------------------------------------------------------------------------
# WebP, GIF und BMP
#
# Drei Formate, drei verschiedene Arten, Metadaten unterzubringen: WebP in
# RIFF-Chunks, GIF in Erweiterungsbloecken, BMP praktisch gar nicht -- ausser
# hinter den Bilddaten, wo kein Betrachter je hinsieht.
# ---------------------------------------------------------------------------

_webp_exif = Image.Exif()
_webp_exif[0x010F] = "Kamerahersteller"
_webp_exif[0x0110] = "Modell XY-2000"
_webp_exif[0x9003] = "2026:03:01 09:12:00"

bild.save(
    os.path.join(ZIEL, "bild_mit_metadaten.webp"),
    "WEBP",
    exif=_webp_exif.tobytes(),
    xmp=b"<?xpacket?><x:xmpmeta><dc:creator>Dr. Anna Beispiel</dc:creator></x:xmpmeta>",
    icc_profile=b"FAKE-ICC-PROFIL-DATEN",
)
manifest.append({
    "datei": "bild_mit_metadaten.webp",
    "format": "WebP",
    "beschreibung": "WebP mit EXIF, XMP und Farbprofil",
    "erwartet": {"hat_gps": False, "hat_vorschaubild": False,
                 "groesse": list(bild.size), "modus": "RGB"},
})

bild.save(
    os.path.join(ZIEL, "bild_mit_metadaten.gif"),
    "GIF",
    comment=b"Erstellt mit Scanner XY-2000, Anna Beispiel",
)
manifest.append({
    "datei": "bild_mit_metadaten.gif",
    "format": "GIF",
    "beschreibung": "GIF mit Kommentar-Erweiterung",
    "erwartet": {"hat_gps": False, "hat_vorschaubild": False,
                 "groesse": list(bild.size), "modus": "RGB"},
})

# BMP traegt selbst nichts -- das Anhaengsel hinter den Bilddaten dagegen
# schon. Es sieht kein Betrachter, mitverschickt wird es trotzdem.
_bmp_pfad = os.path.join(ZIEL, "bild_schlicht.bmp")
bild.save(_bmp_pfad, "BMP")
with open(_bmp_pfad, "rb") as f:
    _roh = f.read()
with open(_bmp_pfad, "wb") as f:
    f.write(_roh + b"HEIMLICHE-NUTZLAST-AM-ENDE")
manifest.append({
    "datei": "bild_schlicht.bmp",
    "format": "BMP",
    "beschreibung": "BMP mit Anhaengsel hinter den Bilddaten",
    "erwartet": {"hat_gps": False, "hat_vorschaubild": False,
                 "groesse": list(bild.size), "modus": "RGB"},
})

with open(os.path.join(ZIEL, "manifest.json"), "w", encoding="utf-8", newline="\n") as f:
    json.dump({
        "beschreibung": "Echte Bilddateien mit echten Metadaten, erzeugt mit Pillow und piexif.",
        "hinweis": (
            "Die Modultests bauen Dateien von Hand und pruefen damit die Struktur. "
            "Diese Vorlagen pruefen zusaetzlich, dass das bereinigte Ergebnis noch "
            "ein gueltiges Bild mit unveraenderten Pixeln ist -- der v1-Palette-Bug "
            "erzeugte eine gueltige Datei mit falschen Farben."
        ),
        "dateien": manifest,
    }, f, indent=2, ensure_ascii=False)
    f.write("\n")

for m in manifest:
    p = os.path.join(ZIEL, m["datei"])
    print(f"  {m['datei']:<24} {os.path.getsize(p):>6} Bytes   {m['beschreibung']}")
print(f"\n  {len(manifest)} Vorlagen nach {ZIEL}/")
