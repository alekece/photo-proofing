# System Architecture

## Operating model

Enoam Gallery is a self-hosted, single-photographer application. Production should run on a dedicated server in a reputable data centre. A home server may be used for local development or encrypted secondary backup, but should not be the sole location for private client photos because physical security, patching, power, and network availability are harder to guarantee.

## Planned stack

- **Backend API**: Rust with Actix Web. It owns authentication, gallery lifecycle, authorisation, share links, favourites, notifications, audit events, photographer source downloads, and short-lived asset access grants.
- **Frontend**: SvelteKit. It provides the photographer dashboard and responsive client gallery over versioned HTTPS API endpoints. It never contains storage credentials or permanent media URLs.
- **Database**: PostgreSQL for gallery data, sessions, favourites, notifications, audit events, jobs, and delivery manifests.
- **Private object storage**: S3-compatible storage with no public bucket or permanent public object URL.
- **Image worker**: a separate process from the same Rust codebase. It creates thumbnails, proofs, optional watermarked proofs, delivery renditions, and ZIP exports. It should use a maintained image library backed by libvips where practical.
- **Job queue**: PostgreSQL-backed durable jobs, documented in [JOB_QUEUE.md](JOB_QUEUE.md).
- **Delivery edge**: reverse proxy or CDN serving only short-lived authorised access. Caches must be private and scoped to asset and gallery.
- **Deployment**: Docker Compose with separate containers for SvelteKit, Actix Web, image worker, PostgreSQL, and reverse proxy. Backups and upgrades must be documented and tested.

## Core entities

- **PhotographerUser**: the owner of the workspace.
- **Gallery**: state, branding, share expiry, retention date, and access settings.
- **Photo**: source metadata, display order, processing status, proof asset, and optional final asset.
- **ShareAccess**: hashed opaque token, optional passcode hash, issued and revoked time, expiry, and rate-limit metadata.
- **ClientSession**: short-lived limited access session after a share-link validation.
- **FavouriteSelection**: client session, photo ID, and timestamps.
- **SelectionSnapshot**: immutable set of selected photo IDs created by the photographer when Retouching begins.
- **DeliveryManifest**: immutable set of explicitly published final asset IDs allowed to a client in Delivery.
- **Notification**: photographer-only grouped unread activity for a gallery.
- **ProcessingJob**: durable task and lifecycle state.
- **AuditEvent**: significant access, configuration, asset, and export event.

## Important boundaries

- The Actix Web API is the only component that authorises client asset access.
- The worker does not accept browser requests. It claims jobs from PostgreSQL and writes generated assets to private storage.
- SvelteKit displays API results but cannot enforce asset security.
- Originals, proofs, finals, and generated ZIPs are separate asset roles and separate private storage prefixes.
- Production and test data are strictly separate. Development and staging use non-client images.

## Deployable units

The API and worker share Rust domain code and database migrations but run as distinct processes. This prevents time-consuming transformation or ZIP creation from slowing normal dashboard and client-gallery requests.

A separate worker is not a separate product or language. It is the same Rust project launched with a worker command, for example `enoam-worker`, instead of the API command.