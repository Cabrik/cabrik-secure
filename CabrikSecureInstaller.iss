[Setup]
AppName=Cabrik Secure
AppVersion=1.0
DefaultDirName={pf}\CabrikSecure
DefaultGroupName=Cabrik Secure
OutputBaseFilename=CabrikSecure_GUI_Installer
Compression=lzma
SolidCompression=yes
SetupIconFile=assets\icon.ico

[Files]
Source: "dist\CabrikSecure\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{group}\Cabrik Secure"; Filename: "{app}\CabrikSecure.exe"
Name: "{userdesktop}\Cabrik Secure"; Filename: "{app}\CabrikSecure.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Desktop-Verknüpfung erstellen"; GroupDescription: "Zusätzliche Aufgaben:"

; --- Dateizuordnung für .enc ---
[Registry]
Root: HKCR; Subkey: ".enc"; ValueType: string; ValueName: ""; ValueData: "CabrikSecure.enc"; Flags: uninsdeletevalue
Root: HKCR; Subkey: "CabrikSecure.enc"; ValueType: string; ValueName: ""; ValueData: "Cabrik Secure Envelope"
Root: HKCR; Subkey: "CabrikSecure.enc\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\CabrikSecure.exe,0"
Root: HKCR; Subkey: "CabrikSecure.enc\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\CabrikSecure.exe"" ""%1"""

[Run]
Filename: "{app}\CabrikSecure.exe"; Description: "Cabrik Secure starten"; Flags: nowait postinstall skipifsilent
