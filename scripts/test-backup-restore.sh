#!/usr/bin/env bash
# Automated controlled-restore test. Builds a synthetic schema with NON-CLIENT fixture
# rows in a throwaway database, backs it up, restores it into a scratch database and
# verifies row counts, content checksums, constraints and sequences. Also checks that
# the restore script refuses to overwrite existing databases. The live database is
# never read or modified.
#
# Usage: scripts/test-backup-restore.sh
# Env:   DATABASE_URL | EG_POSTGRES_CONTAINER (see scripts/lib/postgres.sh); the role needs CREATEDB.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=scripts/lib/postgres.sh
source "$here/lib/postgres.sh"

suffix="$(date -u +%s)_$$"
src="eg_backup_test_src_$suffix"
restored="eg_backup_test_restored_$suffix"
workdir=$(mktemp -d "${TMPDIR:-$PWD}/eg-backup-test.XXXXXX")
failures=0

cleanup() {
  for db in "$src" "$restored"; do
    eg_psql postgres -qc "DROP DATABASE IF EXISTS \"$db\" WITH (FORCE)" >/dev/null 2>&1 || true
  done
  rm -rf "$workdir"
}
trap cleanup EXIT

pass() { echo "ok   - $1"; }
fail() {
  echo "FAIL - $1" >&2
  failures=$((failures + 1))
}
check_eq() { # description expected actual
  if [[ "$2" == "$3" ]]; then pass "$1"; else fail "$1 (expected '$2', got '$3')"; fi
}
q() { eg_psql "$1" -Atq <<<"$2"; }

eg_check_db_name "$src"
echo "== Creating synthetic source database $src"
eg_psql postgres -qc "CREATE DATABASE \"$src\"" >/dev/null
eg_psql "$src" -q >/dev/null <<'SQL'
-- Synthetic, non-client fixture data only.
CREATE TABLE galleries (
  id         bigserial PRIMARY KEY,
  slug       text NOT NULL UNIQUE CHECK (slug ~ '^[a-z0-9-]+$'),
  title      text NOT NULL,
  state      text NOT NULL CHECK (state IN ('draft', 'published', 'archived')),
  created_at timestamptz NOT NULL DEFAULT '2000-01-01T00:00:00Z'
);
CREATE TABLE photos (
  id              bigserial PRIMARY KEY,
  gallery_id      bigint NOT NULL REFERENCES galleries (id) ON DELETE CASCADE,
  position        integer NOT NULL CHECK (position >= 0),
  filename        text NOT NULL,
  sha256          char(64) NOT NULL CHECK (sha256 ~ '^[0-9a-f]{64}$'),
  metadata        jsonb NOT NULL DEFAULT '{}',
  UNIQUE (gallery_id, position)
);
CREATE INDEX photos_gallery_idx ON photos (gallery_id);

INSERT INTO galleries (slug, title, state) VALUES
  ('fixture-alpha', 'FIXTURE gallery alpha (not client data)', 'published'),
  ('fixture-beta',  'FIXTURE gallery beta (not client data)',  'draft'),
  ('fixture-gamma', 'FIXTURE gallery gamma (not client data)', 'archived');
INSERT INTO photos (gallery_id, position, filename, sha256, metadata)
SELECT 1 + (n % 3), n / 3, 'fixture-' || n || '.jpg',
       md5('fixture-a-' || n) || md5('fixture-b-' || n),
       jsonb_build_object('width', 100 + n, 'caption', 'synthetic fixture ' || n)
FROM generate_series(0, 29) AS n;
-- Leave sequences ahead of max(id) so sequence state is distinguishable.
SELECT setval('photos_id_seq', 500);
SQL

