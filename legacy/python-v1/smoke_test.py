"""Round-Trip-Nachweis fuer die v1-Referenzimplementierung nach dem Umzug."""
import os, sys, tempfile, traceback

from cabrik_secure.crypto_core import (
    generate_identity, save_identity_keyfile, load_identity_keyfile,
    encrypt_text, decrypt_text, encrypt_file, decrypt_file,
    secure_delete, b64e, VERSION,
)

PW = "smoke-test-passwort"
results = []


def check(name, fn):
    try:
        fn()
        results.append((name, True, ""))
        print(f"  [OK]   {name}")
    except Exception as e:
        results.append((name, False, str(e)))
        print(f"  [FAIL] {name}: {e}")
        traceback.print_exc()


tmp = tempfile.mkdtemp(prefix="cabrik_smoke_")
print(f"Arbeitsverzeichnis: {tmp}")
print(f"Core-Version: v{VERSION}\n")

state = {}


def t_keygen_signing():
    ident = generate_identity(anonymity=False)
    assert ident.sig_sk is not None, "Signierschluessel fehlt"
    path = os.path.join(tmp, "signing.json")
    save_identity_keyfile(ident, PW, path)
    state["signing_path"] = path
    state["signing_pub"] = b64e(bytes(ident.enc_pk))


def t_keyfile_roundtrip():
    ident = load_identity_keyfile(PW, state["signing_path"])
    assert b64e(bytes(ident.enc_pk)) == state["signing_pub"], "Public Key weicht ab"
    assert ident.sig_sk is not None, "Signierschluessel nach Laden weg"
    state["signing_ident"] = ident


def t_wrong_password_rejected():
    try:
        load_identity_keyfile("falsches-passwort", state["signing_path"])
    except Exception:
        return
    raise AssertionError("Falsches Passwort wurde akzeptiert")


def t_text_signed():
    msg = "Cabrik Secure – Umlaute äöü, Emoji 🔐, Zeilen\nzwei."
    env = encrypt_text(state["signing_pub"], msg, state["signing_ident"], anonymous=False)
    out, info = decrypt_text(state["signing_ident"], env, require_signature=True)
    assert out == msg, "Klartext weicht ab"
    assert info["signature_valid"] is True, "Signatur nicht verifiziert"


def t_text_anonymous():
    msg = "anonyme Nachricht"
    env = encrypt_text(state["signing_pub"], msg, None, anonymous=True)
    out, info = decrypt_text(state["signing_ident"], env, require_signature=True)
    assert out == msg, "Klartext weicht ab"
    assert info["signature_valid"] is True, "Ephemere Signatur nicht verifiziert"


def t_tampering_detected():
    env = encrypt_text(state["signing_pub"], "unveraendert", None, anonymous=True)
    import base64, json
    raw = json.loads(base64.b64decode(env).decode())
    raw["header"]["ts"] = raw["header"]["ts"] + 1          # Header manipulieren
    tampered = b64e(json.dumps(raw, separators=(",", ":")).encode())
    try:
        decrypt_text(state["signing_ident"], tampered, require_signature=False)
    except Exception:
        return
    raise AssertionError("Header-Manipulation wurde nicht erkannt")


def t_file_roundtrip():
    src = os.path.join(tmp, "nutzdaten.bin")
    payload = os.urandom(256 * 1024)
    with open(src, "wb") as f:
        f.write(payload)
    env = encrypt_file(state["signing_pub"], src, state["signing_ident"], anonymous=False)
    dst = os.path.join(tmp, "wieder.bin")
    info = decrypt_file(state["signing_ident"], env, dst, require_signature=True)
    with open(dst, "rb") as f:
        assert f.read() == payload, "Datei weicht ab"
    assert info["header"]["meta"]["filename"] == "nutzdaten.bin", "Metadaten fehlen"
    state["overhead"] = len(env) / len(payload)


def t_anonymity_keyfile():
    ident = generate_identity(anonymity=True)
    assert ident.sig_sk is None, "Anonym-Keyfile hat Signierschluessel"
    path = os.path.join(tmp, "anon.json")
    save_identity_keyfile(ident, PW, path)
    back = load_identity_keyfile(PW, path)
    assert back.sig_sk is None, "Signierschluessel nach Laden aufgetaucht"


def t_secure_delete():
    p = os.path.join(tmp, "weg.txt")
    with open(p, "w") as f:
        f.write("vertraulich")
    secure_delete(p, passes=2)
    assert not os.path.exists(p), "Datei existiert weiterhin"


print("Krypto-Kern:")
check("Keyfile mit Signierschluessel erzeugen", t_keygen_signing)
check("Keyfile laden (Round-Trip)", t_keyfile_roundtrip)
check("Falsches Passwort wird abgelehnt", t_wrong_password_rejected)
check("Text signiert verschluesseln/entschluesseln", t_text_signed)
check("Text anonym verschluesseln/entschluesseln", t_text_anonymous)
check("Header-Manipulation wird erkannt", t_tampering_detected)
check("Datei-Round-Trip (256 KB)", t_file_roundtrip)
check("Anonymitaets-Keyfile ohne Signierschluessel", t_anonymity_keyfile)
check("Secure Delete entfernt die Datei", t_secure_delete)

if "overhead" in state:
    print(f"\nEnvelope-Overhead: Faktor {state['overhead']:.3f} "
          f"(= {(state['overhead'] - 1) * 100:.1f} % Zuwachs)")

ok = sum(1 for _, p, _ in results if p)
print(f"\nErgebnis: {ok}/{len(results)} bestanden")
sys.exit(0 if ok == len(results) else 1)
