# LAN staging deployment

Status: Revised design approved by the user. This revision replaces the custom
deployment launcher with Ansible and standardizes names on `pp`.

Related: [#137](https://github.com/alekece/photo-proofing/issues/137).

## Outcome and boundaries

Run the published PhotoProofing application at an operator-configured HTTPS origin on a private
server, reachable only on the LAN. Preserve database and photo data across
container replacement and ordinary server restarts. Use synthetic trial data.
This does not change product requirements or MVP acceptance.

Deployment remains manually invoked. Configuring Hermes to discover and deploy later
releases is outside this revision. Router DNS and device
certificate trust require configuration by the owner; do not claim these work
from a server-only check.

## Approaches

- Chosen by the user: Ansible manages server configuration and uses the maintained
  `community.docker.docker_compose_v2` module to reconcile the existing Compose
  stack. Docker Compose consumes the `staging` pointer and immutable images.
  Doppler supplies secrets and Caddy supplies internal HTTPS.
- Replaced: the custom Python launcher. It works but adds bespoke deployment
  orchestration and tests to maintain.
- Not selected: documented Compose commands alone. They require less tooling but
  leave configuration installation, release selection and reconciliation manual.
- Not selected: GitHub pushes deployments into the LAN. This requires additional
  network access and deployment credentials.
- Not selected: build application images on the server. This duplicates CD and
  no longer consumes the published artifacts as the release contract.

The user approved this written revision before implementation.

## Runtime configuration

Use the existing Compose file with a YAML override named `compose.staging.yml`;
Docker Compose does not accept TOML overrides. Select the API, worker and web
images using the same full commit SHA resolved once from the `staging` Git tag.
Never build on the server. Authenticate private GHCR pulls using an authorized
server identity without displaying tokens. Registry credential setup is an
operator prerequisite; do not broaden account permissions automatically.

Keep non-secret application settings in
server-owned `/etc/pp/config.toml`, mounted read-only at `/etc/pp/config.toml`
into API and worker containers. Keep root `config.example.toml` as the single
configuration reference. Remove `docker/config.staging.example.toml`; document
container/HTTPS overrides without duplicating the full example. Do not change
local development defaults or add a production configuration.

Make the public HTTPS origin configurable through the TOML `auth.app_origin`,
using a server-owned value. Derive Caddy's hostname and the frontend's external
origin from that value rather than maintaining independent settings. Validate
the origin before deployment; this LAN setup supports an HTTPS hostname without
a path, query, credentials, or nonstandard port. Enable secure cookies, listen
on the container interface, and share the private photo directory. Remove
conflicting development environment overrides so the file actually controls
these settings.

Use Compose project `pp-staging` with persistent PostgreSQL, photo and
Caddy volumes. Only Caddy publishes ports, bound to the server's LAN address;
PostgreSQL, API and web remain internal. Route `/api/*` to API port 8080 and
other requests to the production web server on port 3000. Configure the web's
internal API URL and external origin consistently.

Use Caddy's internal certificate authority. Preserve its state across recreation.
The owner reserves the server's LAN address, adds a LAN DNS record for the
configured hostname, and installs only the CA's public root
certificate on client devices. Changing the hostname also requires updating LAN
DNS. Never copy the CA's private key. Do not configure router port forwarding or
public DNS. Confirm firewall/network controls separately; LAN binding alone is
not a firewall.

## Secrets and safety

Keep deployment tooling in this repository, but keep real installation details
outside it: hostnames, network addresses, volume mappings, secret-manager
project/config identifiers, account details and certificates. Examples and CI
fixtures use reserved `.example` hostnames and synthetic values. Apply the same
rule to documentation, PR/issue text and shared logs. Suppress installation
metadata in ordinary Ansible output and configuration diffs; verbose/debug
logging and fact caching must not be used with real settings.

Select an explicit Doppler project and staging config. Inject only the necessary
database secrets at runtime; fail if unavailable. Generate a correctly URL-encoded
database URL inside the secret-bearing process if Doppler does not already supply
one. Never print secret values, resolved Compose configuration, container
environments, or unfiltered diagnostics that might contain credentials.

Do not store passwords in TOML, Compose files, source control, or plaintext
deployment artifacts. Doppler does not conceal runtime secrets from root or
Docker administrators. Do not overwrite existing data or credentials.

Run Ansible locally on the server under `doppler run`, injecting only
`POSTGRES_PASSWORD` from an operator-selected project/config. Sensitive tasks use `no_log`
and do not cache facts or write secret-bearing environment files. Use standard
Ansible tasks, filters and maintained modules, not a Python/shell wrapper or
custom Ansible plugin. Declare the required Ansible collection explicitly.

## Deployment and acceptance

Resolve and record the selected commit, pull all images before changing running
services, validate configuration without printing it, and reconcile with builds
disabled and readiness waiting enabled through the Compose module. Configure
restart policies and log bounds. Confirm all
services are running, HTTPS succeeds using the Caddy root certificate, API health
returns the expected response, and login is reachable without insecure TLS
bypasses. Recreate containers without removing volumes and verify persistence.

Do not seed an account or manage account credentials in deployment or Doppler.
Fresh installations retain the first-time setup page for testing. Existing
accounts are preserved; deployment is not a database-reset operation.
The owner completes setup before enabling LAN access.

Record the selected SHA and any limitations without secrets. Verify device DNS
and certificate trust separately before declaring browser access ready. Retaining
the previous image SHA supports application-image recovery, but cannot undo
database migrations; no automatic database rollback is promised.

## Operational boundaries

For an existing installation, preserve its origin and binding during migration. Do not
recreate or reset the account, rotate credentials, or change router settings.

The operator selects Doppler project/config privately, injecting only
`POSTGRES_PASSWORD`. Keep the deployment bundle separate from application checkouts.
Move the managed deployment bundle to
`/srv/apps/pp-staging` as part of the migration; application code comes from
published images rather than the old server checkout.

Before renaming the Compose project, discover and record the actual database,
photo and Caddy volume names. Reference these existing volumes explicitly as
external volumes so `pp-staging` cannot silently create an empty installation.
Pull and validate the replacement before stopping old containers. A brief outage
is acceptable while releasing port 443 and recreating containers under the new
project. Never delete volumes or copy the CA private key out of its volume.
Preserve the public CA certificate, relocating its exported copy to
`/etc/pp/staging-root.crt`.

Install the existing non-secret runtime config at the new path without reverting
its values. Restart/recreate API and worker when TOML changes, even when its mount
path and container environment stay unchanged. Remove the custom launcher and its
launcher-specific tests; replace their behavioral coverage with Ansible
syntax/configuration validation and real deployment/idempotence checks. Update
README, CI, and the implementation plan to match the new entry point.

Verify database/account and photo persistence, unchanged CA, authenticated login,
trusted HTTPS, no internal service ports published, and a second deployment that
does not recreate healthy containers unnecessarily. Keep recovery instructions
for restarting the old project with the same volumes if migration fails.

Registry privacy is a separate operator prerequisite. Verify package visibility
and authorized pulls without silently changing package visibility or broadening
account scopes. Private packages require an authorized registry credential.
