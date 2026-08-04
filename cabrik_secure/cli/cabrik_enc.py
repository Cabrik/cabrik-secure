#!/usr/bin/env python3
import argparse, getpass, json
from cabrik_secure.crypto_core import (
    encrypt_text, encrypt_file, load_identity_keyfile,
    inspect_metadata, strip_metadata
)

def main():
    ap = argparse.ArgumentParser(prog="cabrik-enc", description="Cabrik Secure – Encrypt")
    ap.add_argument("--to-pub", required=True, help="Empfänger ENC Public Key (base64)")
    group = ap.add_mutually_exclusive_group(required=True)
    group.add_argument("--text", help="Klartextnachricht")
    group.add_argument("--infile", help="Datei zum Verschlüsseln")
    ap.add_argument("--out", required=True, help="Ausgabedatei (Envelope .enc)")
    ap.add_argument("--anonymous", action="store_true", help="Ephemerer Sign-Schlüssel (keine persistente Signatur)")
    ap.add_argument("--keyfile", help="Eigener Keyfile-Pfad für persistente Signatur (optional)")
    ap.add_argument("--password", help="Passwort für Keyfile (falls --keyfile gesetzt)")
    ap.add_argument("--meta", action="append", help="Zusatz-Metadaten key=value (mehrfach möglich)")
    ap.add_argument("--inspect", action="store_true", help="Metadaten kompakt anzeigen (nur bei --infile)")
    ap.add_argument("--inspect-all", action="store_true", help="Alle Metadaten ausführlich anzeigen (nur bei --infile)")
    ap.add_argument("--strip-metadata", action="store_true", help="Metadaten vorher entfernen (nur bei --infile)")
    args = ap.parse_args()

    sender = None
    if args.keyfile and not args.anonymous:
        pwd = args.password or getpass.getpass("Passwort für Keyfile: ")
        sender = load_identity_keyfile(pwd, args.keyfile)

    extra = None
    if args.meta:
        extra = {}
        for item in args.meta:
            if "=" in item:
                k, v = item.split("=", 1)
                extra[k] = v

    if args.infile:
        path = args.infile

        if args.inspect or args.inspect_all:
            info = inspect_metadata(path)
            if args.inspect:
                compact = {
                    "file": info.get("file"),
                    "type": info.get("type"),
                    "warnings": info.get("warnings"),
                    "keys": list(info.get("metadata", {}).keys())
                }
                print("Metadaten (kompakt):")
                print(json.dumps(compact, indent=2, ensure_ascii=False))
            if args.inspect_all:
                print("Metadaten (vollständig):")
                print(json.dumps(info, indent=2, ensure_ascii=False))

        if args.strip_metadata:
            path = strip_metadata(path)
            print("Metadaten entfernt. Neue Datei:", path)

        envelope_b64 = encrypt_file(args.to_pub, path, sender, args.anonymous)

    else:
        envelope_b64 = encrypt_text(args.to_pub, args.text, sender, args.anonymous)

    with open(args.out, "w", encoding="utf-8") as f:
        f.write(envelope_b64)
    print("Verschlüsselt →", args.out)

if __name__ == "__main__":
    main()
