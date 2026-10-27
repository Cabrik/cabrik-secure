"""Gleicht die Sperrdateien gegen die Sicherheitsmeldungen von Tauri ab.

WARUM ES DAS GIBT. Am 27.10.2026 fiel auf, dass eine hoch eingestufte
Meldung (GHSA-w28w-mhc8-qvjv) einen Monat lang offen stand, ohne dass eine
Pruefung anschlug. Sie stand nur bei Tauri selbst: nicht in RustSec, das
`cargo deny` liest, und nicht in GitHubs globaler Datenbank, aus der
Dependabot warnt. Behoben hat sie ein Sammel-PR, ohne dass es jemand wusste
(`docs/verlauf.md`, 5.7).

WAS ES TUT. Es liest die veroeffentlichten Meldungen der Repositorys in
`REPOSITORYS` ueber die GitHub-API und prueft fuer jedes darin genannte
Paket, ob die Fassung, die hier gebaut wird, im betroffenen Bereich liegt:

    Kisten          aus `Cargo.lock`
    npm-Pakete      aus `app/oberflaeche/package-lock.json`
    das Werkzeug    aus `TAURI_CLI` in `.github/workflows/auslieferung.yml`

WAS ES NICHT TUT. Es ersetzt `cargo deny` und `npm audit` nicht. Es
ergaenzt sie um die eine Quelle, die keines von beiden liest.

Aufruf:
    python3 .github/skripte/tauri_meldungen.py               # Abgleich
    python3 .github/skripte/tauri_meldungen.py --selbsttest  # Bereichslogik

Rueckgabe: 0, wenn nichts betroffen ist. 1, wenn etwas betroffen ist. 2,
wenn eine Abfrage scheiterte oder eine Meldung, die ein Paket von hier
nennt, nicht auswertbar war -- dann ist das Ergebnis unbekannt, und
unbekannt ist nicht dasselbe wie gut.

Lokal unter Windows kann die Abfrage an der Zertifikatspruefung
scheitern, wenn ein Virenwaechter HTTPS mitliest (so am 27.10.2026 mit
F-Secure): Python prueft gegen eigene Wurzelzertifikate, nicht gegen die
von Windows. Dann kommt 2 heraus, nicht 0. Gemeint ist der Lauf in der CI.
"""

import json
import os
import re
import ssl
import sys
import time
import tomllib
import unittest
import urllib.error
import urllib.request

# Die Repositorys, deren Meldungen gelesen werden: alle unter `tauri-apps`,
# deren Kisten in diesem Baum stehen. Am 27.10.2026 hatten nur die ersten
# beiden ueberhaupt Meldungen (9 und 2). Die uebrigen stehen hier, damit
# die erste nicht erst auffaellt, wenn jemand nachsieht.
REPOSITORYS = [
    "tauri-apps/tauri",
    "tauri-apps/plugins-workspace",
    "tauri-apps/wry",
    "tauri-apps/tao",
    "tauri-apps/muda",
    "tauri-apps/tray-icon",
    "tauri-apps/window-vibrancy",
    "tauri-apps/webkit2gtk-rs",
    "tauri-apps/javascriptcore-rs",
]

# Meldungen, die bewusst NICHT anschlagen sollen -- je mit Begruendung und
# Datum, wie die Ausnahmen in `deny.toml`. Leer ist der Normalfall.
AUSNAHMEN: dict[str, str] = {}

STUFEN = {"critical": "kritisch", "high": "hoch", "medium": "mittel", "low": "niedrig"}


# ---------------------------------------------------------------------------
# Fassungen und Bereiche
# ---------------------------------------------------------------------------

_FASSUNG = re.compile(
    r"^v?(\d+)(?:\.(\d+))?(?:\.(\d+))?(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$"
)


