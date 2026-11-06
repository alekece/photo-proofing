# UI redesign

Status: approved by @alekece on 2026-11-06. Issue: #172. The screens themselves are documented in
[`ui/DESIGN.md`](ui/DESIGN.md); this record keeps the why, the choices and what changed.

## Intent

PhotoProofing is for a photographer who uses Pixieset only because it is free. To be worth
switching to, it has to feel more vivid and more made-for-photographers than the free tool, while
keeping the simple steps of the MVP. The earlier [page designs](2026-10-30-page-designs-design.md)
(#84) settled what each page does; this redesign reworks how it looks and feels, and fixes the UX
gaps found while reviewing it.

## Goals

- Photos first: big imagery, a hero header and cover, little chrome.
- One clear journey per gallery: Upload, Share, Collect favorites, Deliver, always visible.
- Nothing ever blocks the photographer: actions warn instead of being disabled.
- Calm, characterful empty and error states (film strips, contact sheet, viewfinder) instead of
  generic messages.
- Every screen works on a phone.

## Non-goals

- New product features beyond the PRD changes below.
- A component library or new frontend tooling: the
  [frontend design system](2026-10-31-frontend-design-system-design.md) still applies.
- Redesigning sign in; its old mockup is kept.

## Anti-goals

- Looking like a generic SaaS dashboard.
- Losing the cream-and-brown theme or the Georgia titles that the photographer already liked.
- Adding steps, confirmations or settings the photographer has to think about.

## Approaches

1. **Polish the existing mockups.** Cheapest, but keeps the disabled actions, the settings panel
   and the generic empty states the review flagged.
2. **Rework page by page, keeping the theme (chosen).** Each page was mocked, reviewed and approved
   in turn, then joined into one clickable prototype. Keeps what worked and fixes the UX.
3. **New visual identity.** Most striking, but throws away a theme the photographer likes and
   would restart every approval.

## Decisions

- **Documentation shape.** One [`DESIGN.md`](ui/DESIGN.md) for shared rules, tokens and motion,
  plus one doc per screen beside its desktop and phone pictures and motion clips (GIF plus a frame
  strip). The prototype HTML is kept to check behaviour. Agents are pointed to it from
  `web/AGENTS.md`. Kept out of the README. *My call:* the folder is `docs/designs/ui/`, so it
  lives with the other designs without a second `design` folder.
- **Dashboard.** Greeting, New gallery, search and status filter; cards with the hero, status,
  a progress bar along the journey and one plain-language line about what happens next;
  notifications and account menu in the top bar. The empty workspace keeps the same toolbar
  (disabled) over a contact-sheet and viewfinder illustration, "Nothing in frame yet."
- **Gallery.** Hero header with the journey steps; a vertical side nav (sections plus Look,
  Access and Selection cards) edited in place, no modal; layout picked with icons; Photos,
  Favorites and Final tabs; select mode on every tab (delete on Photos and Final, download on
  Favorites). No hearts on the Photos tab. Favorites shows a padlock once delivered. No badge on
  the Final tab.
- **Hero.** A toggle (star on the tile and in the viewer); never automatic. Without a hero, the
  header and client cover use a dark warm gradient with faint initials, a film edge and a light
  leak.
- **Viewer and preview.** Dark full-screen viewer. Preview shows the real client page in Desktop or
  Phone mode, read-only: hearts and downloads are visible but disabled.
- **Settings.** Studio name and logo, a watermark toolbar (not sticky) over a live proof sheet,
  cover signature, Save changes and Cancel with short Saved and Reverted feedback, and a toast
  that watermarked proofs are being regenerated.
- **Client.** Cover with View the gallery; hearts on hover (always on touch); All / ♥ filter;
  heart in the viewer top bar on desktop and under the photo on phone; downloads once delivered.
- **Client states.** "Your photos are on their way" (film strip), "Your finished photos are on
  their way", and one unbranded "Focus not locked" page for every unusable link.
- *My call:* the prototype's share dialog shows Email and SMS; they are future scope, not MVP.
  Copy link is the only MVP way to share.

## Superseded decisions

From the [page designs](2026-10-30-page-designs-design.md) (#84):

| Before | Now |
|---|---|
| Share and Deliver disabled until ready | Always enabled; a dialog warns |
| Press-again confirmation | A confirmation dialog |
| Client "photos coming soon" page dropped | Restored as the client empty state |
| Hero defaults to the first photo | No default; no-hero cover instead (as #85 already said) |
| Golden-hour empty dashboard | Contact sheet and viewfinder |
| "Finals" tab | "Final" |
| Settings panel on the gallery page | Side nav |
| The server re-checks every final is ready on Deliver | Deliver warns; only ready finals reach clients |

From the [client selection](2026-11-05-client-selection-design.md) design (#68): the "extra" count
in a warning colour is gone, only the count shows, in the accent colour when over; the client
heart moves from top right to bottom right; and the photographer Favorites tab gains Download,
select mode and per-photo download.

From the [gallery states](2026-11-05-gallery-states-design.md) design (#86): cards show one
status chip, where Expired or Archived replaces the progress state instead of adding a badge, and
the status filter defaults to everything except Archived. The
[hero photo](2026-11-05-hero-photo-design.md) design (#85) said the client page shows no cover
without a hero; it now shows the no-hero cover. Its star was always visible on touch screens;
now touch users set the hero from the viewer.

## PRD changes

Applied to [`SPECIFICATIONS.md`](../SPECIFICATIONS.md) and the [MVP release](../releases/MVP.md),
approved by @alekece:

1. Share and Deliver are never blocked; they warn in a dialog (FR-04, FR-10, AC-04, journey 5).
2. Client empty state and empty delivered state messages (FR-04, FR-10).
3. One generic unavailable page, with no studio branding (FR-13).
4. Selection shows only the count ("13 selected"), in the accent colour when over the allowance, with no "extra" (FR-08, FR-11).
5. Watermark: name, logo or both; repeated or once at one of nine positions; size. Workspace only;
   a gallery only switches it on or off (FR-07).
6. Workspace cover signature: name, logo, both or none (FR-07).
7. Even rows and staggered columns; the hero is the cover, and there is a no-hero cover (FR-03).
8. Status labels Draft, Shared, Delivered (FR-11).
9. Download counts on cards and per-photo client download after delivery (FR-10, FR-11).

Added during the demo review:

- The photographer can download one favorite original, or several with select mode, from the
  Favorites tab (FR-09; previously no individual-original download).
- Saving watermark changes shows that proofs are being regenerated (FR-07).

## Left to the builder

- Mapping the prototype's look onto Tailwind tokens and Svelte components.
- Exact breakpoints between the documented desktop and phone layouts.
- Loading and error states not drawn (network errors use the error toast).
- How regeneration progress is tracked; the UI only needs "in progress" and "done".
