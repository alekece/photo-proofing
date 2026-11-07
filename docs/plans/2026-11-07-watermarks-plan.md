# Watermarks plan (#67)

Design: [watermarks](../designs/2026-11-07-watermarks-design.md). Test first at each step.

1. Migration `0013_workspace_watermark.sql` and `pp_database::watermark` (settings types,
   `WatermarkRepository`: get, save with revision bump and regeneration, gallery update with
   regeneration, stamp read) with database tests.
2. `JobRepository::begin_publication` (job and photo locks, claim and stamp check) with tests.
3. Worker: `ImageRenderer` watermark compositing (text, logo, both, repeat, once); handler reads
   the stamp after `mark_processing`, stages, publishes. Tests with libvips 8.15 (container).
4. API: logo normalisation, `GET/PUT /settings`, `GET /settings/logo`, gallery PATCH through the
   watermark repository, upload config for the logo limits.
5. Web: schemas, server load, `SettingsApi`, page wiring, logo validation messages.
6. README, MVP/spec wording where behaviour requires, worker Dockerfile font, CI font.
7. Review, refactor pass, rerun checks, delete this plan, rebase, PR.
