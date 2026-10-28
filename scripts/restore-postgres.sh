#!/usr/bin/env bash
# Restore a dump made by backup-postgres.sh into a NEW database.
#
# Usage: scripts/restore-postgres.sh --file DUMP [--target-db NAME]
#                                    [--controlled-restore-overwrite-live-database]
# Env:   DATABASE_URL | EG_POSTGRES_CONTAINER (see scripts/lib/postgres.sh)
#
# By default the target must not exist yet (default name: <live>_restore_<UTC stamp>).
# The live database is never touched unless --controlled-restore-overwrite-live-database
# is given: it then DROPS and recreates --target-db (default: the live database).
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
    -h | --help) sed -n '2,9p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
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
  echo "!!! CONTROLLED RESTORE: dropping and recreating database '$target' !!!" >&2
  [[ "$target" != postgres ]] || eg_die "refusing to overwrite the maintenance database"
  if [[ -n "$exists" ]]; then
    eg_psql postgres -qc "DROP DATABASE \"$target\" WITH (FORCE)"
  fi
else
  [[ "$target" != "$live" ]] ||
    eg_die "target '$target' is the live database; pass --controlled-restore-overwrite-live-database to overwrite it"
  [[ -z "$exists" ]] ||
    eg_die "database '$target' already exists; choose a new name"
fi

eg_psql postgres -qc "CREATE DATABASE \"$target\""
if ! eg_pg_restore "$target" <"$file"; then
  echo "error: restore failed; the partially restored database '$target' was left for inspection" >&2
  exit 1
fi
echo "Restored $file into database '$target'." >&2
echo "$target"
