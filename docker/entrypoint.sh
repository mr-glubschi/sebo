#!/bin/sh
# Startet SEBO als PUID:PGID. Läuft der Container als root, werden zuerst
# die Rechte auf /data gesetzt, danach wird auf den Zielbenutzer gewechselt.
set -eu

PUID="${PUID:-99}"
PGID="${PGID:-100}"
DATA_DIR="${DATA_DIR:-/data}"

if [ "$(id -u)" = "0" ]; then
  mkdir -p "$DATA_DIR"
  # Nur bei abweichendem Besitzer alles umstellen (spart Zeit bei vielen Bildern).
  if [ "$(stat -c '%u:%g' "$DATA_DIR")" != "$PUID:$PGID" ]; then
    echo "Setze Besitzer von $DATA_DIR auf $PUID:$PGID"
    chown -R "$PUID:$PGID" "$DATA_DIR"
  fi
  exec su-exec "$PUID:$PGID" /usr/local/bin/sebo "$@"
fi

exec /usr/local/bin/sebo "$@"
