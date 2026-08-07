"""Unabhaengige Referenz fuer Fingerprint und Safety Number.

Bewusst ohne Rueckgriff auf den Rust-Code und ohne HKDF-Bibliothek: Die
Vektoren sollen die *Spezifikation* pruefen, nicht bestaetigen, was der Code
ohnehin tut (spec/test-vectors.md §8). Stimmen zwei unabhaengig geschriebene
Implementierungen ueberein, ist das ein echter Nachweis.

Grundlage: spec/trust-store.md §2 und §3.
"""
import hashlib
import hmac
import json

ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"   # Crockford
ENC_LEN, SIG_LEN, PQ_LEN = 32, 32, 1216


def hkdf_sha256(ikm: bytes, salt: bytes | None, info: bytes, length: int) -> bytes:
    """RFC 5869, von Hand — Extract und Expand."""
    if salt is None:
        salt = bytes(hashlib.sha256().digest_size)
    prk = hmac.new(salt, ikm, hashlib.sha256).digest()
    okm, block, counter = b"", b"", 1
    while len(okm) < length:
        block = hmac.new(prk, block + info + bytes([counter]), hashlib.sha256).digest()
        okm += block
        counter += 1
    return okm[:length]


def fingerprint(enc_pub: bytes,
                sig_pub: bytes | None,
                pq_pub: bytes | None) -> bytes:
    """spec/trust-store.md §2 — mit Praesenz-Bytes."""
    h = hashlib.sha256()
    h.update(b"cabrik-fp-v2")
    h.update(enc_pub)
    h.update(bytes([1 if sig_pub is not None else 0]))
    h.update(sig_pub if sig_pub is not None else bytes(SIG_LEN))
    h.update(bytes([1 if pq_pub is not None else 0]))
    h.update(pq_pub if pq_pub is not None else bytes(PQ_LEN))
    return h.digest()


def base32_encode(data: bytes) -> str:
    """Crockford-Base32, ohne Auffuellung."""
    out, acc, bits = "", 0, 0
    for byte in data:
        acc = (acc << 8) | byte
        bits += 8
        while bits >= 5:
            bits -= 5
            out += ALPHABET[(acc >> bits) & 0x1F]
    if bits:
        out += ALPHABET[(acc << (5 - bits)) & 0x1F]
    return out


def gruppiere(s: str, n: int = 4) -> str:
    return "-".join(s[i:i + n] for i in range(0, len(s), n))


def safety_number(fp_a: bytes, fp_b: bytes) -> str:
    """spec/trust-store.md §3 — 8 Bytes je Gruppe, kein Rejection Sampling."""
    first, second = sorted([fp_a, fp_b])
    base = hashlib.sha256(b"cabrik-sn-v2" + first + second).digest()
    material = hkdf_sha256(base, None, b"cabrik-sn-digits", 96)
    groups = []
    for i in range(12):
        g = int.from_bytes(material[i * 8:(i + 1) * 8], "big")
        groups.append(f"{g % 100000:05d}")
    return " ".join(groups)


def b(pattern: int, length: int) -> bytes:
    return bytes([pattern]) * length


# --- Faelle -----------------------------------------------------------------

cases = [
    ("fp-full", "alle drei Schluessel vorhanden",
     b(0x01, ENC_LEN), b(0x02, SIG_LEN), b(0x03, PQ_LEN)),
    ("fp-no-sig", "Anonymitaets-Identitaet ohne Signierschluessel",
     b(0x01, ENC_LEN), None, b(0x03, PQ_LEN)),
    ("fp-no-pq", "aus v1 migrierter Kontakt ohne PQ-Schluessel",
     b(0x01, ENC_LEN), b(0x02, SIG_LEN), None),
    ("fp-neither", "weder Signatur noch PQ",
     b(0x01, ENC_LEN), None, None),
    # Der Angriffsfall aus §2.1: Null-Schluessel duerfen nicht mit
    # "kein Schluessel" kollidieren.
    ("fp-zero-sig", "Signierschluessel aus lauter Nullen",
     b(0x01, ENC_LEN), bytes(SIG_LEN), None),
    ("fp-zero-pq", "PQ-Schluessel aus lauter Nullen",
     b(0x01, ENC_LEN), None, bytes(PQ_LEN)),
    ("fp-zero-enc", "Verschluesselungsschluessel aus lauter Nullen",
     bytes(ENC_LEN), None, None),
]

fp_vectors = []
computed = {}
for vid, desc, enc, sig, pq in cases:
    fp = fingerprint(enc, sig, pq)
    computed[vid] = fp
    full = base32_encode(fp)
    fp_vectors.append({
        "id": vid,
        "description": desc,
        "input": {
            "enc_pub_byte": enc[0] if enc else 0,
            "has_sig": sig is not None,
            "sig_pub_byte": (sig[0] if sig else None),
            "has_pq": pq is not None,
            "pq_pub_byte": (pq[0] if pq else None),
        },
        "expected": {
            "fingerprint_hex": fp.hex(),
            "display_full": gruppiere(full),
            "display": gruppiere(full[:32]),
            "short": full[:8],
        },
    })

sn_vectors = []
for a, bb in [("fp-full", "fp-no-sig"), ("fp-full", "fp-neither"),
              ("fp-no-pq", "fp-zero-pq")]:
    sn_vectors.append({
        "id": f"sn-{a}-{bb}",
        "description": f"Safety Number zwischen {a} und {bb}",
        "input": {"fingerprint_a": computed[a].hex(),
                  "fingerprint_b": computed[bb].hex()},
        "expected": {"safety_number": safety_number(computed[a], computed[bb])},
    })

doc = {
    "spec_version": "2.0",
    "kind": "fingerprint",
    "description": "Fingerprint und Safety Number nach trust-store.md §2 und §3.",
    "notes": [
        "Erzeugt von einer unabhaengigen Python-Referenz, nicht aus dem",
        "Rust-Code abgeleitet. Eingabeschluessel bestehen aus Wiederholungen",
        "eines einzelnen Bytes; enc_pub_byte, sig_pub_byte und pq_pub_byte",
        "geben dieses Byte an. has_sig bzw. has_pq = false bedeutet, dass",
        "der Schluessel fehlt -- das ist etwas anderes als ein Schluessel aus",
        "lauter Nullen, siehe trust-store.md §2.1.",
    ],
    "fingerprints": fp_vectors,
    "safety_numbers": sn_vectors,
}

with open("testvectors/fingerprint.json", "w", encoding="utf-8", newline="\n") as f:
    json.dump(doc, f, indent=2, ensure_ascii=False)
    f.write("\n")

for v in fp_vectors:
    print(f"  {v['id']:<15} {v['expected']['short']}  {v['expected']['fingerprint_hex'][:16]}...")
print()
for v in sn_vectors:
    print(f"  {v['id']}\n    {v['expected']['safety_number']}")

# Der Angriffsfall aus §2.1 muss nachweislich abgewehrt sein.
assert computed["fp-neither"] != computed["fp-zero-sig"], "Praesenz-Byte sig wirkungslos"
assert computed["fp-neither"] != computed["fp-zero-pq"], "Praesenz-Byte pq wirkungslos"
print("\n  Praesenz-Bytes wirken: 'kein Schluessel' != 'Null-Schluessel'")
