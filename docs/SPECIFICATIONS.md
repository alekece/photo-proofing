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
3. **Choose and review:** the client adds/removes favorites and discusses individual proof photos over one or more visits. The photographer sees activity on gallery cards and in grouped notifications, and replies in the photo viewer. Additional photos beyond the included allowance remain selectable.
4. **Export:** the photographer requests a ZIP of the current selection. The application displays preparation status and initiates the download when ready. Retouching happens outside PhotoProofing; favorites remain editable until delivery.
5. **Deliver and update:** upload an independent set of finished photos, review privately, and click Deliver; delivering before any finished photo is ready is allowed after a warning. At the same link, clients now see finished photos, cannot edit favorites, and can download individually or together. Later additions and removals reach clients without a further publish step.
6. **End access:** expiry or archive blocks access without deleting data. Extending expiry or restoring an archive can restore access at the same link. Confirmed deletion removes the gallery and its assets.

## Functional Requirements

### FR-01

**Account setup and sessions.** An installation with no account offers one-time account creation through its normal login URL. Setup must create only one initial account, including under concurrent requests. Registration closes afterward. Email/password sign-in and logout are required for MVP. Use one persistent cookie-backed session policy without a Remember me control. The operator can configure a human-readable inactivity timeout, defaulting to 30 days. Authenticated user activity renews expiry; passive status checks, background polling, SSE, and keep-alives do not. No absolute lifetime limit applies. Server-side expiry remains authoritative, and logout invalidates the session, including copied tokens, without logging out independent sessions on other devices. Google sign-in and password recovery are not MVP features.

### FR-02

**Workspace ownership and contacts.** Galleries, workspace contacts, saved watermark settings, the interface language, activity, and notification read state belong to the workspace. The MVP provides a single account in a single workspace, with no collaborator-management interface. The ownership boundary must allow later collaboration without assigning existing galleries to isolated personal environments.

The photographer maintains a workspace-wide contact directory (`/contacts`) reachable from the top navigation bar alongside Galleries. Each contact stores a trimmed display name (1–120 characters), a canonical normalized email address unique within the workspace, an optional phone number (up to 40 characters), and optional studio notes (up to 2000 characters), plus the list of workspace galleries linked to that contact. Contacts can be linked to or unlinked from individual galleries (`Studio → Contacts`, including creating and linking a contact in one step). Unlinking removes only that gallery association while keeping the workspace contact; deleting a contact or gallery removes its associations without deleting the other side.

### FR-02a

**Languages.** The product is available in English and French, covering the photographer app, every client page and the unavailable page, including dates. The photographer chooses the language once for the workspace with a switch beside the notifications bell, and it saves immediately. Clients always see the workspace language. New workspaces start in English; the first-run sign-in page follows the browser language, falling back to English. Photographers' own content (gallery titles, studio name) is not translated.

### FR-03

