# Favorite and download dates: implementation plan

Design: `docs/designs/2026-11-09-favorite-download-dates-design.md`. Issue: #296.

1. **Migration `0024_photo_favorite_dates.sql`** (+ regression test in `crates/database/src/photo.rs`): drop the four favorite triggers (events from 0010, notifications from 0021), add `favorited_at`, backfill from `created_at`, drop `favorite`, recreate the triggers on the NULL/non-NULL flip.
2. **Rust:** `Photo.favorited_at`, `COLUMNS`, `set_favorite`, `favorites`, export snapshot, gallery count, notifications `selected`, placeholder row; move every fixture (database, api, image-worker) to `favorited_at`. Add event and date tests.
3. **Front:** schema, `formatDateTime`, `PhotoTile` overlay caption (replaces the round badge), panels pass it, Paraglide messages en + fr, tests.
4. **Docs:** one sentence in `docs/SPECIFICATIONS.md`, README mention, CHANGELOG if hand-kept.
5. **Verify:** `cargo +nightly fmt --all`, clippy, `cargo test --workspace --locked` (disposable PostgreSQL 17), `pnpm --dir web paraglide` then `pnpm --dir web verify`. Open the PR.