def fassung(text):
    """Macht aus "2.0.0-beta.19" einen vergleichbaren Schluessel.

    Fehlende Stellen zaehlen als 0: Die Meldungen schreiben ">=2.0" und
    meinen 2.0.0. Vorabfassungen ordnen sich nach SemVer ein -- vor der
    Fassung, zu der sie gehoeren, untereinander Feld fuer Feld.
    """
    m = _FASSUNG.match(text.strip())
    if not m:
        raise ValueError(f"keine Fassung: {text!r}")
    haupt, neben, flick = (int(g) if g else 0 for g in m.groups()[:3])
    vorab = m.group(4)
    if vorab is None:
        # Ohne Vorabteil ist eine Fassung groesser als jede mit.
        return (haupt, neben, flick, (1,))
    felder = tuple((0, int(f)) if f.isdigit() else (1, f) for f in vorab.split("."))
    return (haupt, neben, flick, (0, felder))


_BEDINGUNG = re.compile(r"^\s*(>=|<=|==|=|>|<)?\s*(\S+)\s*$")


def bereich(text):
    """Zerlegt ">= 2.0.0, <= 2.11.5" in Bedingungen, die ALLE gelten muessen.

    So schreibt GitHub die Bereiche. Was anders aussieht -- etwa ein "||" --,
    wird nicht geraten, sondern abgelehnt: Ein falsch gelesener Bereich
    meldete "nicht betroffen", wo es nicht stimmt.
    """
    if text is None:
        raise ValueError("kein Bereich angegeben")
    teile = [t for t in text.split(",") if t.strip()]
    if not teile:
        raise ValueError(f"leerer Bereich: {text!r}")
    bedingungen = []
    for teil in teile:
        m = _BEDINGUNG.match(teil)
        if not m:
            raise ValueError(f"unlesbarer Bereich: {text!r}")
        bedingungen.append((m.group(1) or "=", fassung(m.group(2))))
    return bedingungen


def betroffen(version, bereichstext):
    """Liegt `version` im Bereich?"""
    v = fassung(version)
    for op, grenze in bereich(bereichstext):
        if op in ("=", "==") and v != grenze:
            return False
        if op == ">=" and not v >= grenze:
            return False
        if op == ">" and not v > grenze:
            return False
        if op == "<=" and not v <= grenze:
            return False
        if op == "<" and not v < grenze:
            return False
    return True


# ---------------------------------------------------------------------------
# Was hier gebaut wird
# ---------------------------------------------------------------------------


def kisten(pfad):
    """Name -> Fassungen aus `Cargo.lock`. Die eigenen Kisten fehlen: Sie
    haben keine `source`, und Tauri meldet nichts ueber sie."""
    with open(pfad, "rb") as f:
        daten = tomllib.load(f)
    bestand = {}
    for p in daten.get("package", []):
        if "source" in p:
            bestand.setdefault(p["name"], set()).add(p["version"])
    return bestand


def npm_pakete(pfad):
    """Name -> Fassungen aus `package-lock.json`, verschachtelte inbegriffen."""
    with open(pfad, encoding="utf-8") as f:
        daten = json.load(f)
    bestand = {}
    for schluessel, eintrag in daten.get("packages", {}).items():
        if not schluessel or "version" not in eintrag:
            continue
        name = eintrag.get("name") or schluessel.rsplit("node_modules/", 1)[-1]
        bestand.setdefault(name, set()).add(eintrag["version"])
    return bestand


def werkzeug(pfad):
    """Die Fassung des Tauri-Werkzeugs aus `TAURI_CLI` in `auslieferung.yml`.

    Fehlt sie, ist das ein Fehler und kein leerer Bestand: Sonst prueft
    dieser Abgleich das Werkzeug stillschweigend nicht mehr, sobald jemand
    die Zeile umbenennt.
    """
    with open(pfad, encoding="utf-8") as f:
        text = f.read()
    m = re.search(r'^\s*TAURI_CLI:\s*"?([0-9][0-9A-Za-z.+-]*)"?\s*$', text, re.M)
    if not m:
        raise ValueError(f"{pfad}: keine Zeile `TAURI_CLI: \"<Fassung>\"`")
    return {"tauri-cli": {m.group(1)}}


# ---------------------------------------------------------------------------
# Abgleich
# ---------------------------------------------------------------------------


