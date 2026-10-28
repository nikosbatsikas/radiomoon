#!/bin/sh
# Settings from the environment (the server remembers the database settings in the data volume):
#   RADIOMOON_DB_HOST (default db)  RADIOMOON_DB_PORT (3306)  RADIOMOON_DB_USER (radiomoon)
#   RADIOMOON_DB_PASSWORD           RADIOMOON_DB_NAME (radiomoon)
#   RADIOMOON_WEB_PASSWORD          password of the web user "admin" at the first start (else random, see webserver.json)
set -e
mkdir -p "$RADIOMOON_DATA"
exec /opt/radiomoon/radiomoon-server \
  --bind "$RADIOMOON_BIND" \
  --data "$RADIOMOON_DATA" \
  --db-host "${RADIOMOON_DB_HOST:-db}" \
  --db-port "${RADIOMOON_DB_PORT:-3306}" \
  --db-user "${RADIOMOON_DB_USER:-radiomoon}" \
  --db-password "${RADIOMOON_DB_PASSWORD:-radiomoon}" \
  --db-name "${RADIOMOON_DB_NAME:-radiomoon}" \
  "$@"