fingerprint() { # database -> counts, checksums, constraints, sequences
  q "$1" "
    SELECT 'galleries=' || count(*) FROM galleries;
    SELECT 'photos=' || count(*) FROM photos;
    SELECT 'galleries_md5=' || md5(string_agg(g::text, '|' ORDER BY id)) FROM galleries g;
    SELECT 'photos_md5=' || md5(string_agg(p::text, '|' ORDER BY id)) FROM photos p;
    SELECT 'constraint=' || conrelid::regclass || ':' || conname || ':' || pg_get_constraintdef(oid)
      FROM pg_constraint WHERE connamespace = 'public'::regnamespace ORDER BY 1;
    SELECT 'index=' || indexdef FROM pg_indexes WHERE schemaname = 'public' ORDER BY 1;
    SELECT 'sequence=' || sequencename || ':' || last_value FROM pg_sequences
      WHERE schemaname = 'public' ORDER BY 1;"
}

echo "== Backing up"
backup=$(BACKUP_DIR="$workdir" "$here/backup-postgres.sh" --database "$src" 2>/dev/null)
check_eq "backup file permissions are 600" "600" "$(stat -c %a "$backup")"
check_eq "backup directory permissions are 700" "700" "$(stat -c %a "$workdir")"

echo "== Restoring into scratch database $restored"
if "$here/restore-postgres.sh" --file "$backup" --target-db "$restored" >/dev/null 2>&1; then
  pass "restore into a new database succeeded"
else fail "restore into a new database"; fi

echo "== Verifying"
expected=$(fingerprint "$src")
actual=$(fingerprint "$restored")
check_eq "counts, checksums, constraints, indexes and sequences match" "$expected" "$actual"
check_eq "photo row count is 30" "30" "$(q "$restored" 'SELECT count(*) FROM photos')"
check_eq "at least 9 constraints present (pk, fk, unique, check)" "t" \
  "$(q "$restored" "SELECT count(*) >= 9 FROM pg_constraint WHERE connamespace = 'public'::regnamespace")"

# Before the rejection checks: failed inserts also consume sequence values.
check_eq "sequence continues after restored value" "501" \
  "$(q "$restored" "INSERT INTO photos (gallery_id, position, filename, sha256) VALUES (1, 99, 'next.jpg', repeat('b', 64)) RETURNING id")"

expect_rejected() { # description sql
  if eg_psql "$restored" -qc "$2" >/dev/null 2>&1; then fail "$1 was accepted"; else pass "$1 is rejected"; fi
}
expect_rejected "foreign key violation" "INSERT INTO photos (gallery_id, position, filename, sha256) VALUES (999, 0, 'x', repeat('a', 64))"
expect_rejected "unique violation" "INSERT INTO galleries (slug, title, state) VALUES ('fixture-alpha', 't', 'draft')"
expect_rejected "check violation" "INSERT INTO galleries (slug, title, state) VALUES ('fixture-new', 't', 'bogus')"
expect_rejected "duplicate (gallery_id, position)" "INSERT INTO photos (gallery_id, position, filename, sha256) VALUES (1, 0, 'x', repeat('a', 64))"

echo "== Safety checks"
if "$here/restore-postgres.sh" --file "$backup" --target-db "$restored" >/dev/null 2>&1; then
  fail "restore over an existing database must be refused"
else pass "restore over an existing database is refused"; fi
live=$(eg_default_db)
if "$here/restore-postgres.sh" --file "$backup" --target-db "$live" >/dev/null 2>&1; then
  fail "restore over the live database must be refused"
else pass "restore over the live database is refused without the controlled-restore flag"; fi

echo "== Controlled restore (explicit overwrite flag) over the scratch database"
if "$here/restore-postgres.sh" --file "$backup" --target-db "$restored" \
  --controlled-restore-overwrite-live-database >/dev/null 2>&1; then
  pass "overwrite with the explicit flag succeeded"
else fail "overwrite with the explicit flag"; fi
check_eq "overwritten database matches the backup again" "$expected" "$(fingerprint "$restored")"

if ((failures)); then
  echo "$failures check(s) failed" >&2
  exit 1
fi
echo "All backup/restore checks passed."
