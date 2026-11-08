# Simplify the web photo list and upload queue

Issue: [#237](https://github.com/alekece/photo-proofing/issues/237).

## Intent

The photo list and upload queue guarded against load the MVP does not have (one photographer, a few
tabs, about 100 photos per gallery). The guards cost more complexity than they protect. Nothing
changes for the photographer: photos still appear, update and disappear live, and a busy server
only delays uploads.

## Photo list: the server list is the only truth

`PhotosPanel` no longer applies events or upload answers to its list. The list the server returns
is what is shown.

- A relevant `photo` event (same gallery and set), a `resync`, a client favorites event, a finished
  upload of this gallery and set, and a finished delete (single or bulk, successful or not) mark
  the list **outdated**.
- **One fetch at a time.** Marking the list outdated starts a fetch when none runs. If one is
  running, a flag asks for exactly one more fetch when it ends, however many marks came meanwhile.
- **Late answers.** A fetch remembers which view of the gallery started it. Switching gallery or
  unmounting invalidates it, so its answer or failure is ignored; the new gallery starts its own
  fetch.
- Deleting still hides the photo at once (optimistic). The refetch after the request shows what the
  server kept, so a failed delete brings the photo back as listed, with an alert.
- Removed: pending-deletion tracking, per-request change records, uploaded-during-load tracking,
  the load counter, the one-second throttle and the restore-in-server-order code.

Cost: a refetch of about 100 small rows per change, coalesced by the follow-up rule. A photo may
reappear briefly if a refetch is answered between a delete request and its commit; the next
refetch removes it.

## Upload queue: wait for `Retry-After`

The queue keeps its concurrency limit and the "Too large" check from `/api/v1/upload/limits`. On a
busy answer (429) the file goes back to waiting and is sent again once the server's `Retry-After`
has passed. There is no counting of busy answers, no exponential backoff, and no limit after which
the file fails. If the limits cannot be read, the waiting files show "Upload failed" with Retry,
which reads them again; there is no one-at-a-time fallback. Other outcomes (storage full, rejected,
signed out, gone, cancelled) are unchanged.

## Approaches considered

1. **Keep the reconciliation and tune it.** Rejected: every rule exists to protect another rule.
2. **Server list as truth with one in-flight fetch plus one follow-up (chosen).** Correct by
   construction: the last fetch always starts after the last change.
3. **Debounce or throttle refetches.** Rejected: delays updates and still needs a rule for answers
   that arrive out of date.
4. **Optimistic merge with versions.** Rejected: needs server versions the API does not have.

## Updated designs

`2026-11-04-photo-upload-ui-design.md` (list merging, busy retry, failed delete) and
`2026-11-04-bulk-photo-delete-design.md` (failed chunk refetch) describe the new behaviour.
