# SEBO

**S**elf-hosted **Bo**okmarks

Eine schlanke, selbst gehostete Bookmark-Verwaltung für Unraid. Bookmarks erscheinen als Karten mit Vorschaubild, sind in Ordnern organisiert und lassen sich per Drag & Drop speichern. Nicht mehr und nicht weniger.

## Funktionen

- **Login mit Zwei-Faktor-Authentifizierung**: Passwort plus Code aus einer Authenticator-App (TOTP), dazu Wiederherstellungscodes
- **Bookmarks per Drag & Drop speichern**: Link aus einem anderen Tab, der Adressleiste oder der Lesezeichenleiste auf die Seite ziehen. Auf einem Ordner in der Seitenleiste abgelegt, landet er direkt dort. Mehrere Links auf einmal gehen auch.
- **Manuell hinzufügen**: über „Hinzufügen“ Adresse, Name und Ordner eingeben. Der Titel der Seite wird vorgeschlagen, ein Hinweis erscheint, wenn die Adresse schon gespeichert ist.
- **Vorschaubilder**: das Vorschaubild der Webseite (`og:image`), sonst das Favicon, sonst ein farbiger Anfangsbuchstabe
- **Drei Ansichten**: Karten, Liste und Kompakt, sortiert nach Name oder Datum. Ansicht und Sortierung merkt sich der Browser.
- **Favoriten**: Bookmarks als Favorit markieren. Die Favoriten sind die Startseite; „Alle Bookmarks“ liegt unter `/alle`.
- **Ordner und Unterordner**: beliebig tief, jeweils mit eigenem Symbol. Bookmarks und Ordner mit der Maus auf einen Ordner ziehen, am Handy lange drücken und ziehen, oder über „Verschieben nach …“. Ein Unterordner kommt auf die oberste Ebene, indem man ihn auf „Hierher: oberste Ebene“ über dem Ordnerbaum zieht.
- **Kontextmenü**: Rechtsklick (oder der ⋯-Knopf, am Handy) auf ein Bookmark oder einen Ordner bietet die wichtigsten Aktionen: öffnen, bearbeiten, als Favorit markieren, verschieben, Link kopieren, löschen.
- **Mehrfachauswahl**: Im Kontextmenü „Auswählen“ wählen, dann weitere Einträge antippen (Umschalt-Klick wählt einen Bereich). Ausgewählte Bookmarks lassen sich gemeinsam verschieben, als Favorit markieren oder löschen, ausgewählte Ordner gemeinsam verschieben, löschen oder zusammenführen.
- **Live-Suche**: unscharfe Suche nach Name und Domain, Treffer erscheinen beim Tippen. Tippfehler und Umlaute sind kein Problem („arzte“ findet „Ärzte“). Taste `/` springt ins Suchfeld, Enter öffnet den ersten Treffer.
- **Import und Export**: im HTML-Format, das alle Browser und Raindrop beherrschen. Ordner bleiben erhalten, doppelte Adressen werden übersprungen. Favoriten und Ordner-Symbole kennt das Format nicht, sie gehen beim Export verloren.
- **Handy-tauglich**: als Web-App installierbar; unter Android Links direkt über „Teilen“ speichern
- **Automatische Sicherung**: tägliche Kopie der Datenbank, die letzten 7 Tage werden aufbewahrt
- **Hell und dunkel**: folgt der Einstellung des Geräts

Gespeichert werden nur Titel, URL und Vorschaubilder, keine Seiteninhalte. Die Bilder liefert SEBO selbst aus, fremde Seiten erfahren also nicht, wann du deine Bookmarks ansiehst.

Ein Container ist für genau einen Benutzer gedacht. Für mehrere Benutzer legst du mehrere Container an.

## Installation auf Unraid

### 1. Container anlegen

**Mit Vorlage (empfohlen):**

1. In der Unraid-Konsole die Vorlage herunterladen:

   ```sh
   wget -O /boot/config/plugins/dockerMan/templates-user/my-sebo.xml      https://raw.githubusercontent.com/mr-glubschi/sebo/main/unraid/sebo.xml
   ```

2. **Docker → Add Container** öffnen und bei **Template** „sebo“ auswählen.
3. Ein **Passwort** mit mindestens 12 Zeichen eintragen und bei Bedarf Port und Datenpfad anpassen.
4. **Apply** klicken.

Die Vorlage setzt bereits die empfohlenen Sicherheitsoptionen (siehe unten).

**Von Hand:** **Docker → Add Container** öffnen und folgende Werte eintragen:

| Feld | Wert |
|---|---|
| Name | `sebo` |
| Repository | `ghcr.io/mr-glubschi/sebo:latest` |
| Network Type | `bridge` |
| Port | Container `8080` → Host z. B. `8080` |
| Pfad | Container `/data` → Host `/mnt/user/appdata/sebo` |
| Variable `PASSWORD` | dein Login-Passwort, mindestens 12 Zeichen (Feld als *Masked* anlegen) |
| Variable `TZ` | `Europe/Berlin` |
| Extra Parameters | `--security-opt=no-new-privileges --cap-drop=ALL --cap-add=CHOWN --cap-add=SETUID --cap-add=SETGID --read-only --tmpfs=/tmp --memory=512m --pids-limit=256` |

