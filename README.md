# SEBO

**S**elf-hosted **Bo**okmarks

Eine schlanke, selbst gehostete Bookmark-Verwaltung für Unraid. Bookmarks erscheinen als Karten mit Vorschaubild, sind in Ordnern organisiert und lassen sich per Drag & Drop speichern. Nicht mehr und nicht weniger.

## Funktionen

- **Login mit Zwei-Faktor-Authentifizierung**: Passwort plus Code aus einer Authenticator-App (TOTP)
- **Bookmarks per Drag & Drop speichern**: Link aus einem anderen Tab oder der Adressleiste auf die Seite ziehen
- **Manuell hinzufügen**: über den „+“-Button URL, Name und Ordner eingeben; Titel und Bild werden automatisch vorgeschlagen
- **Vorschaubilder**: das Vorschaubild der Webseite (`og:image`), sonst das Favicon
- **Drei Ansichten**: Karten, Liste und Kompakt
- **Ordner und Unterordner**: Bookmarks und Ordner per Drag & Drop verschieben
- **Live-Suche**: unscharfe Suche nach Name und URL, Treffer erscheinen beim Tippen
- **Import und Export**: im HTML-Format, das alle Browser und Raindrop beherrschen
- **Handy-tauglich**: als Web-App installierbar; unter Android Links direkt über „Teilen“ speichern

Gespeichert werden nur Titel, URL und Vorschaubilder, keine Seiteninhalte.

Ein Container ist für genau einen Benutzer gedacht. Für mehrere Benutzer legst du mehrere Container an.

## Installation auf Unraid

### 1. Container anlegen

1. In Unraid **Docker → Add Container** öffnen.
2. Folgende Werte eintragen:

| Feld | Wert |
|---|---|
| Name | `sebo` |
| Repository | `ghcr.io/mr-glubschi/sebo:latest` |
| Network Type | `bridge` |
| Port | Container `8080` → Host z. B. `8080` |
| Pfad | Container `/data` → Host `/mnt/user/appdata/sebo` |
| Variable `PASSWORD` | dein Login-Passwort (Feld als *Masked* anlegen) |
| Variable `TZ` | `Europe/Berlin` |

3. **Apply** klicken.

### 2. Zwei-Faktor-Authentifizierung einrichten

1. In Unraid beim Container auf das Symbol klicken und **Logs** öffnen.
2. Im Log steht ein QR-Code. Diesen mit einer Authenticator-App scannen (z. B. Aegis, Google Authenticator, 1Password).
3. Die Weboberfläche öffnen (`http://<unraid-ip>:8080`), mit dem Passwort anmelden und den Code aus der App eingeben.
4. Die angezeigten **Wiederherstellungscodes** sicher aufbewahren. Mit ihnen kommst du rein, falls das Handy verloren geht.

Nach der Bestätigung erscheint der QR-Code nicht mehr im Log.

### 3. 2FA zurücksetzen

Falls weder Authenticator-App noch Wiederherstellungscodes verfügbar sind, in der Unraid-Konsole ausführen:

```sh
docker exec sebo reset-2fa
```

Danach steht im Log ein neuer QR-Code, die Einrichtung beginnt von vorn.

## Umgebungsvariablen

| Variable | Pflicht | Standard | Beschreibung |
|---|---|---|---|
| `PASSWORD` | ja | – | Passwort für den Login |
| `PORT` | nein | `8080` | Port im Container |
| `TZ` | nein | `Europe/Berlin` | Zeitzone |

## Zugriff von außen

Die App sollte nicht direkt ins Internet gestellt werden. Für den Zugriff von unterwegs empfiehlt sich ein Reverse Proxy mit HTTPS (z. B. Nginx Proxy Manager, SWAG) oder ein VPN (z. B. WireGuard, Tailscale). Für die Installation als Web-App am Handy ist HTTPS erforderlich.

## Backup

Alle Daten liegen im Ordner `/mnt/user/appdata/sebo`:

- `app.db`: Bookmarks, Ordner und Einstellungen
- `images/`: Vorschaubilder und Favicons

Zusätzlich lässt sich jederzeit ein HTML-Export über die Weboberfläche erstellen.
