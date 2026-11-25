# SEBO (Self-hosted Bookmarks)

Repo: https://github.com/mr-glubschi/sebo · Image: `ghcr.io/mr-glubschi/sebo`

Selbst gehostete Bookmark-Verwaltung im Stil von Raindrop, bewusst auf das Minimum reduziert. Läuft als einzelner Docker-Container auf Unraid. Sprache: Deutsch.

## Grundsätze

- **Minimal bleiben.** Neue Funktionen nur nach Rücksprache. Kein Tagging, keine Volltextsuche, keine Notizen, keine Sync-Erweiterungen, keine manuelle Sortierung, kein Offline-Modus.
- **Ein Benutzer pro Container.** Mehrere Benutzer = mehrere Container. Kein Benutzerverwaltungs-Code.
- **Keine Seiteninhalte speichern.** Gespeichert werden nur Titel, URL, OG-Bild und Favicon. Kein Text, kein HTML, keine Archivierung.
- **Bilder:** OG-Bild zuerst, Favicon als Ausweichlösung und für Liste/Kompakt-Ansicht. Keine Screenshots per Headless-Browser. Gespeichert als JPEG (OG) und PNG (Favicon), immer neu kodiert.
- **Import/Export nur im Netscape-HTML-Format.** Keine anbieterspezifischen Formate.
- **Muss am Handy funktionieren.** Jede Oberflächenänderung auch auf schmalen Bildschirmen und per Touch prüfen.
- **Wenige Abhängigkeiten.** Bei Crates nur die nötigen Features aktivieren. Kleine Dinge (Sessions, Routing, Drag & Drop) lieber selbst schreiben als eine Bibliothek dazunehmen.

## Sicherheit

- Login = Passwort (`PASSWORD` oder argon2-`PASSWORD_HASH`) + TOTP. Passwort unter 12 Zeichen → Container startet nicht.
- Der TOTP-QR-Code wird nur ins Log geschrieben, solange die Einrichtung nicht bestätigt ist. Nach der Bestätigung nie wieder.
- Zurücksetzen nur über `docker exec <container> reset-2fa`, nicht über die Weboberfläche.
- Geheimnisse und Codes niemals im Klartext loggen (Ausnahme: QR-Code bei der Ersteinrichtung).
- Vergleiche in konstanter Zeit (`subtle`). In der DB nur Hashes von Session-Tokens und Wiederherstellungscodes.
- Fehlversuche: globale, ansteigende Wartezeit. Nicht entfernen, nicht auf IP-basiert umstellen. Fehlversuche mit richtigem Passwort und falschem Code zählen zusätzlich 24 h lang und werden durch einen erfolgreichen Login nicht zurückgesetzt (Schutz gegen langsames Durchprobieren der TOTP-Codes).
- Login: Bei falschem Passwort läuft eine Scheinprüfung des Codes (gleiche Antwortzeit). Codeprüfung und Anlegen der Sitzung in einer Transaktion.
- Bei HTTPS nur das `__Host-`-Cookie akzeptieren. Optionale `ALLOWED_HOSTS` gegen DNS-Rebinding.
- Session-Cookie `SameSite=Lax` + `Origin`-Prüfung bei ändernden Anfragen (nicht `Strict`, sonst bricht das Öffnen per Link und Share Target).
- **SSRF:** Jeder Abruf fremder URLs geht durch die zentrale Abruf-Funktion mit IP-Prüfung nach DNS-Auflösung, festgelegter IP und einzeln geprüften Weiterleitungen. Interne Adressen nur mit `ALLOW_PRIVATE_URLS=true`. Bookmarks auf interne Adressen dürfen trotzdem immer gespeichert werden.
- Alle fremden Eingaben begrenzen (HTML 1 MB, Bild 5 MB + Pixel-Limit, SVG ohne externe Ressourcen, Importgröße).
- Strikte CSP, kein Inline-Skript, kein `{@html}` im Frontend. Bilder nur vom eigenen Server ausliefern, nie SVG.

## Technik

- Backend: Rust, Axum, SQLite über `tokio-rusqlite` (eine Verbindung, WAL), `rusqlite_migration`, `reqwest` + `rustls`, `scraper`, `image`, `resvg`, `totp-rs`, `tracing`
- Frontend: Svelte 5 + Vite + TypeScript als SPA (ohne SvelteKit), uFuzzy, einfaches CSS, vite-plugin-pwa
- API-Typen werden mit `ts-rs` aus Rust erzeugt (`cargo test` schreibt sie nach `web/src/lib/bindings/`), nicht von Hand pflegen. Die CI prüft, dass sie aktuell sind. ts-rs ist nur Dev-Abhängigkeit (`cfg_attr(test, ...)`).
- Frontend wird mit `rust-embed` in das Programm eingebettet
- Bildabruf läuft im Hintergrund über `meta_status` in der DB (max. 4 parallel), übersteht Neustarts. Status `skipped` = interne Adresse, nicht abgerufen.
- Fremde URLs nur über `net::Fetcher` abrufen (SSRF-Schutz), nie direkt mit reqwest. TLS: rustls mit `ring` und `webpki-roots`.
- Test mit echtem Internet: `cargo test -- --ignored`
- Alle persistenten Daten liegen unter `/data` (ein Volume)
- Container: Alpine, Entrypoint setzt Rechte für `PUID`/`PGID` (Standard 99/100) und wechselt per `su-exec`
- Nur amd64, Image-Tags nach Semver über GitHub-Releases
- Kein Subpath-Betrieb

## Dokumentation

- `README.md`: Funktionsbeschreibung und Installation auf Unraid, für das GitHub-Repo. Bei neuen Funktionen oder Umgebungsvariablen aktualisieren.
- Lizenz: AGPL-3.0-only. Die Oberfläche muss einen sichtbaren Link zum Quellcode (GitHub-Repo) enthalten (AGPL §13).
- `Plan.md`: nur während der Erstumsetzung, wird danach gelöscht. Nichts Dauerhaftes dort ablegen.

## Entwicklung

- Aufbau: Rust-Backend im Projektwurzelverzeichnis (`src/`), Svelte-Frontend in `web/`, Docker-Dateien in `docker/`.
- Backend starten: `cargo run` (Daten landen dank `.cargo/config.toml` in `./data`).
- Frontend mit Hot Reload: `cd web && npm run dev`, leitet `/api` an `127.0.0.1:8080` weiter.
- Vor jedem Commit: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo deny check`, in `web/`: `npm run check`.
- TypeScript bleibt auf Version 6, bis `svelte-check` TypeScript 7 ohne Umwege unterstützt.
- Updates kommen über Dependabot. Release: Tag `vX.Y.Z` pushen, GitHub Actions baut und veröffentlicht das Image.
