#!/usr/bin/env python3
import argparse, getpass
from cabrik_secure.crypto_core import generate_identity, save_identity_keyfile, b64e

def main():
    ap = argparse.ArgumentParser(prog="cabrik-keygen", description="Cabrik Secure – Keygen")
    ap.add_argument("--out", required=True, help="Pfad zur Keyfile (JSON)")
    ap.add_argument("--no-signing", action="store_true", help="Keinen persistenten Signierschlüssel anlegen")
    args = ap.parse_args()

    pwd = getpass.getpass("Passwort für Keyfile: ")
    if not pwd:
        ap.error("Leeres Passwort ist nicht erlaubt.")

    ident = generate_identity(anonymity=args.no_signing)
    save_identity_keyfile(ident, pwd, args.out)

    print("Keyfile gespeichert:", args.out)
    print("Recipient ENC Public Key (base64):", b64e(bytes(ident.enc_pk)))
    if ident.sig_vk:
        print("Sender SIG Public Key (base64):", b64e(bytes(ident.sig_vk)))
    else:
        print("(Kein persistenter Signierschlüssel)")

if __name__ == "__main__":
    main()
