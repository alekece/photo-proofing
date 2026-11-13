# PhotoProofing UI

How every screen looks, moves and behaves. Read this page first, then the doc for the screen you
are building. Decisions and their history are in the
[UI redesign design record](../2026-11-06-ui-redesign-design.md); product rules are in
[`SPECIFICATIONS.md`](../../SPECIFICATIONS.md).

**Sources of truth, in order:** the screen doc, then [`prototype.html`](prototype.html) (open it
in a browser and click around), then the pictures. The prototype is throwaway HTML: reproduce its
look and behaviour in SvelteKit and Tailwind, not its markup. Photos in the prototype load from
Unsplash, so it needs a network connection.

## Screens

| Screen | Doc | What it covers |
|---|---|---|
| Dashboard | [dashboard](screens/dashboard.md) | Top section nav (`Galleries`, `Contacts`, disabled `Activity`), greeting, gallery cards, search, status filter, sort, notifications, account menu, new gallery, empty workspace |
| Contacts | [contacts](screens/contacts.md) | Workspace contact directory, search by name/email/phone, master-detail editor, linked gallery pills, create and delete |
| Gallery | [gallery](screens/gallery.md) | Left studio rail (`Collection` + `Studio`), top Journey Wizard bar, `Photos`, `Favorites`, `Comments`, `Final`, `Design`, `Contacts` and `Settings` views, Share, Publish and Delete dialogs |
| Photo viewer | [viewer](screens/viewer.md) | Full-screen photo with set or remove hero, delete, favorite state |
| Preview | [preview](screens/preview.md) | The client page inside the app, Desktop and Phone |
| Settings | [settings](screens/settings.md) | Studio name and logo, watermark, cover signature |
| Client gallery | [client](screens/client.md) | Cover, grid with hearts, favorites filter, viewer, delivered downloads |
| Client states | [client-states](screens/client-states.md) | Photos on their way, finished photos on their way, unavailable link |
| Sign in | [auth](screens/auth.md) | First-run setup and sign in (not redesigned) |

Each screen doc sits next to a folder of the same name holding its pictures:
`<state>.desktop.png` (1280 × 800) and `<state>.phone.png` (390 × 844), plus `<name>.gif` clips
and `<name>.frames.png` strips for anything that moves. The strips show the same motion as still
frames for readers that can't play a GIF.

## Look

"Warm paper": cream and brown, image first, calm. Photos are the colour; the interface stays quiet
around them.

| Token | Value | Use |
|---|---|---|
| Ink | `#2b2420` | Text, primary buttons, selected state, success toasts |
| Muted | `#6f6255` | Secondary text, labels |
| Page | `#f4efe8` | Page background |
| Panel | `#fbf8f3` | Cards, dialogs, menus |
| Line | `#e0d6c8` | Borders, dividers |
| Accent | `#b5381f` | Errors, destructive actions, over-allowance count, hearts |
| Ok | `#3f6b4a` | Done steps, "Saved" |
| Warning on dark | `#ffb59f` | Warnings on photos and dark surfaces |
| Dark | `#16120f` | Viewer, preview frame, client empty states |

- **Type:** Georgia for titles, gallery names and the brand; the system UI font for everything
  else.
- **Icons:** Lucide line icons, 1.5–2 px stroke.
- **Corners:** 10 px on cards and photos, 18 px on dialogs, fully round on pills and toggles.

## Rules for every screen

- **Icons over selects.** Small sets of choices (layout, watermark mark, view mode) are icon
  buttons. The selected one is ink with white content.
- **Disable, don't hide.** A control that can't be used right now stays visible and disabled,
  such as search on an empty dashboard or hearts in Preview. Exception: an empty tab drops its
  Select and Download buttons.
- **Two languages.** Every word on screen exists in English and French, dates included. Layouts must
  hold the longer French text without clipping. The workspace language applies to every screen,
  client pages too.
- **No hint text.** Plain labels, short copy, no counts repeated in two places.
- **Confirm, never block.** Delete always asks in a dialog. Share and Publish ask in a dialog and
  warn when something is missing (no photos, no finals), but never refuse.
- **Toasts.** Success is short ("Saved", "Link copied") on an ink toast with a green tick. Errors
  are red toasts with a short reason and, when possible, Retry.
