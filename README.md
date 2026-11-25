# SEBO

**S**elf-hosted **Bo**okmarks

Eine schlanke, selbst gehostete Bookmark-Verwaltung für Unraid. Bookmarks erscheinen als Karten mit Vorschaubild, sind in Ordnern organisiert und lassen sich per Drag & Drop speichern. Nicht mehr und nicht weniger.

## Funktionen

- **Login mit Zwei-Faktor-Authentifizierung**: Passwort plus Code aus einer Authenticator-App (TOTP)
- **Bookmarks per Drag & Drop speichern**: Link aus einem anderen Tab oder der Adressleiste auf die Seite ziehen
- **Manuell hinzufügen**: über den „+“-Button URL, Name und Ordner eingeben; Titel und Bild werden automatisch vorgeschlagen
- **Vorschaubilder**: das Vorschaubild der Webseite (`og:image`), sonst das Favicon
- **Drei Ansichten**: Karten, Liste und Kompakt, sortiert nach Name oder Datum
- **Ordner und Unterordner**: Bookmarks auf Ordner ziehen oder über „Verschieben nach …“ einsortieren
- **Live-Suche**: unscharfe Suche nach Name und URL, Treffer erscheinen beim Tippen
- **Import und Export**: im HTML-Format, das alle Browser und Raindrop beherrschen
- **Handy-tauglich**: als Web-App installierbar; unter Android Links direkt über „Teilen“ speichern
- **Automatische Sicherung**: tägliche Kopie der Datenbank, die letzten 7 Tage werden aufbewahrt

Gespeichert werden nur Titel, URL und Vorschaubilder, keine Seiteninhalte. Die Bilder liefert SEBO selbst aus, fremde Seiten erfahren also nicht, wann du deine Bookmarks ansiehst.

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
| Variable `PASSWORD` | dein Login-Passwort, mindestens 12 Zeichen (Feld als *Masked* anlegen) |
| Variable `TZ` | `Europe/Berlin` |
| Extra Parameters | `--security-opt=no-new-privileges` |

3. **Apply** klicken.

### 2. Zwei-Faktor-Authentifizierung einrichten

1. In Unraid beim Container auf das Symbol klicken und **Logs** öffnen.
2. Im Log steht ein QR-Code. Diesen mit einer Authenticator-App scannen (z. B. Aegis, Google Authenticator, 1Password). Wird der QR-Code verzerrt angezeigt, steht darunter die `otpauth://`-Adresse zum manuellen Eintragen.
3. Die Weboberfläche öffnen (`http://<unraid-ip>:8080`), mit dem Passwort anmelden und den Code aus der App eingeben.
4. Die angezeigten **Wiederherstellungscodes** sicher aufbewahren. Mit ihnen kommst du rein, falls das Handy verloren geht.

Nach der Bestätigung erscheint der QR-Code nicht mehr im Log. Falls du Unraid-Logs an einen Syslog-Server weiterleitest, landet der QR-Code dort ebenfalls. Richte 2FA in diesem Fall am besten vor dem Einschalten der Weiterleitung ein oder setze 2FA danach einmal zurück.

### 3. 2FA zurücksetzen

Falls weder Authenticator-App noch Wiederherstellungscodes verfügbar sind, in der Unraid-Konsole ausführen:

```sh
docker exec sebo reset-2fa
```

Danach steht im Log ein neuer QR-Code, die Einrichtung beginnt von vorn. Ein Neustart ist nicht nötig.

### Passwort nicht im Klartext speichern (optional)

Unraid speichert Umgebungsvariablen im Klartext in der Template-Datei, und sie sind per `docker inspect` sichtbar. Wer das vermeiden möchte, erzeugt einen Hash:

```sh
docker exec -it sebo hash-password
```

Den ausgegebenen Wert als Variable `PASSWORD_HASH` eintragen und `PASSWORD` entfernen.

## Umgebungsvariablen

| Variable | Pflicht | Standard | Beschreibung |
|---|---|---|---|
| `PASSWORD` | eins von beiden | – | Passwort für den Login, mindestens 12 Zeichen |
| `PASSWORD_HASH` | eins von beiden | – | Hash des Passworts statt Klartext (siehe oben) |
| `PUID` / `PGID` | nein | `99` / `100` | Benutzer und Gruppe, denen die Daten gehören |
| `TZ` | nein | `Europe/Berlin` | Zeitzone |
| `TRUST_PROXY` | nein | `false` | auf `true` setzen, wenn SEBO hinter einem Reverse Proxy läuft |
| `ALLOW_PRIVATE_URLS` | nein | `false` | auf `true` setzen, damit SEBO auch von internen Adressen (z. B. `192.168.x.x`) Titel und Favicon abruft |
| `LOG_LEVEL` | nein | `info` | Ausführlichkeit des Logs (`error`, `warn`, `info`, `debug`) |
| `PORT` | nein | `8080` | Port im Container |

### Bookmarks auf interne Dienste

Bookmarks auf Dienste im Heimnetz (Home Assistant, Unraid-Oberfläche usw.) lassen sich immer speichern. Standardmäßig ruft SEBO von solchen Adressen aber keine Titel und Bilder ab. Das verhindert, dass jemand über SEBO Geräte in deinem Netz ansprechen kann. Wenn du auch dort automatische Favicons möchtest, setze `ALLOW_PRIVATE_URLS=true`.

## Zugriff von außen

Die App sollte nicht direkt ins Internet gestellt werden. Für den Zugriff von unterwegs empfiehlt sich ein Reverse Proxy mit HTTPS (z. B. Nginx Proxy Manager, SWAG) oder ein VPN (z. B. WireGuard, Tailscale). Für die Installation als Web-App am Handy ist HTTPS erforderlich.

- Hinter einem Reverse Proxy `TRUST_PROXY=true` setzen.
- SEBO braucht eine eigene (Sub-)Domain, z. B. `sebo.example.com`. Ein Unterpfad wie `example.com/sebo` wird nicht unterstützt.

## Updates

- `latest` bekommt immer die neueste Version.
- Für feste Versionen `ghcr.io/mr-glubschi/sebo:1.2.3`, `:1.2` oder `:1` verwenden.
- Vor jeder Änderung an der Datenbankstruktur legt SEBO automatisch eine Sicherung an. Ein Wechsel auf eine ältere Version wird nicht unterstützt. Spiele dafür die Sicherung von vor dem Update ein.

## Backup

Alle Daten liegen im Ordner `/mnt/user/appdata/sebo`:

- `app.db`: Bookmarks, Ordner und Einstellungen
- `images/`: Vorschaubilder und Favicons
- `backups/`: tägliche Sicherungen der Datenbank (die letzten 7)

Die Dateien in `backups/` sind immer in sich stimmig und lassen sich auch im laufenden Betrieb sicher kopieren. Zusätzlich lässt sich jederzeit ein HTML-Export über die Weboberfläche erstellen.

## Lizenz

SEBO steht unter der [GNU Affero General Public License v3.0](LICENSE). Du darfst SEBO frei nutzen, verändern und weitergeben. Wer eine veränderte Version weitergibt oder als Webdienst für andere betreibt, muss den geänderten Quellcode unter derselben Lizenz zugänglich machen.
