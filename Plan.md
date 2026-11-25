# Umsetzungsplan

> Temporäre Datei. Wird nach Abschluss der Umsetzung gelöscht. Dauerhaft Wichtiges steht in `CLAUDE.md` (für die Entwicklung) und `README.md` (für Nutzer).

Name: **SEBO** (Self-hosted Bookmarks), Repo `mr-glubschi/sebo`

## Ziel

Schlanke, selbst gehostete Bookmark-Verwaltung im Stil von Raindrop, reduziert auf das Minimum. Läuft als einzelner Docker-Container auf Unraid, ein Benutzer pro Container.

## Funktionsumfang

1. **Login mit Passwort + 2FA (TOTP)**
   - Passwort aus `PASSWORD` (Klartext) oder `PASSWORD_HASH` (argon2, erzeugt mit `docker exec sebo hash-password`). Fehlt beides oder ist das Passwort kürzer als 12 Zeichen, startet der Container nicht.
   - Beim ersten Start: TOTP-Geheimnis erzeugen, QR-Code (Unicode) und `otpauth://`-URL ins Log schreiben, mit Hinweis, dass beides nach der Bestätigung verschwindet.
   - QR-Code erscheint nur, bis die Einrichtung im Browser mit einem gültigen Code bestätigt wurde.
   - Danach 10 Wiederherstellungscodes einmalig anzeigen (SHA-256-gehasht gespeichert, je einmal nutzbar).
   - Jeder TOTP-Code wird nur einmal angenommen (Zeitfenster ±1).
   - `docker exec sebo reset-2fa` setzt 2FA zurück (läuft als Zielbenutzer), neuer QR-Code im Log, kein Neustart nötig.
   - Fehlversuche: globale, ansteigende Wartezeit statt harter Sperre.
2. **Bookmarks hinzufügen**
   - Drag & Drop eines Links auf die ganze Seite (`text/uri-list`, `text/plain`; Titel aus `text/x-moz-url` bzw. `text/html` als Vorschlag). Ablage auf einem Ordner speichert direkt dort.
   - „+“-Button mit Formular: URL, Name, Ordner, Einfügen-Button für die Zwischenablage. Titel und Bild werden vorausgefüllt, alles überschreibbar.
   - Bookmarks auf interne Adressen sind immer speicherbar; nur der automatische Abruf ist standardmäßig gesperrt (siehe Sicherheit).
   - Gespeichert werden nur Titel, URL und Bilder. Kein Seitentext, kein HTML.
3. **Ansichten**
   - Karten: OG-Bild groß, darunter Favicon, Name und Domain.
   - Liste: Favicon, Name, URL und Ordner in einer Zeile.
   - Kompakt: nur Favicon und Name.
   - Ausweichkette: OG-Bild → Favicon groß → farbiger Anfangsbuchstabe.
   - Sortierung nach Name oder Datum, keine manuelle Reihenfolge.
   - Gewählte Ansicht und Sortierung werden im Browser gemerkt.
4. **Ordner mit Unterordnern**
   - Ordnerbaum in der Seitenleiste, beliebig tief.
   - Anlegen, umbenennen, löschen (Rückfrage: Inhalt mitlöschen oder in den übergeordneten Ordner verschieben).
   - Bookmarks und Ordner auf einen Ordner ziehen (eigene Umsetzung mit Pointer Events, Maus und Touch, langes Drücken startet das Ziehen).
   - Zusätzlich „Verschieben nach …“ mit Ordnerauswahl, vor allem fürs Handy.
   - Bookmarks ohne Ordner landen in „Unsortiert“.
   - Aktueller Ordner steht in der URL (`/f/<id>`), damit die Zurück-Taste funktioniert.
5. **Bearbeiten, Löschen, Live-Suche**
   - Name, URL, Ordner bearbeiten; Vorschaubild neu laden oder entfernen.
   - Unscharfe Suche mit uFuzzy (`intraMode: 1`) über Name und Domain, Treffer beim Tippen.
   - Alle Bookmarks werden beim Start in den Browser geladen und dort durchsucht.
6. **HTML-Import und -Export**
   - Netscape-Bookmark-Format, Ordnerstruktur bleibt erhalten.
   - Import: Größenlimit, maximale Anzahl Einträge, nur `http(s)`-URLs, doppelte URLs überspringen. Bilder werden im Hintergrund nachgeladen.
   - Export: alle Texte HTML-maskiert.
