# Umsetzungsplan

> Temporäre Datei. Wird nach Abschluss der Umsetzung gelöscht. Dauerhaft Wichtiges steht in `CLAUDE.md` (für die Entwicklung) und `README.md` (für Nutzer).

Name: **SEBO** (Self-hosted Bookmarks), Repo `mr-glubschi/sebo`

## Ziel

Schlanke, selbst gehostete Bookmark-Verwaltung im Stil von Raindrop, reduziert auf das Minimum. Läuft als einzelner Docker-Container auf Unraid, ein Benutzer pro Container.

## Funktionsumfang

1. **Login mit Passwort + 2FA (TOTP)**
   - Passwort kommt aus der Umgebungsvariable `PASSWORD` (Unraid-Template, Feld maskiert).
   - Beim ersten Start: TOTP-Geheimnis erzeugen, QR-Code als ASCII ins Docker-Log schreiben.
   - QR-Code erscheint nur, bis die Einrichtung im Browser mit einem gültigen Code bestätigt wurde.
   - Nach der Bestätigung: 10 Wiederherstellungscodes einmalig anzeigen (gehasht gespeichert, je einmal nutzbar).
   - `docker exec <container> reset-2fa` setzt 2FA zurück, danach erscheint ein neuer QR-Code im Log.
   - Sperre nach mehreren Fehlversuchen (z. B. 5 Versuche → 15 Minuten Pause, ansteigend).
   - Session-Cookie: `HttpOnly`, `SameSite=Strict`, `Secure` wenn per HTTPS aufgerufen. Gültig 30 Tage.
2. **Bookmarks hinzufügen**
   - Drag & Drop eines Links auf die Seite (`text/uri-list` bzw. `text/plain`). Ablage auf einem Ordner speichert direkt dort.
   - „+“-Button mit Formular: URL, Name, Ordner. Titel und Bild werden aus der URL vorausgefüllt, alles überschreibbar.
   - Server holt Metadaten: `<title>` / `og:title`, `og:image`, Favicon (`<link rel="icon">`, sonst `/favicon.ico`).
   - Kein Seitentext und kein HTML wird gespeichert, nur Titel, URL und Bilder.
3. **Ansichten**
   - Karten: OG-Bild groß, darunter Favicon, Name und Domain.
   - Liste: Favicon, Name, URL und Ordner in einer Zeile.
   - Kompakt: nur Favicon und Name.
   - Ausweichkette für Bilder: OG-Bild → Favicon groß → farbiger Anfangsbuchstabe.
   - Gewählte Ansicht wird im Browser gemerkt.
4. **Ordner mit Unterordnern**
   - Ordnerbaum in der Seitenleiste, beliebig tief.
   - Ordner anlegen, umbenennen, löschen (mit Rückfrage: Inhalt mitlöschen oder in den übergeordneten Ordner verschieben).
   - Bookmarks und Ordner per Drag & Drop verschieben.
   - Bookmarks ohne Ordner landen in „Unsortiert“.
5. **Bearbeiten, Löschen, Live-Suche**
   - Name, URL, Ordner bearbeiten; Vorschaubild neu laden oder entfernen.
   - Unscharfe Suche über Name und URL, Treffer erscheinen beim Tippen.
   - Alle Bookmarks werden beim Start in den Browser geladen und dort durchsucht.
6. **HTML-Import und -Export**
   - Netscape-Bookmark-Format (Standard aller Browser und von Raindrop).
   - Ordnerstruktur bleibt erhalten. Beim Import werden Bilder im Hintergrund nachgeladen.
   - Doppelte URLs beim Import überspringen.
7. **Handy**
   - Responsive Oberfläche, Seitenleiste wird zum ausklappbaren Menü.
   - Installierbar als PWA (Manifest + Service Worker).
   - Android: Web Share Target, Links über „Teilen“ direkt speichern.
   - iPhone: kein Share Target möglich; „+“-Dialog mit Einfügen-Button aus der Zwischenablage.

## Technik

| Bereich | Wahl |
|---|---|
| Laufzeit | Node.js 22 LTS, TypeScript |
| Webserver | Fastify |
| Datenbank | SQLite (`better-sqlite3`), Datei unter `/data/app.db` |
| 2FA | `otplib` (TOTP), `qrcode-terminal` für den QR-Code im Log |
| Metadaten | `fetch` + `cheerio`, nur der `<head>` wird ausgewertet |
| Bilder | `sharp`, Umwandlung in WebP, OG-Bild max. 600 px breit, Favicon 64 px |
| Frontend | Svelte + Vite, als statische Dateien vom Fastify-Server ausgeliefert |
| Suche | Fuse.js im Browser |
| Container | `node:22-alpine`, Multi-Stage-Build, läuft als `99:100` (Unraid nobody:users) |
| Veröffentlichung | GitHub Actions baut das Image und schiebt es nach GHCR |

### Datenmodell

```
folders    id, parent_id (NULL = oberste Ebene), name, position, created_at
bookmarks  id, folder_id (NULL = Unsortiert), title, url, image, favicon, position, created_at, updated_at
settings   key, value   (totp_secret, totp_confirmed, recovery_codes, ...)
sessions   id, expires_at
```

Bilder liegen unter `/data/images/<bookmark-id>-og.webp` und `/data/images/<bookmark-id>-icon.webp`.

### Umgebungsvariablen

| Variable | Pflicht | Standard | Zweck |
|---|---|---|---|
| `PASSWORD` | ja | – | Login-Passwort |
| `PORT` | nein | `8080` | Port im Container |
| `TZ` | nein | `Europe/Berlin` | Zeitzone |

### Sicherheit beim Abruf fremder Seiten

- Zeitlimit 5 Sekunden, maximal 1 MB HTML und 5 MB pro Bild.
- Nur `http` und `https`.
- Weiterleitungen begrenzt (max. 5).

## Phasen

- [x] **0a. Name und Repo:** Name festgelegt, GitHub-Repo angelegt
- [ ] **0b. Ordner umbenennen** (`code/Neuer Ordner` → `code/sebo`)
- [ ] **0c. Grundgerüst:** (Backend, Frontend, Docker)
- [ ] **1. Login:** Passwort, TOTP-Einrichtung mit QR-Code im Log, Wiederherstellungscodes, `reset-2fa`, Fehlversuchssperre, Sessions
- [ ] **2. API:** Ordner und Bookmarks (anlegen, lesen, ändern, löschen, verschieben), Metadaten- und Bildabruf
- [ ] **3. Oberfläche:** Login-Seite, Seitenleiste mit Ordnerbaum, drei Ansichten, „+“-Dialog, Bearbeiten-Dialog
- [ ] **4. Drag & Drop und Suche:** Links hineinziehen, Bookmarks und Ordner verschieben, Live-Suche
- [ ] **5. Import und Export:** HTML-Import mit Ordnerstruktur, HTML-Export
- [ ] **6. Handy:** responsive Layout, PWA, Share Target
- [ ] **7. Auslieferung:** Dockerfile final, Unraid-Template, GitHub Actions, README fertigstellen
- [ ] **8. Abschluss:** Test auf Unraid, Plan.md löschen

## Offene Punkte

- keine
