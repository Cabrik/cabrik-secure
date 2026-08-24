# Baut die Installer — signiert, wenn ein Zertifikat da ist, und sagt es,
# wenn keines da ist.
#
# # Warum es dieses Skript gibt
#
# Weil das Signieren sonst ein Umbau wird statt eines Schalters. Wenn das
# Zertifikat kommt — und bis dahin dauert es (siehe `docs/signatur.md`) —,
# soll niemand die Bauweise anfassen müssen. Es sollen zwei
# Umgebungsvariablen gesetzt werden, sonst nichts.
#
# # Was es NICHT tut
#
# Es hält den Bau nicht an, weil kein Zertifikat da ist. Ein Skript, das
# beim täglichen Bauen scheitert, wird umgangen. Es **schweigt** aber auch
# nicht: Ein unsigniertes Ergebnis wird als unsigniert ausgewiesen, in der
# Zusammenfassung und in den Prüfsummen.
#
# Wer für eine echte Auslieferung baut, hängt `-Signaturpflicht` an. Dann
# ist die fehlende Signatur ein Fehler — und zwar bevor gebaut wird, nicht
# nach zwanzig Minuten.
#
# # Warum die Signatur nachgeprüft wird
#
# Weil „der Signierbefehl lief durch" und „die Datei ist signiert" zwei
# verschiedene Aussagen sind. Ein Werkzeug, das den falschen Pfad bekommt
# oder still fehlschlägt, liefert einen Rückgabewert von null und eine
# unsignierte Datei. Geprüft wird deshalb am Ergebnis, mit
# `Get-AuthenticodeSignature`, und der Name im Zertifikat wird ausgegeben:
# Er ist genau das, was der Nutzer später in der Warnung liest.
#
# # Aufruf
#
#     .\auslieferung.ps1                      # baut, signiert falls möglich
#     .\auslieferung.ps1 -Buendel msi         # nur MSI (siehe unten)
#     .\auslieferung.ps1 -Signaturpflicht     # bricht ab, wenn nicht signiert wird
#
# `-Buendel msi` gibt es aus einem konkreten Grund: Auf der
# Entwicklungsmaschine hindert der Virenwächter (F-Secure) makensis daran,
# `nsis-output.exe` anzulegen. Das ist keine Eigenschaft des Projekts,
# sondern eine dieser Maschine — und ein Vorgeschmack darauf, was ein
# unsignierter Installer beim Nutzer auslöst. Siehe `docs/ROADMAP.md`, 5.3.
#
# # Signieren einschalten
#
# Zwei Umgebungsvariablen, mehr nicht:
#
#     $env:CABRIK_SIGNIERWERKZEUG  = "C:\Pfad\zu\signtool.exe"
#     $env:CABRIK_SIGNIERARGUMENTE = "sign|/fd|sha256|/tr|http://zeitstempel|/td|sha256|%1"
#
# Getrennt wird an `|`, nicht am Leerzeichen: Pfade enthalten Leerzeichen,
# und ein an Leerzeichen zerlegter Pfad ist der klassische stille Fehler.
# `%1` MUSS vorkommen — Tauri ersetzt es durch die zu signierende Datei.
#
# Die Zugangsdaten des Signierwerkzeugs stehen NICHT hier und nicht im
# Repository. Sie gehören in die Umgebung des Werkzeugs selbst; dieses
# Skript sieht sie nie.

param(
    [switch]$Signaturpflicht,
    [string]$Buendel = "msi,nsis"
)

# NICHT "Stop": Cargo schreibt seinen Fortschritt auf die Fehlerausgabe,
# und Windows PowerShell 5.1 macht daraus einen NativeCommandError. Was
# zählt, ist der Rückgabewert.
$ErrorActionPreference = "Continue"
$wurzel = $PSScriptRoot
$buendelordner = Join-Path $wurzel "target\release\bundle"