7. **Handy**
   - Responsive Oberfläche, Seitenleiste als ausklappbares Menü.
   - PWA (vite-plugin-pwa): cacht nur die App-Hülle, keine API-Antworten, kein Offline-Modus.
   - Android: Web Share Target per `GET /share`; URL in `url`, `text` oder `title` suchen; Bestätigungsdialog vor dem Speichern.
   - iPhone: kein Share Target; „+“-Dialog mit Einfügen-Button. Installierte Web-App braucht eigenen Login (eigene Cookies).

## Technik

### Backend (Rust)

| Bereich | Wahl | Hinweise |
|---|---|---|
| Webserver | Axum + `tower-http` | Body-Limits, Timeouts, Kompression |
| Datenbank | SQLite über `tokio-rusqlite` (rusqlite, `bundled`) | eine Verbindung, WAL-Modus, `busy_timeout`, nur parametrisierte Abfragen |
| Migrationen | `rusqlite_migration` | vor jeder Migration Sicherungskopie |
| HTTP-Client | `reqwest` ohne Standard-Features, `rustls` + `webpki-roots` | Größenlimit beim Lesen erzwingen, eigener User-Agent |
| HTML | `scraper` | nur die ersten 1 MB |
| Bilder | `image` (nur jpeg, png, gif, webp lesen, ico) | Decoder-Limits, immer neu kodieren; OG-Bild JPEG Q80 max. 600 px, Favicon PNG 64 px |
| SVG-Favicons | `resvg` | nur als letzte Wahl, keine externen Ressourcen, kein Text/Systemschriften, Größen-/Zeitlimit |
| 2FA | `totp-rs` (`otpauth`, `gen_secret`), `qrcode` (Unicode) | |
| Hashes | `sha2` + `subtle` (Passwort-Vergleich, Wiederherstellungscodes, Session-Tokens), `argon2` nur für `PASSWORD_HASH` | |
| Frontend einbetten | `rust-embed` | unbekannte Pfade → `index.html` |
| Logging | `tracing` | nur stdout, `LOG_LEVEL` |
| Typen fürs Frontend | `ts-rs` | erzeugt TypeScript-Typen der API |
| Unterbefehle | über `argv[0]` / erstes Argument, ohne `clap` | `serve`, `reset-2fa`, `hash-password` |

Release-Profil: `strip = true`, `lto = "thin"`. Ziel: `x86_64-unknown-linux-musl`.

### Frontend

| Bereich | Wahl |
|---|---|
| Framework | Svelte 5 + Vite, Single-Page-App ohne SvelteKit, TypeScript |
| Routing | eigenes kleines Routing über die History-API |
| Suche | uFuzzy |
| Drag & Drop | nativ für Links von außen, eigene Pointer-Events-Umsetzung für Verschieben |
| Styling | einfaches CSS mit CSS-Variablen (Dunkelmodus), Kartenraster mit CSS Grid |
| Bilder | `loading="lazy"`, `decoding="async"`, festes `aspect-ratio`, `content-visibility: auto` |
| PWA | vite-plugin-pwa |

### Datenmodell

```
folders    id, parent_id (NULL = oberste Ebene), name, created_at
bookmarks  id, folder_id (NULL = Unsortiert), url, title, title_auto, has_image, has_icon,
           meta_status (pending|done|failed|skipped), created_at, updated_at
settings        key, value   (totp_secret, totp_confirmed, totp_last_step)
sessions        token_hash, created_at, last_seen_at   (Ablauf: 30 Tage nach last_seen_at)
recovery_codes  code_hash   (benutzte Codes werden gelöscht)
```

Bilder: `/data/images/<id>-og.jpg`, `/data/images/<id>-icon.png`. Sicherungen: `/data/backups/`.

### Hintergrund-Abruf

- Neue und importierte Bookmarks bekommen `meta_status = pending`.
- Ein Arbeitsprozess holt offene Einträge, maximal 4 Abrufe gleichzeitig.
- Nach einem Neustart läuft er bei den offenen Einträgen weiter.

### Umgebungsvariablen

