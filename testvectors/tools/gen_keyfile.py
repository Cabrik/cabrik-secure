"""Unabhaengige Referenz fuer das Keyfile-Format v2.

Nutzt libsodium (ueber PyNaCl) fuer Argon2id und ChaCha20-Poly1305-IETF.
Der Rust-Kern nutzt die RustCrypto-Crates. Stimmen beide ueberein, ist das
ein Nachweis ueber zwei voellig getrennte Implementierungen hinweg.

Grundlage: spec/keyfile-v2.md §2 und §3.
"""
import base64
import json
import struct

from nacl.bindings import crypto_aead_chacha20poly1305_ietf_encrypt
from nacl.pwhash import argon2id

MAGIC = bytes([0xCA, 0x4B])
VERSION = 0x02
AAD_LEN = 28

TAG_ENC_SK, TAG_SIG_SK, TAG_CREATED, TAG_LABEL, TAG_PQ_SEED = 1, 2, 3, 4, 5


def tlv(fields):
    """TLV nach envelope-v2.md §7.2: type u8, length u16 BE, value.

    Die Felder muessen aufsteigend sortiert sein; das wird hier geprueft,
    damit die Referenz nicht versehentlich eine nicht-kanonische Kodierung
    erzeugt.
    """
    out = b""
    last = None
    for ty, value in fields:
        assert last is None or ty > last, f"TLV-Typen nicht aufsteigend: {last} -> {ty}"
        assert len(value) <= 0xFFFF
        out += bytes([ty]) + struct.pack(">H", len(value)) + value
        last = ty
    return out


def secret_block(enc_sk, sig_sk, created, label, pq_seed):
    fields = [(TAG_ENC_SK, enc_sk)]
    if sig_sk is not None:
        fields.append((TAG_SIG_SK, sig_sk))
    fields.append((TAG_CREATED, struct.pack(">Q", created)))
    if label is not None:
        fields.append((TAG_LABEL, label.encode("utf-8")))
    fields.append((TAG_PQ_SEED, pq_seed))
    return tlv(fields)


def build_keyfile(password, salt, m_cost, t_cost, p_cost,
                  enc_sk, sig_sk, created, label, pq_seed):
    assert p_cost == 1, "libsodium nutzt Argon2id immer mit Parallelitaet 1"

    head = (MAGIC
            + bytes([VERSION])
            + struct.pack(">I", m_cost)
            + struct.pack(">I", t_cost)
            + bytes([p_cost])
            + salt)
    assert len(head) == AAD_LEN, f"Kopf ist {len(head)} statt {AAD_LEN} Bytes"

    # m_cost steht in KiB, libsodium erwartet Bytes.
    kek = argon2id.kdf(32, password, salt,
                       opslimit=t_cost, memlimit=m_cost * 1024)

    plain = secret_block(enc_sk, sig_sk, created, label, pq_seed)
    ct = crypto_aead_chacha20poly1305_ietf_encrypt(plain, head, bytes(12), kek)

    return head + struct.pack(">I", len(ct)) + ct, kek


def b64(data):
    return base64.b64encode(data).decode("ascii")


def rep(byte, length):
    return bytes([byte]) * length


# --- Faelle -----------------------------------------------------------------

PASSWORD = b"korrekt-pferd-batterie-heftklammer"
SALT = bytes(range(16))
M_COST, T_COST, P_COST = 65536, 3, 1        # Mindestwerte -- Tests sollen schnell sein

cases = [
    ("kf-signing", "Identitaet mit Signierschluessel",
     rep(0x11, 32), rep(0x22, 32), 1_700_000_000, None, rep(0x33, 32)),
    ("kf-anonymous", "Anonymitaets-Identitaet ohne Signierschluessel",
     rep(0x44, 32), None, 1_700_000_001, None, rep(0x55, 32)),
    ("kf-labelled", "mit Bezeichnung, inkl. Nicht-ASCII",
     rep(0x66, 32), rep(0x77, 32), 1_700_000_002, "Arbeitsidentität ✱", rep(0x88, 32)),
]

vectors = []
for vid, desc, enc_sk, sig_sk, created, label, pq_seed in cases:
    data, kek = build_keyfile(PASSWORD, SALT, M_COST, T_COST, P_COST,
                              enc_sk, sig_sk, created, label, pq_seed)
    vectors.append({
        "id": vid,
        "description": desc,
        "input": {
            "password": PASSWORD.decode(),
            "salt_b64": b64(SALT),
            "m_cost": M_COST, "t_cost": T_COST, "p_cost": P_COST,
            "enc_sk_b64": b64(enc_sk),
            "sig_sk_b64": (b64(sig_sk) if sig_sk else None),
            "created": created,
            "label": label,
            "pq_seed_b64": b64(pq_seed),
        },
        "expected": {
            "keyfile_b64": b64(data),
            "keyfile_len": len(data),
            "kek_b64": b64(kek),
        },
    })
    print(f"  {vid:<14} {len(data):>4} Bytes   KEK {kek.hex()[:16]}...")

doc = {
    "spec_version": "2.0",
    "kind": "keyfile",
    "description": "Keyfile-Format v2 nach keyfile-v2.md §2 und §3.",
    "notes": [
        "Erzeugt mit libsodium (PyNaCl) fuer Argon2id und",
        "ChaCha20-Poly1305-IETF. Der Rust-Kern nutzt die RustCrypto-Crates --",
        "zwei getrennte Implementierungen.",
        "p_cost ist 1, weil libsodium Argon2id ausschliesslich mit",
        "Parallelitaet 1 ausfuehrt. Hoehere Werte sind im Format erlaubt,",
        "lassen sich mit dieser Referenz aber nicht gegenpruefen.",
        "m_cost und t_cost stehen auf den Mindestwerten, damit die Tests",
        "schnell bleiben. Beim Schreiben empfiehlt die Spec 262144 KiB.",
    ],
    "vectors": vectors,
}

with open("testvectors/keyfile.json", "w", encoding="utf-8", newline="\n") as f:
    json.dump(doc, f, indent=2, ensure_ascii=False)
    f.write("\n")

print(f"\n  {len(vectors)} Vektoren nach testvectors/keyfile.json")
