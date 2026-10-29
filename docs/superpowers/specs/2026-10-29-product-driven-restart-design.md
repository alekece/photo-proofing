# PhotoProofing product-driven restart

Status: Approved in conversation, including the release-history and delivery-readiness defaults. This document consolidates the brainstorming decisions; it does not claim that the product features are implemented.

## Intent and scope

Restart development around a small, complete photographer-to-client workflow. The MVP is a locally deployed, local-network trial with replaceable data, usually 50–100 JPEG photos per gallery. That range is a sizing expectation, not a hard limit. The photographer keeps source photos elsewhere. Billing is outside this repository's product scope.

This change reorganizes and writes documentation and updates repository agent workflow guidance. It does not implement the application, install dependencies, refactor existing code, publish GitHub items, or change deployment exposure. Preserve unrelated staged and working-tree changes.

## Documentation ownership

```text
README.md
AGENTS.md
docs/
  SPECIFICATIONS.md
  releases/
    MVP.md
  adr/
  superpowers/
    specs/
    plans/
```

- README.md: concise product overview, truthful current implementation status, setup/run/verification instructions, and documentation links. Incorporate useful CONTRIBUTING.md material, correct obsolete references to closed issues, and remove CONTRIBUTING.md. Verify commands against current files rather than presenting planned features as working setup.
- AGENTS.md: the single shared agent guidance file. Retain the approved Rust policy; add document ownership and lifecycle rules. No duplicate harness-specific policy files.
- SPECIFICATIONS.md: authoritative, evolving product-wide requirements. Use sections: Vision & Problem Statement; Target Users; User Journeys; Functional Requirements; Non-Functional Requirements; Technical Constraints; Scope & Exclusions; Acceptance Criteria. Distinguish personas from application permission roles. Distinguish permanent exclusions from deferred features.
- releases/MVP.md: release scope, explicit deferrals, and an observable end-to-end acceptance checklist. Reference product requirements instead of restating the whole PRD. Once completed, retain the release as a historical record; do not rewrite it to match later scope. Record the accepted PRD commit/tag at release completion so historical references cannot silently change meaning.
- adr/: significant architectural decisions, their context, alternatives, and consequences. Start with a concise technology-baseline ADR reaffirming current and agreed choices; do not invent historical rationale or create one ADR per dependency. Additional ADRs need a significant independent decision.
- superpowers/specs/: retain approved designs. Product-wide specifications own current requirements; dated designs explain particular changes and can be explicitly superseded when requirements change. Preserve the existing approved Rust-guidance design.
- superpowers/plans/: working implementation plans remain through implementation and PR review, then are removed before merge. Before removal, retain any lasting decisions or needed verification information in the appropriate permanent document or PR. Permanent documents must not depend on links to removed plans. This applies to the existing Rust-guidance plan at its merge boundary too, not by deleting it during this design discussion.

## Approaches

Chosen: one current product specification plus release-specific scope documents. This separates current requirements from historical release commitments without duplicating the PRD.

Alternative: a single specification with version sections. Fewer files initially, but it mixes evolving requirements with immutable release history.

Alternative: detailed documents per subsystem and extensive ADRs. Rejected for the restart because they add maintenance and encourage implementation-first planning.

Development starts from a user journey and a bounded user-visible slice. Use a Superpowers design when decisions need discussion, then a temporary implementation plan. Existing scaffolding is input, not proof that historical product scope is still required. No automated issue/PR orchestration is introduced.

## Agreed MVP behavior

### Installation and authentication

- One installation contains one workspace and one photographer account. Galleries, settings, activity, and notification state belong to the workspace, leaving room for collaborators later without implementing collaboration now.
- On an empty database, the normal login URL offers one-time account creation. Once an account exists, registration closes. Concurrent setup requests must not create multiple initial accounts.
- Email/password authentication only. A Remember me checkbox enables persistence across browser restarts; unchecked uses a browser-session cookie. Both use bounded server-side expiry; logout invalidates the session. Browser session restoration behavior is not a guarantee the application can control.
- Google sign-in, password recovery, account-management/collaboration screens, roles, invitations, workspace switching, and SaaS provisioning are not MVP features.
- All screens support mobile and desktop browsers. No native app.

### Gallery setup, access, and lifecycle

