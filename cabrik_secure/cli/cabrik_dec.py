#!/usr/bin/env python3
import argparse, getpass, json
from cabrik_secure.crypto_core import load_identity_keyfile, decrypt_text, decrypt_file

def main():
    ap = argparse.ArgumentParser(prog="cabrik-dec", description="Cabrik Secure – Decrypt")
    ap.add_argument("--keyfile", required=True, help="Eigene Keyfile (Empfänger)")
    ap.add_argument("--in", dest="infile", required=True, help="Input Envelope (.enc)")
    group = ap.add_mutually_exclusive_group(required=True)
    group.add_argument("--out", help="Pfad für entschlüsselte Datei")
    group.add_argument("--print", action="store_true", help="Entschlüsselten Text auf STDOUT ausgeben")
    ap.add_argument("--require-signature", action="store_true", help="Fehler, wenn keine Signatur vorhanden")
    args = ap.parse_args()

    pwd = getpass.getpass("Passwort für Keyfile: ")
    ident = load_identity_keyfile(pwd, args.keyfile)

    with open(args.infile, "r", encoding="utf-8") as f:
        envelope_b64 = f.read().strip()

    if args.out:
        info = decrypt_file(ident, envelope_b64, args.out, require_signature=args.require_signature)
    else:
        text, info = decrypt_text(ident, envelope_b64, require_signature=args.require_signature)

    report = {
        "version": info["header"].get("version"),
        "purpose": info["header"].get("purpose"),
        "ts": info["header"].get("ts"),
        "signature_valid": info.get("signature_valid"),
        "sender_sig_fingerprint": info.get("sender_sig_fingerprint"),
        "meta": info["header"].get("meta"),
    }
    print("\n[Verifikation]")
    print(json.dumps(report, indent=2, ensure_ascii=False))

if __name__ == "__main__":
    main()
