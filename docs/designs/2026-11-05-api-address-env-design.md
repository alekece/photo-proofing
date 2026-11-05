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

SvelteKit 3 declares environment variables in `web/src/env.ts` with `defineEnvVars`, and
`$app/env/private` exposes them to server code. The `$env/*` modules are deprecated, so this is the
current way to read private env.

- `PP_API_INTERNAL_URL` has a validator, `resolveApiAddress` in `web/src/lib/api-address.ts`:
  - set: used, without a trailing slash;
  - unset in dev (`pnpm dev`): the default `http://127.0.0.1:8080`;
  - unset while building: accepted, since the build runs without deployment settings;
  - otherwise: an error naming the variable.
- SvelteKit runs the validator when the server starts, so a misconfigured deployment refuses to
  start with a clear message rather than failing on its first request. This replaces the earlier
  idea of failing on first request.
- `apiUrl(path)` in `web/src/lib/server/api.ts` is the single place that joins the address and a
  path. `auth.ts` and `galleries.ts` use it, and the `node:process` reads and duplicated fallbacks
  go away.

The README sections that describe the default are updated: it applies to `pnpm dev` only, and
production must set the variable.

## Testing

- Unit tests for `resolveApiAddress`: configured (trailing slash removed), dev default, building,
  and unset or empty in production naming the variable.
- Manual check of the built server: it exits at startup without the variable and listens with it.
