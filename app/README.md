# Frontend

Entsteht in **Phase 3**. Stack: Svelte 5 + TypeScript + Tailwind.

Wird zunächst **gegen Mock-Daten** gebaut, ohne jede Rust-Anbindung — die
Integration folgt erst in Phase 4. Grund: nie an zwei Unbekannten gleichzeitig
arbeiten.

## Architekturregel

Schlüsselmaterial verlässt Rust nicht. Das Frontend erhält ausschließlich
Handles, Statuswerte und Fortschritt — niemals Secrets. Das ist der eigentliche
Grund für Tauri statt Electron oder einer Web-App.

## Bildschirme

Onboarding · Identität/Schlüssel · Kontakte mit QR-Verifikation ·
Senden · Empfangen · Werkzeuge

Wireframes vor Code. Hier entsteht das, was das Produkt ansprechend macht —
nicht in der Krypto.
