"""Filtert die offiziellen RFC-9180-Testvektoren auf die von uns
implementierte Ciphersuite.

Quelle: https://raw.githubusercontent.com/cfrg/draft-irtf-cfrg-hpke/master/test-vectors.json
(rund 5,6 MB, alle Kombinationen aus KEM, KDF und AEAD)

Wir behalten ausschliesslich:
    mode    = 0   (Base)
    kem_id  = 0x0020  DHKEM(X25519, HKDF-SHA256)
    kdf_id  = 0x0001  HKDF-SHA256
    aead_id = 0x0003  ChaCha20-Poly1305

Alle uebrigen Suites lehnt der Leser mit UNSUPPORTED_SUITE ab; ihre Vektoren
koennten also gar nichts pruefen.

Nur Mode Base: Das Envelope-Format nutzt HPKE ausschliesslich im Base-Modus
und authentifiziert den Absender ueber eine Ed25519-Signatur im
verschluesselten Trailer (envelope-v2.md §2, §9). HPKE-Auth kommt nicht vor.

Aufruf:
    python testvectors/tools/filter_rfc9180.py <pfad-zur-vollen-datei>
"""
import json
import sys

MODE_BASE = 0
KEM_X25519_HKDF_SHA256 = 0x0020
KDF_HKDF_SHA256 = 0x0001
AEAD_CHACHA20POLY1305 = 0x0003

# Felder, die wir uebernehmen. Der Rest (exports, key_schedule_context ...)
# wird nicht geprueft und wuerde die Datei nur aufblaehen.
KEEP = [
    "mode", "kem_id", "kdf_id", "aead_id",
    "info", "ikmE", "pkEm", "skEm", "ikmR", "pkRm", "skRm",
    "enc", "shared_secret", "key", "base_nonce",
]

if len(sys.argv) < 2:
    sys.exit("Aufruf: filter_rfc9180.py <pfad-zur-vollen-test-vectors.json>")

with open(sys.argv[1], "r", encoding="utf-8") as f:
    alle = json.load(f)

passend = [
    v for v in alle
    if v.get("mode") == MODE_BASE
    and v.get("kem_id") == KEM_X25519_HKDF_SHA256
    and v.get("kdf_id") == KDF_HKDF_SHA256
    and v.get("aead_id") == AEAD_CHACHA20POLY1305
]

vectors = []
for i, v in enumerate(passend):
    eintrag = {k: v[k] for k in KEEP if k in v}
    # Die ersten Verschluesselungen je Vektor genuegen; sie pruefen Nonce-
    # Fortschaltung und AAD-Behandlung.
    eintrag["encryptions"] = [
        {k: e[k] for k in ("aad", "ct", "nonce", "pt") if k in e}
        for e in v.get("encryptions", [])[:4]
    ]
    eintrag["id"] = f"rfc9180-base-x25519-chacha-{i:02d}"
    vectors.append(eintrag)

doc = {
    "spec_version": "2.0",
    "kind": "hpke-rfc9180",
    "description": (
        "Offizielle RFC-9180-Vektoren, gefiltert auf Mode Base, "
        "DHKEM(X25519, HKDF-SHA256) + HKDF-SHA256 + ChaCha20-Poly1305."
    ),
    "source": "https://github.com/cfrg/draft-irtf-cfrg-hpke test-vectors.json",
    "notes": [
        "Fremde Vektoren -- nicht von uns erzeugt. Ohne sie testet die",
        "Implementierung nur gegen sich selbst.",
        "ikmE ist das Eingangsmaterial des ephemeren Schluessels. Wer es",
        "fixiert, macht die Verschluesselung deterministisch -- genau das",
        "Verfahren, das test-vectors.md §3 fuer eigene Vektoren vorschreibt.",
    ],
    "vectors": vectors,
}

with open("testvectors/hpke/rfc9180-x25519-chacha.json", "w",
          encoding="utf-8", newline="\n") as f:
    json.dump(doc, f, indent=2)
    f.write("\n")

modi = {}
for v in alle:
    modi[v.get("mode")] = modi.get(v.get("mode"), 0) + 1

print(f"  Vektoren gesamt in der Quelle : {len(alle)}")
print(f"  davon Mode Base              : {modi.get(0, 0)}")
print(f"  passend zu unserer Suite     : {len(passend)}")
print(f"  uebernommen                  : {len(vectors)}")
