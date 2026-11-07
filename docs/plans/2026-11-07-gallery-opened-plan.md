# Gallery opened plan

Design: docs/designs/2026-11-07-gallery-opened-design.md. Issue #211.

1. Migration `opened_at`; `Gallery.opened_at`; `GalleryRepository::record_opened` with tests.
2. `POST /shared/{token}/open` behind `ClientAccess`; API tests.
3. Web `recordOpened` and the `/g/[token]` page ping; tests.
4. README status; checks; review; remove this plan.