# Ausdruecklich in die Wurzel stellen, statt sich auf den Aufrufer zu
# verlassen.
#
# `cargo tauri build` haengt am Arbeitsverzeichnis: Es sucht dort das
# Manifest, und Tauri loest die Pfade seines Vorlaufs relativ dazu auf.
# Beim ersten Installerbau ist genau daran ein Lauf zerbrochen -- aus
# `crates/cabrik-fenster` aufgerufen suchte npm nach `C:\Dev\app`. Ein
# Skript, das nur aus einem bestimmten Ordner funktioniert, ist eine
# Falle fuer den Naechsten.
Set-Location -LiteralPath $wurzel

function Sagen($text) { Write-Host $text }
function Ueberschrift($text) { Write-Host ""; Write-Host "=== $text" }

# ---------------------------------------------------------------- Signatur
Ueberschrift "Signatur"

$werkzeug = $env:CABRIK_SIGNIERWERKZEUG
$argumente = $env:CABRIK_SIGNIERARGUMENTE
$signiert_gewollt = -not [string]::IsNullOrWhiteSpace($werkzeug)

if ($signiert_gewollt) {
    if ([string]::IsNullOrWhiteSpace($argumente)) {
        Sagen "CABRIK_SIGNIERWERKZEUG ist gesetzt, CABRIK_SIGNIERARGUMENTE nicht."
        Sagen "Ohne Argumente weiss das Werkzeug nicht, welche Datei es signieren soll."
        exit 1
    }
    if ($argumente -notmatch [regex]::Escape("%1")) {
        Sagen "In CABRIK_SIGNIERARGUMENTE fehlt der Platzhalter %1."
        Sagen "Tauri ersetzt ihn durch den Pfad der zu signierenden Datei;"
        Sagen "ohne ihn signiert das Werkzeug irgendetwas oder nichts."
        exit 1
    }
    Sagen "Werkzeug:  $werkzeug"
    Sagen "Argumente: $argumente"
}
else {
    Sagen "Kein Zertifikat eingerichtet -- es wird UNSIGNIERT gebaut."
    Sagen "(CABRIK_SIGNIERWERKZEUG ist nicht gesetzt. Siehe Kopf dieses Skripts.)"
    if ($Signaturpflicht) {
        Sagen ""
        Sagen "Abbruch: -Signaturpflicht verlangt eine Signatur."
        exit 1
    }
}

# ------------------------------------------------------------------ Bauen
Ueberschrift "Bauen ($Buendel)"

# Vorher raeumen, damit die Pruefsummen am Ende NUR das beschreiben, was
# dieser Lauf erzeugt hat. Ein liegengebliebenes Buendel von gestern in
# einer Pruefsummenliste von heute waere eine Falschaussage.
if (Test-Path -LiteralPath $buendelordner) {
    Remove-Item -LiteralPath $buendelordner -Recurse -Force -ErrorAction SilentlyContinue
}

$befehl = @("tauri", "build", "--bundles", $Buendel)

$zusatzdatei = $null
if ($signiert_gewollt) {
    # Die Signaturangaben stehen NICHT in `tauri.conf.json`. Sie kommen als
    # Zusatzkonfiguration dazu, die Tauri ueber die Grundangaben legt.
    #
    # Warum: Damit die eingecheckte Konfiguration nichts ueber Zertifikate,
    # Werkzeuge oder Pfade eines bestimmten Rechners behauptet. Wer das
    # Repository klont, baut unsigniert und ohne eine Zeile zu aendern.
    $liste = $argumente -split [regex]::Escape("|")
    $zusatz = @{
        bundle = @{
            windows = @{
                signCommand = @{
                    cmd  = $werkzeug
                    args = @($liste)
                }
            }
        }
    }
    $zusatzdatei = Join-Path ([IO.Path]::GetTempPath()) ("cabrik-signatur-" + [guid]::NewGuid().ToString("N") + ".json")
    $zusatz | ConvertTo-Json -Depth 8 | Out-File -FilePath $zusatzdatei -Encoding utf8
    $befehl += @("--config", $zusatzdatei)
}

