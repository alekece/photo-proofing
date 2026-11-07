# Settings and photographer gallery fidelity

Issue: [#206](https://github.com/alekece/photo-proofing/issues/206). Follows #201, #205 and #207.

## Context

#207 already rebuilt Settings and the gallery page toward `docs/designs/ui/prototype.html`. This
design records an audit of what still differs, rendered at 1280, 768 and 390 px against the
screenshots in `docs/designs/ui/screens/` (the app run against a mock API with an eight-photo
draft gallery: ready, processing, waiting and failed photos).

## Gap inventory

| Acceptance criterion | Finding |
|---|---|
| Settings: title, Cancel / Save changes, logo tile, inline studio name, single toolbar, live preview, dirty/saved/discard, toast | Match at desktop. Phone: the position grid wraps to its own row, where the prototype keeps mark, pattern and position in one row. **Gap 1.** |
| Gallery: hero header, no-hero, journey, tabs, select pill, uploading, dialogs | Match at 1280, 768 and 390 px. |
| Gallery: failed-upload retry | On small tiles (two columns on a phone) the **Retry** button covers the failure text. The prototype lays the label, reason and button out in one overlay. **Gap 2.** |
| Gallery: Client link / access / Look cards | Content matches. The phone icon rail uses a sliders and a palette icon; the prototype uses a calendar for access and the layout icon for Look. **Gap 3.** |
| Missing backend | Logo is kept in the page (data URL, not stored) and Save resolves through `SettingsApi`, the seam for #67. Archive, Delete and Deliver keep their "later release" errors (#73, #71). No new backend. |

Known, deliberately unchanged:

- The date input follows the browser locale; formatting it in the workspace language belongs
  with the UI-languages work.
- The notification badge needs #72.
- The prototype seeds data (logo, name, dates); the app starts empty, as `settings.md` says.

## Goals and non-goals

Goals: close gaps 1–3 and keep the audit as the record that #206 is delivered. Non-goals: any
API, storage or notification work; new states beyond the screen docs.

## Approaches

1. **Fix the three gaps in place (recommended).** Small, local CSS/markup changes with tests for
   the behavioural parts. Fits the audit: nothing else differs.
2. Rebuild the toolbar and tile from the prototype markup. Larger diff, no visible gain.

My call: approach 1.

## Decisions (my call)

- Failed tiles put the reason and **Retry** in the tile's normal flow, so they can never overlap.
- Phone toolbar buttons shrink (44 → 40 px) so Mark, Pattern and Position share a row at 390 px.
- The phone rail uses `Calendar` for Client access and the staggered-layout icon for Look.

## Test plan

- `PhotoTile`: a failed tile shows its label and a Retry button, and clicking Retry calls back.
- `GallerySettings`: the rail buttons stay labelled "Show Client access" and "Show Look".
- Visual check at 390 px for gaps 1 and 2; `pnpm --dir web verify`.
