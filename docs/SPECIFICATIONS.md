# PhotoProofing Product Specifications

This is the authoritative, evolving product requirements document. It describes required behavior, not current implementation status. See the [README](../README.md) for what currently runs and the [MVP release](releases/MVP.md) for the first delivery scope and acceptance checklist. Completed release records identify the specification revision they were accepted against.

## Vision & Problem Statement

PhotoProofing gives a photographer a private place to share proofs, collect a client's favorite photos, export the selection for retouching outside the application, and deliver finished photos through the same gallery link.

The product reduces scattered messages and ambiguous selections while keeping proofing originals out of client reach. It discourages reuse of unfinished photos through reduced-quality previews and optional watermarks. It cannot prevent screenshots or copying images already displayed in a browser.

The first release tests this complete workflow on a local network with replaceable data. It is not yet a production client-work or hosted SaaS offering.

## Target Users

- **Photographer:** sets up the installation, manages galleries and watermark settings, reviews shared client activity, exports selections, and delivers finished photos.
- **Client:** accesses a gallery through its private link, selects favorites, and downloads the delivered finished photos. Several people may act as the same client; they share one selection.
- **Installation operator:** runs the application and its persistent storage. For the MVP this is the photographer. This persona does not introduce an application permission role.

One workspace groups galleries, settings, and activity. The MVP has one photographer account and one workspace per installation. Additional accounts in that workspace are future scope; role management, per-person client selections, and workspace switching are not implied.

## User Journeys

1. **Set up:** the photographer deploys locally, creates the first account through the login page, and signs in with email/password. The session persists across browser restarts until inactivity expiry or logout.
2. **Prepare and share:** create a gallery, choose its title, hero, layout, optional expiry, optional included photo count, and watermark enablement. Upload JPEGs and share the private link. Sharing an empty gallery is allowed after a warning; only ready photos appear to clients.
3. **Choose and review:** the client adds/removes favorites over one or more visits. The photographer sees activity on gallery cards and in grouped notifications. Additional photos beyond the included allowance remain selectable.
4. **Export:** the photographer requests a ZIP of the current selection. The application displays preparation status and initiates the download when ready. Retouching happens outside PhotoProofing; favorites remain editable until delivery.
5. **Deliver and update:** upload an independent set of finished photos, review privately, and click Deliver; delivering before any finished photo is ready is allowed after a warning. At the same link, clients now see finished photos, cannot edit favorites, and can download individually or together. Later additions and removals reach clients without a further publish step.
6. **End access:** expiry or archive blocks access without deleting data. Extending expiry or restoring an archive can restore access at the same link. Confirmed deletion removes the gallery and its assets.

## Functional Requirements

### FR-01

**Account setup and sessions.** An installation with no account offers one-time account creation through its normal login URL. Setup must create only one initial account, including under concurrent requests. Registration closes afterward. Email/password sign-in and logout are required for MVP. Use one persistent cookie-backed session policy without a Remember me control. The operator can configure a human-readable inactivity timeout, defaulting to 30 days. Authenticated user activity renews expiry; passive status checks, background polling, SSE, and keep-alives do not. No absolute lifetime limit applies. Server-side expiry remains authoritative, and logout invalidates the session, including copied tokens, without logging out independent sessions on other devices. Google sign-in and password recovery are not MVP features.

### FR-02

**Workspace ownership.** Galleries, saved watermark settings, the interface language, activity, and notification read state belong to the workspace. The MVP provides a single account in a single workspace, with no collaborator-management interface. The ownership boundary must allow later collaboration without assigning existing galleries to isolated personal environments.

### FR-02a

**Languages.** The product is available in English and French, covering the photographer app, every client page and the unavailable page, including dates. The photographer chooses the language once for the workspace with a switch beside the notifications bell, and it saves immediately. Clients always see the workspace language. New workspaces start in English; the first-run sign-in page follows the browser language, falling back to English. Photographers' own content (gallery titles, studio name) is not translated.

### FR-03

**Gallery configuration.** The photographer can configure title, hero photo, layout, optional expiry, optional included photo count, and watermark enablement. Offer even rows by default and staggered columns, chosen with layout icons and sharing viewer and selection controls. There is no automatic hero: a gallery has one hero or none, and without one the gallery header and client cover show a default cover built from the gallery title. Exact title validation is a technical default; none of this may prevent the empty-sharing journey. The included count is informational, editable after sharing, and optional; no value means unlimited selection. The New gallery dialog asks for a title only; every other setting is configured on the gallery page.