def abgleichen(meldungen, bestand):
    """Prueft jede Meldung gegen den Bestand.

    `meldungen`: Repository -> Liste der Meldungen, wie die API sie liefert.
    `bestand`: "rust" / "npm" / "werkzeug" -> Name -> Fassungen.

    Gibt (befunde, fehler, fremd) zurueck: je Paket von hier eine Zeile,
    die Meldungen, die sich nicht auswerten liessen, und die Zahl der
    Eintraege, deren Paket hier nicht vorkommt.
    """
    befunde, fehler, fremd = [], [], 0
    for repo, liste in meldungen.items():
        for m in liste:
            if m.get("withdrawn_at"):
                continue
            kennung = m.get("ghsa_id", "?")
            stufe = STUFEN.get(m.get("severity") or "", m.get("severity") or "?")
            for eintrag in m.get("vulnerabilities") or []:
                paket = eintrag.get("package") or {}
                oeko = (paket.get("ecosystem") or "").lower()
                name = paket.get("name") or ""
                # Die aeltesten Meldungen fuehren Tauri unter einem eigenen
                # "Oekosystem" `tauri-apps` und schreiben "Tauri". Gemeint
                # ist die Kiste; deshalb wird der Name dort in allen
                # Bestaenden gesucht.
                if oeko == "rust":
                    quellen = [("Cargo.lock", bestand["rust"], name),
                               ("auslieferung.yml", bestand["werkzeug"], name)]
                elif oeko == "npm":
                    quellen = [("package-lock.json", bestand["npm"], name)]
                else:
                    klein = name.lower()
                    quellen = [("Cargo.lock", bestand["rust"], klein),
                               ("auslieferung.yml", bestand["werkzeug"], klein),
                               ("package-lock.json", bestand["npm"], klein)]
                treffer = [(q, n, v) for q, b, n in quellen for v in sorted(b.get(n, ()))]
                if not treffer:
                    fremd += 1
                    continue
                spanne = eintrag.get("vulnerable_version_range")
                behoben = eintrag.get("patched_versions") or "?"
                for quelle, paketname, version in treffer:
                    try:
                        drin = betroffen(version, spanne)
                    except ValueError as e:
                        fehler.append(f"{kennung} ({repo}), {paketname} {version}: {e}")
                        continue
                    befunde.append({
                        "kennung": kennung, "stufe": stufe, "paket": paketname,
                        "fassung": version, "quelle": quelle, "spanne": spanne,
                        "behoben": behoben, "betroffen": drin,
                        "ausnahme": AUSNAHMEN.get(kennung),
                    })
    return befunde, fehler, fremd


# ---------------------------------------------------------------------------
# Abfrage
# ---------------------------------------------------------------------------


def _anfrage(url, token):
    kopf = {
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "cabrik-secure-tauri-meldungen",
    }
    if token:
        kopf["Authorization"] = f"Bearer {token}"
    return urllib.request.Request(url, headers=kopf)


def _naechste(link):
    for teil in link.split(","):
        if 'rel="next"' in teil:
            return teil[teil.index("<") + 1:teil.index(">")]
    return None


def lies_meldungen(repo, token):
    """Alle veroeffentlichten Meldungen eines Repositorys, seitenweise.

    Drei Versuche je Seite: Ein Netzaussetzer soll keinen roten Lauf
    machen. Ein dauerhafter Fehler dagegen schon -- ein Abgleich, der
    nichts lesen konnte, darf nicht "nichts betroffen" sagen.
    """
    url = (f"https://api.github.com/repos/{repo}/security-advisories"
           "?state=published&per_page=100")
    alle = []
    while url:
        for versuch in range(3):
            try:
                with urllib.request.urlopen(_anfrage(url, token), timeout=30) as antwort:
                    alle.extend(json.load(antwort))
                    url = _naechste(antwort.headers.get("Link", ""))
                break
            except urllib.error.HTTPError as e:
                # Die Meldungen sind oeffentlich. Ein Schluessel, dem eine
                # Berechtigung fehlt, soll nicht schlechter stehen als
                # gar keiner -- also einmal ohne.
                if e.code in (401, 403) and token:
                    token = None
                    continue
                if e.code < 500 or versuch == 2:
                    raise
            except urllib.error.URLError as e:
                # Ein Zertifikat, das nicht passt, passt beim zweiten
                # Versuch nicht besser.
                if versuch == 2 or isinstance(e.reason, ssl.SSLCertVerificationError):
                    raise
            time.sleep(5 * (versuch + 1))
    return alle


