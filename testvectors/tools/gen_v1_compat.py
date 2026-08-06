"""Erzeugt v1-Kompatibilitaetsvektoren mit der Referenzimplementierung.

Die Originaldateien aus der v1-Entwicklung unter _archive_v1/ sind dafuer
unbrauchbar -- ihre Passwoerter sind nicht bekannt. Hier entstehen frische
mit dokumentiertem Passwort.

Grundlage: legacy/python-v1/cabrik_secure/crypto_core.py, envelope-v2.md §13,
keyfile-v2.md §5.

Der heikle Teil ist die AAD des Envelopes: v1 bildet sie mit
    json.dumps(header, separators=(",", ":"), sort_keys=True)
also SORTIERT, waehrend der Envelope selbst UNSORTIERT serialisiert wird.
Der Rust-Leser muss die AAD daher neu erzeugen und kann nicht die Bytes aus
der Datei nehmen. Deshalb liegt die erwartete AAD hier ausdruecklich bei --
so laesst sich die kanonische Serialisierung einzeln pruefen, nicht nur
ueber den fertigen Envelope.
"""
import base64
import json
import os
import sys

sys.path.insert(0, os.path.join("legacy", "python-v1"))

from cabrik_secure.crypto_core import (  # noqa: E402
    generate_identity, save_identity_keyfile, encrypt_payload, b64e,
)

PASSWORT = "v1-kompat-testpasswort"


def b64(data):
    return base64.b64encode(data).decode("ascii")


def aad_von(envelope_b64):
    """Bildet die AAD so, wie v1 sie beim Verschluesseln gebildet hat."""
    env = json.loads(base64.b64decode(envelope_b64).decode("utf-8"))
    return json.dumps(env["header"], separators=(",", ":"),
                      sort_keys=True).encode("utf-8")


# --- Keyfiles ---------------------------------------------------------------

keyfiles = []
identitaeten = {}

for vid, desc, anonym in [
    ("kf-v1-signing", "v1-Keyfile mit Signierschluessel", False),
    ("kf-v1-anonymous", "v1-Keyfile ohne Signierschluessel", True),
]:
    ident = generate_identity(anonymity=anonym)
    pfad = f"_tmp_{vid}.json"
    save_identity_keyfile(ident, PASSWORT, pfad)
    with open(pfad, "rb") as f:
        rohdaten = f.read()
    os.remove(pfad)

    identitaeten[vid] = ident
    keyfiles.append({
        "id": vid,
        "description": desc,
        "input": {"password": PASSWORT, "keyfile_b64": b64(rohdaten)},
        "expected": {
            "enc_sk_b64": b64(bytes(ident.enc_sk)),
            "sig_sk_b64": (b64(bytes(ident.sig_sk)) if ident.sig_sk else None),
            "enc_pub_b64": b64(bytes(ident.enc_pk)),
        },
    })
    print(f"  {vid:<18} {len(rohdaten):>5} Bytes")

# --- Envelopes --------------------------------------------------------------

empf = identitaeten["kf-v1-signing"]
empf_pub = b64e(bytes(empf.enc_pk))

faelle = [
    ("env-v1-text-signed", "Text, mit persistenter Signatur",
     b"Hallo aus Version 1", "text", False, empf, None),
    ("env-v1-text-anon", "Text, anonym (ephemerer Signierschluessel)",
     b"anonyme Nachricht", "text", True, None, None),
    ("env-v1-file", "Datei mit Namen und Groesse im Klartext-Header",
     b"Dateiinhalt" * 40, "file", False, empf,
     {"filename": "Kuendigung_vertraulich.pdf", "size": str(11 * 40)}),
    ("env-v1-umlaut", "Dateiname mit Umlauten -- prueft ensure_ascii",
     b"x", "file", False, empf,
     {"filename": "Bericht_Grün_Übersicht.pdf", "size": "1"}),
    ("env-v1-empty", "leerer Klartext",
     b"", "text", True, None, None),
]

envelopes = []
for vid, desc, pt, purpose, anon, sender, extra in faelle:
    env = encrypt_payload(empf_pub, pt, sender, anon, purpose=purpose,
                          extra_aad=extra)
    header = json.loads(base64.b64decode(env).decode("utf-8"))["header"]
    envelopes.append({
        "id": vid,
        "description": desc,
        "input": {
            "envelope_b64": env,
            "recipient_keyfile": "kf-v1-signing",
        },
        "expected": {
            "plaintext_b64": b64(pt),
            "purpose": purpose,
            "signed": header.get("sender_sig_pub") is not None,
            "meta": header.get("meta"),
            # Die kanonische AAD -- einzeln pruefbar.
            "aad_utf8": aad_von(env).decode("utf-8"),
        },
    })
    print(f"  {vid:<18} {len(env):>5} Zeichen")

doc = {
    "spec_version": "2.0",
    "kind": "v1-compat",
    "description": "v1-Keyfiles und -Envelopes zur Pruefung des Migrationslesers.",
    "notes": [
        "Erzeugt mit legacy/python-v1, der eingefrorenen Referenz-",
        "implementierung. Passwort steht im Klartext dabei -- es sind",
        "Wegwerfschluessel.",
        "aad_utf8 ist die kanonische JSON-Serialisierung des Headers",
        "(sortierte Schluessel, keine Leerzeichen, ensure_ascii). v1 nutzt",
        "sie als AEAD-AAD, serialisiert den Envelope selbst aber",
        "UNSORTIERT -- die AAD muss daher neu gebildet werden und kann",
        "nicht aus der Datei uebernommen werden.",
        "v1 nutzt XChaCha20-Poly1305 (24-Byte-Nonce) und Argon2id mit",
        "libsodium-MODERATE: opslimit 3, memlimit 256 MiB, Parallelitaet 1.",
    ],
    "keyfiles": keyfiles,
    "envelopes": envelopes,
}

with open("testvectors/v1-compat.json", "w", encoding="utf-8", newline="\n") as f:
    json.dump(doc, f, indent=2, ensure_ascii=False)
    f.write("\n")

print(f"\n  {len(keyfiles)} Keyfiles, {len(envelopes)} Envelopes")
print(f"  Beispiel-AAD: {envelopes[0]['expected']['aad_utf8'][:70]}...")
