# Restore-expired prompt: implementation plan

Design: `docs/designs/2026-11-07-restore-expired-prompt-design.md`. Issue: #175.

1. Tests first: `RestoreExpiredDialog.test.ts` (dialog behaviour) and gallery page tests (restore wiring, prompt only when still expired, merge of `states`, failures).
2. Add `RestoreExpiredDialog.svelte` on the lower-level `Dialog`: date input (min today), Set date, No expiry, Keep expired, inline error.
3. Add a short expiry date formatter to `web/src/lib/gallery/format.ts` and reuse it in `PublishDialog`.
4. Wire `restore()` on the gallery page: flush pending edits, call `restoreGallery`, merge `states` and `updated_at`, toast "Restored", open the prompt when `states` includes `expired`; save through the page's `save`.
5. Run `pnpm --dir web verify`; independent review; refactor pass; delete this plan.