### FR-04

**Private sharing.** Share gallery creates an unguessable private link; MVP sharing is copying that link. Anyone possessing it can act on the gallery's one shared selection while access is permitted. No viewer account or access code is required for MVP. Share is never disabled: sharing works before photos are uploaded or ready, after a warning dialog, and clients then see an empty state ("Your photos are on their way."). The photographer may add/remove proofing photos after sharing. Only fully processed photos appear. Removing a selected photo requires confirmation and removes it from the selection. Shared access must still respect expiry, archive, deletion, and finished-delivery state.

**Client preview.** The signed-in photographer can preview the client page inside the app from the gallery page, in desktop and mobile views, whatever the gallery's sharing or expiry state. The preview is read-only, creates no share link, records no client activity, and shows only reduced previews, never originals.

### FR-05

**Uploads and processing status.** MVP proofing and finished-photo inputs are JPEG files, validated by actual content. Preserve uploaded originals and finished files unchanged. Show upload transfer progress separately from waiting/processing/ready/failed processing status. Processing continues after navigation away, and current status is available on return. Preserve successful uploads when others fail. Failed uploads restart from the beginning; partial-upload resumption is not required for MVP. Clean up incomplete files. Failed processing is visible and can be retried. Bound file size, decoded dimensions, and processing resources; concrete limits belong in technical design.

### FR-06

**Proof previews.** Generate thumbnails and reduced-resolution/quality previews rather than exposing proofing originals. Preserve aspect ratio, correct orientation, and correct color appearance. Omit source identifying metadata, including GPS, from generated thumbnails and proof previews. Exact resolution/compression choices are implementation details; MVP has no quality-preset, custom-dimension, or metadata-policy controls. Uploaded originals and downloadable finished photos remain unchanged. Client access must never expose proofing originals, regardless of whether watermarking is enabled. Right-click blocking is not a security guarantee or MVP acceptance requirement.

### FR-07

**Optional watermarking.** Save the watermark once at workspace level, using the studio name, logo, or both. Each gallery can enable or disable watermarking on generated proofs. The logo is a PNG or JPEG of at most 1 MiB and 4 megapixels, separate from JPEG-only photo uploads; the server re-encodes it as a metadata-free PNG no larger than 512 px. The watermark is repeated across the photo or placed once at one of nine positions, at a chosen size. Per-gallery watermark designs are not required for MVP: a gallery only switches watermarking on or off. Saving changed watermark settings tells the photographer that watermarked proofs are being regenerated, and affected galleries show that work in progress. Changes must not expose proofing originals; regeneration mechanics belong in the processing design. The workspace also chooses how the studio signs the client cover: name, logo, both or none.

### FR-08

**Favorites and allowance.** Persist each favorite change immediately in the shared gallery selection; clients can edit across visits until finished photos are delivered. Do not require submission, a client confirmation button, or a retouching phase. Downloading a selection does not lock it. When an included count is configured, allow overflow and show the selected count (for example, 13 selected), emphasised in the accent colour when over the included count, without a separate extra count. Changing the included count never deletes favorites or prevents additional choices. Reaching it does not mean the client is finished. Do not introduce prices, invoices, payment checks, or billing workflows.

### FR-09

**Photographer selection export.** A single Download selection action prepares a ZIP containing the selected originals and initiates download when ready. Provide clear preparation/failure status. The ZIP represents the selection when requested; later changes do not alter that export or lock the live selection. The Favorites tab also lets the photographer download one favorite original, or several chosen with select mode. No separate download-management screen is required for MVP. Empty-selection handling and temporary ZIP retention belong in export design.

### FR-10

**Finished delivery.** Finished photos form an independent set, with no required count, filename match, or mapping to favorites. Upload privately, individually or in batches, review, then click Deliver to make them available at the existing private link. Deliver is never disabled: when no finished photo is ready, or some are still uploading, processing or failed, a dialog warns before delivering, and only ready photos reach clients. A delivered gallery with no ready finished photos shows clients "Your finished photos are on their way." After delivery, clients see only finished photos, cannot change favorites, and can download one photo or the whole delivered set as a ZIP. There is no separate publish step: later additions and removals reach clients without notice, and a photo is exposed only once it is ready. Removing every finished photo after delivery is allowed and leaves clients an empty delivered gallery, with no revert to proofing and no unavailable page. Restore from archive does not revert delivery to proofing; withdrawal to proofing is not an MVP feature.

