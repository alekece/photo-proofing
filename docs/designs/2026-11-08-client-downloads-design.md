# Client downloads of finished photos

Issue: #275 (split from #71). Requirements: FR-06, FR-10, FR-11, FR-13, NF-01, AC-02, AC-05. Builds on the
[deliver step](2026-11-08-deliver-step-design.md) and the [selection ZIP](2026-11-07-selection-zip-design.md)
(including its #220 follow-up: short-lived ZIPs reused by hash). Decisions marked **my call** are mine
and are for review to confirm.

## Intent

A client on a delivered link downloads one finished photo or the whole delivered set as one ZIP, and the
photographer sees what was downloaded, live. The ZIP is prepared ahead of time so the click is instant.

## Goals

- Download one final, or all finals as a ZIP, byte-identical to the uploads.
- The delivery ZIP is built when the gallery is delivered and kept current as finals change.
- Record the first download request of each final and of the ZIP; tell the photographer live.
- Reuse the export table, the worker job, the builder and the event stream.

## Non-goals

Gallery-card counts, "All downloaded" and notifications (#72 reads what is recorded here); a client event
stream (the client page polls); watermarks; splitting an oversized set into several ZIPs.

## Approaches

1. **A `kind` on `exports` (chosen).** A delivery ZIP is an export of the delivered finals: same job,
   retry, status event, builder and cleanup. Costs: uniqueness and replacement become per `(gallery, kind)`,
   and the retention sweep must treat the kinds differently.
2. **A separate `deliveries` table and job.** Clean on paper, but duplicates the job, status, cleanup and
   event code. Rejected.
3. **Build on the client's first click.** No triggers, but the click waits and clients cannot be notified;
   the maintainer wants it ready. Rejected.

## Decisions

- **Freshness without a counter (my call).** A delivery ZIP is *current* when its included photo ids, in
  order, equal the ids of the gallery's ready finals now, checked on read when the client asks. A stale ZIP
  is never served. No revision column and no hash.
- **Download all (maintainer).** There is no "Preparing" state. The client clicks **Download all**; the
  button shows "Downloading…" and the browser download starts as soon as a current ZIP exists: at once if
  it does, after the build otherwise. The click `POST`s `/shared/{token}/download`, which creates a build
  if none is current and answers JSON (200 ready, 202 building, 503 failed); the page polls that quietly
  and, once ready, starts the download once with `GET /shared/{token}/download`, which streams the file.
  Splitting the two keeps a large ZIP out of a script's memory. If the build fails, a toast says "Couldn't prepare the download.
  Please try again." and the button is usable again.
- **When builds start.** Lazily, at the client's click, plus a warm-up right after deliver. Finals that
  change later trigger nothing: the next click finds the ZIP stale and replaces it. A late completion of a
  replaced build finds its row gone and is dropped.
- **Empty set.** No ZIP is built; the delivery export may hold
  zero photos only if the builder needs a row, otherwise none is created (builder's call).
- **Kind-scoped export rules.** The "one preparing per gallery" index, the replace-previous step, `latest`
  and `ready` become per kind, so a photographer's selection ZIP never deletes the client's delivery ZIP.
- **Lifetime and stale ZIPs.** Selection ZIPs keep their 15 minutes. A delivery ZIP has no age limit. It is
  replaced, never accumulated: requesting a new build for a gallery deletes the previous delivery export
  (row and job, in one transaction, file removed after the commit), as the selection flow already does. The maintenance sweep is the safety net
  and removes (a) the delivery ZIPs of expired or archived galleries, files and rows, jobs first, and (b)
  ZIP files no row owns, as it already does; the 15-minute rule skips delivery ZIPs. Extending the expiry
  or restoring the gallery needs no extra code: the next client click finds no current ZIP and builds one.
  **Product clarification (approved):** FR-13's "no automatic asset deletion" covers photos, selections
  and activity, not derived files; FR-13 gets a sentence saying so.
- **Recording (my call).** `photos.downloaded_at` is set once, at the first authorized request of that
  final. A ZIP request sets `exports.downloaded_at` once and, per FR-11, sets `downloaded_at` on every final
  in the ZIP that has none, in one transaction. Repeats change nothing. Downloaded means requested, not
  saved. A removed final's flag disappears with it; a re-uploaded final is a new, undownloaded photo.
- **Events.** A trigger on `photos.downloaded_at` and `exports.downloaded_at` announces ids only on the
  photographer's stream, as the existing events do; clients get none. The dashboard marks downloaded finals
  (the tile mark exists) and shows that the ZIP was downloaded.
- **Authorization.** Single and ZIP routes take the client link and are checked against the current
  delivered set: finals only, ready only, same gallery; proofs, drafts, originals of proofs and other
  galleries are 404; expired, archived and deleted links are the same generic 404. Responses are
  `Cache-Control: private, no-store`, `nosniff`, with a safe attachment filename. The ZIP route serves only
  a current ready delivery ZIP, otherwise 409 (building or failed) or 404.
- **Client page.** A download control per photo and a "Download all" as described above. No download-all
  control when the delivered set is empty.
- **Failures (my call).** A failed delivery build retries through the queue; on final failure the client
  gets the toast above and the photographer sees the failed export with Retry.

## Left to the builder

Table and column shape, poll interval, trigger channel names, the empty-set representation, test fixtures.

## Verification

Byte-identical single and ZIP contents; stale ZIP never served after add, remove and ready transitions; click waits then downloads once;
rebuild after expiry extension and restore; sweep removes delivery ZIPs of expired and archived galleries
only and leaves selection ZIPs on their own rule; selection request never touches the delivery ZIP;
first-request-only `downloaded_at`, ZIP marking every included final, events emitted; authorization across
proofs, drafts, galleries, workspaces and blocked links; failure and retry; accessible client controls.
