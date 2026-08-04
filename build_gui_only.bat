@echo off
REM venv aktivieren (falls nicht längst aktiv)
call .venv\Scripts\activate

REM 1) GUI-only bauen
pyinstaller CabrikSecure_GUIonly.spec --noconfirm

REM 2) Installer bauen (pfad ggf. anpassen)
"C:\Program Files (x86)\Inno Setup 6\ISCC.exe" installer_gui_only.iss

echo.
echo Fertig! Installer liegt in: Output\CabrikSecure_GUI_Installer.exe
pause
