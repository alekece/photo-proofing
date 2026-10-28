#!/usr/bin/env bash
# Restore a dump made by backup-postgres.sh into a NEW database.
#
# Usage: scripts/restore-postgres.sh --file DUMP [--target-db NAME]
#                                    [--controlled-restore-overwrite-live-database]
# Env:   DATABASE_URL | EG_POSTGRES_CONTAINER (see scripts/lib/postgres.sh)
#
# By default the target must not exist yet (default name: <live>_restore_<UTC stamp>).
# The live database is never touched unless --controlled-restore-overwrite-live-database
# is given: it then replaces --target-db (default: the live database): the dump is restored into
# a staging database first, then the old database is dropped and the staging one renamed.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=scripts/lib/postgres.sh
source "$here/lib/postgres.sh"

file=""
target=""
overwrite=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --file) file=${2:?--file needs a value}; shift 2 ;;
    --target-db) target=${2:?--target-db needs a value}; shift 2 ;;
    --controlled-restore-overwrite-live-database) overwrite=1; shift ;;
    -h | --help) sed -n '2,11p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) eg_die "unknown argument: $1" ;;
  esac
done

[[ -n "$file" ]] || eg_die "--file is required"
[[ -r "$file" ]] || eg_die "cannot read $file"
eg_pg_restore_list <"$file" >/dev/null || eg_die "$file is not a valid pg_dump custom archive"

live=$(eg_default_db)
if [[ -z "$target" ]]; then
  if ((overwrite)); then
    target=$live
  else
    target="${live}_restore_$(date -u +%Y%m%d%H%M%S)"
  fi
fi
eg_check_db_name "$target"

exists=$(eg_psql postgres -Atc "SELECT 1 FROM pg_database WHERE datname = '$target'")

if ((overwrite)); then
  [[ "$target" != postgres ]] || eg_die "refusing to overwrite the maintenance database"
  echo "!!! CONTROLLED RESTORE: replacing database '$target' !!!" >&2
else
  [[ "$target" != "$live" ]] ||
    eg_die "target '$target' is the live database; pass --controlled-restore-overwrite-live-database to overwrite it"
  [[ -z "$exists" ]] ||
    eg_die "database '$target' already exists; choose a new name"
fi

# Overwrite mode restores into a staging database first so a failed restore never
# destroys the existing data; only a complete restore is swapped in.
staging=$target
((overwrite)) && staging="${target}_incoming_$(date -u +%s)"
eg_check_db_name "$staging"

eg_psql postgres -qc "CREATE DATABASE \"$staging\""
if ! eg_pg_restore "$staging" <"$file"; then
  if ((overwrite)); then
    eg_psql postgres -qc "DROP DATABASE IF EXISTS \"$staging\" WITH (FORCE)" || true
    echo "error: restore failed; existing database '$target' was left untouched" >&2
  else
    echo "error: restore failed; the partially restored database '$target' was left for inspection" >&2
  fi
  exit 1
fi

if ((overwrite)); then
  if [[ -n "$exists" ]]; then
    eg_psql postgres -qc "DROP DATABASE \"$target\" WITH (FORCE)"
  fi
  eg_psql postgres -qc "ALTER DATABASE \"$staging\" RENAME TO \"$target\""
fi
echo "Restored $file into database '$target'." >&2
echo "$target"
