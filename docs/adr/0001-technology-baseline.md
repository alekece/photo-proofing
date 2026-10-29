# ADR 0001 — Technology baseline for the product-driven restart

Status: Accepted in the product-driven restart design.

Date: 2026-10-29.

## Context

PhotoProofing is restarting around a complete, local-network proofing-to-delivery trial. The repository has a Rust/Actix API scaffold, SvelteKit frontend, idle Rust worker, and a PostgreSQL/Compose development stack. The initial scale is one photographer/workspace and representative galleries of 50–100 JPEGs, not a multi-tenant SaaS platform.

This ADR records the choices reaffirmed or selected for the restart and their present rationale. It does not reconstruct undocumented historical reasons. Product behavior and release scope remain in the [PRD](../SPECIFICATIONS.md) and [MVP record](../releases/MVP.md).

## Decision

| Area | Baseline | Reason and consequence |
|---|---|---|
| Backend | Rust with Actix Web | Rust is a firm project preference; Actix is familiar and already present. Retain it rather than migrating the scaffold without a concrete product benefit. |
| Persistence | PostgreSQL with SQLx and migrations | PostgreSQL is the selected persistent store. SQLx keeps database access explicit in Rust. Database integration, schema, migration and query-validation workflows still need implementation design. |
| Background work | Separate Rust image worker with PostgreSQL queue tables | Processing must survive restarts and safely retry. Reuse the selected database rather than adding a separate queue service. Claims, recovery, and bounded retries remain real engineering work. |
| Photo storage | Persistent local filesystem | Fits the local trial and avoids operating an object-storage server. Keep access cohesive and private; future object storage may require adaptation. Do not build unused backends now. |
| Frontend | SvelteKit with Tailwind CSS and shadcn-svelte | Retain the application framework and use a consistent styling/component starting point. Add only needed components; adopted component source requires maintenance, customization, and accessibility verification. |
| Browser updates | SSE through Actix and actix-web-lab | Updates are primarily server-to-browser; normal HTTP handles actions. Avoid a two-way messaging protocol or periodic frontend polling as the default update mechanism. Reconnect must refresh database-backed state. |
| Deployment | Docker Compose | Retain the existing deployment scaffold for one isolated installation. Database and photo persistence, LAN configuration, and service readiness need validation before the MVP journey is considered delivered. |

Rust, Actix, SvelteKit, PostgreSQL in Compose, and the worker process are existing repository foundations. SQLx integration, durable queue operation, photo storage/processing, Tailwind/shadcn-svelte, and SSE are selected implementation direction, not claims of completed integration. Verify library/runtime compatibility against the actual project versions before installing dependencies.

## Alternatives

- **Axum:** credible alternative with built-in SSE responses and Tower middleware integration. We retain Actix because of project preference and existing code; SSE alone does not require a framework change.
- **Polling or WebSockets:** polling is a smaller initial transport but delays updates and repeats requests; WebSockets support two-way messaging that current journeys do not need. SSE matches processing, gallery, ZIP-readiness, and notification updates. Worker-to-API signaling is still to be designed; SSE does not itself connect the worker process to the API.
- **Actix streaming without actix-web-lab:** avoids that dependency but leaves event encoding/keep-alive handling to project code. We accept the existing SSE utility instead of maintaining our own implementation.
- **Local S3-compatible storage and separate message services:** possible later deployment options, but add services, credentials, and failure modes without a current local-trial need. No additional broker is required by the chosen design.
- **Svelte CSS alone or unstyled primitives:** fewer styling dependencies or more visual control, but more work creating consistent controls. Tailwind with shadcn-svelte is the chosen starting point, not a requirement to use every component.

## Consequences and limits

- `actix-web-lab` explicitly has an evolving API and expects breaking changes. Accept that dependency cost, localize SSE-specific use, use the lockfile for reproducibility, and review upgrades. Its availability is not a reason to adopt unrelated extractors/middleware without need. See its [compatibility policy](https://docs.rs/actix-web-lab/latest/actix_web_lab/#things-to-know-about-this-crate).
- PostgreSQL remains authoritative for jobs, selections, publication, and activity. SSE delivery is not a durable event history. Validate authorization, reconnection, slow-client handling, process signaling, and current-state refresh in the event design.
- Filesystem persistence requires explicit mounts, private serving boundaries, cleanup, and deletion coordination with queued work. Restart persistence is required even though backup/restore tooling is deferred for this replaceable-data trial.
- Current Compose ports bind to localhost, and the web container runs a development server. This is not evidence of cross-device LAN reachability or production/public-internet readiness.
- Each installation initially serves one photographer and one workspace. Future SaaS may provision isolated installations; routing alone is not isolation, and SaaS provisioning is not implemented here.
- Numeric resource limits, session lifetimes, queue schemas/leases, image libraries, logo formats, and ZIP retention belong in the relevant implementation designs. Introduce further ADRs only for significant decisions with meaningful alternatives and consequences.
