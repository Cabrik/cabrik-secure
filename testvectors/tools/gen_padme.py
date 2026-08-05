import json
PAD_MIN = 256
def padme(L):
    if L <= PAD_MIN: return PAD_MIN
    E = L.bit_length() - 1
    S = (E.bit_length() - 1) + 1
    z = E - S
    mask = (1 << z) - 1
    return (L + mask) & ~mask

cases = [
    (0,      "leerer Klartext"),
    (1,      "ein Byte"),
    (255,    "knapp unter PAD_MIN"),
    (256,    "genau PAD_MIN"),
    (257,    "knapp ueber PAD_MIN"),
    (1000,   "Beispiel aus envelope-v2 §10.2"),
    (1024,   "Zweierpotenz, bereits ausgerichtet"),
    (1025,   "knapp ueber Zweierpotenz"),
    (10000,  "Beispiel aus envelope-v2 §10.2"),
    (32769,  "groesster Verschnitt (6,25 %)"),
    (65536,  "Chunk-Grenze"),
    (65537,  "knapp ueber Chunk-Grenze"),
    (1000000,   "Beispiel aus envelope-v2 §10.2"),
    (10000000,  "Beispiel aus envelope-v2 §10.2"),
    (2**32,     "4 GiB"),
    (2**40 + 1, "1 TiB + 1"),
]
vectors = []
for L, desc in cases:
    r = padme(L)
    vectors.append({
        "id": f"padme-{L}",
        "description": desc,
        "input": {"plaintext_size": L},
        "expected": {"padded_size": r, "padding_len": r - L},
    })

doc = {
    "spec_version": "2.0",
    "kind": "padme",
    "description": "PADME nach envelope-v2.md §10.2. PAD_MIN = 256.",
    "notes": [
        "floor(log2(x)) MUSS als Ganzzahloperation berechnet werden.",
        "Gleitkomma-log2 liefert an Zweierpotenzen plattformabhaengige",
        "Ergebnisse und wuerde diese Vektoren brechen."
    ],
    "vectors": vectors,
}
with open("testvectors/padme.json", "w", encoding="utf-8", newline="\n") as f:
    json.dump(doc, f, indent=2, ensure_ascii=False)
    f.write("\n")

for v in vectors:
    L = v["input"]["plaintext_size"]; r = v["expected"]["padded_size"]
    ov = f"{(r-L)/L*100:.2f} %" if L else "—"
    print(f"{L:>13} -> {r:>13}  {ov:>9}   {v['description']}")
