#!/usr/bin/env python3
import argparse, json
from cabrik_secure.crypto_core import inspect_metadata, strip_metadata

def main():
    ap = argparse.ArgumentParser(prog="cabrik-meta", description="Cabrik Secure – Metadaten")
    sub = ap.add_subparsers(dest="cmd", required=True)

    insp = sub.add_parser("inspect", help="Metadaten ansehen")
    insp.add_argument("path")
    insp.add_argument("--all", action="store_true", help="Alle Metadaten ausführlich")

    st = sub.add_parser("strip", help="Metadaten entfernen")
    st.add_argument("path")
    st.add_argument("--out", help="Output-Pfad (optional)")

    args = ap.parse_args()

    if args.cmd == "inspect":
        info = inspect_metadata(args.path)
        if args.all:
            print(json.dumps(info, indent=2, ensure_ascii=False))
        else:
            compact = {
                "file": info.get("file"),
                "type": info.get("type"),
                "warnings": info.get("warnings"),
                "keys": list(info.get("metadata", {}).keys())
            }
            print(json.dumps(compact, indent=2, ensure_ascii=False))

    elif args.cmd == "strip":
        out = strip_metadata(args.path, args.out)
        print("Cleaned →", out)

if __name__ == "__main__":
    main()
