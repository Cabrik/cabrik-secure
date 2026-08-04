# Spezifikation

Entsteht in **Phase 1**. Wird geschrieben, *bevor* Rust-Code entsteht — das
Format muss über Desktop, iOS und Android identisch sein und jahrelang halten.

Geplante Dokumente:

| Datei | Inhalt |
|---|---|
| `envelope-v2.md` | HPKE (RFC 9180), Binärformat, Chunked Streaming, Mehrfachempfänger, Passwort-Modus |
| `keyfile-v2.md` | Argon2id-Parameter versioniert, Migration von v1 |
| `trust-store.md` | Kontaktspeicher, Fingerprint-Verifikation, die drei Signaturzustände im UI |
| `threat-model.md` | Wogegen Cabrik Secure schützt — und wogegen ausdrücklich nicht |

Sobald die Spec steht, ist sie eingefroren. Änderungen danach nur über eine
neue Formatversion mit explizitem Migrationspfad.