### FR-11

**Client activity indicators.** Gallery cards show the status (Draft, Shared or Delivered, or Expired or Archived when access is blocked), current selected counts (emphasised when over the included count), and distinct finished-photo download counts. Downloaded means the client requested a download, not proof of successful saving. Repeated requests for one photo do not increase its distinct count. A full-gallery request counts the files included in that download. Show All downloaded only for a nonempty currently delivered set whose photos have all been requested. Reading notifications does not clear activity counts. Default: counts refer to the current delivered photo identities; newly added photos start undownloaded, and removed photos no longer contribute. Replacement identity semantics will be defined in delivery design.

### FR-12

**Grouped notifications.** The top bar reports selection changes and finished-photo download activity. Maintain one unread notification per gallery, accumulating activity until read. Later activity creates a new unread notification after the prior one is read. Read state is workspace-wide. Opening the notification panel alone does not mark anything read. Clicking a notification opens its gallery and marks that gallery's notification read; the panel's single Mark all as read link clears every remaining unread notification without navigation. Opening a gallery by another route does not implicitly mark it read. Details must describe the relevant activity rather than treating every favorite change as a separate unread item.

### FR-13

**Expiry, archive, and deletion.** Expiry is optional; when reached, it blocks client access to gallery content and downloads without deleting photos, selections, or activity. Extending it restores access unless archived. Archive also blocks client access and preserves all data. Restoring an archive reuses the same link and still respects expiry. The photographer retains access to manage expired/archived galleries. Confirmed deletion removes the gallery and its assets, including generated files. Active jobs must not recreate deleted assets. None of these operations can revoke copies a client already obtained. No automatic asset deletion occurs on expiry or archive. Clients get one generic unavailable page for every link they cannot use, without studio branding and without revealing why.

## Non-Functional Requirements

### NF-01

**Access and privacy.** Authenticate photographer operations and authorize client operations against the shared link and current gallery access state. File paths or guessed URLs must not bypass those boundaries. Keep proofing originals and draft delivery files out of client access, including downloads and events. Use established password hashing and session security practices; local-network scope does not waive validation or authorization. Invalid uploads and user-controlled filenames must not permit unintended file access. Concrete session, request-protection, and media-serving mechanisms belong in technical design.

### NF-02

**Persistence and consistency.** Preserve accounts, galleries, favorites, activity, queue work, and files through normal application restarts and container recreation with persistent storage. Accepted jobs survive restarts; interrupted work becomes eligible again. Retry safely without duplicate published assets or corrupt results. Bound repeated failures, expose their status, and allow manual retry. Selection exports and delivery publication must be consistent under concurrent changes. Deletion must coordinate with processing and publication so late work cannot recreate accessible deleted content. Database migrations must support schema evolution; migration details belong in implementation design.

### NF-03

**Usability and accessibility.** All photographer and client journeys must work in mobile and desktop browsers. Provide responsive layouts, keyboard-operable controls, visible focus, meaningful labels and status/error messages, and useful empty/loading/failed states. Do not require native apps or disable accessible browser interaction to suggest image protection. Define the concrete browser test matrix in frontend design rather than promising support for every browser.

### NF-04

**Resource use and responsiveness.** Evaluate the MVP with representative galleries of 50–100 JPEG photos; this is not a hard count limit or a fabricated latency target. Bound upload, decode, processing, connection buffering, and temporary-file resource use. Offload image processing to the worker and expose its status without blocking normal gallery interaction. Disk-full and processing failures must not corrupt accepted data or be reported as successful completion. Choose numeric defaults and concurrency limits in technical design.

## Technical Constraints

### TC-01

**Application stack.** Rust backend with Actix Web; SQLx with PostgreSQL for persistent application data; SvelteKit frontend with Tailwind CSS and shadcn-svelte. Use needed components rather than importing an entire UI catalog. Verify dependency compatibility during implementation. Some choices are not yet installed or implemented; see [ADR 0001](adr/0001-technology-baseline.md).

### TC-02

**Deployment.** Docker Compose is the deployment baseline. MVP is a local-network workflow trial, with one photographer and workspace per installation. It is not a public-internet/SaaS release. Initial account setup uses the normal login URL. Host bindings, local-network reachability, session configuration, and browser transport behavior must be verified during deployment design; the current scaffold's localhost-only bindings do not establish LAN access.

### TC-03

