# Remove the client event stream: plan

Design: `docs/designs/2026-11-07-remove-client-event-stream-design.md`. Issue: #174.

1. API test first: a valid shared link gets 404 on `/events` while `/photos` answers 200; a
   workspace stream test delivers its own `favorites` event and not a foreign one.
2. Remove the link route, `Scope::Link`, link tests and docs; replace `StreamOwner` by
   `WorkspaceId`.
3. Web: drop the `EventSource` in `ClientGallery.svelte`, `Selection.refresh` and their tests.
4. Edit `2026-11-05-client-selection-design.md` in place.
5. Run cargo fmt/clippy/test and `pnpm --dir web verify`; independent review; refactor pass;
   delete this plan; PR.
