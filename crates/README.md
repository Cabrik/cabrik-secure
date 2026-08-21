# Rust-Crates

Entstehen in **Phase 2** (Core und CLI) sowie **Phase 4** (Tauri-Backend).

| Crate | Rolle | Lizenz |
|---|---|---|
| `cabrik-core` | Krypto-Kern: HPKE, Streaming, Keyfiles, Trust Store | Apache-2.0 |
| `cabrik-metadata` | Erkennen und Entfernen von Metadaten in Nutzdateien | Apache-2.0 |
| `cabrik-shred` | Sicheres Löschen mit ehrlichen Garantien | Apache-2.0 |
| `cabrik-ablage` | Wo die Dateien liegen und wie sie geschrieben werden | Apache-2.0 |
| `cabrik-speicher` | Speicher, den das Betriebssystem nicht auslagern darf | Apache-2.0 |
| `cabrik-app` | Befehlsschicht über dem Kern | proprietär |
| `cabrik-bruecke` | Der Vertrag zwischen Rust und der Oberfläche | proprietär |
| `cabrik-cli` | Referenz-CLI (`cabrik`), deckt den Core vollständig ab | proprietär |
| `cabrik-fenster` | Die Tauri-Fensterhülle | proprietär |
| `cabrik-v1` | Lesezugriff auf das alte Format v1 (Migration) | proprietär |

Die Spalte hieß einmal „Lizenz (geplant)" und stand bei jeder Kiste auf
„quelloffen". Entschieden ist es seit dem 19.08.2026, und die Entscheidung
fiel anders: Quelloffen ist, was Sicherheit **zusagt** — der Grund dafür
steht im `README.md` der Wurzel.

`cabrik-metadata` ist aus demselben Grund eigenständig wie `cabrik-v1`, nur
mit noch mehr Gewicht: Metadaten-Bereinigung heißt Parser für viele
Dateiformate, und Parser sind Angriffsfläche.

`cabrik-shred` ist eigenständig, weil es als einziger Teil des Systems
**Dateien zerstört**. Diese Trennung hält den Kern frei von einer Fähigkeit,
die dort nichts zu suchen hat, und macht die Prüfung des heiklen Codes
überschaubar. Es hat keine Abhängigkeit auf Krypto — nur auf `getrandom`.

`cabrik-cli` ist mehr als ein Werkzeug: Sie ist der **erste echte Aufrufer**
des Kerns und deckt ihn deshalb vollständig ab. Modultests rufen Funktionen so
auf, wie die Autorin sie gedacht hat; eine Bedienoberfläche muss sie so
aufrufen, wie ein Mensch sie braucht. Jeder schwere Entwurfsfehler dieses
Projekts kam an genau dieser Naht heraus — zuletzt vier Stück auf einmal,
siehe `docs/ROADMAP.md` 2.11. Sie kommt deshalb bewusst **vor** dem Frontend.

Als einzige Kiste kennt sie Dateien und Pfade: Der Kern verarbeitet nur Bytes,
damit er per UniFFI nach Swift und Kotlin gehen kann. Auch die Verschlüsselung
des Kontaktspeichers liegt hier, nicht im Kern.

`cabrik-speicher` ist die **einzige Kiste, in der `unsafe` erlaubt ist** —
und der Grund, warum sie überhaupt eigenständig ist. Sie tut genau eine
Sache: die Speicherseiten mit dem Passwortpuffer festnageln, damit sie nicht
ausgelagert werden. Das sind vier Systemaufrufe, und Systemaufrufe gehen
nicht ohne `unsafe`.

Sie einzeln zu halten ist der ganze Zweck: So bleibt `forbid` in den acht
anderen stehen, und wer die `unsafe`-Stellen dieses Programms prüfen will,
liest eine Datei statt neun Kisten. Dass ihre Regelliste nicht von der des
Arbeitsbereichs abdriftet, bewacht `tests/gleichlauf.rs`.

`cabrik-v1` ist bewusst eigenständig: v1 ist JSON über Base64, und beides
soll nicht in den auditierten Kern, der per UniFFI nach iOS und Android
geht. Der Inhalt ist eingefroren — einmal geschrieben, gegen die
Referenzimplementierung geprüft, danach unverändert.

## Verbindliche Standards

- `unsafe_code = "forbid"` im ganzen Arbeitsbereich, mit **einer**
  benannten Ausnahme: `cabrik-speicher` steht auf `deny` und hebt es an
  einzeln benannten Stellen auf, jede mit ihrer Begründung darüber. Wie
  viele es sind, zählt ein Test nach — eine mehr ist damit eine
  Entscheidung und kein Nebenprodukt
- `zeroize` für sämtliches Schlüsselmaterial
- `cargo fuzz` auf dem Envelope-Parser
- `cargo deny` für Lizenz- und CVE-Prüfung
- Differenztests gegen `legacy/python-v1` in CI

## Reihenfolge der Implementierung

Folgt bewusst der Rust-Lernkurve, nicht der fachlichen Logik:
Helfer → Keyfile → HPKE Single-Shot → Streaming → Mehrfachempfänger.
Streaming kommt spät, weil dort Lifetimes und Iteratoren zusammenkommen.
