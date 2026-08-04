@echo off
REM Build der eingefrorenen v1-Referenzimplementierung.
REM Aus diesem Verzeichnis ausfuehren (legacy\python-v1).
REM Die .venv liegt seit der Umstrukturierung im Repository-Wurzelverzeichnis.
call ..\..\.venv\Scripts\activate

REM 1) GUI-only bauen
pyinstaller CabrikSecure_GUIonly.spec --noconfirm

REM 2) Installer bauen (Pfad ggf. anpassen)
"C:\Program Files (x86)\Inno Setup 6\ISCC.exe" installer_gui_only.iss

echo.
echo Fertig! Installer liegt in: Output\CabrikSecure_GUI_Installer.exe
pause
