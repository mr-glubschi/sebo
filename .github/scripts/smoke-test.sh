#!/bin/sh
# Rauchtest für das fertige Image: startet den Container mit denselben
# Härtungsoptionen wie das Unraid-Template und prüft Login, API und Rechte.
set -eu

IMAGE="${1:-sebo:test}"
DATA="$(mktemp -d)"
# Wie ein neuer appdata-Ordner auf Unraid (mktemp legt 700 an)
chmod 755 "$DATA"
BASE="http://127.0.0.1:8080"
JAR="$(mktemp)"

fail() {
  echo "FEHLER: $*"
  docker logs sebo || true
  exit 1
}

docker run -d --name sebo -p 127.0.0.1:8080:8080 \
  --read-only --tmpfs /tmp \
  --security-opt no-new-privileges \
  --cap-drop ALL --cap-add CHOWN --cap-add SETUID --cap-add SETGID \
  -e PASSWORD=rauchtest-passwort-123 \
  -v "$DATA:/data" \
  "$IMAGE"

for _ in $(seq 1 30); do
  curl -fsS "$BASE/healthz" >/dev/null 2>&1 && break
  sleep 1
done
curl -fsS "$BASE/healthz" | grep -q ok || fail "healthz antwortet nicht"
echo "ok: läuft"

# Prozess läuft als 99:100, Daten gehören 99:100
docker top sebo -o uid,gid,comm | grep -Eq '^99 +100 +sebo' || { docker top sebo -o uid,gid,comm; fail "läuft nicht als 99:100"; }
[ "$(stat -c '%u:%g' "$DATA/app.db")" = "99:100" ] || fail "Datenbank gehört nicht 99:100"
echo "ok: Benutzer und Rechte"

# Oberfläche und Sicherheits-Header
curl -fsS "$BASE/" | grep -q '<div id="app">' || fail "index.html fehlt"
curl -fsSI "$BASE/" | grep -qi "content-security-policy: default-src 'self'" || fail "CSP fehlt"
echo "ok: Oberfläche"

# Login mit TOTP aus dem Log
OTP_URL="$(docker logs sebo 2>&1 | grep -o 'otpauth://[^ ]*' | head -1)"
[ -n "$OTP_URL" ] || fail "kein QR-Code im Log"
CODE="$(node .github/scripts/totp.mjs "$OTP_URL")"
LOGIN="$(curl -fsS -c "$JAR" -H "Origin: $BASE" -H 'Content-Type: application/json' \
  -d "{\"password\":\"rauchtest-passwort-123\",\"code\":\"$CODE\"}" "$BASE/api/auth/login")"
echo "$LOGIN" | grep -q recovery_codes || fail "Login fehlgeschlagen: $LOGIN"
echo "ok: Login mit 2FA"

curl -fsS -b "$JAR" -H "Origin: $BASE" -H 'Content-Type: application/json' \
  -d '{"url":"http://192.168.1.1/","title":"Router"}' "$BASE/api/bookmarks" | grep -q '"Router"' \
  || fail "Bookmark nicht angelegt"
curl -fsS -b "$JAR" "$BASE/api/library" | grep -q '192.168.1.1' || fail "Bibliothek leer"
curl -fsS -b "$JAR" "$BASE/api/export" | grep -q 'NETSCAPE-Bookmark-file-1' || fail "Export fehlt"
echo "ok: API"

# Ohne Origin abgelehnt (CSRF)
STATUS="$(curl -s -o /dev/null -w '%{http_code}' -b "$JAR" -H 'Content-Type: application/json' \
  -d '{"url":"https://example.com/"}' "$BASE/api/bookmarks")"
[ "$STATUS" = "403" ] || fail "CSRF-Schutz greift nicht ($STATUS)"
echo "ok: CSRF"

# Tägliche Sicherung
sleep 1
ls "$DATA/backups" | grep -Eq '^sebo-[0-9]{8}\.db$' || fail "keine Sicherung"
echo "ok: Sicherung"

# 2FA zurücksetzen über docker exec
docker exec sebo reset-2fa | grep -q 'otpauth://' || fail "reset-2fa ohne QR-Code"
curl -fsS -b "$JAR" "$BASE/api/auth/status" | grep -q '"authenticated":false' || fail "Sitzung nach reset-2fa noch gültig"
echo "ok: reset-2fa"

docker rm -f sebo >/dev/null
echo "Rauchtest bestanden"
