# Dasselbe Tor wie in der CI — nur hier auf dem Rechner.
#
# # Warum es das gibt
#
# Weil `.github/workflows/pruefung.yml` erst läuft, wenn dieses Repository
# einen Remote hat. Bis dahin wäre die CI ein Versprechen ohne Deckung.
#
# Und danach bleibt das Skript nützlich: Wer den Fehlschlag erst nach dem
# Push sieht, hat ihn schon in der Geschichte stehen.
#
# # Was es nur zum Teil prüft
#
# macOS und Linux. Ausgeführt wird dort nur in der CI — auf einem Windows-
# Rechner gibt es kein `/proc` und kein `mlock`.
#
# **Gelesen** wird der fremde Quelltext aber sehr wohl, seit es Code gibt,
# der hier gar nicht übersetzt wird: `crates/cabrik-speicher` hat für jedes
# System einen eigenen Zweig, und was hinter `#[cfg(unix)]` steht, sieht ein
# Windows-Clippy nie. Zweimal ist genau daran ein Lauf zerbrochen, und beide
# Male hätte ein Blick genügt.
#
# Der Schritt „Fremde Systeme" holt das nach: `cargo clippy --target` prüft
# gegen die Standardbibliothek des anderen Systems, ohne zu binden. Er wird
# übersprungen, wenn das Ziel nicht installiert ist —
#
#     rustup target add x86_64-unknown-linux-gnu
#     rustup target add aarch64-apple-darwin
#
# # Aufruf
#
#     .\pruefung.ps1
#     .\pruefung.ps1 -Schnell    # ohne cargo deny (spart den Netzabruf)

param(
    [switch]$Schnell
)

# NICHT "Stop": Cargo und npm schreiben ihren Fortschritt auf die
# Fehlerausgabe, und Windows PowerShell 5.1 macht daraus einen
# NativeCommandError -- der Lauf bräche schon beim ersten „Checking …" ab,
# obwohl nichts schiefgegangen ist. Was zählt, ist der Rückgabewert, und
# den prüft `Schritt` selbst.
$ErrorActionPreference = "Continue"
$wurzel = $PSScriptRoot
$fehler = @()

function Schritt {
    param([string]$Was, [scriptblock]$Tun)

    Write-Host ""
    Write-Host "=== $Was" -ForegroundColor Cyan
    & $Tun
    if ($LASTEXITCODE -ne 0) {
        $script:fehler += $Was
        Write-Host "    gescheitert" -ForegroundColor Red
    }
}

# Dieselbe Reihenfolge wie in der CI: Formatierung zuerst, sie ist in
# Sekunden erledigt und erspart die Minuten dahinter.
#
# Dieser Schritt fehlte hier, weil er auch in meiner Fassung der CI fehlte
# -- und dadurch waren 63 Stellen im Baum unformatiert, ohne dass es
# jemandem auffiel. Der wiederhergestellte Ablauf hat es beim ersten Lauf
# auf allen drei Plattformen gemeldet.
Schritt "Formatierung (Rust)" {
    Set-Location $wurzel
    cargo fmt --all --check
}

Schritt "Clippy (Rust)" {
    Set-Location $wurzel
    cargo clippy --workspace --all-targets --locked -- -D warnings
}

# Was auf diesem Rechner nie übersetzt wird.
#
# Kein Ausführen -- dafür braucht es die Systeme selbst. Aber Clippy liest
# den Zweig hinter `#[cfg(unix)]`, und die beiden Fehler, die diesen Schritt
# veranlasst haben, waren beide von der lesbaren Sorte: eine unerfüllte
# `expect`-Erwartung und ein Test, der prozessweite Zahlen misst.
#
# Nur `cabrik-speicher`: Sie ist die einzige Kiste mit systemabhängigen
# Zweigen. Die ganze Werkbank für ein fremdes Ziel zu übersetzen zöge Tauri
# samt WebKit und GTK herein, und das gäbe es hier ohnehin nicht.
Schritt "Fremde Systeme (nur lesen, nicht ausfuehren)" {
    Set-Location $wurzel
    $vorhanden = (rustup target list --installed) -split "`n" | ForEach-Object { $_.Trim() }
    $geprueft = 0
    foreach ($ziel in @("x86_64-unknown-linux-gnu", "aarch64-apple-darwin")) {
        if ($vorhanden -contains $ziel) {
            Write-Host "    $ziel"
            cargo clippy -p cabrik-speicher --all-targets --target $ziel -- -D warnings
            if ($LASTEXITCODE -ne 0) { return }
            $geprueft++
        } else {
            Write-Host "    $ziel nicht installiert -- uebersprungen" -ForegroundColor Yellow
        }
    }
    if ($geprueft -eq 0) {
        Write-Host "    kein fremdes Ziel installiert; siehe Kopf dieser Datei" -ForegroundColor Yellow
    }
    $global:LASTEXITCODE = 0
}

# Darunter läuft `vertragsmuster.rs`. Es VERGLEICHT die Prüfmuster mit den
# eingecheckten -- `MUSTER_SCHREIBEN` darf hier nicht gesetzt sein, sonst
# schreibt es sie und stimmt immer zu.
Schritt "Tests (Rust)" {
    Set-Location $wurzel
    if ($env:MUSTER_SCHREIBEN) {
        Write-Host "    MUSTER_SCHREIBEN ist gesetzt -- die Vertragsmuster" -ForegroundColor Yellow
        Write-Host "    wuerden geschrieben statt geprueft. Wird entfernt." -ForegroundColor Yellow
        Remove-Item Env:MUSTER_SCHREIBEN
    }
    cargo test --workspace --locked
}

# `npm run pruefung` und nichts Selbstgebautes: svelte-check MIT
# `--tsconfig ./tsconfig.app.json` und danach vitest. Ohne die tsconfig
# bleiben die Testdateien ungeprueft -- genau so sind hier schon Typfehler
# durchgerutscht.
Schritt "Typen und Tests (Oberfläche)" {
    Set-Location (Join-Path $wurzel "app\oberflaeche")
    npm run pruefung
}

if (-not $Schnell) {
    Schritt "Abhängigkeiten (Lizenzen, Lücken, Quellen)" {
        Set-Location $wurzel
        cargo deny check
    }
}

Set-Location $wurzel

Write-Host ""
if ($fehler.Count -eq 0) {
    Write-Host "Alles grün." -ForegroundColor Green
    exit 0
}

Write-Host "Gescheitert:" -ForegroundColor Red
foreach ($f in $fehler) { Write-Host "  - $f" -ForegroundColor Red }
exit 1
