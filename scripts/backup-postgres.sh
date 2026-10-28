#!/usr/bin/env bash
# Back up a PostgreSQL database to a timestamped custom-format dump (pg_dump -Fc).
#
# Usage: scripts/backup-postgres.sh [--database NAME] [--output-dir DIR]
# Env:   DATABASE_URL | EG_POSTGRES_CONTAINER (see scripts/lib/postgres.sh), BACKUP_DIR
# Output: the dump path on stdout; progress on stderr. Credentials are never printed.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=scripts/lib/postgres.sh
source "$here/lib/postgres.sh"

database=""
output_dir=${BACKUP_DIR:-backups}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --database) database=${2:?--database needs a value}; shift 2 ;;
    --output-dir) output_dir=${2:?--output-dir needs a value}; shift 2 ;;
    -h | --help) sed -n '2,6p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) eg_die "unknown argument: $1" ;;
  esac
done

[[ -n "$database" ]] || database=$(eg_default_db)
eg_check_db_name "$database"

# Dumps contain private data: owner-only directory and file.
umask 077
mkdir -p "$output_dir"
chmod 700 "$output_dir"

stamp=$(date -u +%Y%m%dT%H%M%SZ)
final="$output_dir/${database}-${stamp}.dump"
partial="$final.partial"
[[ ! -e "$final" ]] || eg_die "refusing to overwrite existing $final"
trap 'rm -f "$partial"' EXIT

echo "Dumping database '$database' (mode: $(eg_mode))..." >&2
eg_pg_dump "$database" >"$partial"
[[ -s "$partial" ]] || eg_die "dump is empty"
eg_pg_restore_list <"$partial" >/dev/null || eg_die "dump is not a valid archive"

chmod 600 "$partial"
mv "$partial" "$final"
echo "Backup written ($(wc -c <"$final") bytes), archive verified readable." >&2
echo "$final"
