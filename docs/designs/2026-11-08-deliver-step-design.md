# Deliver step

Issue: [#71](https://github.com/alekece/photo-proofing/issues/71). Requirements: FR-10, FR-13, AC-01, AC-03, AC-04 in `docs/SPECIFICATIONS.md`. Builds on the [finals draft](2026-11-07-finals-draft-design.md), [gallery states](2026-11-05-gallery-states-design.md) and [remove client event stream](2026-11-07-remove-client-event-stream-design.md). Approved by the maintainer in conversation; choices he did not make are marked **my call**.

## Intent

Pressing Deliver switches the client's private link from proofs to the finished photos. Downloads (single photo, ZIP, download-request activity) are split to [#275](https://github.com/alekece/photo-proofing/issues/275) and wait for #220.

## Goals

- `POST /api/v1/galleries/{id}/deliver` sets `delivered_at`; the header button and warning dialog (already built) call it.
- A delivered link lists, serves and counts only ready finals; proofs and favorite writes are gone for clients.
- No publish step: after delivery a final is visible as soon as it is ready, and removing one hides it at once.

## Non-goals

Downloads and download activity (#275), un-delivering, matching finals to favorites, client live updates (clients have no event stream).

## Approaches

1. **Keep `published_at`/`removed_at`** and set `published_at` on Deliver. Conflicts with the PRD: later additions would need a publish step.
2. **Delivery is the publication (chosen).** The delivered set is the ready finals of a gallery whose `delivered_at` is set. `published_at` and `removed_at` and their retention are dropped; removing a final deletes it like a proof.

## Decisions

- **Migration `0019_deliver_step.sql`:** drops `published_at`, `removed_at`, their CHECKs and the `removed_at` trigger, and recreates `notify_photo_event` without the removal branch. Soft removal existed only to keep published files until the next publish. (my call)
- **Deliver.** One conditional `UPDATE` sets `delivered_at = COALESCE(delivered_at, now())` for a shared, unarchived gallery of the workspace, so repeats are idempotent and keep the first time. Missing gallery: 404. Not shared or archived: 409. The server never blocks on empty, pending or failed finals (the dialog warns); clients only ever see ready ones.
- **Atomic switch.** `set_favorite` locks the gallery row (`FOR SHARE`) while it checks `delivered_at`, and Deliver's `UPDATE` takes the row lock, so a write either completes before delivery or is rejected (409) after it; never both.
- **Client set.** `ClientPhotoSet` is `Finals` when `SharedAccess::delivered`, else `Proofs`; every client route already takes the kind from it. Finals carry no watermark, so `ClientAccess::watermarked` is false for them. The cover stays the proof hero, always read as a proof. (my call)
- **Web.** `deliverGallery(id)` calls the endpoint; the page's `deliver()` replaces its placeholder and updates `saved.states` from the response, like `share`.

## Verification

Database: deliver idempotence/not-shared/archived/cross-workspace, favorite rejected after delivery and during a concurrent delivery, migration keeps data. API: deliver authorisation and codes; delivered client lists only ready finals, serves their derivatives, 404s on proof ids, rejects favorites; draft and removed finals stay hidden; empty delivered set lists nothing. Web: API client and page wiring.
