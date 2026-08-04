# app_gui_only.spec
# GUI-only Build für Cabrik Secure
from PyInstaller.utils.hooks import collect_submodules, collect_data_files
from PyInstaller.building.build_main import Analysis, PYZ, EXE, COLLECT

# Einstiegspunkt (deine GUI)
entry_script = r"C:\Dev\CabrikSecure\cabrik_secure\gui\app.py"

# Libraries, die wir sicherheitshalber einsammeln
hidden = []
hidden += collect_submodules('nacl')
hidden += collect_submodules('PIL')
hidden += collect_submodules('pikepdf')
hidden += collect_submodules('docx')          # python-docx
hidden += collect_submodules('piexif')

# Daten-Dateien (z.B. qpdf-DLLs von pikepdf, Pillow Plugins etc.)
datas = []
datas += collect_data_files('pikepdf', include_py_files=False)
datas += collect_data_files('PIL', include_py_files=False)

a = Analysis(
    [entry_script],
    pathex=[r"C:\Dev\CabrikSecure"],
    binaries=[],
    datas=datas,
    hiddenimports=hidden,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
)

pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    name='CabrikSecure',
    icon=None,              # Falls du ein Icon hast: r"C:\Dev\CabrikSecure\assets\cabrik.ico"
    console=False,          # GUI-Mode (kein Terminal)
)

coll = COLLECT(
    exe,
    a.binaries,
    a.zipfiles,
    a.datas,
    strip=False,
    upx=False,
    name='CabrikSecure'
)
