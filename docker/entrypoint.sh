#!/bin/sh
# Startet SEBO als PUID:PGID. Läuft der Container als root, werden zuerst
# die Rechte auf /data gesetzt, danach wird auf den Zielbenutzer gewechselt.
set -eu

# Datenbank und Sicherungen enthalten das 2FA-Geheimnis: nur für den App-Benutzer lesbar.
umask 077

PUID="${PUID:-99}"
PGID="${PGID:-100}"
DATA_DIR="${DATA_DIR:-/data}"

if [ "$(id -u)" = "0" ]; then
  mkdir -p "$DATA_DIR"
  # Nur bei abweichendem Besitzer alles umstellen (spart Zeit bei vielen Bildern).
  if [ "$(stat -c '%u:%g' "$DATA_DIR")" != "$PUID:$PGID" ]; then
    echo "Setze Besitzer von $DATA_DIR auf $PUID:$PGID"
    if ! chown -R -h "$PUID:$PGID" "$DATA_DIR"; then
      echo "FEHLER: Rechte auf $DATA_DIR konnten nicht gesetzt werden." >&2
      echo "Der Ordner auf dem Host muss für root lesbar sein (z. B. chmod 755) oder" >&2
      echo "schon $PUID:$PGID gehören (chown -R $PUID:$PGID <ordner>)." >&2
      exit 1
    fi
  fi
  exec su-exec "$PUID:$PGID" /usr/local/bin/sebo "$@"
fi

exec /usr/local/bin/sebo "$@"
