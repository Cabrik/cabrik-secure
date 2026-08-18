# Cabrik Secure v1.0 — Referenzimplementierung (eingefroren)

> ## Das ist **nicht** Cabrik Secure
>
> Dieser Ordner enthält die **abgelöste Version 1** von 2025 — Python und
> Tkinter, ein anderes Programm. Das heutige Cabrik Secure steht in
> [`crates/`](../../crates/) und ist in Rust geschrieben.
>
> **Benutzen Sie den Code hier nicht.** Er hat bekannte Schwächen, die
> weiter unten einzeln aufgeführt sind — unter anderem ein Passwort, das
> dauerhaft im Klartext im Arbeitsspeicher liegt, und ein sicheres
> Löschen, das Fehler verschluckt und trotzdem Erfolg meldet. Sie sind
> **dokumentiert und nicht behoben**; v1 wird nicht mehr gepflegt.
>
> ### Warum er trotzdem hier liegt
>
> Weil er der **unabhängige Gegenprüfer** ist. Jeder Rust-Test dieses
> Projekts füttert einen Leser mit Dateien, die derselbe Code erzeugt hat
> — das prüft, ob der Leser zum eigenen Schreiber passt, nicht ob beide
> recht haben. Für das v1-Format ist dieser Python-Code die einzige
> fremde Umsetzung, gegen die sich das prüfen lässt:
> [`testvectors/tools/gen_v1_compat.py`](../../testvectors/tools/gen_v1_compat.py)
> erzeugt die Vergleichsvektoren damit, und der Arbeitsablauf
> [`Gegenprobe`](../../.github/workflows/gegenprobe.yml) ruft es bei jedem
> Lauf auf.
>
> Ihn zu löschen ersetzte eine Gegenprüfung durch einen Zirkelschluss.
>
> ### Wenn Sie noch v1 benutzen
>
> Ihre alten Envelopes und Schlüsseldateien bleiben lesbar — v2 kann
> beides. Der Weg dorthin steht in [`docs/ROADMAP.md`](../../docs/ROADMAP.md).

Der ausgelieferte Stand von v1.0 (Python 3.11 + Tkinter, PyInstaller).
**Bekommt keine neuen Features.** Er existiert aus zwei Gründen:

1. **Orakel für Differenztests.** Der Rust-Core in Phase 2 wird gegen die
   Ausgabe dieser Implementierung geprüft. Steckt man in Rust fest, lassen
   sich Zwischenergebnisse mit einer bekannten Sprache vergleichen.
2. **Quelle der v1-Kompatibilitätsvektoren.** v2 muss v1-Envelopes und
   v1-Keyfiles lesen können.

## Verwendung

```bash
# aus dem Repository-Wurzelverzeichnis
.venv\Scripts\activate
pip install -e legacy/python-v1

python -m cabrik_secure.gui.app          # GUI
cabrik-keygen --out mein.json            # CLI (nach Installation)
```

## Round-Trip-Nachweis

```bash
python legacy/python-v1/smoke_test.py
```

Prüft Keyfile-Round-Trip, Ablehnung falscher Passwörter, signierte und anonyme
Textnachrichten, Erkennung von Header-Manipulation, Datei-Round-Trip,
Anonymitäts-Keyfiles und Secure Delete. Muss 9/9 bestehen.

## Änderungen gegenüber dem Auslieferungsstand

Rein strukturell, die Krypto ist unverändert:

- `pyproject.toml`: fehlende `dependencies` und `[project.scripts]` ergänzt —
  ohne sie erzeugte `pip install .` ein nicht lauffähiges Paket
- `CabrikSecure_GUIonly.spec`: absolute Pfade durch `SPECPATH` ersetzt
- `build_gui_only.bat`: Pfad zur `.venv` an den neuen Ort angepasst
- Drei redundante PyInstaller-Specs und ein zweites Inno-Setup-Skript entfernt
  (über Commit `c3fc68d` wiederherstellbar)
- `smoke_test.py` neu hinzugefügt

## Bekannte Schwächen

Dokumentiert, nicht behoben — sie sind die Aufgabenliste für v2:

| Fundort | Problem |
|---|---|
| `crypto_core.py:225` | Signaturprüfschlüssel stammt aus dem Header derselben Nachricht — kein Nachweis *wer* signiert hat |
| `crypto_core.py:132` | Eigene Schlüsselableitung ohne Transcript-Binding statt HPKE |
| `crypto_core.py:35` | Fingerprint nur 8 Hex-Zeichen (32 Bit) |
| `crypto_core.py:253` | Ganze Datei im RAM, kein Streaming |
| Envelope-Format | 78,1 % Größen-Overhead durch Base64 über JSON über Base64 |
| `crypto_core.py:390` | `secure_delete` verschluckt alle Fehler und meldet trotzdem Erfolg |
| `crypto_core.py:345` | Palette-PNGs (Mode `P`) verlieren beim Metadaten-Strip die Farbpalette |
| `gui/app.py:114` | Falsches Keyfile führt zu Traceback statt verständlicher Meldung |
| `gui/app.py:187` | Passwort bleibt im Klartext in `STATE` |
| Entschlüsselung | `header["version"]` und `alg` werden nie validiert |