# ---------------------------------------------------------------------------
# Ablauf
# ---------------------------------------------------------------------------


def main():
    wurzel = os.path.normpath(os.path.join(os.path.dirname(__file__), "..", ".."))
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    in_actions = os.environ.get("GITHUB_ACTIONS") == "true"

    bestand = {
        "rust": kisten(os.path.join(wurzel, "Cargo.lock")),
        "npm": npm_pakete(os.path.join(wurzel, "app", "oberflaeche", "package-lock.json")),
        "werkzeug": werkzeug(os.path.join(wurzel, ".github", "workflows", "auslieferung.yml")),
    }

    meldungen, abfragefehler = {}, []
    for repo in REPOSITORYS:
        try:
            meldungen[repo] = lies_meldungen(repo, token)
        except (urllib.error.URLError, ValueError) as e:
            abfragefehler.append(f"{repo}: {e}")
            continue
        print(f"{repo}: {len(meldungen[repo])} Meldungen")

    befunde, fehler, fremd = abgleichen(meldungen, bestand)
    fehler = abfragefehler + fehler

    print()
    for b in sorted(befunde, key=lambda b: (not b["betroffen"], b["kennung"], b["paket"])):
        if b["betroffen"] and b["ausnahme"]:
            urteil = f"betroffen, AUSGENOMMEN: {b['ausnahme']}"
        elif b["betroffen"]:
            urteil = f"BETROFFEN, behoben ab {b['behoben']}"
        else:
            urteil = "nicht betroffen"
        print(f"{b['kennung']}  {b['stufe']:<8} {b['paket']} {b['fassung']} "
              f"({b['quelle']}) -- {urteil}  [betroffen: {b['spanne']}]")
        if b["betroffen"] and not b["ausnahme"] and in_actions:
            print(f"::error title={b['kennung']} ({b['stufe']})::{b['paket']} "
                  f"{b['fassung']} liegt in {b['spanne']}; behoben ab {b['behoben']}")
    print(f"\n{fremd} weitere Eintraege nennen Pakete, die hier nicht vorkommen.")

    for f in fehler:
        print(f"NICHT AUSWERTBAR: {f}")
        if in_actions:
            print(f"::error title=Tauri-Meldungen::{f}")

    offen = [b for b in befunde if b["betroffen"] and not b["ausnahme"]]
    if offen:
        print(f"\n{len(offen)} betroffen.")
        return 1
    if fehler:
        print("\nErgebnis unbekannt: Nicht alles liess sich lesen.")
        return 2
    print("\nNichts betroffen.")
    return 0


# ---------------------------------------------------------------------------
# Selbsttest
# ---------------------------------------------------------------------------