& cargo @befehl
$baufehler = $LASTEXITCODE

if ($zusatzdatei -and (Test-Path -LiteralPath $zusatzdatei)) {
    Remove-Item -LiteralPath $zusatzdatei -Force -ErrorAction SilentlyContinue
}

if ($baufehler -ne 0) {
    Sagen ""
    Sagen "Bau gescheitert (Rueckgabewert $baufehler)."
    exit 1
}

# ------------------------------------------------------------- Nachpruefen
Ueberschrift "Ergebnis"

if (-not (Test-Path -LiteralPath $buendelordner)) {
    Sagen "Kein Buendelordner entstanden -- es gibt nichts auszuliefern."
    exit 1
}

$dateien = Get-ChildItem -LiteralPath $buendelordner -Recurse -File |
    Where-Object { $_.Extension -in @(".msi", ".exe") } |
    Sort-Object FullName

if ($dateien.Count -eq 0) {
    Sagen "Keine Installer im Buendelordner."
    exit 1
}

$zeilen = @()
$ungezeichnet = @()

foreach ($d in $dateien) {
    $groesse = "{0:N1} MB" -f ($d.Length / 1MB)
    $summe = (Get-FileHash -LiteralPath $d.FullName -Algorithm SHA256).Hash.ToLower()

    # Am Ergebnis geprueft, nicht am Rueckgabewert des Signierbefehls.
    $sig = Get-AuthenticodeSignature -LiteralPath $d.FullName
    $zustand = $sig.Status.ToString()
    if ($zustand -eq "Valid") {
        $wer = $sig.SignerCertificate.Subject
        Sagen ""
        Sagen "  $($d.Name)  ($groesse)"
        Sagen "     signiert: $wer"
    }
    else {
        $ungezeichnet += $d.Name
        Sagen ""
        Sagen "  $($d.Name)  ($groesse)"
        Sagen "     NICHT SIGNIERT ($zustand)"
    }
    Sagen "     sha256:   $summe"
    $zeilen += "$summe  $($d.Name)"
}

# Die Pruefsummen als Datei -- der Fahrplan verlangt sie unter
# „Nachvollziehbare Builds": veroeffentlichte Summen, damit jeder abgleichen
# kann, dass sein Download dem entspricht, was hier gebaut wurde.
$summendatei = Join-Path $buendelordner "pruefsummen.txt"
$zeilen | Out-File -FilePath $summendatei -Encoding utf8
Sagen ""
Sagen "Pruefsummen: $summendatei"

# --------------------------------------------------------- Zusammenfassung
Ueberschrift "Zusammenfassung"

if ($signiert_gewollt -and $ungezeichnet.Count -gt 0) {
    # Das ist ein echter Fehler und keine Nachlaessigkeit: Es wurde signiert
    # verlangt, der Befehl meldete Erfolg, und die Datei traegt trotzdem
    # keine gueltige Signatur.
    Sagen "Signieren war eingerichtet, aber diese Dateien tragen keine gueltige Signatur:"
    foreach ($n in $ungezeichnet) { Sagen "   $n" }
    exit 1
}

if ($ungezeichnet.Count -gt 0) {
    Sagen 'Gebaut, aber UNSIGNIERT.'
    Sagen ''
    Sagen 'Was das beim Nutzer bedeutet: SmartScreen meldet "Windows hat Ihren PC'
    Sagen 'geschuetzt" und nennt keinen Herausgeber; unter Windows 11 blockt Smart'
    Sagen 'App Control die Ausfuehrung ganz. Zum Weiterreichen taugt das nicht.'
    exit 0
}

Sagen 'Gebaut und signiert.'
Sagen ''
Sagen 'Auch damit ist die erste SmartScreen-Warnung nicht weg -- sie nennt jetzt'
Sagen 'nur den geprueften Herausgeber statt "Unbekannter Herausgeber". Die'
Sagen 'Reputation entsteht erst ueber Downloads (siehe docs/signatur.md).'
exit 0
