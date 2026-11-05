# Configurable API limits and job attempts

Issues: #162, #163.

## Goal

Tune a few hard-coded API values from the environment, CLI or TOML file without a rebuild. Defaults equal today's values, so behaviour is unchanged until someone sets them.

## Settings

| Setting | Variable | Default | Bounds |
| --- | --- | --- | --- |
| `photo_upload.body_idle_timeout` | `PP_PHOTO_UPLOAD_BODY_IDLE_TIMEOUT` | `30s` | 1s to 10m |
| `auth.hash_concurrency` | `PP_AUTH_HASH_CONCURRENCY` | 2 | at least 1 |
| `events.capacity` | `PP_EVENTS_CAPACITY` | 1024 | 1 to 65535 |
| `events.relay_backoff_start` | `PP_EVENTS_RELAY_BACKOFF_START` | `250ms` | 10ms to 5m |
| `events.relay_backoff_cap` | `PP_EVENTS_RELAY_BACKOFF_CAP` | `30s` | 10ms to 5m |
| `queue.max_attempts` | `PP_QUEUE_MAX_ATTEMPTS` | 5 | at least 1 |

Invalid values are rejected at startup, including from the TOML file. A backoff cap below the start is treated as the start.

## Design

- `BoundedDuration` now counts its bounds in milliseconds, because the listener backoff starts below one second. Existing aliases keep their one-second minimum.
- `queue.max_attempts` is read by the API only, since the API writes the attempt count onto each job when it enqueues it; the worker only reads it back. It lives in the same `[queue]` section as the worker's settings, which each process ignores in the other's keys.
- The photo intake and retry repositories take a `JobOptions` (default: 5 attempts) through the existing repository options. The API passes the configured value; the worker and tests keep using the default.
- The hub and `ApiState` read their values from config instead of constants.

## Kept as constants

API-contract and safety limits (bulk delete size, filename and email lengths, the JSON body limit), derivative sizes and quality (they will become workspace settings), and the lease renewal ratio.

## Tests

Each setting has defaults, TOML and out-of-range checks; the configured attempt count is checked on jobs created by both intake and retry.
