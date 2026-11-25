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
- Alle fremden Eingaben begrenzen (HTML 1 MB, Bild 5 MB + Pixel-Limit, SVG ohne externe Ressourcen, Import 20 MB / 50 000 Bookmarks / 10 000 Ordner).
- Fremdes HTML (Seiten, Importdateien) vor dem Parsen mit `meta::nesting_within` prüfen: html5ever braucht bei tiefer Verschachtelung quadratische Zeit, rekursiver Code würde den Stack sprengen. Von Seiten nur den Kopf (bis `<body`) parsen.
- Rechenintensive Arbeit nur über `worker::run_limited` (Platz bleibt belegt, bis der Thread fertig ist).
- `/share` darf nichts automatisch auslösen (keine Vorschau, kein Speichern): Fremde Seiten können dorthin navigieren.
- Datenordner 700, Dateien 600 (`umask 077` im Container): Die Datenbank enthält das 2FA-Geheimnis.
- Migrationen sind nach dem ersten Release unveränderlich, Änderungen immer als neue Migration.
- Strikte CSP, kein Inline-Skript, kein `{@html}` im Frontend. Bilder nur vom eigenen Server ausliefern, nie SVG.
- Titel und URLs stammen von fremden Seiten: im Frontend nur als Text ausgeben, nie als HTML. Links nur mit der vom Server geprüften URL und `rel="noopener noreferrer"`.
- Bilder werden erst unter temporärem Namen geschrieben und nur übernommen, wenn das Ergebnis gespeichert wurde (Bookmark nicht inzwischen gelöscht/geändert). Abrufe haben eine Gesamtfrist (30 s), Bildverarbeitung läuft begrenzt (2 gleichzeitig, 10 s). Nach 3 abgebrochenen Versuchen: `failed`.

## Technik

- Backend: Rust, Axum, SQLite über `tokio-rusqlite` (eine Verbindung, WAL), `rusqlite_migration`, `reqwest` + `rustls`, `scraper`, `image`, `resvg`, `totp-rs`, `tracing`
- Frontend: Svelte 5 + Vite + TypeScript als SPA (ohne SvelteKit), uFuzzy, einfaches CSS. PWA ohne Plugin: `web/public/manifest.webmanifest` und eigener `web/public/sw.js`, der nur `/assets/*` cacht (nie API oder Bilder). Bei Änderungen am Service Worker den Cache-Namen hochzählen.
- Drag & Drop selbst geschrieben (`web/src/lib/drag.svelte.ts`): Pointer Events, Touch nach langem Drücken, Ablageziele per `data-drop`. Natives Ziehen innerhalb der Seite ist unterbunden.
- Lange Listen werden schrittweise gerendert (`BookmarkView`, 150 pro Schritt). Nicht wieder alles auf einmal rendern, das kostet bei 5000 Bookmarks ein Vielfaches an Speicher.
- API-Typen werden mit `ts-rs` aus Rust erzeugt (`cargo test` schreibt sie nach `web/src/lib/bindings/`), nicht von Hand pflegen. Die CI prüft, dass sie aktuell sind. ts-rs ist nur Dev-Abhängigkeit (`cfg_attr(test, ...)`).
- Frontend wird mit `rust-embed` in das Programm eingebettet
- Bildabruf läuft im Hintergrund über `meta_status` in der DB (max. 4 parallel), übersteht Neustarts. Status `skipped` = interne Adresse, nicht abgerufen.
- Fremde URLs nur über `net::Fetcher` abrufen (SSRF-Schutz), nie direkt mit reqwest. TLS: rustls mit `ring` und `webpki-roots`.
- Test mit echtem Internet: `cargo test -- --ignored`
- Alle persistenten Daten liegen unter `/data` (ein Volume): `app.db`, `images/`, `backups/` (täglich `sebo-JJJJMMTT.db`, 7 Stände, `backup.rs`)
- Import/Export: `netscape.rs` (Parser und Export), `library::import` (eine Transaktion, gleichnamige Ordner zusammenführen, doppelte URLs überspringen)
- gzip nur für die Bibliotheks-API und statische Dateien, nicht für `/api/auth/*` (keine Geheimnisse neben reflektierten Daten komprimieren)
- Container: Alpine, Entrypoint setzt Rechte für `PUID`/`PGID` (Standard 99/100) und wechselt per `su-exec`. Muss mit `--read-only --cap-drop=ALL --cap-add=CHOWN,SETUID,SETGID --security-opt=no-new-privileges` laufen (so im Unraid-Template `unraid/sebo.xml`). Die CI prüft das mit `.github/scripts/smoke-test.sh`.
- Basis-Images im Dockerfile per Digest, Dependabot aktualisiert sie.
- Nur amd64, Image-Tags nach Semver über GitHub-Releases
- Kein Subpath-Betrieb

