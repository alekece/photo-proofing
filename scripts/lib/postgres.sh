#!/usr/bin/env bash
# Shared helpers for the PostgreSQL backup/restore scripts. Source it, do not run it.
#
# Connection mode, first match wins:
#   DATABASE_URL            host psql/pg_dump/pg_restore against a postgres:// URL
#                           (client major version must be >= the server's)
#   EG_POSTGRES_CONTAINER   `docker exec` into that running container
#   (default)               `docker compose exec` into the Compose `postgres` service
#
# Nothing here ever prints DATABASE_URL or a password.

eg_die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

eg_mode() {
  if [[ -n "${DATABASE_URL:-}" ]]; then
    echo url
  elif [[ -n "${EG_POSTGRES_CONTAINER:-}" ]]; then
    echo container
  else
    echo compose
  fi
}

# Runs a client program inside the container with the container's own credentials.
# Usage: eg_exec_in_container <program> <database> [args...]
eg_exec_in_container() {
  local program=$1 database=$2
  shift 2
  # shellcheck disable=SC2016 # expanded by the shell inside the container
  local script='prog=$1; db=$2; shift 2; exec "$prog" -U "$POSTGRES_USER" -d "$db" "$@"'
  case "$(eg_mode)" in
    container)
      docker exec -i "$EG_POSTGRES_CONTAINER" sh -c "$script" _ "$program" "$database" "$@"
      ;;
    compose)
      docker compose exec -T postgres sh -c "$script" _ "$program" "$database" "$@"
      ;;
  esac
}

# Replaces the database name in DATABASE_URL (postgres://user:pw@host:port/db?params).
eg_url_for() {
  local name=$1 base=${DATABASE_URL%%\?*} query=""
  [[ "$DATABASE_URL" == *\?* ]] && query="?${DATABASE_URL#*\?}"
  [[ "${base#*://}" == */* ]] || eg_die "DATABASE_URL must include a database name"
  printf '%s/%s%s' "${base%/*}" "$name" "$query"
}

# Name of the configured (live) database.
eg_default_db() {
  if [[ "$(eg_mode)" == url ]]; then
    local base=${DATABASE_URL%%\?*}
    printf '%s' "${base##*/}"
  else
    case "$(eg_mode)" in
      container) docker exec "$EG_POSTGRES_CONTAINER" printenv POSTGRES_DB ;;
      compose) docker compose exec -T postgres printenv POSTGRES_DB ;;
    esac | tr -d '\r\n'
  fi
}

# Usage: eg_psql <database> [psql args...]   (stdin is passed through)
eg_psql() {
  local database=$1
  shift
  if [[ "$(eg_mode)" == url ]]; then
    psql -X -v ON_ERROR_STOP=1 "$(eg_url_for "$database")" "$@"
  else
    eg_exec_in_container psql "$database" -X -v ON_ERROR_STOP=1 "$@"
  fi
}

# Writes a custom-format dump of <database> to stdout.
eg_pg_dump() {
  local database=$1
  if [[ "$(eg_mode)" == url ]]; then
    pg_dump --format=custom --no-owner "$(eg_url_for "$database")"
  else
    eg_exec_in_container pg_dump "$database" --format=custom --no-owner
  fi
}

# Restores a custom-format dump read from stdin into an existing <database>.
eg_pg_restore() {
  local database=$1
  if [[ "$(eg_mode)" == url ]]; then
    pg_restore --no-owner --exit-on-error --dbname="$(eg_url_for "$database")"
  else
    eg_exec_in_container pg_restore "$database" --no-owner --exit-on-error
  fi
}

# Prints the archive table of contents from stdin; fails if it is not a valid archive.
eg_pg_restore_list() {
  if [[ "$(eg_mode)" == url ]]; then
    pg_restore --list
  else
    eg_exec_in_container pg_restore postgres --list
  fi
}

# Quotes a database name as an SQL identifier after validating it.
eg_check_db_name() {
  [[ "$1" =~ ^[A-Za-z_][A-Za-z0-9_]{0,62}$ ]] ||
    eg_die "invalid database name '$1' (use letters, digits, underscore; max 63)"
}
