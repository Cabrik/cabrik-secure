#!/usr/bin/env python3
import os, json, time, base64, hashlib
from dataclasses import dataclass
from typing import Optional, Tuple, Dict, Any

from nacl import public, signing, utils, exceptions
from nacl.bindings import (
    crypto_scalarmult,
    crypto_aead_xchacha20poly1305_ietf_encrypt as aead_xchacha_encrypt,
    crypto_aead_xchacha20poly1305_ietf_decrypt as aead_xchacha_decrypt,
)
from nacl.pwhash import argon2id

from cryptography.hazmat.primitives.kdf.hkdf import HKDF
from cryptography.hazmat.primitives import hashes

VERSION = 1
BRANDING = "Cabrik Secure"
ALG = {
    "kdf": "HKDF-SHA256",
    "aead": "XChaCha20-Poly1305",
    "kdf_keys": "argon2id",
    "asym": "X25519",
    "sig": "Ed25519",
}

# ---------- helpers ----------

def b64e(b: bytes) -> str:
    return base64.b64encode(b).decode("ascii")

def b64d(s: str) -> bytes:
    return base64.b64decode(s.encode("ascii"))

def fingerprint(pubkey_bytes: bytes, length=8) -> str:
    h = hashlib.sha256(pubkey_bytes).hexdigest()
    return f"{h[:length]}"

def hkdf_sha256(key_material: bytes, info: bytes, length: int = 32, salt: Optional[bytes]=None) -> bytes:
    hkdf = HKDF(algorithm=hashes.SHA256(), length=length, salt=salt, info=info)
    return hkdf.derive(key_material)

# ---------- identities / keyfiles ----------

@dataclass
class Identity:
    enc_sk: public.PrivateKey        # X25519 (ECDH)
    enc_pk: public.PublicKey
    sig_sk: Optional[signing.SigningKey]  # Ed25519 (None im Anonymitätsmodus)
    sig_vk: Optional[signing.VerifyKey]

def generate_identity(anonymity: bool = False) -> Identity:
    enc_sk = public.PrivateKey.generate()
    enc_pk = enc_sk.public_key
    if anonymity:
        return Identity(enc_sk=enc_sk, enc_pk=enc_pk, sig_sk=None, sig_vk=None)
    sig_sk = signing.SigningKey.generate()
    sig_vk = sig_sk.verify_key
    return Identity(enc_sk=enc_sk, enc_pk=enc_pk, sig_sk=sig_sk, sig_vk=sig_vk)

def _argon2id_kdf_key(password: str, salt: bytes, length: int = 32) -> bytes:
    return argon2id.kdf(
        size=length,
        password=password.encode("utf-8"),
        salt=salt,
        opslimit=argon2id.OPSLIMIT_MODERATE,
        memlimit=argon2id.MEMLIMIT_MODERATE,
    )

def save_identity_keyfile(identity: Identity, password: str, path: str) -> None:
    salt = utils.random(16)
    kek = _argon2id_kdf_key(password, salt, 32)

    enc_sk_bytes = bytes(identity.enc_sk)
    sig_sk_bytes = bytes(identity.sig_sk) if identity.sig_sk else None

    aad = f"cabrik-keyfile|v{VERSION}|{BRANDING}".encode()
    n1 = utils.random(24)
    enc_priv_ct = aead_xchacha_encrypt(enc_sk_bytes, aad, n1, kek)

    if sig_sk_bytes:
        n2 = utils.random(24)
        sig_priv_ct = aead_xchacha_encrypt(sig_sk_bytes, aad, n2, kek)
    else:
        n2 = b""
        sig_priv_ct = b""

    keyfile = {
        "version": VERSION,
        "branding": BRANDING,
        "alg": ALG,
        "created": int(time.time()),
        "enc_pub": b64e(bytes(identity.enc_pk)),
        "sig_pub": b64e(bytes(identity.sig_vk)) if identity.sig_vk else None,
        "salt": b64e(salt),
        "enc_priv": {"nonce": b64e(n1), "ciphertext": b64e(enc_priv_ct)},
        "sig_priv": {
            "nonce": b64e(n2) if sig_priv_ct else None,
            "ciphertext": b64e(sig_priv_ct) if sig_priv_ct else None,
        },
    }
    with open(path, "w", encoding="utf-8") as f:
        json.dump(keyfile, f, indent=2)