## Bewusste Entscheidungen

- Nicht umgesetzt: `PASSWORD_FILE`, iOS-Kurzbefehle mit API-Token, arm64-Image, manuelle Sortierung, Offline-Modus, Subpath-Betrieb. Lange Listen werden schrittweise gerendert statt virtuell gescrollt.
- Die Passwort-Variable bleibt im Prozess (Entfernen bringt nichts, `docker inspect` zeigt sie ohnehin). Wer Klartext vermeiden will, nutzt `PASSWORD_HASH`.
- `reset-2fa` zeigt den QR-Code in der Konsole des Befehls, nicht im Log.
- Wiederherstellungscodes: Crockford-Base32, 12 Zeichen, O/0 und I/L/1 werden bei der Eingabe korrigiert.
- rustls mit `ring` + `webpki-roots` (aws-lc-rs bräuchte CMake/NASM im Build).
- Status `skipped` = interne Adresse, gespeichert, aber nicht abgerufen. `title_auto` = Titel darf vom Abruf ersetzt werden; leerer Titel setzt sofort den Hostnamen.
- Import führt gleichnamige Ordner an gleicher Stelle zusammen und überspringt doppelte URLs, damit ein zweiter Import nichts verdoppelt. Nicht-UTF-8-Dateien werden als Windows-1252 gelesen.

## Leistung

- Richtwerte (Release-Build, 10 000 Bookmarks): 17–21 MB RAM, Import 0,1 s, `GET /api/library` 36 ms und 133 KB gzip, Frontend ~33 MB JS-Speicher bei 5000 Bookmarks. Bei Änderungen nicht deutlich verschlechtern.
- `GET /api/library` hat einen ETag; das Frontend schickt ihn beim Nachfragen (`If-None-Match`) und bekommt bei unveränderten Daten 304. Die Antwort bleibt `no-store`.
- Tokio mit 2–4 Arbeits-Threads, unabhängig von der Kernzahl des Servers.
- Vorschaubilder werden vor dem Verkleinern auf das Kartenformat 1,91:1 zugeschnitten (spart RAM beim Skalieren und Speicherplatz). Ändert sich das Kartenformat im Frontend, `CARD_RATIO` in `images.rs` anpassen.
- Der Hintergrund-Abruf füllt freie Plätze sofort nach (kein Warten auf ganze Stapel).

## Dokumentation

- `README.md`: Funktionsbeschreibung und Installation auf Unraid, für das GitHub-Repo. Bei neuen Funktionen oder Umgebungsvariablen aktualisieren.
- Lizenz: AGPL-3.0-only. Die Oberfläche muss einen sichtbaren Link zum Quellcode (GitHub-Repo) enthalten (AGPL §13).

## Entwicklung

- Aufbau: Rust-Backend im Projektwurzelverzeichnis (`src/`), Svelte-Frontend in `web/`, Docker-Dateien in `docker/`.
- Backend starten: `cargo run` (Daten landen dank `.cargo/config.toml` in `./data`).
- Frontend mit Hot Reload: `cd web && npm run dev`, leitet `/api` an `127.0.0.1:8080` weiter.
- Vor jedem Commit: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo deny check`, in `web/`: `npm run check`.
- TypeScript bleibt auf Version 6, bis `svelte-check` TypeScript 7 ohne Umwege unterstützt.
- Updates kommen über Dependabot. Release: Tag `vX.Y.Z` pushen, GitHub Actions baut und veröffentlicht das Image.
