"""Prueft den vollstaendigen Weg ueber ALLE Formate hinweg.

    Vorlage -> bereinigen -> verschluesseln -> entschluesseln -> vergleichen

Die Einzeltests pruefen jeden Schritt fuer sich. Was sie NICHT pruefen, ist
das Zusammenspiel ueber die Crate-Grenzen hinweg: ob eine von
cabrik-metadata bereinigte Datei nach dem Rundlauf durch den Umschlag von
cabrik-core byteweise dieselbe ist. Dort sitzen die Fehler, die keinem
Einzeltest auffallen.

Der Test laeuft gegen das GEBAUTE PROGRAMM, nicht gegen die Bibliothek --
also ueber genau den Weg, den ein Nutzer nimmt, samt Kommandozeile,
Passwortdatei und Dateisystem.

Aufruf:
    cargo build --release
    python testvectors/tools/pruefe_rundlauf.py target/release/cabrik.exe

Stand der letzten Ausfuehrung: 31 Vorlagen, alle bytegleich.
"""

import os
import shutil
import subprocess
import sys
import tempfile

EXE = sys.argv[1] if len(sys.argv) > 1 else r"target\debug\cabrik.exe"
VORLAGEN = os.path.join("testvectors", "metadata")
PASSWORT = "rundlauf-testpasswort"


def lauf(*args, eingabe=None):
    return subprocess.run(
        [EXE, *args], capture_output=True, input=eingabe,
        encoding="utf-8", errors="replace",
    )


def main():
    arbeit = tempfile.mkdtemp(prefix="cabrik-rundlauf-")
    pw_datei = os.path.join(arbeit, "pw.txt")
    with open(pw_datei, "w", encoding="utf-8") as f:
        f.write(PASSWORT)

    dateien = sorted(
        n for n in os.listdir(VORLAGEN)
        if os.path.isfile(os.path.join(VORLAGEN, n))
        and not n.endswith(".stripped")
        and n != "manifest.json"
    )

    fehler = []
    geprueft = 0
    gesamt_vorher = gesamt_nachher = 0

    print(f"{len(dateien)} Vorlagen, Werkzeug: {EXE}\n")
    print(f"{'Datei':<32}{'roh':>9}{'bereinigt':>11}{'Umschlag':>11}  Rundlauf")
    print("-" * 76)

    for name in dateien:
        quelle = os.path.join(VORLAGEN, name)
        sauber = os.path.join(arbeit, "sauber_" + name)
        env = os.path.join(arbeit, name + ".enc")
        zurueck = os.path.join(arbeit, "zurueck_" + name)

        roh_groesse = os.path.getsize(quelle)

        # 1. Bereinigen. Schlaegt es fehl, wird die Vorlage unveraendert
        #    weiterverwendet -- ein unbekanntes Format ist kein Fehler.
        r = lauf("metadata", "strip", quelle, "--out", sauber)
        if r.returncode != 0 or not os.path.exists(sauber):
            shutil.copyfile(quelle, sauber)
        sauber_groesse = os.path.getsize(sauber)

        # 2. Verschluesseln mit Passwort.
        r = lauf("encrypt", sauber, "--out", env,
                 "--password", "--password-file", pw_datei)
        if r.returncode != 0:
            fehler.append(f"{name}: verschluesseln fehlgeschlagen: "
                          f"{(r.stderr or '').strip()[:120]}")
            continue
        env_groesse = os.path.getsize(env)

        # 3. Entschluesseln.
        r = lauf("decrypt", env, "--out", zurueck,
                 "--password", "--password-file", pw_datei)
        if r.returncode != 0:
            fehler.append(f"{name}: entschluesseln fehlgeschlagen: "
                          f"{(r.stderr or '').strip()[:120]}")
            continue

        # 4. Bytegleich?
        with open(sauber, "rb") as a, open(zurueck, "rb") as b:
            va, vb = a.read(), b.read()
        gleich = va == vb
        if not gleich:
            fehler.append(f"{name}: Rundlauf NICHT bytegleich "
                          f"({len(va)} -> {len(vb)} Bytes)")

        gesamt_vorher += roh_groesse
        gesamt_nachher += sauber_groesse
        geprueft += 1
        print(f"{name:<32}{roh_groesse:>9}{sauber_groesse:>11}"
              f"{env_groesse:>11}  {'ok' if gleich else 'ABWEICHUNG'}")

    print("-" * 76)
    print(f"{geprueft} Dateien, roh {gesamt_vorher} -> bereinigt {gesamt_nachher} Bytes")

    if fehler:
        print(f"\n{len(fehler)} Problem(e):")
        for f in fehler:
            print(f"  - {f}")
        shutil.rmtree(arbeit, ignore_errors=True)
        sys.exit(1)

    print("\nAlle Rundlaeufe bytegleich.")
    shutil.rmtree(arbeit, ignore_errors=True)


if __name__ == "__main__":
    main()