**Gallery configuration.** The photographer can configure title, optional shoot date (a calendar day, shown in the studio and formatted on the client cover when set), photographer-only gallery favorite pinning (sorting favorited galleries first across all dashboard sort modes without exposing that pin to clients), hero photo, cover layout, cover focal point, client heading typography preset, photo grid layout, grid density, grid spacing, optional expiry, optional included photo count, optional 6-digit numeric access code (`000000`–`999999`, configurable, auto-generatable, and clearable while in `Draft`, encrypted at rest when the installation's `[gallery_password]` key is configured, and permanently locked once shared), and watermark enablement. Offer four selectable client cover layouts (`full_bleed` by default, `split`, `framed`, and `banner`) and an interactive percentage focal point (`x`, `y` in `0..=100`, defaulting to `50, 35`) that anchors the cover crop across client covers, previews, the studio cover rail, and dashboard cards without modifying stored image files. Offer four self-hosted client heading typography presets (`editorial` by default, `fine_art`, `timeless`, and `modern`) applied strictly to the client cover heading and client sticky-bar heading without external font CDN requests or changes to studio controls, buttons, counts, or badges. Offer even rows by default and staggered columns for the photo grid, paired with client grid density (`compact` by default, `large` with fewer columns and 1 column on narrow phone viewports) and spacing (`compact` by default, `generous` with wider gaps), sharing viewer and selection controls while photographer management grids remain unchanged. There is no automatic hero: a gallery has one hero or none, and without one the studio cover card and all four client cover layouts show a default cover built from the gallery title and disable focal-point editing. Exact title validation is a technical default; none of this may prevent the empty-sharing journey. The included count is informational, editable after sharing, and optional; no value means unlimited selection. The New gallery dialog asks for a title and an optional shoot date; every other setting is configured on the gallery page, including a live desktop/phone client preview in Studio `Design`, and the dashboard supports multi-gallery selection for batch favoriting, quick-editing titles and shoot dates, and confirmed batch deletion. Dashboard gallery cards show subtle inline icons beside the title when a gallery is favorited (`Star`), requires an access code (`Lock`), or when client access is closed by expiry or archiving (`EyeOff`).

The client hero uses the selected photo's clean reduced preview, with the studio signature drawn separately, even when its grid/viewer images are watermarked. It remains available during marked-copy updates. Clients can save this clean hero preview; the exception never exposes a proof original or arbitrary clean proofs in an enabled gallery. All ordinary photographer images stay clean.

### FR-04

**Private sharing.** Share gallery creates an unguessable private link; MVP sharing is copying that link. Linking a workspace contact to a gallery also creates a separate unguessable 22-character token for that `(gallery, contact)` association, omitted from photographer gallery-contact JSON responses and valid only while the parent gallery is shared, unarchived, and unexpired (linking a contact to a Draft gallery never grants client access before sharing). Both the general link and any linked contact's personal link resolve to the gallery's one shared selection, comments, and downloads, and client responses never expose contact identifiers or contact details; unlinking or deleting a contact immediately revokes only that contact's link while keeping the general link active. Anyone possessing an active link can act on the gallery's one shared selection while access is permitted; no viewer account is required, and when a gallery has an optional 6-digit access code configured before sharing, clients first pass a rate-limited full-screen access-code gate over the darkened clean cover photograph (or default backdrop) showing only the studio signature, gallery title, and optional shoot date before any gallery photos, favorites, comments, downloads, or open activity are served or recorded (unlocking sets a stateless signed `HttpOnly` cookie valid across both the general link and personal contact links). Share is never disabled: sharing works before photos are uploaded or ready, after a warning dialog, and clients then see an empty state ("Your photos are on their way."). The photographer may add/remove proofing photos after sharing. Only photos with the currently permitted pair prepared appear: initial processing exposes neither pair; marked updates do not hide valid clean proofs in disabled galleries. Removing a selected photo requires confirmation and removes it from the selection. Shared access must still respect expiry, archive, deletion, and finished-delivery state. When two or more photos are shown, clients can play or pause a sequential 4-second slideshow from the sticky gallery bar or inside the client photo viewer (including in owner preview and delivered galleries), without speed controls; playback pauses on manual navigation, favorite toggles, opening comments, reaching the last photo, or closing the viewer.

**Client preview.** The signed-in photographer can preview the client page inside the app from the gallery page, in desktop and mobile views, whatever the gallery's sharing or expiry state. The preview is read-only, creates no share link, records no client activity, and shows only reduced previews, never originals.

This preview matches client watermark selection and availability, including its clean hero, rather than the clean images on ordinary photographer surfaces.

### FR-05

**Uploads and processing status.** MVP proofing and finished-photo inputs are JPEG files, validated by actual content. Preserve uploaded originals and finished files unchanged. Show upload transfer progress separately from waiting/processing/ready/failed processing status. Processing continues after navigation away, and current status is available on return. Preserve successful uploads when others fail. Failed uploads restart from the beginning; partial-upload resumption is not required for MVP. Clean up incomplete files. Failed processing is visible and can be retried. Bound file size, decoded dimensions, and processing resources; concrete limits belong in technical design.

Each proof upload prepares clean and watermarked thumbnail/preview pairs regardless of the gallery switch. Marked-copy updates preserve already-published clean images and owner viewer/hero actions while still showing processing or failure and retry feedback.

### FR-06

**Proof previews.** Generate thumbnails and reduced-resolution/quality previews rather than exposing proofing originals. Preserve aspect ratio, correct orientation, and correct color appearance. Omit source identifying metadata, including GPS, from generated thumbnails and proof previews. The photographer picks one workspace-wide proof quality (low, medium or high) in Settings, which sets the preview and thumbnail resolution and compression; changing it regenerates every existing proof. Exact resolutions are implementation details; MVP has no custom-dimension, per-gallery quality, or metadata-policy controls. Uploaded originals and downloadable finished photos remain unchanged. Client access must never expose proofing originals, regardless of whether watermarking is enabled. The public client gallery suppresses the browser's image context menu and the iOS image long-press callout as casual deterrents; this does not protect against screenshots or other capture methods.

### FR-07

**Optional watermarking.** Save the watermark once at workspace level, using the studio name, logo, or both. Each gallery instantly enables or disables watermarking by selecting existing proof files, without background jobs or photo resets. Only the client gallery and its authenticated preview use marked files; dashboard/gallery images, tabs, viewers and Settings samples stay clean. The logo is a PNG or JPEG of any dimensions, separate from JPEG-only photo uploads. It is bounded only by a fixed byte cap of about 20 MiB and a fixed decompression-bomb cap of about 100 megapixels; the server shrinks it while loading and re-encodes it as a metadata-free PNG no larger than 512 px. The watermark is repeated across the photo or placed once at one of nine positions, at one of ten sizes. Per-gallery watermark designs are not required for MVP: a gallery only switches watermarking on or off. Saving an effective watermark change regenerates only marked copies across all galleries, including disabled ones, with truthful progress and retry feedback. Enabled client proofs remain hidden until their marked revision is current; disabled clients, clean covers and photographer images stay usable. Existing favorites are retained. Changes must not expose proofing originals; regeneration mechanics belong in the processing design. The workspace also chooses how the studio signs the client cover: name, logo, both or none, at one of ten sizes. Settings previews the watermark and the cover signature on one shared sample image, not a client photo.

### FR-08

Before the first share, the photographer's Favorites tab remains visible but disabled.

**Favorites and allowance.** Persist each favorite change immediately in the shared gallery selection; clients can edit across visits until finished photos are delivered. Do not require submission, a client confirmation button, or a retouching phase. Downloading a selection does not lock it. When an included count is configured, allow overflow and show the selected count (for example, 13 selected), emphasised in the accent colour when over the included count, without a separate extra count. Changing the included count never deletes favorites or prevents additional choices. Reaching it does not mean the client is finished. Do not introduce prices, invoices, payment checks, or billing workflows.

### FR-08a

**Photo comments.** Clients and the photographer share a plain-text conversation per proof photo, independent of favorites. Comments are trimmed, 1–500 characters, with line breaks but no rich formatting. Anyone using the private link may edit or delete client comments; the photographer may edit or delete only her own replies. Edits keep only the latest text and show an edited marker. Deleting a client comment leaves a placeholder for clients and retains the latest text, struck through, for the photographer until the photo or gallery is deleted; deleting a photographer reply removes it for good.

Client comment access requires an available shared gallery and ends on delivery, expiry or archive. The photographer retains access to proof threads, including after delivery. Comments are not enabled on finished photos in this version. Clients see replies on reopening the thread or reloading, with no live stream, email or phone notification. New client comments reach the photographer's grouped bell activity and refresh open threads through the existing event stream; edits, deletions and photographer replies do not create notifications.

The viewer keeps comments closed by default, opening beside rather than over the photo on desktop and below it on phones. Photo tiles show the current nondeleted comment count. Client writes are paced per gallery (configurable; default one post, edit or deletion per two seconds). Client posts stop at a configurable thread cap (default 100 messages, including deleted comments and photographer replies); photographer writes are exempt from both limits.

### FR-09

**Photographer selection export.** A single Download action prepares a ZIP containing the selected originals and initiates download when ready. Show disabled Downloading… while pending, replace it with Download afterward, and surface failures without a separate download-started message. The ZIP represents the selection when requested; later changes do not alter that export or lock the live selection. The Favorites tab also lets the photographer download one favorite original, or several chosen with select mode. No separate download-management screen is required for MVP. Empty-selection handling and temporary ZIP retention belong in export design.

### FR-10

The photographer's Final tab remains visible but disabled until the gallery has
been shared; sharing enables Favorites and Final without reloading the page.
Existing draft files are preserved.

**Finished delivery.** Finished photos form an independent set, with no required count, filename match, or mapping to favorites. Upload privately, individually or in batches, review, then click Publish to make them available at the existing private link. Publish is never disabled: when no finished photo is ready, or some are still uploading, processing or failed, a dialog warns before publishing, and only ready photos reach clients. A published gallery with no ready finished photos shows clients "Your finished photos are on their way." After publishing, clients see only finished photos, cannot change favorites, and can download one photo or the whole delivered set as a ZIP. Later additions and removals reach clients without notice, and a photo is exposed only once it is ready. Removing every finished photo after publishing is allowed and leaves clients an empty delivered gallery, with no revert to proofing and no unavailable page. Restore from archive does not revert delivery to proofing; withdrawal to proofing is not an MVP feature.

### FR-11

**Client activity indicators.** Gallery cards show the status (Draft, Prepublish or Publish, or Expired or Archived when access is blocked), current selected counts (emphasised when over the included count), and distinct finished-photo download counts. Downloaded means the client requested a download, not proof of successful saving. Repeated requests for one photo do not increase its distinct count. A full-gallery request counts the files included in that download. Show All downloaded only for a nonempty currently delivered set whose photos have all been requested. The gallery page shows when the client last opened the shared gallery, or that it has not been opened yet. Photo tiles show when the client favorited a photo and when they first downloaded a finished photo. Reading notifications does not clear activity counts. Default: counts refer to the current delivered photo identities; newly added photos start undownloaded, and removed photos no longer contribute. Replacement identity semantics will be defined in delivery design.

### FR-12

**Grouped notifications.** The top bar reports selection changes, finished-photo download activity, and when a client opens the gallery. Maintain one unread notification per gallery, accumulating activity until read. Later activity creates a new unread notification after the prior one is read. Read state is workspace-wide. Opening the notification panel alone does not mark anything read. Clicking a notification opens its gallery and marks that gallery's notification read; the panel's single Mark all as read link clears every remaining unread notification without navigation. Opening a gallery by another route does not implicitly mark it read. Details must describe the relevant activity rather than treating every favorite change as a separate unread item.

### FR-13

**Expiry, archive, and deletion.** Expiry is optional; when reached, it blocks client access to gallery content and downloads without deleting photos, selections, or activity. Extending it restores access unless archived. Archive also blocks client access and preserves all data. Restoring an archive reuses the same link and still respects expiry. The photographer retains access to manage expired/archived galleries. Confirmed deletion removes the gallery and its assets, including generated files. Active jobs must not recreate deleted assets. None of these operations can revoke copies a client already obtained. No automatic asset deletion occurs on expiry or archive, except that derived download files (the delivered-set ZIP) may be removed and are rebuilt when access returns. Clients get one generic unavailable page for every link they cannot use, without studio branding and without revealing why.

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

**Deferred possibilities, subject to need:** password recovery, backup/restore tooling, client confirmation of selection, post-share access-code rotation or per-contact separate access codes, configurable metadata policies, resumable uploads, object storage, more gallery layouts, and manual photo ordering (out of the MVP unless a design specifies it). They are not automatically promised for the next release.

**Permanent exclusion:** billing, pricing, invoices, payment processing, and enforcement of payment status. Included photo counts are proofing information only.

Detailed database schemas, API contracts, cookie lifetimes, event formats, upload limits, logo formats, image/ZIP libraries, and retry defaults belong in approved implementation designs. Resolve these before the affected feature is implemented; do not expose every engineering default as a photographer setting.

## Acceptance Criteria

These product-wide guarantees guide release-specific acceptance; they are requirements, not evidence that implementation is complete.

- **AC-01 — Access boundaries:** unshared/expired/archived/deleted galleries and draft delivery files cannot be retrieved by clients through page, media, download, unlock, or event routes (always returning the same unavailable response rather than an access-code gate). While an active shared gallery with a 6-digit access code is locked, only gate metadata, the clean cover preview, and the studio logo are served; photo lists, thumbnails, previews, downloads, favorites, comments, and open tracking are refused until unlocked. A valid proofing link never grants access to uploaded proofing originals. Photographer access remains available for nondeleted galleries. See FR-03, FR-04, FR-06, FR-10, FR-13, NF-01.
- **AC-02 — Stable source files:** selection ZIP entries preserve uploaded originals; finished individual and ZIP downloads preserve the delivered uploaded files. Generated previews are separate derivatives with correct orientation/color and without source identifying metadata. See FR-05–FR-06, FR-09–FR-10.
- **AC-03 — Selection consistency:** shared favorites persist across visits, informational allowance changes never erase/block choices, and a ZIP reflects the selection at request time without locking the live selection. After delivery, favorite changes are rejected by the server. See FR-08–FR-10, NF-02.
- **AC-03a — Photo conversations:** clients can add, edit and delete client comments only on available proofing links; the photographer can reply in her own proof threads. Deleted client text is never sent to clients but remains visible to the photographer; a deleted photographer reply is removed entirely. Counts exclude deleted messages; new client posts notify only the photographer. The panel starts closed, does not cover the photo, and is absent on finals. The server enforces client write pacing and the thread cap, including concurrent posts. See FR-08a, NF-01, NF-03.
- **AC-04 — Delivery:** proofing can be shared empty and populated progressively; delivery is allowed after a warning even with no ready finished photos and switches clients to the finished set. After delivery, finished-photo additions and removals reach clients without a publish step, and an empty finished set is allowed. Archive/restore does not change proofing/delivery state. See FR-04, FR-10, FR-13.
- **AC-05 — Activity truth:** counts represent distinct current delivered-photo download requests, not confirmed saves; notifications group unread activity by gallery and follow explicit read actions. New activity after reading becomes unread. See FR-11–FR-12.
- **AC-06 — Recovery and deletion:** restart/retry does not lose accepted jobs or corrupt/duplicate published results; stale jobs cannot recreate a deleted gallery's accessible assets. Expiry/archive delete nothing. See FR-13, NF-02, TC-03–TC-04.
- **AC-07 — Responsive use:** the complete photographer/client journey is usable in mobile and desktop browsers, including keyboard interaction and visible processing/failure states. Representative trial galleries remain operable within defined implementation resource limits. See NF-03–NF-04.
- **AC-08 — Setup and updates:** only one initial account is created, persistent sessions, activity renewal, inactivity expiry, and logout follow their contracts, and an SSE reconnect restores authoritative state without leaking inaccessible gallery data. See FR-01–FR-02, NF-01, TC-05.
