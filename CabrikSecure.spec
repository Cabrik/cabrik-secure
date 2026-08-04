# -*- mode: python ; coding: utf-8 -*-
import os
from PyInstaller.utils.hooks import collect_data_files

datas = collect_data_files('cabrik_secure')

a = Analysis(
    ['cabrik_secure/gui/app.py'],
    pathex=['.'],
    binaries=[],
    datas=datas + [('assets/*', 'assets')],
    hiddenimports=[],
    hookspath=[],
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=None)

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
    console=False,
    icon='assets/icon.ico'  # auf macOS automatisch .icns via BUNDLE unten
)

app = BUNDLE(
    exe,
    name='CabrikSecure.app',
    icon='assets/icon.icns',
    bundle_identifier='com.cabrik.secure'
) if os.name != 'nt' else None

coll = COLLECT(
    exe,
    a.binaries,
    a.zipfiles,
    a.datas,
    strip=False,
    upx=True,
    name='CabrikSecure'
)
