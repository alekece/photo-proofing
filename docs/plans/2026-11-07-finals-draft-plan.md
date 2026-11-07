# Finals draft plan

Design: `docs/designs/2026-11-07-finals-draft-design.md`. Issue: #70. Test first at each step.

1. Migration `0013_finals_draft.sql`: kind enum, `published_at`, `removed_at`, CHECKs, event trigger update (kind in payload, `removed_at` trigger).
2. `database`: `PhotoKind`; `PhotoRepository` options = kind (default proof) with kind and `removed_at` filters; `NewPhoto.kind`; `PhotoEvent.kind`; kind and soft removal in `deletion` and `photo_retry`; `kind = 'proof'` in gallery SQL.
3. `api`: `{photos|finals}` path segment in photo routes; kind-scoped calls; tests for independence, denial, bytes, bounds.
4. `image-worker`: test that a final is processed.
5. `web`: API client and queue `set`; event `kind`; `PhotosPanel` `set`; Final tab uses it; remove `FinalPanel`.
6. Checks, reviewer, refactor pass, remove plan, PR.
