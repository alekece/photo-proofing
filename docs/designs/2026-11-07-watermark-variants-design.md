# Instant watermark switching and a clean hero

Status: approved by the user on 2026-11-07; implemented on the working branch.
Related issue: [#67](https://github.com/alekece/photo-proofing/issues/67).
Revises [the watermark design](2026-11-07-watermarks-design.md) and builds on
[proof derivatives](2026-11-04-proof-derivatives-design.md).
Requirements affected: FR-03 to FR-07 in
[the product specifications](../SPECIFICATIONS.md).

## Intent

A photographer can turn a gallery's watermark on or off without rebuilding images or
temporarily losing photos. The client cover stays clean, with the studio signature drawn
separately rather than appearing over a watermark already baked into the hero.

The user chose to generate both image pairs at upload and accepted that clients can save a
clean, reduced-resolution preview of the selected hero. Proofing originals remain private.
The user also accepted hiding watermarked proofs while a changed watermark design is rendered,
without hiding clean covers or interrupting galleries with watermarking disabled.

## Goals and boundaries

- Every successfully processed proof has a clean thumbnail and preview, plus a thumbnail and
  preview for the current workspace watermark, regardless of its gallery's switch.
- On/off switches update the gallery only: no image jobs, file changes or photo-status changes.
- The client cover uses the clean preview of the explicitly selected hero. Its grid tile and
  proof viewer still follow the gallery's watermark switch.
- Watermarked images belong only to the client-gallery presentation. Photographer surfaces
  always use clean derivatives: dashboard cards, gallery headers, photo/favorites tabs and
  their viewers, and Settings samples. The Settings watermark sketch overlays the configured
  mark on a clean sample, never on an already marked image.
- Changing the actual watermark rebuilds marked copies in every gallery, including galleries
  with watermarking disabled, so later toggles do not start processing.
- Preserve current orientation, colour conversion, aspect ratio, metadata stripping, access
  boundaries, durable jobs, failure reporting and retry behavior.
- Finals and original ZIP exports stay unchanged and never receive a watermark.
- No original-file hero, larger cover rendition, browser watermark overlay, extra settings,
  per-gallery watermark design or layout redesign.

This is a cross-layer change to processing, storage, readiness, serving and image URLs, not
just a cover fix. It costs additional derivative storage and initial rendering work.

## Approaches

1. **Generate both pairs at upload (chosen).** Extra encoding and storage upfront, but every
   prepared proof can switch immediately. Rendering and protection remain server-side.
2. **Generate marked copies on first enable.** Avoids some upfront work, but the first switch
   still waits for jobs; rejected because it does not satisfy instant switching.
3. **Render on request or overlay in the browser.** Request-time rendering adds latency and
   cache complexity; a browser overlay exposes the clean underlying image. Neither fits.

For the hero, the clean existing preview was chosen instead of the original. Its long edge
is at most 1600 px, JPEG quality 75, with no upscaling or source metadata. Large displays or
strong cover crops can look softer; a larger cover derivative is outside this change.

## Image generation and storage

The worker prepares the image once, retaining the existing JPEG validation, bounded decoding,
orientation and colour handling. For each existing output size it encodes a clean image and a
marked image from the same resized pixels; it does not watermark an already compressed JPEG.

Keep the existing thumbnail/preview keys for clean derivatives and add distinct marked
thumbnail/preview variants. Originals and final-photo keys do not change. When no effective
watermark is configured, the marked outputs can contain the same encoded bytes as the clean
outputs, so four valid files still exist and enabling the switch requires no special job.
Do not invent a default watermark or change existing settings validation.

Initial processing stages all four files and publishes them under the existing job/photo
publication locks. A watermark revision check is independent of the gallery switch. A
switch during processing must neither invalidate a render nor requeue it.

An update to the watermark stages and replaces only the marked pair when valid clean copies
already exist. Preserve the existing guarded publication protocol: a stale job cannot
overwrite newer files or declare stale marked copies current. Settings changes overlapping
initial processing, retries or another settings change must leave a durable job for the
latest revision. Publication after deletion must clean up all variants.

## Readiness and regeneration

**Builder decisions proposed here:** record clean-pair readiness and the published watermark
revision on each proof. File existence alone is not readiness. Keep the current photo
processing states and failure reasons for the photographer, but do not use one global
`ready` status as the sole client-access test during marked-only regeneration.

- A new proof is not exposed until initial publication succeeds.
- Clean-pair readiness remains valid during marked-only regeneration and its failures.
- The marked pair is usable only when its published revision equals the workspace's current
  effective-watermark revision.
- A changed effective watermark increments that revision and queues marked regeneration for
  proofs across the workspace, regardless of gallery enablement. Cover-signature-only and
  other ineffective settings changes still regenerate nothing.
- Existing waiting jobs read the latest revision. Processing jobs are superseded safely and
  requeued using the established transaction/lock ordering.
- Failed photos keep their explicit failure and manual retry behavior. Clean copies, if
  already published, remain usable; missing or stale marked copies remain inaccessible.
- Ready photos become ready again when their marked pair is current. Turning watermarking
  off while a marked update is pending or failed immediately allows their valid clean pair;
  turning it on immediately applies the current-revision gate, without creating a job.

This distinction also applies to client photo lists and selection eligibility: with
watermarking disabled a clean-pair-ready proof stays visible and selectable while marked
copies update; with watermarking enabled a stale proof is hidden and cannot be newly
selected. Existing favorites are retained, not deleted by a settings update. Original export
contracts are unchanged.

Reuse current progress events and retry UI. After a settings change, tell the photographer
that marked copies are updating, including in disabled galleries; do not claim that a
gallery toggle starts an update. Image-processing failures remain visible to the owner.

## Serving and client cover

The server, not an arbitrary browser parameter, chooses the image variant:

- Ordinary client thumbnail/preview routes choose the clean or current marked pair from the
  gallery's saved switch, preserving the existing private-link, expiry and photo-kind checks.
- Client routes never offer an unrestricted clean-variant selector. Missing marked copies
  must not fall back to clean copies or originals when watermarking is enabled.
- Add a dedicated cover-image route scoped to the shared gallery. It serves only the clean
  preview of that gallery's current explicit hero, when its clean pair is valid, even during
  marked-only regeneration. It accepts no arbitrary photo id.
- Resolve the cover separately from the visible proof list. A hero hidden from the marked
  grid during regeneration must still appear on the cover. Change the shared-gallery hero
  lookup as well: it currently suppresses heroes whose global photo status is not `ready`.
  Hero eligibility uses valid clean-pair readiness instead during marked-only updates.
- Removing/changing the hero, deleting its photo, expiry, unsharing, wrong-gallery access or
  absence of a valid clean pair must deny the relevant cover request. No automatic hero.
- The cover must obey the same permitted photo kind as the ordinary client photo set.
  It is an exception to watermark-variant readiness, not to photo-set authorization.
  Currently clients see proofs only, including for a gallery marked delivered. If a delivery
  change switches clients to finals, a proof hero outside that set is not served; the default
  title cover is shown. Choosing a different delivery hero policy is outside this change.
- Owner cover surfaces always use the clean hero. The authenticated photographer
  thumbnail/preview/list routes take an optional `?watermark=true|false` and default to
  clean; the caller chooses, and the database only checks that those files are ready.
  Photographer tabs and viewers stay usable during marked-only regeneration whenever their
  clean pair is valid; they still show truthful marked-copy processing/failure feedback.
- The owner's "Preview client gallery" matches the client: its grid and viewer follow the
  gallery switch and current marked-pair readiness, while its hero stays clean. It requests
  the variant matching the gallery switch from the ordinary owner routes. Public shared routes
  never accept a selector: they derive the variant from the saved switch. Its URLs change with
  the variant/revision even without photo events.

Clean hero access is an intentional exception, not copy prevention: clients may retain a
clean preview they previously received even after the hero changes or watermarking changes.
No policy can revoke already downloaded images.

Do not allow a browser cache to turn switching on into continued clean serving on client
surfaces. Ensure client image responses cannot be reused across watermark switches or
workspace revisions. Ordinary owner image URLs do not change merely because the switch
changes, since those images always stay clean.
Current image responses allow private caching. Change switch-sensitive proof image responses
to private/no-store; variant/revision URL identity must also prevent already-loaded owner
components from retaining the wrong image. Keep originals/final downloads outside this policy
change. Cover URLs must change when the hero changes so the browser replaces the displayed cover.
An already-open client page is not a live-sync feature in this change; subsequent requests
and reloads honor saved settings, but already displayed/downloaded bytes cannot be withdrawn.

## Existing installations

Existing thumbnail/preview files may already contain watermarks, so they must not be treated
as clean merely because they occupy the old keys. Introduce readiness with an unprepared
default and durably reprocess existing proofs once from their originals, generating all four
variants. Do not copy potentially marked old files into the clean pair.

During this one-time preparation, unprepared proofs and their covers are unavailable, with
the normal waiting/failure/retry feedback. Finals are not reprocessed. Deployment must prevent
old workers from publishing the old two-file format after the new readiness rules take effect.
Backfill jobs survive restart; failures do not silently declare old files usable.

Deletion, gallery cleanup and late-publication cleanup must remove both pairs along with
the existing original. Temporary staged files retain the existing cleanup behavior.

## Documentation changes after approval

Update FR-07 to describe instant gallery toggles, regeneration of marked copies across all
galleries, and continued clean-gallery/cover availability. Clarify FR-03's clean hero exception
without relaxing the prohibition on client access to proofing originals.

Update the existing watermark design in place to match the new processing/publication model
and link this decision; do not leave its chosen replacement-on-toggle behavior described as
current. Keep README implementation claims aligned with the delivered code.

## Acceptance and verification

- Upload with watermarking on or off produces valid clean and marked pairs; no configured
  watermark still yields usable pairs. Dimensions, orientation, colour and metadata policies
  are unchanged.
- A prepared gallery toggles in either direction without jobs, file writes or photo resets.
  Newly requested client images select the correct bytes; ordinary photographer grids,
  viewers, dashboard cards and Settings samples stay clean in either switch position.
- The clean hero is available when enabled, including while its marked copies update. Its
  ordinary thumbnail/viewer remain marked. Other clean proofs and all proofing originals
  remain unreachable through client routes when enabled.
- A watermark-design update rebuilds marked copies in enabled and disabled galleries, leaves
  clean bytes unchanged, preserves disabled-gallery selection and clean-cover access, and
  never serves stale marked copies. Toggling during the update creates no work.
- Repeat changes, concurrent uploads, superseded jobs, commit/rename failures, crashes and
  retries cannot publish a stale revision as current.
- Failed marked regeneration reports its error and supports retry without hiding usable
  clean copies in a disabled gallery. Initial-generation failure exposes neither pair.
- Cover authorization covers expired/unshared links, foreign heroes, changed/removed heroes,
  deleted or unprepared photos and delivered-gallery boundaries.
- Upgrade tests prove that old marked files cannot be mistaken for clean copies, backfill is
  durable, finals remain untouched, and deletion removes every new variant.
- Existing favorites, original exports, finals and settings validation retain their contracts.
- Settings previews apply their watermark overlay once to a clean sample; no photographer
  surface accidentally consumes the stored marked pair.
- The authenticated client-gallery preview matches client watermarking and visibility without
  changing the clean images served to ordinary photographer surfaces.

## Left to the builder

Exact column and variant names, route naming, query/helper boundaries, batched SQL shape,
deployment sequencing, progress wording and test organization. The readiness, access and
no-toggle-job invariants above are required regardless of those choices.
