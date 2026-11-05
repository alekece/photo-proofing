# Web server API address from SvelteKit env

Issue: #153.

## Intent

A deployment that forgot `PP_API_INTERNAL_URL` says so plainly, instead of quietly trying
`127.0.0.1:8080` and failing in confusing ways.

## Goals

- The API address is read in one place, `web/src/lib/server/api.ts`, through
  `$env/dynamic/private`.
- In production, a missing or empty `PP_API_INTERNAL_URL` fails with an error naming the variable.
- Local dev (`pnpm dev`) keeps the documented default `http://127.0.0.1:8080`.

## Non-goals

- Changing the variable's name, the compose or staging values, or the browser's `/api` proxy.
- Validating that the address is reachable.

## Approach

`apiUrl(path)` returns the configured base joined with the path:
- In dev (`dev` from `$app/environment`), it falls back to the default.
- Otherwise, it throws `PP_API_INTERNAL_URL is not set`.

`auth.ts` and `galleries.ts` (the three current call sites) use it. The `node:process` reads and
the duplicated fallback go away.

The failure happens on the first request rather than at server start. SvelteKit has no startup
hook here, and adapter-node also runs the build step, where the variable is legitimately absent.
Callers resolve `apiUrl(path)` before their `try` around `fetch`. Otherwise the existing catch
blocks would turn the configuration error into an ordinary 503 and hide it. Left uncaught,
SvelteKit logs it with the variable name and shows the generic error page.

The README sections that describe the default (setup and deployment) are updated: the default
applies to `pnpm dev` only, and production must set the variable.

## Testing

Unit tests for `apiUrl` mock `$env/dynamic/private` and `$app/environment`. They cover:
- a configured value;
- dev with no value (default);
- production with no value (error naming the variable);
- a trailing slash in the configured value.

Caller tests check that `getAuthStatus` and the gallery readers let the named error through
rather than turning it into a 503.