- Photographer can set a title, hero photo, layout, optional expiry, optional included photo allowance, and per-gallery watermark enablement.
- Two layouts: uniform grid by default, and masonry. They share viewer and selection behavior.
- Share gallery generates an unguessable private link. No client account or access code in MVP. Anyone with the link shares the same selection; do not model individual viewers as separate clients.
- Sharing is allowed even when no photos are ready, with an empty-gallery warning. Clients see a useful empty state until photos become ready. Adding/removing proofing photos remains possible after sharing; only fully processed photos appear.
- Removing a selected proofing photo requires confirmation and removes its favorite too.
- Expiry is optional and only blocks client access. It deletes no assets, selections, or activity. Extending expiry restores access unless archived.
- Archive blocks client access and preserves everything. Restore reuses the same link and still respects expiry.
- Explicit confirmed deletion removes the gallery and its assets. Active jobs and existing media links must not bypass deletion or recreate deleted assets.

### Uploads and previews

- JPEG-only photo uploads for proofing and finished delivery. Store uploaded files unchanged. Validate actual content and bound byte size, decoded dimensions, and resource use; exact defaults belong in technical design.
- Successful uploads remain when another upload fails. Retry failed files from the beginning; resumable partial uploads are deferred. Incomplete files must be cleaned up.
- Show upload progress separately from processing status (waiting, processing, ready, failed). Processing continues after navigating away, and current status is available on return.
- Generated proof previews reduce resolution and quality to discourage their use as substitutes for finished photos. Exact dimensions/compression are implementation choices, without photographer quality controls in MVP. They cannot prevent screenshots or copying browser-visible pixels.
- Optional watermark: saved workspace text, logo, or both, with a gallery on/off setting. Logo format validation is separate from the JPEG-only photo requirement; supported logo formats and limits need a technical-design decision.
- Generated thumbnails and proof previews omit source identifying metadata, including GPS. Correct orientation and color appearance must be preserved. Original files and finished downloadable uploads remain unchanged. Metadata options are deferred.
- Proofing originals are not client-accessible, including through guessed/direct file URLs. Disabling right-click is not a protection guarantee; protective UX may be considered later without impairing accessibility.

### Selection and photographer export

- Favorites save immediately and remain editable until delivery. No submission button, retouching phase, or selection lock triggered by downloads.
- An optional included photo allowance is informational, editable after sharing, and never blocks additional favorites or removes choices. With no allowance, selection is unlimited. Show selected and extra counts where applicable; reaching the allowance is not a completion signal.
- No billing, pricing, invoices, payment processing, or enforcement of payment status.
- Download selection is one action producing a ZIP of selected originals. Show Preparing download, then initiate the download when ready. No separate download-management or individual-original-download interface is needed.
- Export uses the selection at request time; later edits do not change that ZIP. This consistency is a technical requirement, not a client selection lock.

### Finished delivery

- Finished photos form an independent set: no matching algorithm, filename mapping, or required correspondence to selected photos.
- Photographer can upload finished photos singly or in batches and review them privately. Deliver publishes the reviewed delivery set at the same private link; see the approved readiness rule below. Client then sees only finished photos, cannot change favorites, and can download individually or all together as a ZIP.
- After delivery, photographer can add/remove finished photos singly or in batches. Proposed changes remain private until Deliver publishes the update; the current published set remains available until then. Publication must not expose incomplete results or remove files still needed by the currently published set.
- Expiry and archive restrict finished delivery access too.

### Activity and notifications

- Gallery cards show client activity: selected count, included/extra counts where relevant, and distinct delivered-photo download counts. Do not label the included allowance as All selected or imply a client submission.
- Downloaded means a requested download, not proof the client saved the file. Repeat requests for the same delivered photo do not increase the distinct count. A full-gallery download counts all files included in that request. Use All downloaded when all currently published delivered photos have been requested; never imply completion for an empty delivery.
- Top-bar notifications cover selection changes and delivery download activity. One unread notification per gallery accumulates activity; activity after it is read creates a new unread notification. Read state is workspace-wide.
- Opening the notification panel does not mark notifications read. Clicking a notification opens its gallery and marks it read; a separate Mark as read control clears unread state without navigation. Opening a gallery by another route does not implicitly mark it read.

## Technical baseline and invariants