Die Extra Parameters härten den Container: Er darf keine zusätzlichen Rechte erlangen, hat nur die drei Fähigkeiten, die er zum Start braucht (Rechte auf `/data` setzen und auf den Benutzer `PUID`/`PGID` wechseln), kann außerhalb von `/data` nichts schreiben und höchstens 512 MB Arbeitsspeicher belegen (im Alltag braucht SEBO nur einen Bruchteil davon). SEBO selbst läuft nie als root.

### 2. Zwei-Faktor-Authentifizierung einrichten

1. In Unraid beim Container auf das Symbol klicken und **Logs** öffnen.
2. Im Log steht ein QR-Code. Diesen mit einer Authenticator-App scannen (z. B. Aegis, Google Authenticator, 1Password). Wird der QR-Code verzerrt angezeigt, steht darunter die `otpauth://`-Adresse zum manuellen Eintragen.
3. Die Weboberfläche öffnen (`http://<unraid-ip>:8080`), mit dem Passwort anmelden und den Code aus der App eingeben.
4. Die angezeigten **Wiederherstellungscodes** sicher aufbewahren. Mit ihnen kommst du rein, falls das Handy verloren geht.

Nach der Bestätigung erscheint der QR-Code nicht mehr im Log. Die alten Log-Einträge bleiben aber stehen, bis der Container neu erstellt wird (z. B. durch Bearbeiten und **Apply** in Unraid). Mach das einmal nach der Einrichtung, dann ist das Geheimnis auch aus dem Log verschwunden. Falls du Unraid-Logs an einen Syslog-Server weiterleitest, landet der QR-Code dort ebenfalls. Richte 2FA in diesem Fall am besten vor dem Einschalten der Weiterleitung ein oder setze 2FA danach einmal zurück.

### 3. 2FA zurücksetzen

Falls weder Authenticator-App noch Wiederherstellungscodes verfügbar sind, in der Unraid-Konsole ausführen:

```sh
docker exec sebo reset-2fa
```

Der neue QR-Code erscheint direkt in der Konsole, in der du den Befehl ausführst (und bis zur Bestätigung auch beim nächsten Start im Log). Alle Sitzungen werden abgemeldet, ein Neustart ist nicht nötig.

### Passwort ändern

Neues Passwort in der Variable `PASSWORD` (oder `PASSWORD_HASH`) eintragen und den Container neu starten. SEBO erkennt die Änderung beim Start und meldet alle Geräte ab. Auch ein Wechsel zwischen `PASSWORD` und `PASSWORD_HASH` gilt als Änderung.

### Passwort nicht im Klartext speichern (optional)

Unraid speichert Umgebungsvariablen im Klartext in der Template-Datei, und sie sind per `docker inspect` sichtbar. Wer das vermeiden möchte, erzeugt einen Hash:

```sh
docker exec -it sebo hash-password
```

Den ausgegebenen Wert als Variable `PASSWORD_HASH` eintragen und `PASSWORD` entfernen. Der Hash enthält `$`-Zeichen. In der Unraid-Oberfläche ist das kein Problem, in einer `docker-compose.yml` muss jedes `$` als `$$` geschrieben werden.

## Umgebungsvariablen

| Variable | Pflicht | Standard | Beschreibung |
|---|---|---|---|
| `PASSWORD` | eins von beiden | – | Passwort für den Login, mindestens 12 Zeichen |
| `PASSWORD_HASH` | eins von beiden | – | Hash des Passworts statt Klartext (siehe oben) |
| `PUID` / `PGID` | nein | `99` / `100` | Benutzer und Gruppe, denen die Daten gehören |
| `TZ` | nein | `Europe/Berlin` | Zeitzone |
| `TRUST_PROXY` | nein | `false` | auf `true` setzen, wenn SEBO hinter einem Reverse Proxy läuft |
| `ALLOWED_HOSTS` | nein | – | Kommagetrennte Liste der Adressen, unter denen SEBO erreichbar ist, z. B. `sebo.example.com,192.168.1.5`. Andere werden abgewiesen (Schutz vor DNS-Rebinding). Empfohlen. |
| `ALLOW_PRIVATE_URLS` | nein | `false` | auf `true` setzen, damit SEBO auch von internen Adressen (z. B. `192.168.x.x`) Titel und Favicon abruft |
| `LOG_LEVEL` | nein | `info` | Ausführlichkeit des Logs (`error`, `warn`, `info`, `debug`) |
| `PORT` | nein | `8080` | Port im Container |

### Bookmarks auf interne Dienste

