# cabrik_secure.spec
# PyInstaller build spec for Cabrik Secure

# ------------------------
# Grund-Imports
# ------------------------
block_cipher = None

from PyInstaller.utils.hooks import collect_submodules

# Falls cabrik_secure dynamische Imports hat:
hiddenimports = collect_submodules('cabrik_secure')

a = Analysis(
    ['cabrik_secure/gui/app.py'],   # Haupt-Entry
    pathex=['.'],
    binaries=[],
    datas=[
        ('assets/*', 'assets'),    # Icons und evtl. Assets
    ],
    hiddenimports=hiddenimports,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

# ------------------------
# Windows-EXE
# ------------------------
exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name='CabrikSecure',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    console=False,   # Kein Terminalfenster
    icon='assets/cabrik_secure.ico'  # Windows Icon
)

# ------------------------
# Mac-Bundle
# ------------------------
app = BUNDLE(
    exe,
    name='CabrikSecure.app',
    icon='assets/cabrik_secure.icns',
    bundle_identifier='com.cabrik.secure'
)

coll = COLLECT(
    exe,
    a.binaries,
    a.zipfiles,
    a.datas,
    strip=False,
    upx=True,
    upx_exclude=[],
    name='CabrikSecure'
)