- Backend: Rust and Actix Web; database access with SQLx and PostgreSQL; deployment with Docker Compose.
- Frontend: SvelteKit, Tailwind CSS, and shadcn-svelte, using only needed components. Verify exact version compatibility before implementation; the stack decision is not evidence dependencies are installed.
- Local-disk photo storage persisted outside container writable layers and public static directories. Keep storage responsibilities cohesive without introducing unused backends or a speculative storage framework. S3-compatible storage is deferred.
- Separate image worker and durable queue represented by PostgreSQL tables. Accepted work survives restarts, interrupted jobs become eligible again, retries do not duplicate/corrupt results, and repeated failures are bounded and visible with manual retry. No separate queue service.
- SSE delivers browser updates for processing, ZIP readiness, gallery changes, and notifications; normal HTTP handles user actions. Use actix-web-lab's SSE utilities, accepting its API evolution policy. Localize its usage and verify compatibility. It is not necessary to adopt its other utilities unless useful.
- PostgreSQL is authoritative. Event delivery is not a durable record; reconnecting clients refresh current state. Events and media must respect photographer authentication and client gallery access restrictions. Worker-to-API signaling, keep-alives, connection cleanup, and slow-consumer behavior belong in technical design. No separate event broker required by this design.
- Preserve data through normal process/container restarts and recreation with persistent storage. Use database migrations. Backup/restore tooling is excluded from the workflow-trial MVP, not a permanent product exclusion.
- Resource limits, file cleanup, retry safety, access checks, and consistent publication/export are required even in the trial. Exact schemas, routes, image/ZIP libraries, cookie lifetimes, and numeric limits are deferred to implementation designs.
- Accessibility includes keyboard-operable controls, meaningful status/error messages, visible focus, and responsive client and photographer screens. Avoid fabricated performance targets; validate the trial journey with representative 50–100-photo galleries.

## Later product scope and exclusions

Agreed future scope: built-in email/SMS sharing, Google sign-in, additional photo formats including RAW, collaborator accounts in the workspace, and SaaS provisioning with private setup links. These are future directions, not committed release dates.

Possible later capabilities, without release commitment: client selection confirmation, access codes, configurable metadata handling, resumable uploads, object storage, backup/restore tooling, and further layouts. Evaluate actual need before implementation.

Permanent exclusion: billing and payment processing. Do not describe outside billing as a future built-in feature.

## Remaining implementation decisions and author proposals

- Choose bounded upload/decode limits, logo formats, queue retry/lease defaults, session lifetimes, SSE signaling, and ZIP retention during the relevant technical designs.
- Product defaults not yet explicitly chosen (e.g. initial hero selection or title validation) should be documented as proposed defaults in the PRD draft, not claimed as user-approved behavior.
- Propose that delivered-photo counts refer to identities in the current published set; newly added photos start undownloaded, and removed photos no longer contribute. File replacement semantics should be resolved in delivery design.
- Approved default: Deliver requires at least one finished photo and all photos in the intended draft delivery set to be ready. Pending/failed draft photos must finish, be retried, or be removed before publication; do not silently omit them. This guard is specific to delivery, not the explicitly permitted empty/progressively populated proofing share.
- No withdrawal from finished delivery back to proofing has been requested. Do not introduce such an action implicitly through archive/restore.
- Do not turn every engineering default into a new photographer setting.

## Documentation-change acceptance criteria

- Target documents have clear ownership and valid local links; README setup reflects current repository behavior and clearly distinguishes planned MVP behavior.
- PRD uses the agreed eight-section format, contains product-wide constraints and acceptance guarantees, and distinguishes current MVP, future scope, and permanent exclusions.
- MVP release scope includes the complete account-setup → gallery → sharing → favorites → export → delivery → downloads journey, plus activity, archive/restore, expiry, and failure/restart acceptance cases.
- Release acceptance criteria reference relevant PRD requirements rather than creating competing definitions. Historical releases and approved designs remain preserved.
- A concise baseline ADR records reasons and consequences for the agreed stack without inventing past decisions. AGENTS.md describes the documentation lifecycle and retains its approved Rust rules.
- CONTRIBUTING.md is removed only after useful setup and contributor instructions are consolidated. Unrelated code/configuration changes remain untouched.
- Working plans survive PR review and are removed at the pre-merge step; permanent design documents remain meaningful without them. No automatic commit, push, PR creation, or merge is part of this documentation task.

## Design review

An independent read-only builder review found no blocking contradictions. Historical PRD revision references and delivery readiness were identified during review and subsequently approved by the user. These decisions can be revised through future product changes. Dependency compatibility and application behavior have not been tested as part of this documentation design.

## Documentation delivery record

The PRD, MVP scope/acceptance record, baseline ADR, README consolidation, and agent lifecycle guidance were implemented and independently reviewed with no blocking findings. Requirement references and local links/anchors, preservation of existing Rust guidance, Compose configuration, and diff checks passed. Final Rust verification passed nightly formatting, workspace Clippy with warnings denied, and all 18 tests. Application startup and the unimplemented MVP journey were not tested. Completed implementation plans were removed before direct publication to main at the user request; approved designs remain. No product features or new dependencies were implemented by the documentation change.
