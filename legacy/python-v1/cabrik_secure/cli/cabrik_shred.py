#!/usr/bin/env python3
import argparse, os, sys
from cabrik_secure.crypto_core import secure_delete

def main():
    ap = argparse.ArgumentParser(prog="cabrik-shred", description="Cabrik Secure – Secure Delete")
    ap.add_argument("paths", nargs="+", help="Dateipfade")
    ap.add_argument("--passes", type=int, default=3, help="Überschreib-Pässe (default 3)")
    args = ap.parse_args()

    ok, fail = 0, 0
    for p in args.paths:
        if not os.path.exists(p):
            print(f"[WARN] Nicht gefunden: {p}", file=sys.stderr)
            fail += 1
            continue
        try:
            secure_delete(p, passes=args.passes)
            if os.path.exists(p):
                print(f"[FAIL] Existiert noch: {p}", file=sys.stderr)
                fail += 1
            else:
                print(f"[OK] Gelöscht: {p}")
                ok += 1
        except Exception as e:
            print(f"[FAIL] {p}: {e}", file=sys.stderr)
            fail += 1

    print(f"\nErgebnis: OK={ok}  FAIL={fail}")

if __name__ == "__main__":
    main()
