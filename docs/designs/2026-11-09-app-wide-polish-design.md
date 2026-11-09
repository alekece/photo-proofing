# App-wide polish: footer, French "retouchées", back navigation

Issue: [#291](https://github.com/alekece/photo-proofing/issues/291). Small change; decisions marked *my call* are the author's and open to change in review.

## Intent

Three audit findings that touch every photographer page: a pointless control in the footer, French wording for delivered photos, and a back path that is hard to discover.

## Goals

- The footer shows the version only. No copy button, no "Copied" message, no manual-copy fallback.
- French says "retouchées" where it says "finales": the "Finales" tab becomes "Retouchées" and "photos finales" becomes "photos retouchées", with correct agreement. English is unchanged.
- The breadcrumb crumb beside "PhotoProofing" goes away. Gallery and Settings pages show a "← Back to galleries" link that goes to `/dashboard`.

## Non-goals

- Renaming "finals" in code, routes, API or English copy.
- Redesigning the header (language menu, bell, account menu stay).

## Approaches

1. **Keep the breadcrumb and style the parent crumb as a link (rejected).** The page title is already the `h1` right below, so the crumb repeats it, and the brand stays the only way back.
2. **Replace the crumb with a back link under the header (chosen).** One obvious, labelled way back on every sub-page. It reuses the `back` prop `AppHeader` already has (unused today), moved out of the header row.
3. **Back arrow inside the header row (rejected).** On the gallery page the header floats over the hero photo, where a small arrow is easy to miss.

## Decisions

- **Footer:** remove the copy button, `copy()`, the manual-copy input and the status line from `SiteFooter.svelte`, along with the `copyable` prop and the `footer_copied`, `footer_copy_failed`, `footer_copy_build` and `footer_full_build` messages. The version text and its full-value `title` stay.
- **French:** change every French message that says "finale(s)" for the delivered photos (`tab_final`, `step_upload_finals`, `photos_*_finals`, `summary_finals_delivered`, the count plurals, `client_finals_on_their_way`). Feminine plural "retouchées" agrees with "photos". *My call:* "Retouchées" alone for the tab, "Photos retouchées" where a noun is needed, and "Importez les photos retouchées, puis livrez" for the step.
- **Back link:** `AppHeader` drops `crumb`. Settings and the gallery page render a link `← Back to galleries` (`m.header_back_to_galleries`) to `/dashboard`: on Settings in the page flow above the title, on the gallery page inside the hero at the top left under the header so it stays readable over the photo. A real `<a>` with hover underline, pointer and the usual focus ring. The logo link stays.

## Testing

- `SiteFooter.test.ts`: version shown, no button, no status.
- `AppHeader.test.ts`: no crumb.
- Page tests for Settings and the gallery page: the back link points to `/dashboard`.
- A message check that no French string still contains "finale".

## Left to the builder

Exact spacing and the hero overlay treatment of the back link, within `docs/designs/ui/DESIGN.md`.