class Selbsttest(unittest.TestCase):
    def test_ordnung_nach_semver(self):
        # Die Reihe aus der SemVer-Spezifikation, Abschnitt 11.
        reihe = ["1.0.0-alpha", "1.0.0-alpha.1", "1.0.0-alpha.beta", "1.0.0-beta",
                 "1.0.0-beta.2", "1.0.0-beta.11", "1.0.0-rc.1", "1.0.0"]
        schluessel = [fassung(v) for v in reihe]
        self.assertEqual(schluessel, sorted(schluessel))
        self.assertEqual(len(set(schluessel)), len(reihe))

    def test_fehlende_stellen_zaehlen_als_null(self):
        self.assertEqual(fassung("2.0"), fassung("2.0.0"))
        self.assertEqual(fassung("1.3"), fassung("1.3.0"))

    def test_die_beiden_unerkannt_behobenen(self):
        # Genau die Fassungen, die bis zum 26.10.2026 im Baum standen.
        self.assertTrue(betroffen("2.11.5", ">= 2.0.0, <= 2.11.5"))
        self.assertFalse(betroffen("2.11.6", ">= 2.0.0, <= 2.11.5"))
        self.assertTrue(betroffen("2.5.1", ">= 2.0.0, < 2.5.2"))
        self.assertFalse(betroffen("2.5.2", ">= 2.0.0, < 2.5.2"))

    def test_schreibweisen_aus_den_meldungen(self):
        self.assertTrue(betroffen("2.11.0", ">=2.0, <=2.11.0"))
        self.assertFalse(betroffen("2.12.5", ">=2.0, <=2.11.0"))
        self.assertTrue(betroffen("1.4.0", "=1.4.0"))
        self.assertFalse(betroffen("1.4.1", "=1.4.0"))
        self.assertTrue(betroffen("1.2.9", "< 1.3"))
        self.assertFalse(betroffen("1.3.0", "< 1.3"))
        self.assertTrue(betroffen("2.2.0", "<=2.2.0"))
        self.assertTrue(betroffen("2.0.0-beta.19", ">= 2.0.0-beta.0, <=2.0.0-beta.19"))
        self.assertFalse(betroffen("2.0.0", ">= 2.0.0-beta.0, <=2.0.0-beta.19"))
        self.assertFalse(betroffen("2.0.0-alpha.16", ">= 2.0.0-alpha.0, < 2.0.0-alpha.16"))

    def test_unlesbares_wird_nicht_geraten(self):
        for text in [None, "", "irgendwas mit leerzeichen", ">= 1.0 || < 0.5", ">= x.y"]:
            with self.assertRaises(ValueError, msg=repr(text)):
                betroffen("1.0.0", text)

    def test_abgleich(self):
        bestand = {
            "rust": {"tauri": {"2.11.5"}, "tauri-plugin-fs": {"2.7.1"}},
            "npm": {"@tauri-apps/api": {"2.12.4"}},
            "werkzeug": {"tauri-cli": {"2.12.4"}},
        }

        def meldung(kennung, oeko, name, spanne, **mehr):
            return {"ghsa_id": kennung, "severity": "high", **mehr,
                    "vulnerabilities": [{"package": {"ecosystem": oeko, "name": name},
                                         "vulnerable_version_range": spanne,
                                         "patched_versions": "?"}]}

        meldungen = {"tauri-apps/tauri": [
            meldung("GHSA-a", "rust", "tauri", ">= 2.0.0, <= 2.11.5"),
            meldung("GHSA-b", "rust", "tauri-plugin-fs", ">= 2.0.0, < 2.5.2"),
            meldung("GHSA-c", "tauri-apps", "Tauri", "< 2.12"),
            meldung("GHSA-d", "rust", "tauri-cli", ">= 2.12.0, < 2.12.5"),
            meldung("GHSA-e", "npm", "@tauri-apps/plugin-shell", "<=2.2.0"),
            meldung("GHSA-f", "rust", "tauri", "<= 2.11.5", withdrawn_at="2026-01-01"),
            meldung("GHSA-g", "rust", "tauri", "kaputt kaputt"),
        ]}
        befunde, fehler, fremd = abgleichen(meldungen, bestand)
        urteil = {(b["kennung"], b["paket"]): b["betroffen"] for b in befunde}
        self.assertEqual(urteil, {
            ("GHSA-a", "tauri"): True,
            ("GHSA-b", "tauri-plugin-fs"): False,
            ("GHSA-c", "tauri"): True,
            ("GHSA-d", "tauri-cli"): True,
        })
        self.assertEqual(fremd, 1)
        self.assertEqual(len(fehler), 1)
        self.assertIn("GHSA-g", fehler[0])


if __name__ == "__main__":
    if "--selbsttest" in sys.argv:
        unittest.main(argv=[sys.argv[0]], verbosity=2)
    sys.exit(main())