- **Statuses.** Draft, Prepublish, Publish, plus the flags Expired and Archived.
- **Selection count.** Only the number ("13 selected"). Over the included allowance it turns
  bold and accent-coloured; no "extra" count.
- **Hearts.** A white disc with a red heart when favorited; an outline heart on a dark translucent
  disc otherwise.
- **Photo actions** (upload, delete, hero) live in the Photos tab only; Favorites and Final are
  for reviewing and downloading.
- **Phone.** Every screen works at 390 px. On the gallery page, the studio rail collapses into a
  compact identity bar, a 4-bar Journey row and a horizontally scrollable pill strip; dialogs stay
  centred and fit the width. Hover-only controls are always visible on touch screens.
- **Accessibility.** Keyboard reachable, visible focus, toggles use `aria-pressed` or
  `role="switch"`, dialogs trap focus and return it on close.

## Motion

Small and purposeful: motion confirms an action or gives an empty screen some life. Everything
respects `prefers-reduced-motion`: one-shot animations become instant, loops stop at a calm frame.

| Element | Trigger | What moves | Duration and easing | Reduced motion | Clip |
|---|---|---|---|---|---|
| Menus, dropdowns, notifications | Open | Drop 4 px and fade in | 150–180 ms, ease-out | Instant | — |
| Dialogs | Open | Scale from 96 % and fade | 220 ms, `cubic-bezier(.22,1,.36,1)` | Instant | [gallery/confirm-motion](screens/gallery/confirm-motion.gif) |
| Share done | Confirm Share | Photo stack flies out, green check pops, tick draws | 700 ms fly, 450 ms pop after 600 ms, 350 ms draw after 1 s | Instant check | [gallery/confirm-motion](screens/gallery/confirm-motion.gif) |
| Toasts | Show, dismiss | Slide up in; fade down out | 250 ms in `cubic-bezier(.22,1,.36,1)`, 200 ms out ease-in | Instant | — |
| Upload tiles | Waiting, Uploading, Processing | Indeterminate bar sweeps | 1.4 s loop, ease-in-out | Static full bar | [gallery/upload-motion](screens/gallery/upload-motion.gif) |
| Deleted tile | Confirmed delete | Shrinks and fades | 280 ms, ease-in | Instant | — |
| Hero photo | Page open, hero change | Fades in | 500 ms, ease-out | Instant | [gallery/hero-motion](screens/gallery/hero-motion.gif) |
| No-hero header | Always | Warm light leak drifts across | 16 s loop | Hidden | [gallery/no-hero-motion](screens/gallery/no-hero-motion.gif) |
| Empty dashboard | Always | Contact sheet scrolls slowly, focus box breathes, light leak sweeps | 160 s scroll, 7 s breathe, 13 s leak | Still frame, no leak | [dashboard/empty-motion](screens/dashboard/empty-motion.gif) |
| Settings Save | Save | Button turns green, "✓ Saved" pops | 450 ms `cubic-bezier(.3,1.6,.5,1)`, shown 1.6 s | Instant | [settings/save-motion](screens/settings/save-motion.gif) |
| Settings Cancel | Cancel | Undo icon spins back, "Reverted" | 500 ms ease-out, shown 1.4 s | Instant | [settings/revert-motion](screens/settings/revert-motion.gif) |
| Heart | Favorite | Heart pops with a ring | 420 ms `cubic-bezier(.3,1.6,.5,1)`, 500 ms ring | Instant | [client/heart-motion](screens/client/heart-motion.gif) |
| Heart | Unfavorite | Heart shrinks back | 250 ms, ease-out | Instant | — |
| Favorites count | Changes | Number bumps | 300 ms, ease-out | Instant | — |
| Client cover title | Page open | Rises 12 px and fades in | 600 ms, ease-out | Instant | — |
| Film strip | Client empty states | Strip scrolls; frames glow softly | 60 s scroll, 9 s glow | Still, frames dimmed | [client-states/film-strip-motion](screens/client-states/film-strip-motion.gif) |
| Viewfinder | Unavailable link | Bokeh hunts, focus box seeks, meter needle swings, "Focus not locked" blinks | 3.2 s loop | Still frame | [client-states/viewfinder-motion](screens/client-states/viewfinder-motion.gif) |
| Viewer photo | Next or previous | Fades in | 250 ms, ease-out | Instant | — |
| Spinner | Watermark update | Rotates | 900 ms linear loop | Static ring | — |
