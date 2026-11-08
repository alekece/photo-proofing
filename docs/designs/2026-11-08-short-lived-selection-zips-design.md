# Short-lived selection ZIPs

Issue: [#220](https://github.com/alekece/photo-proofing/issues/220). Requirements: FR-09, NF-02. Changes the
retention and reuse rules of the [selection ZIP design](2026-11-07-selection-zip-design.md). Decisions marked
**my call** are for review.

## Intent

A selection ZIP is a temporary hand-off file, not a stored artifact. It should vanish from disk minutes after it
is built, and pressing **Download** again on an unchanged selection should not rebuild it.

## Goals

- A ZIP is unavailable and deleted 15 minutes after it is ready, downloaded or not.
- Requesting a ZIP for the same selection as a live ZIP reuses it: no new snapshot, job or file.
- Disk reclaimed within minutes, without a workspace-wide quota.

## Non-goals

- Tracking whether a download completed: the server cannot know the browser saved the file.
- Splitting a selection above `export-max-size` into several ZIPs (later release).
- A workspace-wide ZIP quota: one export per gallery, the size cap and the short life bound disk use.

## Approaches

1. **Delete after download completes.** Needs `downloaded_at`, a grace period and a second lifetime, and still
   cannot tell that the file was saved. Rejected.
2. **Short fixed lifetime plus reuse by selection (chosen).** One rule (15 minutes from ready), no download
   tracking, and the repeat click is cheap.
3. **Keep 24 h and add a quota.** Bounds disk but keeps files for a day and adds a policy to tune. Rejected.

## Decisions

- **Lifetime (my call).** `export-retention` defaults to 15 minutes (`PP_EXPORT_RETENTION`, still
  configurable). The API already treats an older export as gone (status 204, download 404) and the maintenance
  sweep deletes its row and file, so correctness does not depend on the sweep.
- **Sweep interval (my call).** `cleanup-sweep-interval` defaults to 5 minutes instead of 1 hour, so a
  file is reclaimed shortly after it expires. The sweep also covers photos and archives; it is cheap.
- **Reuse.** When a request finds the gallery's newest export ready and unexpired, and its snapshot holds the
  same photo ids as the selection now requested (the whole selection or the chosen subset, in archive order),
  it returns that export with `200` and queues nothing. Originals never change, so equal ids mean equal
  content. Reuse does not extend the lifetime. A different selection replaces the export as today.
  Comparing the stored snapshot needs no hash column.
- **Tab.** No change in flow: the tab already starts the download when the export it requested is ready.
  It never offers a ready export on page load.
- **Docs.** The selection ZIP design and the README configuration text state the new defaults.

## Left to the builder

Placement of the reuse query, and the test fixtures.

## Verification

Reuse for an unchanged selection and for an identical subset; rebuild after a favorite change, a different
subset, an expired export, or a failed export; no extension of the lifetime on reuse; expiry at 15 minutes in the
API and in the sweep; the new defaults in configuration.
