#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

DB_NAME="commander_test"

if [[ "$DB_NAME" != *test* ]]; then
  echo "refusing to reset non-test database: $DB_NAME" >&2
  exit 1
fi

docker compose up -d postgres

until docker compose exec -T postgres pg_isready -U postgres >/dev/null; do
  sleep 1
done

docker compose exec -T postgres dropdb -U postgres --if-exists --force "$DB_NAME"
docker compose exec -T postgres createdb -U postgres "$DB_NAME"

for file in $(find db/migrations/agent -maxdepth 1 -name 'V*.sql' | sort -V); do
  echo "applying $file"
  docker compose exec -T postgres psql -U postgres -d "$DB_NAME" -v ON_ERROR_STOP=1 < "$file"
done

echo
echo "test database is ready"