Bookmarks auf Dienste im Heimnetz (Home Assistant, Unraid-Oberfläche usw.) lassen sich immer speichern. Standardmäßig ruft SEBO von solchen Adressen aber keine Titel und Bilder ab. Das verhindert, dass jemand über SEBO Geräte in deinem Netz ansprechen kann. Wenn du auch dort automatische Favicons möchtest, setze `ALLOW_PRIVATE_URLS=true`.

Eine Lücke lässt sich nicht allgemein schließen: Eine fremde Webseite kann SEBO auf deine eigene öffentliche IP-Adresse weiterleiten. Manche Router (z. B. FRITZ!Box) antworten darauf aus dem Heimnetz mit ihrer Oberfläche. SEBO liest dabei nur Titel und Bilder, sendet nichts und kann sich nirgends anmelden. Wer das ausschließen will, schaltet am Router den Zugriff auf die Oberfläche über die öffentliche Adresse ab.

## Zugriff von außen

Die App sollte nicht direkt ins Internet gestellt werden. Für den Zugriff von unterwegs empfiehlt sich ein Reverse Proxy mit HTTPS (z. B. Nginx Proxy Manager, SWAG) oder ein VPN (z. B. WireGuard, Tailscale). Für die Installation als Web-App am Handy ist HTTPS erforderlich.

- Hinter einem Reverse Proxy `TRUST_PROXY=true` setzen.
- `ALLOWED_HOSTS` auf deine Domain (und ggf. die LAN-Adresse) setzen.
- SEBO braucht eine eigene (Sub-)Domain, z. B. `sebo.example.com`. Ein Unterpfad wie `example.com/sebo` wird nicht unterstützt.

## Updates

- `latest` (Standard in der Vorlage) bekommt immer die neueste Version, auch neue Hauptversionen. Lies vor einem Update auf eine neue Hauptversion (z. B. 2.0) die Release-Notizen, dort stehen nötige Anpassungen.
- `:1` bekommt nur Updates ohne inkompatible Änderungen. Für feste Versionen `:1.2.3` oder `:1.2` verwenden.
- Vor jeder Änderung an der Datenbankstruktur legt SEBO automatisch eine Sicherung an. Ein Wechsel auf eine ältere Version wird nicht unterstützt. Spiele dafür die Sicherung von vor dem Update ein.

## Backup

Alle Daten liegen im Ordner `/mnt/user/appdata/sebo`:

- `app.db`: Bookmarks, Ordner und Einstellungen
- `images/`: Vorschaubilder und Favicons
- `backups/`: tägliche Sicherungen der Datenbank als `sebo-JJJJMMTT.db` (die letzten 7, Datum in UTC), dazu Sicherungen vor Datenbank-Updates

Die Datenbank und die Sicherungen enthalten das 2FA-Geheimnis. SEBO macht den Ordner deshalb nur für den eigenen Benutzer (`PUID`) lesbar. Gib `appdata/sebo` nicht per SMB frei und bewahre Kopien sicher auf.

Die Dateien in `backups/` sind immer in sich stimmig und lassen sich auch im laufenden Betrieb sicher kopieren. Vorschaubilder sind nicht enthalten, SEBO lädt sie bei Bedarf neu („Titel und Bilder neu laden“). Zusätzlich lässt sich jederzeit ein HTML-Export über die Weboberfläche erstellen.

**Sicherung einspielen:** Container stoppen, `app.db` durch die gewünschte Sicherung ersetzen (umbenennen in `app.db`), die Dateien `app.db-wal` und `app.db-shm` löschen, falls vorhanden, Container starten.

## Am Handy

- **Android (Chrome):** SEBO über HTTPS öffnen, im Menü „Zum Startbildschirm hinzufügen“ bzw. „App installieren“. Danach erscheint SEBO im „Teilen“-Menü anderer Apps: Link teilen, SEBO wählen, Ordner aussuchen, speichern.
- **iPhone (Safari):** „Teilen“ → „Zum Home-Bildschirm“. iOS unterstützt kein Teilen an Web-Apps. Link kopieren und im „Hinzufügen“-Dialog mit dem Einfügen-Knopf übernehmen. Die installierte Web-App hat eigene Cookies, du meldest dich dort einmal neu an.
- Zum Verschieben ein Bookmark lange drücken und auf einen Ordner im Menü ziehen.
- Der ⋯-Knopf auf Karten und Ordnern öffnet das Menü mit allen Aktionen, darunter „Auswählen“ für die Mehrfachauswahl.

## Lizenz

SEBO steht unter der [GNU Affero General Public License v3.0](LICENSE). Du darfst SEBO frei nutzen, verändern und weitergeben. Wer eine veränderte Version weitergibt oder als Webdienst für andere betreibt, muss den geänderten Quellcode unter derselben Lizenz zugänglich machen.

Die Ordner-Symbole stammen aus [Lucide](https://lucide.dev) (ISC-Lizenz, siehe `web/public/licenses/lucide.txt`).
