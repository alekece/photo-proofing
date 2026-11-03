# Pre-production container publication

Status: Approved, including the Git tag name `staging`.

Related: [#135](https://github.com/alekece/photo-proofing/issues/135);
[MVP journey #74](https://github.com/alekece/photo-proofing/issues/74).

## Intent and scope

When a commit is pushed to `main` and its CI checks succeed, build and publish
versioned application container images. Mark the successful commit as the
current pre-production artifact so a separately managed deployment process can
consume it.

This design covers GitHub-side continuous delivery (CD) and the application
image/configuration changes needed to produce those artifacts. It does not
specify how the private server discovers updates, pulls images, configures
Docker Compose, or runs them. That consumer is an internal deployment detail
and may use Hermes or another mechanism.

No server access, server setup, timer/automation, LAN DNS, hostname, TLS/Caddy,
server-owned configuration or persistent-volume changes are part of this
design. No public application service is introduced.

## Approaches

- **Chosen: immutable commit tags and a staging pointer.** Publish each
  application image with a full-commit tag, then advance a Git tag named
  `staging` only after all images exist. This identifies one consistent set of
  images without requiring server details in the publishing workflow.
- **Not chosen: moving `dev` image tags only.** A `dev` tag is an alias whose
  target changes; it does not identify a commit. Separate images can temporarily
  point at different builds, and a consumer cannot reliably tell which exact
  source version it received.
- **Not chosen: have GitHub connect to and deploy the private server.** This
  would couple CD to private-server access and require deployment credentials
  or network connectivity, outside this artifact-publication scope.

Do not publish `dev` image aliases; they add a second, ambiguous version
reference without helping the selected consumer interface. PostgreSQL and
Caddy are not application images and are not built or published by this work.

## CD publication flow

1. Run publication in a separate CD workflow only after the CI workflow
   completes successfully for a push to `main`. Pull request, failed, cancelled,
   or incomplete CI runs cannot publish or advance the pre-production pointer.
   Existing intentionally skipped path-filtered jobs retain their current
   semantics; deployment-related inputs must receive suitable checks.
2. Build the API, worker and production frontend images from the exact tested
   commit. Use these package names:
   - `ghcr.io/alekece/pp-api`
   - `ghcr.io/alekece/pp-image-worker`
   - `ghcr.io/alekece/pp-web`
3. Tag each image `sha-<full-commit-sha>`. Do not overwrite an existing
   commit-tagged image on retry; reuse the already-published artifact.
4. Only after all three images are available, advance the Git tag `staging` to
   that commit. Serialize updates and prevent an older run finishing late from
   moving the pointer backwards. The pointer means the latest eligible
   successfully published commit; it is not an image tag.
5. Grant the publishing workflow only the GitHub package and repository
   permissions it needs. Never put runtime configuration, credentials or user
   data into images or build logs.

Publish the GHCR packages privately. Only authorized consumers can pull them;
the deployment consumer must authenticate, but its credential setup is outside
this design. Private packages restrict access to ready-to-run images, not to
the application source, which remains public. Images must not contain runtime
configuration, credentials or user data.

## Application image requirements

The API and worker images use the existing release Docker builds. The frontend
must be a production image, not the current Vite development server. Replace
SvelteKit's automatic adapter with its supported Node adapter and package the
built app in a Node runtime image. Preserve local frontend development.

The current session cookie sets `Secure=false`. Add a boolean setting using the
existing configuration pattern so local HTTP development can retain the
current behavior and an HTTPS pre-production runtime can enable secure
cookies. Apply the setting consistently when issuing, renewing and clearing
cookies. Do not prescribe where the deployment consumer stores or supplies
this setting.

No PostgreSQL, proxy or TLS image is built by this workflow. No Compose
deployment topology or pre-production server configuration is defined here.

## Delivery and verification

Link this design to a focused GitHub issue before implementation planning.
Update README only for the published application-image contract and the
secure-cookie application setting; do not add server deployment instructions.
Update the existing session ADR for the configurable secure-cookie behavior.

Verify that:
- Non-main pushes and unsuccessful/incomplete CI cannot publish or advance
  `staging`.
- The three immutable image tags refer to the same tested commit.
- `staging` advances only after every image is published and cannot regress
  due to out-of-order workflow completion.
- Retries do not replace a published commit image with different contents.
- The frontend image runs its production server.
- Secure-cookie behavior is configurable without changing local HTTP behavior
  by default.

Run relevant repository Rust and frontend checks. Do not claim server-side
deployment, LAN access, HTTPS trust, or persistence verification as part of
this work.

## Accepted limitations and builder decisions

Publishing takes place after CI passes; there is no promise of zero downtime or
an automatic database rollback because this work does not run deployments.
The deployment consumer is responsible for interpreting the `staging` Git tag
and selecting the matching `sha-<commit>` images.

The builder chooses workflow/file names, build caching and focused test
infrastructure.
