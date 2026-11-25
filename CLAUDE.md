# SEBO (Self-hosted Bookmarks)

Repo: https://github.com/mr-glubschi/sebo · Image: `ghcr.io/mr-glubschi/sebo`

Selbst gehostete Bookmark-Verwaltung im Stil von Raindrop, bewusst auf das Minimum reduziert. Läuft als einzelner Docker-Container auf Unraid. Sprache: Deutsch.

## Grundsätze

- **Minimal bleiben.** Neue Funktionen nur nach Rücksprache. Kein Tagging, keine Volltextsuche, keine Notizen, keine Sync-Erweiterungen.
- **Ein Benutzer pro Container.** Mehrere Benutzer = mehrere Container. Kein Benutzerverwaltungs-Code.
- **Keine Seiteninhalte speichern.** Gespeichert werden nur Titel, URL, OG-Bild und Favicon. Kein Text, kein HTML, keine Archivierung.
- **Bilder:** OG-Bild zuerst, Favicon als Ausweichlösung und für Liste/Kompakt-Ansicht. Keine Screenshots per Headless-Browser (würde das Image stark vergrößern).
- **Import/Export nur im Netscape-HTML-Format.** Keine anbieterspezifischen Formate.
- **Muss am Handy funktionieren.** Jede Oberflächenänderung auch auf schmalen Bildschirmen prüfen.

## Sicherheit

- Login = Passwort aus Umgebungsvariable `PASSWORD` + TOTP.
- Der TOTP-QR-Code wird nur ins Docker-Log geschrieben, solange die Einrichtung nicht bestätigt ist. Nach der Bestätigung nie wieder.
- Zurücksetzen nur über `docker exec <container> reset-2fa`, nicht über die Weboberfläche.
- Wiederherstellungscodes und Geheimnisse niemals im Klartext loggen (Ausnahme: QR-Code bei der Ersteinrichtung).
- Passwortvergleich in konstanter Zeit. Fehlversuchssperre nicht entfernen.

## Technik

- Backend: Node.js 22, TypeScript, Fastify, SQLite (`better-sqlite3`)
- Frontend: Svelte + Vite, Suche mit Fuse.js im Browser
- Bilder: `sharp`, gespeichert als WebP unter `/data/images/`
- Alle persistenten Daten liegen unter `/data` (ein Volume)
- Container läuft als `99:100` (Unraid-Standard)

## Dokumentation

- `README.md`: Funktionsbeschreibung und Installation auf Unraid, für das GitHub-Repo. Bei neuen Funktionen oder Umgebungsvariablen aktualisieren.
- `Plan.md`: nur während der Erstumsetzung, wird danach gelöscht. Nichts Dauerhaftes dort ablegen.