**File storage.** Use persistent local-disk storage outside public static directories and container writable layers. Keep storage responsibilities cohesive; do not implement unused backends or an extensible storage framework for hypothetical needs. S3-compatible object storage is not an MVP dependency.

### TC-04

**Worker and queue.** A separate image worker uses a durable queue represented by PostgreSQL tables, with the invariants in NF-02. No separate queue service is required. Table schema, claims/leases, retry policy, and recovery mechanisms belong in the technical design.

### TC-05

**Browser updates.** Use SSE through Actix and actix-web-lab for processing, ZIP readiness, gallery changes, and notification updates; normal HTTP handles actions. Accept the dependency's evolving API, localize its use, and verify compatibility. PostgreSQL remains authoritative; SSE is not a durable activity record. Reconnecting clients refresh current state. Worker-to-API signaling, keep-alives, cleanup, slow consumers, and access-state changes need technical design; do not introduce another event broker by default.

## Scope & Exclusions

The [MVP release document](releases/MVP.md) defines delivery scope and acceptance. Its fixed defaults are local-network evaluation, one account/workspace, JPEG photo inputs, direct local-disk storage, full retry of failed uploads, and unchanged source/finished downloads. Keep resource validation, privacy, accessibility, and restart safety in scope.

**Future product direction:** more languages, a language per gallery, built-in email/SMS sharing, Google sign-in, additional photo formats including RAW, collaborator accounts in a workspace, and isolated-installation SaaS provisioning with private setup links. These have no committed release dates.

**Deferred possibilities, subject to need:** password recovery, backup/restore tooling, client confirmation of selection, access codes, configurable metadata policies, resumable uploads, object storage, more gallery layouts, and manual photo ordering (out of the MVP unless a design specifies it). They are not automatically promised for the next release.

**Permanent exclusion:** billing, pricing, invoices, payment processing, and enforcement of payment status. Included photo counts are proofing information only.

Detailed database schemas, API contracts, cookie lifetimes, event formats, upload limits, logo formats, image/ZIP libraries, and retry defaults belong in approved implementation designs. Resolve these before the affected feature is implemented; do not expose every engineering default as a photographer setting.

## Acceptance Criteria

These product-wide guarantees guide release-specific acceptance; they are requirements, not evidence that implementation is complete.

- **AC-01 — Access boundaries:** unshared/expired/archived/deleted galleries and draft delivery files cannot be retrieved by clients through page, media, download, or event routes. A valid proofing link never grants access to uploaded proofing originals. Photographer access remains available for nondeleted galleries. See FR-04, FR-06, FR-10, FR-13, NF-01.
- **AC-02 — Stable source files:** selection ZIP entries preserve uploaded originals; finished individual and ZIP downloads preserve the delivered uploaded files. Generated previews are separate derivatives with correct orientation/color and without source identifying metadata. See FR-05–FR-06, FR-09–FR-10.
- **AC-03 — Selection consistency:** shared favorites persist across visits, informational allowance changes never erase/block choices, and a ZIP reflects the selection at request time without locking the live selection. After delivery, favorite changes are rejected by the server. See FR-08–FR-10, NF-02.
- **AC-04 — Delivery:** proofing can be shared empty and populated progressively; delivery is allowed after a warning even with no ready finished photos and switches clients to the finished set. After delivery, finished-photo additions and removals reach clients without a publish step, and an empty finished set is allowed. Archive/restore does not change proofing/delivery state. See FR-04, FR-10, FR-13.
- **AC-05 — Activity truth:** counts represent distinct current delivered-photo download requests, not confirmed saves; notifications group unread activity by gallery and follow explicit read actions. New activity after reading becomes unread. See FR-11–FR-12.
- **AC-06 — Recovery and deletion:** restart/retry does not lose accepted jobs or corrupt/duplicate published results; stale jobs cannot recreate a deleted gallery's accessible assets. Expiry/archive delete nothing. See FR-13, NF-02, TC-03–TC-04.
- **AC-07 — Responsive use:** the complete photographer/client journey is usable in mobile and desktop browsers, including keyboard interaction and visible processing/failure states. Representative trial galleries remain operable within defined implementation resource limits. See NF-03–NF-04.
- **AC-08 — Setup and updates:** only one initial account is created, persistent sessions, activity renewal, inactivity expiry, and logout follow their contracts, and an SSE reconnect restores authoritative state without leaking inaccessible gallery data. See FR-01–FR-02, NF-01, TC-05.
