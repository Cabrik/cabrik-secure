"""Extrahiert die X-Wing-Testvektoren aus dem IETF-Entwurf.

Quelle: https://www.ietf.org/archive/id/draft-connolly-cfrg-xwing-kem-10.txt

Zweck: Die Crate `x-wing` setzt Draft 06 um, der Entwurf steht bei 10. Laut
Changelog sind die Aenderungen dazwischen rein redaktionell -- diese Vektoren
pruefen das empirisch statt durch Interpretation.

Felder je Vektor:
    seed   32 Bytes  Eingangsmaterial der Schluesselerzeugung
    sk     32 Bytes  privater Schluessel (= seed, siehe Nsk in §7)
    pk   1216 Bytes  oeffentlicher Schluessel
    eseed  64 Bytes  Eingangsmaterial der Kapselung
    ct   1120 Bytes  Kapsel
    ss     32 Bytes  gemeinsames Geheimnis

Aufruf:
    python testvectors/tools/extract_xwing.py <pfad-zum-draft.txt>
"""
import json
import re
import sys

FELDER = ("seed", "sk", "pk", "eseed", "ss", "ct")
LAENGEN = {"seed": 32, "sk": 32, "pk": 1216, "eseed": 64, "ss": 32, "ct": 1120}

if len(sys.argv) < 2:
    sys.exit("Aufruf: extract_xwing.py <pfad-zum-draft-10.txt>")

with open(sys.argv[1], "r", encoding="utf-8") as f:
    text = f.read()

start = text.rfind("Appendix C.")
ende = text.find("Appendix D.", start)
if start < 0 or ende < 0:
    sys.exit("Anhang C nicht gefunden")
block = text[start:ende]

vektoren = []
aktuell = {}
feld = None

for zeile in block.split("\n"):
    # Seitenumbrueche und Kopfzeilen des Entwurfs ueberspringen.
    if ("Connolly" in zeile or "Internet-Draft" in zeile
            or "[Page" in zeile or zeile.startswith("\f")):
        continue

    # "name   hexwert" oder nur "name" (Wert folgt eingerueckt)
    m = re.match(r"^\s{0,5}([a-zA-Z_]+)\s*([0-9a-f]*)\s*$", zeile)
    if m and m.group(1) in FELDER:
        neu = m.group(1)
        # Ein erneutes "seed" beginnt einen neuen Vektor.
        if neu == "seed" and aktuell:
            vektoren.append(aktuell)
            aktuell = {}
        feld = neu
        aktuell[feld] = m.group(2)
        continue

    # Fortsetzungszeile: nur Hexziffern, eingerueckt.
    if feld and re.match(r"^\s+[0-9a-f]+\s*$", zeile):
        aktuell[feld] += zeile.strip()
        continue

    # Leerzeilen beenden das Feld NICHT. Die Hexbloecke laufen ueber
    # Seitenumbrueche hinweg, und dort stehen Leerzeilen -- wer hier
    # abbricht, verliert den halben Schluessel.
    if not zeile.strip():
        continue

    # Alles andere (Prosa) beendet das laufende Feld.
    feld = None

if aktuell:
    vektoren.append(aktuell)

vollstaendig = []
for i, v in enumerate(vektoren):
    fehlend = [f for f in FELDER if f not in v or not v[f]]
    if fehlend:
        print(f"  Vektor {i}: unvollstaendig, fehlt {fehlend} -- uebersprungen")
        continue
    schlecht = [f"{f}={len(v[f]) // 2}!={LAENGEN[f]}"
                for f in FELDER if len(v[f]) // 2 != LAENGEN[f]]
    if schlecht:
        print(f"  Vektor {i}: Laengen falsch: {schlecht} -- uebersprungen")
        continue
    v["id"] = f"xwing-draft10-{i:02d}"
    vollstaendig.append(v)
    print(f"  {v['id']}  seed={v['seed'][:16]}...  pk={len(v['pk']) // 2} Bytes")

if not vollstaendig:
    sys.exit("Kein vollstaendiger Vektor extrahiert")

doc = {
    "spec_version": "2.0",
    "kind": "xwing",
    "description": "X-Wing-Testvektoren aus draft-connolly-cfrg-xwing-kem-10, Anhang C.",
    "source": "https://www.ietf.org/archive/id/draft-connolly-cfrg-xwing-kem-10.txt",
    "notes": [
        "Fremde Vektoren. Die Crate `x-wing` setzt Draft 06 um, der Entwurf",
        "steht bei Revision 10. Laut Changelog sind die Aenderungen",
        "dazwischen rein redaktionell -- diese Vektoren pruefen das",
        "empirisch. Stimmen sie, ist die Frage geklaert.",
        "Der Entwurf markiert den Anhang mit einem TODO ('replace with test",
        "vectors that re-use ML-KEM, X25519 values'). Das betrifft die Wahl",
        "der Eingabewerte, nicht das Verfahren.",
        "sk ist identisch mit seed -- Nsk betraegt 32 Bytes, der private",
        "Schluessel IST der Seed (bestaetigt keyfile-v2.md §3.2).",
    ],
    "vectors": vollstaendig,
}

with open("testvectors/xwing/draft10.json", "w", encoding="utf-8", newline="\n") as f:
    json.dump(doc, f, indent=2)
    f.write("\n")

print(f"\n  {len(vollstaendig)} Vektoren nach testvectors/xwing/draft10.json")
print(f"  Gegenprobe sk == seed: {all(v['sk'] == v['seed'] for v in vollstaendig)}")
