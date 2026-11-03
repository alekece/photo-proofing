# LAN staging deployment

Status: Approved. Server-owned configuration path and origin derivation are
builder decisions following the request to proceed autonomously.

Related: [#137](https://github.com/alekece/photo-proofing/issues/137).

## Outcome and boundaries

Run the published PhotoProofing application at `https://pp.test` on the private
server, reachable only on the LAN. Preserve database and photo data across
container replacement and ordinary server restarts. Use synthetic trial data.
This does not change product requirements or MVP acceptance.

The first deployment is manual. Configuring Hermes to discover and deploy later
releases is deferred until the initial deployment works. Router DNS and device
certificate trust require configuration by the owner; do not claim these work
from a server-only check.

## Approaches

- Chosen: server-side Docker Compose consumes the existing `staging` Git
  pointer and immutable images. Doppler supplies secrets and Caddy supplies
  internal HTTPS. This matches the installed tools without exposing a deployment
  endpoint or private-server credentials to GitHub.
- Not selected: GitHub pushes deployments into the LAN. This requires additional
  network access and deployment credentials.
- Not selected: build application images on the server. This duplicates CD and
  no longer consumes the published artifacts as the release contract.

The server-side deployment approach is approved.

## Runtime configuration

Use the existing Compose file with a YAML override named `compose.staging.yml`;
Docker Compose does not accept TOML overrides. Select the API, worker and web
images using the same full commit SHA resolved once from the `staging` Git tag.
Never build on the server. Authenticate private GHCR pulls using an authorized
server identity without displaying tokens. Registry credential setup is an
operator prerequisite; do not broaden account permissions automatically.

Keep non-secret application settings in
server-owned `/etc/photo-proofing/config.toml`, mounted read-only into API and
worker containers. Keep only a non-secret example in the repository; do not add
a production configuration before a production deployment exists.

Make the public HTTPS origin configurable through the TOML `auth.app_origin`,
initially `https://pp.test`. Derive Caddy's hostname and the frontend's external
origin from that value rather than maintaining independent settings. Validate
the origin before deployment; this LAN setup supports an HTTPS hostname without
a path, query, credentials, or nonstandard port. Enable secure cookies, listen
on the container interface, and share the private photo directory. Remove
conflicting development environment overrides so the file actually controls
these settings.

Use a staging-specific Compose project with persistent PostgreSQL, photo and
Caddy volumes. Only Caddy publishes ports, bound to the server's LAN address;
PostgreSQL, API and web remain internal. Route `/api/*` to API port 8080 and
other requests to the production web server on port 3000. Configure the web's
internal API URL and external origin consistently.

Use Caddy's internal certificate authority. Preserve its state across recreation.
The owner reserves the server's LAN address, adds a LAN DNS record for the
configured hostname (initially `pp.test`), and installs only the CA's public root
certificate on client devices. Changing the hostname also requires updating LAN
DNS. Never copy the CA's private key. Do not configure router port forwarding or
public DNS. Confirm firewall/network controls separately; LAN binding alone is
not a firewall.

## Secrets and safety

Select an explicit Doppler project and staging config. Inject only the necessary
database secrets at runtime; fail if unavailable. Generate a correctly URL-encoded
database URL inside the secret-bearing process if Doppler does not already supply
one. Never print secret values, resolved Compose configuration, container
environments, or unfiltered diagnostics that might contain credentials.

Do not store passwords in TOML, Compose files, source control, or plaintext
deployment artifacts. Doppler does not conceal runtime secrets from root or
Docker administrators. Do not overwrite existing data or credentials.

## Deployment and acceptance

Resolve and record the selected commit, pull all images before changing running
services, validate configuration without printing it, and start with
`up -d --no-build --wait`. Configure restart policies and log bounds. Confirm all
services are running, HTTPS succeeds using the Caddy root certificate, API health
returns the expected response, and login is reachable without insecure TLS
bypasses. Recreate containers without removing volumes and verify persistence.

Bootstrap the photographer account before making the service available to other
LAN users. Account credentials remain owner-controlled.

Record the selected SHA and any limitations without secrets. Verify device DNS
and certificate trust separately before declaring browser access ready. Retaining
the previous image SHA supports application-image recovery, but cannot undo
database migrations; no automatic database rollback is promised.

## Operational boundaries

The initial deployment binds to loopback for owner-controlled account bootstrap.
After setup, the owner explicitly reruns the launcher with the reserved LAN
address. Until then, LAN access remains intentionally unavailable.

The server uses Doppler project `photo-proofing`, config `stg`, injecting only
`POSTGRES_PASSWORD`. The deployment bundle is separate from the existing checkout
at `/srv/apps/photo-proofing-staging`; application code comes from published
images rather than the old server checkout.

Registry privacy is a separate operator prerequisite: the initial API image
manifest was accessible with an empty Docker credential configuration, despite
the publication design expecting private packages. Do not silently change package
visibility or broaden account scopes. Private packages require an authorized
registry credential; the existing GitHub CLI token lacks `read:packages`.
