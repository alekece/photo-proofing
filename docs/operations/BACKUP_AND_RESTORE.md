# Backup and controlled restore (PostgreSQL)

Part of #36. This covers the PostgreSQL database only. Private objects are not covered yet (see [Known limits](#known-limits)).

Scripts (bash, need `docker` for Compose mode or PostgreSQL 17 client tools for URL mode):

| Script | Purpose |
| --- | --- |
| `scripts/backup-postgres.sh` | `pg_dump` custom format, timestamped, owner-only permissions |
| `scripts/restore-postgres.sh` | restore into a **new** database; overwrite only with an explicit flag |
| `scripts/test-backup-restore.sh` | automated restore test on a synthetic, non-client schema |

## Connection modes

First match wins; credentials are never printed by the scripts.

1. `DATABASE_URL=postgres://user:password@host:5432/dbname` uses host `psql`/`pg_dump`/`pg_restore`. The client major version must be at least the server's (17).
2. `EG_POSTGRES_CONTAINER=<name>` runs the tools inside that running container with its own credentials.
3. Default: the `postgres` service of the Docker Compose stack (`docker compose exec`). Use `COMPOSE_PROJECT_NAME` if you changed it.

## 1. Back up

```sh
docker compose up -d --wait postgres      # if the stack is not running
scripts/backup-postgres.sh                # prints e.g. backups/eg-20261028T203957Z.dump
```

Options: `--database NAME`, `--output-dir DIR` (or `BACKUP_DIR`, default `backups/`, gitignored). A newly created directory is `0700` (an existing one is left as is) and the file `0600`. The script checks that the archive is readable (`pg_restore --list`) before keeping it. Backups contain private data: treat them accordingly.

## 2. Restore into a new database (default)

```sh
scripts/restore-postgres.sh --file backups/eg-20261028T203957Z.dump
# prints the new database name, e.g. eg_restore_20261028203957
```

`--target-db NAME` picks the name. The script refuses if the target exists or is the live database. Inspect the result without touching the live data:

```sh
docker compose exec postgres psql -U eg -d eg_restore_20261028203957 -c '\dt'
```

Drop it when done: `docker compose exec postgres psql -U eg -d postgres -c 'DROP DATABASE eg_restore_20261028203957'`.

## 3. Controlled restore over the live database

Only for real recovery. Stop the API and worker first (`docker compose stop api worker`), take a fresh backup of what is left, then:

```sh
scripts/restore-postgres.sh --file backups/<dump> --controlled-restore-overwrite-live-database
docker compose start api worker
```

The flag **replaces** the target (default: the live database). The dump is first restored into a staging database; only if that succeeds is the old database dropped (`WITH (FORCE)`, disconnecting other sessions) and the staging one renamed. A failed restore leaves the existing database untouched, but you still need a fresh backup first. There is no confirmation prompt; the deliberately long flag name is the safeguard.

## 4. Verify the procedure (automated test)

```sh
docker compose up -d --wait postgres
scripts/test-backup-restore.sh
```

The test creates throwaway databases (needs a role with `CREATEDB`; the live database is never read), loads synthetic `galleries`/`photos`-like tables with fixtures labelled "not client data", backs them up, restores into a scratch database and checks:

- row counts and an MD5 content checksum of every row of each table;
- constraints (primary/foreign/unique/check), indexes, and that violating inserts are still rejected;
- sequence values, and that the next `nextval` continues from the restored value;
- backup file/directory permissions, and that restore refuses existing and live databases while the explicit overwrite flag works.

It exits non-zero on any failure and drops its databases on exit. CI runs it on every change to `scripts/` against a PostgreSQL 17 service container (`.github/workflows/backup-restore.yml`). It uses a synthetic schema, so it does not exercise the real application migrations; re-point it at them once they land.

## Known limits

- **Private objects (S3/MinIO) are not backed up or restored.** Object storage lands with #16; backing up and verifying private assets after restore is remaining work for #36.
- No point-in-time recovery: a restore returns to the instant of the dump. WAL archiving is out of scope.
- Encryption of dumps and off-site copies are the operator's responsibility; the scripts only restrict local file permissions.
- Tested only with the same PostgreSQL major version (17) for dump and restore. Cross-major restores are unsupported here.
- Dumps use `--no-owner`: objects are owned by the restoring role. Roles and cluster-wide settings are not included (`pg_dumpall --globals-only` is not run).
- In `DATABASE_URL` mode the URL (with password) is passed to the client programs as an argument, so other local users can see it in the process list. Prefer container/Compose mode on shared hosts.
- `DATABASE_URL` mode needs a database name in the URL and a role allowed to `CREATE DATABASE`.
- The dump is consistent (single snapshot) but the application is not quiesced; stop writers if you need a dump aligned with object storage.
