# Staging frontend cleanup

Issue: [#224](https://github.com/alekece/photo-proofing/issues/224).
Related: [#219](https://github.com/alekece/photo-proofing/issues/219) (original downloads),
[#222](https://github.com/alekece/photo-proofing/issues/222) (cover signature),
[#71](https://github.com/alekece/photo-proofing/issues/71) (future delivery).

Status: approved by the maintainer, with a final independent check confirming
that it matches the staging feedback.

## Intent and boundaries

Make the photographer's gallery, Settings and dashboard calm, responsive and
accurate, addressing all fourteen staging observations and #224's existing
journey-header and select-mode requirements. Preserve the warm-paper design,
existing access rules, upload recovery and asynchronous processing.

This is not a redesign. It does not implement delivery, client OTP, email/SMS,
notifications, quota policy or unrelated unfinished product journeys.

## Approaches

Keep one bounded cleanup with targeted changes to existing components and APIs.
A frontend-only patch cannot complete original downloads or reliably address
watermark freshness. A redesign or separate preview-link mode adds unnecessary
scope. These are rejected in favor of reusing current infrastructure.

For photographer access, the maintainer explicitly chose removing Open and
keeping the existing read-only Preview instead of adding a second link mode.
A photographer who copies the real client URL can still act as a client.
A query flag must not become an authorization bypass.

## Observable behavior

1. Upload state labels and the progress animation stay inside their own tiles,
   in the intended status area, for waiting, uploading and processing. Check
   both initial empty uploads and uploads into an existing gallery.
2. Photo-deletion confirmation visibly enters a busy state, prevents duplicate
   confirmation and retains failure feedback. Use the established dialog
   transition and animate the pending confirmation, not just the removed tile.
   Successful deletion uses the
   existing shrink/fade motion; reduced motion is respected. This does not
   implement the unfinished whole-gallery deletion endpoint.
3. Favorites offer individual original downloads on hover, keyboard focus and
   touch. Wire the existing tile/viewer download seams to authenticated
   photographer access, never exposing stored originals through a client token.
   Select-mode download uses the explicitly selected favorite subset; the
   normal Download action exports the whole favorite selection. Individual
   originals are streamed directly, without creating or replacing a ZIP job.
   Whole-selection and subset ZIP requests share one active ZIP slot per gallery:
   while one is preparing, both ZIP actions are disabled and the server rejects
   competing requests explicitly rather than silently replacing an active job.
4. Remove the photographer's Open client-link shortcut; preserve Copy link and
   the existing read-only Preview, with no client-open tracking or favorite
   mutation originating from Preview.
5. The Deliver dialog reports the finals count, including zero, independently
   of the proof and favorite counts. Delivery itself remains explicitly
   unavailable until #71; do not turn its existing error into apparent success.
6. Remove persistent Saved text near Preview and repeated success toasts for
   gallery autosaves. Preserve accessible saving state and actionable errors;
   title retry and out-of-order-save protection continue to work.
7. Saving gallery edits does not remount photo panels, clear loaded photos,
   reset the viewer or cause page flashes. Diagnose reactive dependencies,
   requests and navigation before editing; refresh only what actually changed.
8. Favorites' normal export button reads Download. From request dispatch through
   ZIP preparation it reads Downloading... and is disabled, including before the
   POST responds. Once the browser download is initiated it returns to Download.
   Remove separate download-started success text. Keep visible preparation/
   download errors and allow retry. Browser initiation is not proof that the
   entire file finished transferring.
9. Cover signature uses centered logo above centered text when both are selected.
   Implement the saved signature on the client cover and Preview, including
   text-only, logo-only and none. Logo access follows the shared gallery's
   availability and access checks; do not expose private Settings wholesale.
10. Settings Save changes becomes disabled Saving... while the request is
    pending. Retain the existing brief saved-button completion animation and
    error feedback without overlapping timers or duplicate requests.
11. After watermark processing finishes, every proof, including the first
    viewer image and hero where applicable, uses the current rendered asset.
    Diagnose worker publication, events and image caching before choosing a fix.
    Use meaningful server asset revision data if cache invalidation is needed,
    not random timestamps or unconditional reloads. Finals and original downloads
    remain unwatermarked. A database migration and revision propagation through
    publication, API responses and asset URLs are in scope if reproduction
    confirms that existing metadata cannot identify the current asset.
12. Photos, Favorites and Finals share the same empty-state strip background,
    motion and responsive sizing; retain each tab's distinct wording.
13. Dashboard search has a warm, restrained, visible keyboard-focus indicator,
    without the heavy black border or layout changes.
14. Dashboard status menu remains aligned to the input's right edge as labels
    change. Selection does not move the menu anchor or resize its trigger.
15. Preserve #224's existing scope: center gallery journey steps; select mode
    keeps the toolbar in place, close/count on the left, Select all and the
    appropriate Delete/Download action, without a dark replacement bar.
16. Until a gallery has been shared, Favorites and Finals tabs remain visible
    but disabled for pointer and keyboard interaction. Sharing enables both
    immediately; no reload is required. Existing photo data is preserved.

## Implementation boundaries and builder decisions

Use existing SvelteKit components, async button/dialog state, upload queue,
events, API authentication, original storage and ZIP-job infrastructure.
Validate requested export subsets against gallery membership and proof kind.
Reuse the existing export constraints, failure propagation and retention.
Bound request size; reject invalid or unavailable selections explicitly.
Extend the ZIP request shape and persisted snapshot for explicit subsets; do not
build a concurrent multi-export system. Direct originals use photographer
authentication and scoped storage resolution. The client signature-logo route
must reuse `ClientAccess` and the same unavailable-link response semantics as
existing shared routes, with a fixed normalized image content type.

A builder may choose endpoint and payload details within those constraints,
extract a shared empty-state presentation if it removes duplication, and fix
reactive state or asset-version boundaries supported by reproduction evidence.
Do not guess a backend watermark cause from its UI symptom.

Update affected UI screen docs and current implementation claims. Reconcile
only the product requirements directly changed by this approved design; retain
the explicit delivery restriction. Track overlap with #219 and #222 in the PR
without silently closing partially delivered issues.

## Evidence and completion

Regression tests cover pending/double-click/error states; zero/different finals
counts; rapid and out-of-order edits; no panel reset on unrelated saves; ZIP
whole/subset/single-photo access; client original-download denial; signature
variants; watermark revision changes including the first viewer image.

Inspect the real UI at desktop and 390 px with mixed proofs/finals, favorites,
empty tabs, delayed responses and failed requests. Capture before/after PNGs for
the visual changes, including upload, delete confirmation, signature, empty
states and dashboard focus/menu positioning. Respect reduced motion and keyboard
access, and use existing localization conventions.

Run `pnpm --dir web verify` and the Rust format/lint/tests required by
`crates/AGENTS.md` for backend changes. Independent review must check correctness
and this design. Perform a bounded refactor pass, rerun affected checks and
retain the implementation plan through review before final merge cleanup.