def load_identity_keyfile(password: str, path: str) -> Identity:
    with open(path, "r", encoding="utf-8") as f:
        kf = json.load(f)
    salt = b64d(kf["salt"])
    kek = _argon2id_kdf_key(password, salt, 32)
    aad = f"cabrik-keyfile|v{kf['version']}|{kf.get('branding', BRANDING)}".encode()

    n1 = b64d(kf["enc_priv"]["nonce"])
    c1 = b64d(kf["enc_priv"]["ciphertext"])
    enc_sk_bytes = aead_xchacha_decrypt(c1, aad, n1, kek)
    enc_sk = public.PrivateKey(enc_sk_bytes)
    enc_pk = enc_sk.public_key

    sig_priv = kf.get("sig_priv", {})
    sig_sk = None
    sig_vk = None
    if sig_priv and sig_priv.get("ciphertext"):
        n2 = b64d(sig_priv["nonce"])
        c2 = b64d(sig_priv["ciphertext"])
        sig_sk_bytes = aead_xchacha_decrypt(c2, aad, n2, kek)
        sig_sk = signing.SigningKey(sig_sk_bytes)
        sig_vk = sig_sk.verify_key

    return Identity(enc_sk=enc_sk, enc_pk=enc_pk, sig_sk=sig_sk, sig_vk=sig_vk)

# ---------- session key & aead ----------

def _derive_session_key(sender_sk: public.PrivateKey, recipient_pk: public.PublicKey, context: Dict[str, Any]) -> Tuple[bytes, bytes]:
    shared = crypto_scalarmult(bytes(sender_sk), bytes(recipient_pk))
    salt = utils.random(16)
    info = f"cabrik|v{VERSION}|{ALG['asym']}|{ALG['aead']}|{context.get('purpose','msg')}".encode()
    session_key = hkdf_sha256(shared, info=info, length=32, salt=salt)
    return session_key, salt

def _aead_encrypt(session_key: bytes, plaintext: bytes, aad: bytes) -> Tuple[bytes, bytes]:
    nonce = utils.random(24)
    ciphertext = aead_xchacha_encrypt(plaintext, aad, nonce, session_key)
    return nonce, ciphertext

def _aead_decrypt(session_key: bytes, nonce: bytes, ciphertext: bytes, aad: bytes) -> bytes:
    return aead_xchacha_decrypt(ciphertext, aad, nonce, session_key)

# ---------- high-level payload ----------

def encrypt_payload(
    recipient_enc_pub_b64: str,
    plaintext: bytes,
    sender_identity: Optional[Identity] = None,
    anonymous: bool = False,
    attach_sender_sign_pub: bool = True,
    purpose: str = "msg",
    extra_aad: Optional[Dict[str, str]] = None,
) -> bytes:
    recipient_pk = public.PublicKey(b64d(recipient_enc_pub_b64))

    eph_sk = public.PrivateKey.generate()
    eph_pk = eph_sk.public_key

    sess_key, hkdf_salt = _derive_session_key(eph_sk, recipient_pk, {"purpose": purpose})

    sign_pub_b64 = None
    sig_b64 = None
    if not anonymous and sender_identity and sender_identity.sig_sk:
        signer = sender_identity.sig_sk
        sign_pub_b64 = b64e(bytes(signer.verify_key))
    elif anonymous:
        signer = signing.SigningKey.generate()  # ephemerer Sign-Schlüssel
        sign_pub_b64 = b64e(bytes(signer.verify_key))
    else:
        signer = None

    header = {
        "version": VERSION,
        "branding": BRANDING,
        "alg": ALG,
        "purpose": purpose,
        "ts": int(time.time()),
        "recipient_fp": fingerprint(bytes(recipient_pk)),
        "eph_pub": b64e(bytes(eph_pk)),
        "hkdf_salt": b64e(hkdf_salt),
        "sender_sig_pub": sign_pub_b64 if attach_sender_sign_pub and sign_pub_b64 else None,
    }
    if extra_aad:
        header["meta"] = extra_aad

    aad = json.dumps(header, separators=(",", ":"), sort_keys=True).encode("utf-8")
    nonce, ct = _aead_encrypt(sess_key, plaintext, aad)

    if signer:
        to_sign = nonce + ct + hashlib.sha256(aad).digest()
        sig = signer.sign(to_sign).signature
        sig_b64 = b64e(sig)

    envelope = {"header": header, "nonce": b64e(nonce), "ciphertext": b64e(ct), "signature": sig_b64}
    return b64e(json.dumps(envelope, separators=(",", ":")).encode("utf-8"))

