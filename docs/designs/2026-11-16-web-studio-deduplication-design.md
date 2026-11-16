# Fix studio tabpanel ARIA wiring and deduplicate SvelteKit API clients, previews, and studio controls

Issue: [#376](https://github.com/alekece/photo-proofing/issues/376).

## Intent

Fix broken ARIA `tablist` / `tabpanel` wiring in `GalleryStudio.svelte` so all eight `role="tab"` buttons reference real `role="tabpanel"` elements in the DOM, and remove ~300 lines of duplicated SvelteKit client/SSR HTTP wrappers, preview loaders and viewport frames, redundant `/api/v1/settings` network re-fetches, chained design `$derived` runes, and studio copy/readiness helpers.

## Goals

- **Studio tabpanel accessibility (`GalleryStudio.svelte`):**
  - Give each of the eight mounted studio section wrappers (`photos`, `favorites`, `comments`, `final`, `design`, `share`, `contacts`, `settings`) its own `role="tabpanel"`, `id="panel-{sectionId}"`, `aria-labelledby="tab-{sectionId}"`, and `hidden={section !== sectionId}`, replacing the single outer dynamic `tabpanel` wrapper whose changing `id` left seven `aria-controls` attributes pointing at missing DOM nodes.
- **Typed HTTP and SSR client consolidation (`web/src/lib`):**
  - Create `#lib/http.ts` exporting one shared `ApiError` (`status`, `message`, `code?`), `requestJson`, and `requestEmpty`, and migrate `#lib/gallery/api.ts`, `#lib/gallery/photos.ts`, `#lib/gallery/comments.ts`, `#lib/gallery/export.ts`, `#lib/contacts/api.ts`, `#lib/settings/settings.ts`, and their callers/tests off `GalleryApiError`, `ContactApiError`, `SettingsApiError`, and repeated `fetch` + Valibot boilerplate.
  - Create `#lib/server/http.ts` (`readWorkspaceJson`, `readSharedJson`) and reuse it across `#lib/server/contacts.ts`, `#lib/server/galleries.ts`, and `#lib/server/settings.ts` for SSR cookie forwarding, `401` sign-in redirect, optional `404` handling, Valibot parsing, and `503` fallback errors.
  - Move pure contact presentation helpers (`contactInitials`, `sortContacts`) to `#lib/contacts/format.ts` and gallery-scoped contact endpoints (`listGalleryContacts`, `linkGalleryContact`, `unlinkGalleryContact`) to `#lib/gallery/contacts.ts`, keeping `#lib/contacts/api.ts` focused on `/api/v1/contacts` CRUD.
- **Preview and workspace settings reuse (`GalleryDesignPanel.svelte`, `ClientPreview.svelte`):**
  - Extract `#lib/gallery/preview.ts` (`loadClientPreviewData`) and `PreviewFrame.svelte` (Desktop/Phone mode toggle and phone bezel viewport container) and reuse both in `GalleryDesignPanel.svelte` and `ClientPreview.svelte`.
  - Pass SSR `data.settings` (with `DEFAULT_SETTINGS` fallback) from `routes/dashboard/[id]/+page.svelte` into `ClientPreview.svelte` (alongside `heroRevision` so `ClientPreview`'s `coverUrl` cache-busts after cover edits) and from `GalleryStudio.svelte` into `GalleryDesignPanel.svelte`, removing all client `/api/v1/settings` fetches in studio preview components.
  - Replace `GalleryDesignPanel.svelte`'s 15 chained `$derived` design fields (`saved*` / `committed*` / `current*`) and 5 `set*` functions with a reactive override map over the saved gallery fields and one generic `setDesignField` helper.
- **Studio UI control and readiness reuse (`CopyButton.svelte`, `CopyLink.svelte`, `GallerySettingsPanel.svelte`, `PublishDialog.svelte`, `GallerySharePanel.svelte`):**
  - Extract `CopyButton.svelte` encapsulating clipboard write, 3-second `copied` reset timer, `ClickPop` checkmark animation, `role="status"` live announcement, and optional fallback `<input>` focus/select on clipboard failure; reuse it in `CopyLink.svelte` and both locked and editable access-code rows of `GallerySettingsPanel.svelte`.
  - Extract `summarizePhotoReadiness(photos, uploading)` in `#lib/gallery/photos.ts` (`ready`, `inFlight`, `failed`, `empty`, `warned`) and reuse it in `PublishDialog.svelte` and `GallerySharePanel.svelte`.

## Non-goals

- Changing any visual layout, Paraglide user-facing copy, keyboard shortcuts, or backend API contracts.

## Constraints

- Follow `web/AGENTS.md`: PascalCase component names, camelCase TypeScript modules, Paraglide messages for all user-visible text and ARIA labels, and zero warnings across `pnpm --dir web verify`.

## Approaches

- **A. Single `ApiError` + `#lib/http.ts` & `#lib/server/http.ts` + `CopyButton.svelte` + `PreviewFrame.svelte` (chosen).** Eliminates three identical error classes, keeps `CopyLink.svelte` simple while sharing all clipboard/status behavior with editable pre-share access codes via `CopyButton.svelte`, and removes duplicate preview/SSR plumbing cleanly.
- **B. Keep `GalleryApiError` / `ContactApiError` / `SettingsApiError` subclasses and add an `editable` mode to `CopyLink.svelte`.** Touches fewer caller imports, at the cost of keeping redundant error subclasses and complicating `CopyLink.svelte` with editable-input and regenerate-action props; rejected.

## Decisions

- **Maintainer decision:** Use Approach A (`ApiError` + `#lib/http.ts` / `#lib/server/http.ts` + `CopyButton.svelte` + `PreviewFrame.svelte`).
- **Author's call — `PreviewFrame.svelte` header flexibility:** Keep `ClientPreview.svelte`'s full-screen modal `<header>` (`Preview`, Desktop/Phone toggle, `Close`) and `GalleryDesignPanel.svelte`'s embedded card header (`Client preview`, Desktop/Phone toggle) sharing the Desktop/Phone toggle + viewport bezel via `PreviewFrame.svelte` props/snippets so neither screen changes its existing layout or ARIA landmarks.

## Left to the builder

- Exact prop shapes on `PreviewFrame.svelte` and `CopyButton.svelte`, and option shape on `#lib/http.ts` / `#lib/server/http.ts`.
- Component and unit tests covering all eight `role="tabpanel"` / `aria-controls` pairs in `GalleryStudio.test.ts`, `ClientPreview` SSR settings + `heroRevision` cache-busting, `CopyButton` / `GallerySettingsPanel` copy states, and `#lib/http.ts` / `#lib/server/http.ts` error handling.
