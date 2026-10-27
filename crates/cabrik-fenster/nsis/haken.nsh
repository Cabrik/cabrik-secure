; Haken für das Windows-Setup (NSIS).
;
; Eingebunden über `bundle > windows > nsis > installerHooks` in
; `tauri.conf.json`. Tauri setzt die Makros unten an festen Stellen seines
; Installers ein (tauri-bundler, `installer.nsi`).
;
; # Wozu
;
; Tauris Deinstaller bietet ein Häkchen „Lösche die Anwendungsdaten“ an.
; Es löscht `%APPDATA%\de.cabrik.secure` und `%LOCALAPPDATA%\de.cabrik.secure`
; -- die Daten der Webansicht. Schlüssel und Kontakte liegen woanders, in
; `%APPDATA%\CabrikSecure` (`cabrik_ablage::verzeichnis`), und blieben
; deshalb liegen, auch mit gesetztem Häkchen.
;
; Entschieden am 27.10.2026: Wer deinstalliert, darf sie mitlöschen -- auf
; Wunsch und mit deutlicher Warnung. Voreingestellt ist Behalten, dreifach:
;
; 1. Das Häkchen ist aus.
; 2. Ist es an, fragt ein eigenes Fenster nach -- mit der Folge, nicht mit
;    der Regel. Vorgewählt ist „Nein“.
; 3. Bei einem Update (`/UPDATE`) wird nie gelöscht, und ohne Fenster
;    (`/S`) gilt „Nein“.
;
; Ein Update, das den Schlüssel verwürfe, machte alles unlesbar, was an ihn
; gerichtet ist. Deshalb lieber einmal zu oft gefragt.
;
; # Warum die Sprache zur Laufzeit gewählt wird
;
; Diese Datei steht im Installer VOR den Sprachen: `LangString` kennt hier
; die Sprachkennung LANG_GERMAN noch nicht. `$LANGUAGE` dagegen ist beim
; Deinstallieren gesetzt -- 1031 ist Deutsch, alles andere bekommt
; Englisch.
;
; `tests/deinstaller.rs` prüft, dass der Ordner hier derselbe ist, den
; `cabrik-ablage` benutzt, und dass die drei Sperren oben dastehen.

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    SetShellVarContext current
    ${If} ${FileExists} "$APPDATA\CabrikSecure\*.*"
      Push $R9
      ${If} $LANGUAGE = 1031
        StrCpy $R9 "Auch Ihre Schlüssel und Kontakte löschen?$\r$\n$\r$\nSie liegen in $APPDATA\CabrikSecure. Danach lässt sich nichts mehr öffnen, was an Sie verschlüsselt wurde – auch nicht mit dem richtigen Passwort.$\r$\n$\r$\nEine Sicherung der Schlüsseldatei an einem anderen Ort bleibt unberührt.$\r$\n$\r$\nJa: unwiderruflich löschen.$\r$\nNein: behalten."
      ${Else}
        StrCpy $R9 "Also delete your keys and contacts?$\r$\n$\r$\nThey are stored in $APPDATA\CabrikSecure. Once deleted, nothing that was encrypted to you can be opened again – not even with the right password.$\r$\n$\r$\nA backup of the key file stored elsewhere is not affected.$\r$\n$\r$\nYes: delete permanently.$\r$\nNo: keep them."
      ${EndIf}
      ${If} ${Cmd} `MessageBox MB_YESNO|MB_ICONEXCLAMATION|MB_DEFBUTTON2 "$R9" /SD IDNO IDYES`
        RMDir /r "$APPDATA\CabrikSecure"
      ${EndIf}
      Pop $R9
    ${EndIf}
  ${EndIf}
!macroend
