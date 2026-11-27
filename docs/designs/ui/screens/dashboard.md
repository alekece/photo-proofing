# Dashboard

The photographer's home: what's new, and every gallery as a cover. Route `/dashboard`.

| | Desktop | Phone |
|---|---|---|
| Galleries | ![](dashboard/default.desktop.png) | ![](dashboard/default.phone.png) |
| Empty workspace | ![](dashboard/empty.desktop.png) | ![](dashboard/empty.phone.png) |
| Notifications | ![](dashboard/notifications.desktop.png) | ![](dashboard/notifications.phone.png) |
| Account menu | ![](dashboard/menu.desktop.png) | ![](dashboard/menu.phone.png) |
| New gallery | ![](dashboard/new-gallery.desktop.png) | ![](dashboard/new-gallery.phone.png) |

## Layout

1. **Top bar:** brand on the left; on the right the **language button** ("EN" or "FR", opening a small menu with English and
   Français; saves at once with the usual toast), the notifications bell (dot when unread) and the
   avatar. See the [languages design](../../2026-11-06-ui-languages-design.md).
2. **Greeting:** a Georgia title that says what needs attention, for example "Good morning, Léa.
   *Two galleries* have news." or "Good morning, Léa. *All quiet* for now." Below it, up to three
   news chips (gallery thumbnail, title, "13 selected" or "4 new downloads"). The **+ New
   gallery** button sits on the right.
3. **Toolbar:** search, Status filter, Sort.
4. **Grid of cover cards**, ending with a "Who's in front of your lens next?" card that also
   creates a gallery.
5. **Footer:** workspace name and totals ("9 galleries · 412 photos").

Search keeps a restrained, visible warm focus outline. The Status trigger has a fixed desktop
width, truncates long summaries, and anchors the menu's right edge to its own right edge, so
toggling statuses never moves the menu.

## Languages

The workspace has one language, English or French, chosen from the top bar. Every screen follows it,
including the client pages. Dates and plurals follow the language too, except the browser's native date input in the prototype. User content (gallery titles,
studio name, file names) is never translated.

| | Desktop | Phone |
|---|---|---|
| Language menu | ![](dashboard/language-menu.desktop.png) | ![](dashboard/language-menu.phone.png) |
| Galleries in French | ![](dashboard/default-fr.desktop.png) | ![](dashboard/default-fr.phone.png) |

## Gallery card

See the [approved studio examples](../../2026-11-22-photographer-studio-design.md). Square corners frame the cover; captions overlay it in Full image or sit below it in Print.

- A 26 px disc on a separate 44 px target selects without opening. The card shrinks slightly; it has no selection outline. The bulk bar offers Favorite, Quick edit and Delete.
- Quick edit saves once on validation. A partial failure keeps the failed rows selected for retry.
- The unread dot sits at the upper right, the star at the lower right, and status above the title. Keep access protection and closed access distinct.
- one line that says where it stands:

| Status | Line |
|---|---|
| Draft | "24 photos" or "No photos yet" |
| Shared | "48 photos · 13 selected" (count bold accent when over the allowance) |
| Expired | "48 photos · 13 selected · expired 3 days ago" |
| Delivered | "20 finals · 12 downloaded" or "· all downloaded" |
| Archived | "20 finals · archived" |

No hero: a warm neutral placeholder instead of the photo. Clicking the card opens the gallery.

## Toolbar

- **Search** filters by title as you type and highlights the match.
- **Status** shows one checked choice with a count. The default, All statuses, includes archived galleries.
- **Sort:** Last updated (default), Newest created, Oldest created, Title A–Z.
- **No results:** "No gallery called “…”" or "No galleries with these statuses", with a hint when
  matches hide in unselected statuses and a **Clear search and status** button.

## Notifications

The bell opens a compact panel of recent activity, including read notifications. Each row has a
thumbnail, gallery title, activity summary and relative time; unread rows have a dot and a bolder
title. Opening the panel marks nothing. Clicking a row opens the gallery and marks its activity
read. **Mark all as read** clears the dots and bell count without closing the panel or removing
rows, and is disabled when nothing is unread. The close button is in the top-right corner.
Empty: "Nothing new."

## Account menu

The avatar opens a small panel: name and email, then **Settings** and **Sign out**. Both items
use ink text on the panel colour with a light hover, never link blue.

## New gallery

A dialog asking for the title only, then the new gallery opens on its Photos tab.

## Empty workspace

Shown only when there are no galleries. The greeting, **+ New gallery**, search, Status and Sort
stay in place; search and filters are disabled. Below them, a dark contact sheet of empty film
frames with sprocket edges and edge print, seen through viewfinder corner brackets; a camera
readout ("1/125 · F2.8 · ISO 400 · [ 0 ]") and the line "Nothing in frame yet."

Motion: the sheet scrolls very slowly, the focus box breathes, a warm light leak sweeps across.

![Empty dashboard motion](dashboard/empty-motion.gif)
![Empty dashboard frames](dashboard/empty-motion.frames.png)