def decrypt_payload(
    recipient_identity: Identity,
    envelope_b64: bytes,
    require_signature: bool = False,
) -> Tuple[bytes, Dict[str, Any]]:
    envelope = json.loads(base64.b64decode(envelope_b64).decode("utf-8"))
    header = envelope["header"]
    nonce = b64d(envelope["nonce"])
    ct = b64d(envelope["ciphertext"])
    sig_b64 = envelope.get("signature")

    eph_pub = public.PublicKey(b64d(header["eph_pub"]))
    hkdf_salt = b64d(header["hkdf_salt"])
    shared = crypto_scalarmult(bytes(recipient_identity.enc_sk), bytes(eph_pub))
    info = f"cabrik|v{header['version']}|{ALG['asym']}|{ALG['aead']}|{header.get('purpose','msg')}".encode()
    sess_key = hkdf_sha256(shared, info=info, length=32, salt=hkdf_salt)

    aad = json.dumps(header, separators=(",", ":"), sort_keys=True).encode("utf-8")
    pt = _aead_decrypt(sess_key, nonce, ct, aad)

    verified = False
    signer_fp = None
    if sig_b64:
        sig = b64d(sig_b64)
        spub_b64 = header.get("sender_sig_pub")
        if spub_b64:
            vk = signing.VerifyKey(b64d(spub_b64))
            to_verify = nonce + ct + hashlib.sha256(aad).digest()
            try:
                vk.verify(to_verify, sig)
                verified = True
                signer_fp = fingerprint(bytes(vk))
            except exceptions.BadSignatureError:
                raise ValueError("Signaturprüfung fehlgeschlagen.")
        else:
            raise ValueError("Signatur vorhanden, aber kein sender_sig_pub im Header.")
    elif require_signature:
        raise ValueError("Signatur wird verlangt, aber nicht vorhanden.")

    info_out = {"header": header, "signature_valid": verified, "sender_sig_fingerprint": signer_fp}
    return pt, info_out

# ---------- convenience text / files ----------

def encrypt_text(recipient_enc_pub_b64: str, text: str, sender: Optional[Identity], anonymous: bool) -> str:
    return encrypt_payload(recipient_enc_pub_b64, text.encode("utf-8"), sender, anonymous, purpose="text")

def decrypt_text(recipient: Identity, envelope_b64: str, require_signature: bool = False):
    pt, info = decrypt_payload(recipient, envelope_b64.encode("ascii"), require_signature=require_signature)
    return pt.decode("utf-8"), info

def encrypt_file(recipient_enc_pub_b64: str, in_path: str, sender: Optional[Identity], anonymous: bool) -> str:
    with open(in_path, "rb") as f:
        data = f.read()
    extra = {"filename": os.path.basename(in_path), "size": str(len(data))}
    return encrypt_payload(recipient_enc_pub_b64, data, sender, anonymous, purpose="file", extra_aad=extra)

def decrypt_file(recipient: Identity, envelope_b64: str, out_path: str, require_signature: bool = False):
    pt, info = decrypt_payload(recipient, envelope_b64.encode("ascii"), require_signature=require_signature)
    with open(out_path, "wb") as f:
        f.write(pt)
    return info