| Variable | Pflicht | Standard | Zweck |
|---|---|---|---|
| `PASSWORD` | eins von beiden | – | Login-Passwort im Klartext |
| `PASSWORD_HASH` | eins von beiden | – | argon2-Hash des Passworts |
| `PUID` / `PGID` | nein | `99` / `100` | Benutzer und Gruppe für `/data` |
| `TZ` | nein | `Europe/Berlin` | Zeitzone |
| `TRUST_PROXY` | nein | `false` | `X-Forwarded-*` auswerten (Secure-Cookie, Log-IP) |
| `ALLOWED_HOSTS` | nein | – | erlaubte Hostnamen (DNS-Rebinding-Schutz), leer = alle |
| `ALLOW_PRIVATE_URLS` | nein | `false` | Metadaten auch von internen Adressen abrufen |
| `LOG_LEVEL` | nein | `info` | Ausführlichkeit des Logs |
| `PORT` | nein | `8080` | Port im Container |

## Sicherheit

- **SSRF:** eigener DNS-Resolver, aufgelöste IP prüfen (privat, Loopback, Link-Local, CGNAT, Multicast, ULA, `169.254.169.254` gesperrt, außer `ALLOW_PRIVATE_URLS=true`), Verbindung auf die geprüfte IP festlegen, Weiterleitungen manuell folgen (max. 5) und jedes Ziel neu prüfen, nur `http(s)`, Zeitlimit 5 s, max. 1 MB HTML, 5 MB pro Bild.
- **Sessions:** 32 Zufallsbytes, in der DB nur SHA-256-Hash, Cookie `HttpOnly`, `SameSite=Lax`, `Secure` + `__Host-`-Präfix bei HTTPS, Ablauf bei Inaktivität (30 Tage), Logout löscht die Session, neue Session nach Login.
- **CSRF:** `Origin`-Prüfung bei allen ändernden Anfragen, JSON nur mit `Content-Type: application/json`.
- **Header:** strikte CSP (`default-src 'self'`, kein Inline-Skript), `X-Content-Type-Options: nosniff`, `frame-ancestors 'none'`, `Referrer-Policy: no-referrer`.
- **Bilder** werden nur vom eigenen Server ausgeliefert, nie Originaldateien oder SVG.
- **Frontend:** kein `{@html}`.
- **Container:** `no-new-privileges`, nur nötige Capabilities (`CHOWN`, `SETUID`, `SETGID` für den Start), danach als `PUID:PGID`.
- **Lieferkette:** `Cargo.lock` und `package-lock.json` committen, `cargo deny` und `npm audit` in der CI, Dependabot (kein App-Setup nötig), Basis-Images per Digest.

## Betrieb

- Basis-Image Alpine (Shell für Unraid-Konsole, `tzdata`, `su-exec`).
- Entrypoint: Rechte auf `/data` für `PUID:PGID` setzen, dann per `su-exec` wechseln.
- Healthcheck `/healthz` (ohne Login) + `HEALTHCHECK` im Dockerfile.
- Täglicher Snapshot per `VACUUM INTO` nach `/data/backups/sebo-YYYYMMDD.db`, 7 Stände.
- Image-Tags: `latest`, `1`, `1.2`, `1.2.3` über GitHub-Release-Tags. Nur amd64.
- Kein Subpath-Betrieb, nur eigene Subdomain hinter Reverse Proxy.
- Unraid-Template `unraid/sebo.xml` (Icon, Support, WebUI, `PASSWORD` maskiert, Kategorie Tools/Productivity).

## Phasen

