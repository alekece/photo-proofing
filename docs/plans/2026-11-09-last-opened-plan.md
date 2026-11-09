# Last-opened indicator: implementation plan

Design: [2026-11-09-last-opened-design.md](../designs/2026-11-09-last-opened-design.md). Issue: #292.

1. **Database.** Migration `0026_notification_opened.sql`: widen `notifications_kind_known` with `open`, add a trigger on `galleries.last_opened_at` that calls `record_notification(…, 'opened', 1)` when the date moves forward. Add `NotificationKind::Opened`. Tests: accumulation, no record when the date moves back, new row after a read, kind switching like the other kinds, and a migration regression test on a transaction-scoped schema.
2. **API.** No code change (the gallery JSON already has `last_opened_at`, the list serializes the kind); extend the shared-route test to check the `open` notification.
3. **Front end.** `last_opened_at` in `gallerySchema`, `open` in the notification kind, texts and badge in `notifications.svelte.ts`, `Last opened <date>` / `Not opened yet` in the `GalleryHeader` meta line (shared galleries only), English and French messages. Tests for the bell texts and the header.
4. **Docs.** README sentence and the two approved sentences in `docs/SPECIFICATIONS.md`.
5. **Checks.** `cargo +nightly fmt`, clippy `-D warnings`, `cargo test`; `pnpm --dir web paraglide` and `verify`. Remove this plan before final merge approval.