# ---------- metadata inspect & strip ----------

def inspect_metadata(path: str) -> Dict[str, Any]:
    out: Dict[str, Any] = {"file": os.path.basename(path), "type": None, "metadata": {}}
    lower = path.lower()

    try:
        if lower.endswith((".jpg", ".jpeg", ".tiff", ".png", ".webp")):
            out["type"] = "image"
            try:
                from PIL import Image
                import piexif
                with Image.open(path) as im:
                    exif = im.info.get("exif")
                if exif:
                    exif_dict = piexif.load(exif)
                    meta = {}
                    for ifd in ("0th", "Exif", "GPS", "1st"):
                        for k, v in exif_dict.get(ifd, {}).items():
                            meta[f"{ifd}:{k}"] = str(v)[:200]
                    out["metadata"] = meta
            except Exception:
                pass

        elif lower.endswith(".pdf"):
            out["type"] = "pdf"
            try:
                import pikepdf
                with pikepdf.open(path) as pdf:
                    info = pdf.docinfo
                    meta = {k: str(v) for k, v in info.items()}
                    out["metadata"] = meta
            except Exception:
                pass

        elif lower.endswith((".docx",)):
            out["type"] = "docx"
            try:
                import docx
                d = docx.Document(path)
                core = d.core_properties
                meta = {
                    "author": core.author,
                    "created": str(core.created),
                    "last_modified_by": core.last_modified_by,
                    "last_printed": str(core.last_printed),
                    "modified": str(core.modified),
                    "title": core.title,
                    "subject": core.subject,
                }
                out["metadata"] = {k: v for k, v in meta.items() if v}
            except Exception:
                pass
    except Exception:
        pass

    warns = []
    if out["type"] == "image" and any("GPS" in k for k in out["metadata"].keys()):
        warns.append("GPS-Daten gefunden (Lokation).")
    if out["type"] == "pdf" and any(k.lower() in ("/author", "/creator", "/producer") for k in out["metadata"].keys()):
        warns.append("PDF-Autoren/Ersteller-Metadaten gefunden.")
    if out["type"] == "docx" and out["metadata"].get("author"):
        warns.append("DOCX-Autor/Ersteller gefunden.")

    if warns:
        out["warnings"] = warns
    return out

def strip_metadata(path: str, out_path: Optional[str] = None) -> str:
    lower = path.lower()
    if out_path is None:
        base, ext = os.path.splitext(path)
        out_path = f"{base}.clean{ext}"

    if lower.endswith((".jpg", ".jpeg", ".tiff", ".png", ".webp")):
        from PIL import Image
        import piexif
        with Image.open(path) as im:
            data = list(im.getdata())
            mode = im.mode
            size = im.size
        im2 = Image.new(mode, size)
        im2.putdata(data)
        im2.save(out_path)
        if out_path.lower().endswith((".jpg", ".jpeg", ".tiff")):
            piexif.remove(out_path)

    elif lower.endswith(".pdf"):
        import pikepdf
        with pikepdf.open(path) as pdf:
            pdf.docinfo.clear()
            pdf.save(out_path)

    elif lower.endswith(".docx"):
        import docx
        d = docx.Document(path)
        d.core_properties.author = None
        d.core_properties.title = None
        d.core_properties.subject = None
        d.core_properties.last_modified_by = None
        d.core_properties.comments = None
        d.save(out_path)
    else:
        import shutil
        shutil.copy2(path, out_path)

    return out_path

# ---------- secure delete ----------

def secure_delete(path: str, passes: int = 3) -> None:
    if not os.path.isfile(path):
        return
    size = os.path.getsize(path)
    try:
        with open(path, "r+b", buffering=0) as f:
            for _ in range(passes):
                f.seek(0)
                f.write(os.urandom(size))
                f.flush()
                os.fsync(f.fileno())
        with open(path, "r+b", buffering=0) as f:
            f.seek(0)
            f.write(b"\x00" * size)
            f.flush()
            os.fsync(f.fileno())
    except Exception:
        pass
    try:
        os.remove(path)
    except Exception:
        pass