- [x] **0a. Name und Repo:** Name festgelegt, GitHub-Repo angelegt
- [x] **0b. Ordner umbenennen** (`code/Neuer Ordner` → `code/sebo`)
- [x] **0c. Architektur festlegen** (mit Review aus Sicht Security, Backend, Frontend, Betrieb)
- [x] **0d. Grundgerüst:** Cargo-Projekt, Vite/Svelte-Projekt, Dockerfile mit cargo-chef, Entrypoint mit PUID/PGID, Healthcheck, CI, Release-Workflow, cargo deny, Dependabot
- [x] **1. Login:** Passwort/Hash, TOTP-Einrichtung mit QR-Code im Log, Wiederherstellungscodes, `reset-2fa`, `hash-password`, Fehlversuchs-Wartezeit, Sessions, CSRF, Header
- [x] **2. API:** Ordner und Bookmarks, Metadaten- und Bildabruf mit SSRF-Schutz, Hintergrund-Abruf
- [x] **3. Oberfläche:** Login-Seite, 2FA-Einrichtung, Seitenleiste, drei Ansichten, „+“-Dialog, Bearbeiten-Dialog, Routing
- [ ] **4. Drag & Drop und Suche:** Links hineinziehen, auf Ordner ziehen, „Verschieben nach …“, Live-Suche
- [ ] **5. Import und Export**
- [ ] **6. Handy:** responsive Layout, PWA, Share Target
- [ ] **7. Betrieb:** tägliche Sicherung, Unraid-Template, Basis-Images per Digest pinnen, Container-Härtung im Template, README fertigstellen, erstes Release taggen
- [ ] **8. Abschluss:** Test auf Unraid, Plan.md löschen

## Bewusst nicht umgesetzt

- `PASSWORD_FILE` (unter Unraid unüblich)
- iOS-Kurzbefehle mit API-Token (neue Funktion)
- arm64-Image, Image-Signierung und SBOM (später bei Bedarf)
- Manuelle Sortierung, virtuelles Scrollen, Offline-Modus, Subpath-Betrieb

## Abweichungen während der Umsetzung

- Phase 1: Die Passwort-Umgebungsvariable wird nach dem Start nicht aus dem Prozess entfernt. Das bringt keinen echten Schutz (sie bleibt in `docker inspect` und `/proc/1/environ` sichtbar) und bräuchte `unsafe`-Code. Wer den Klartext vermeiden will, nutzt `PASSWORD_HASH`.
- Phase 1: `reset-2fa` zeigt den neuen QR-Code direkt in der Konsole des Befehls statt im Container-Log. Das ist sicherer (nichts im Log) und bequemer.
- Phase 1: Wiederherstellungscodes nutzen Crockford-Base32 (12 Zeichen, 60 Bit), verwechselbare Zeichen (O/0, I/L/1) werden bei der Eingabe korrigiert.
- Phase 1 (nach Security-Review): zusätzliche Zählung „Passwort richtig, Code falsch“ über 24 h (bis 1 h Wartezeit, wird durch Erfolg nicht zurückgesetzt), Scheinprüfung bei falschem Passwort gegen Timing-Orakel, Codeprüfung und Sitzung in einer Transaktion, bei HTTPS nur `__Host-`-Cookie, IP aus letztem `X-Forwarded-For`-Eintrag, neue Variable `ALLOWED_HOSTS`, `chown -h`.
- Phase 2: rustls mit `ring` und `webpki-roots` statt der reqwest-Voreinstellung (aws-lc-rs bräuchte CMake/NASM im Build, Plattform-Zertifikate wären vom Basis-Image abhängig).
- Phase 2: Neuer Status `skipped` für Bookmarks auf interne Adressen (gespeichert, aber nicht abgerufen). `title_auto` merkt sich, ob der Titel vom Abruf überschrieben werden darf.
- Phase 2: Zusätzlicher Endpunkt `POST /api/preview` liefert den Titelvorschlag für den „+“-Dialog.
- Phase 2 (nach Security-Review): Versuchszähler `fetch_attempts` gegen Absturzschleifen, Gesamtfrist 30 s pro Bookmark, begrenzte Bildverarbeitung (2 gleichzeitig, 10 s, 64 MB, SVG max. 256 KB), höchstens 8 Favicon-Kandidaten, Bilder erst nach erfolgreichem Speichern übernehmen, bei URL-Änderung alte Bilder löschen, Bilder nur ausliefern, wenn sie laut DB existieren, `AUTOINCREMENT` für Bookmark-IDs, lokale NAT64-Präfixe gesperrt. Migration 002 wurde dafür direkt geändert (noch kein Release).
- Phase 3: Adressen `/` (alle), `/unsortiert`, `/f/<id>`. Die Ordneransicht zeigt nur die Bookmarks direkt im Ordner, Unterordner erscheinen oben als Schaltflächen. Solange Bookmarks im Hintergrund geladen werden, fragt die Oberfläche alle 1,5 s nach (höchstens 40-mal). Getestet mit Playwright (eigenes Chromium, nur lokal, kein Projekt-Bestandteil).
