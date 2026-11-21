# PhotoProofing workspace visual refresh

Approved scope: [#423](https://github.com/alekece/photo-proofing/issues/423). This is
the outcome of the visual workshop and the audit, with the final mobile and motion
decisions. It supersedes the older screen designs where they differ.

## Intent and approach

The photographer should recognize the shooting first, know what changed, and get
back to the same place after opening it. Keep the warm cream and ink palette.
Reserve terracotta for meaningful attention, not ordinary hover states.

The workshop compared reusing the existing screens with a screen-by-screen visual
replacement. Keep the existing interactions and restyle shared components once;
ship all affected screens in a single PR. Use existing primitives for spacing,
buttons, icon alignment, selection bars, focus and dialogs. Keep specialized
controls where their behavior differs. The prototype is a visual reference, not
the branch to ship.

## Galleries and navigation

- Galleries is the entry screen. Show the matching gallery count beside the title
  and the common search, status and sort bar above the covers. Keep the round
  selection affordance and shared bulk bar. Saving favorite changes does not
  reorder the list; leaving selection does not undo a change already saved.
- Put a cream lifecycle label at the bottom left of the cover: Draft, Shared,
  Delivered or Archived. Put access indicators and the gold favorite star at the
  bottom right, and one terracotta unread dot at the top right. Show an expired
  link as closed access, without changing how the existing Expired filter matches.
  Leave the title below the cover. Highlight just the number selected when it
  exceeds the allowance.
- Put the bell next to PhotoProofing on desktop and phone; the account and version
  stay at the bottom on desktop. The count is galleries with unread notifications.
  The menu opens on activation, not on hover or focus alone.
- Share the workspace frame and content spacing so the menu and footer do not
  overlap. Workspace, Gallery Studio and Settings menus share white idle,
  cream hover and warm-black selected states. Returning from a gallery restores
  search, filters, sort and scroll.
  Browser Back still goes to the actual origin, such as Activity. Restore UI state
  independently of protected data; do not cache authenticated responses.

## Gallery Studio and supporting screens

- Keep the two menus and gallery identity. Put the journey at the left of one
  header and the active section's action at its right. Leave the journey in place
  when no action is available. Collection contains Photos, Favorites and
  Retouched, including the latter's quiet empty drop area. Keep cover choice in
  Design, with the desktop inline preview and Desktop/Phone in full preview.
- Move Comments into Studio beside Activity. Show one row per commented photo
  with a non-interactive thumbnail and latest message. Expand the existing thread
  beneath the row, with the composer and own-message actions. Do not change
  which photos accept comments or which clients can read them.
- Remove the preview column from gallery Settings. Leave the shoot date alone.
  Put expiry presets to the right of the calendar on desktop. Keep Included
  photos numeric, with steppers and no placeholder when unset.
- Keep Photos a private masonry library, with gallery/type filters and
  chronological sort. Search by gallery name, not filename. Do not overlay a
  file name or a proof badge. Open the viewer over the grid; download and delete
  stay on selection, not on the viewer.
- Keep Contacts compact with initials, a disclosure for details, and actions at
  the right. Delete from the shared selection bar after confirmation, not from
  individual rows. Use one plus icon and one form for creation and editing.
  Preserve the meaning of associating and dissociating contacts.
- Keep the activity feed in date order with bold actor, warm ink icons and a
  filled favorite heart. Put the gallery pill after the sentence and time to the
  right. Wrap favorite thumbnails across the full width and show all recorded
  thumbnails when expanded. Keep cursor-based pagination, and omit the Gallery
  filter inside a gallery.

## Workspace settings

Use the workspace sidebar plus an ungrouped secondary menu: General, Watermark
and Cover signature. General holds name/logo, the workspace's only language
selector, and quality, without a preview. Both editors use a compact toolbar
over a large live preview. The always-visible Cancel/Save bar is disabled without
edits and its divider spans the full panel. Keep edits across sections and cancel
back to the saved state.

In Single watermark mode, show the nine placement controls on the preview.
In Repeated mode, hide them and remember the position. Keep compact size and
opacity controls without a displayed percentage; the minimum for a new opacity
adjustment is 5%, without rewriting earlier values on load. The color changes
the text only; the logo keeps the colors supplied by its file.

Cover signature keeps None, Name, Logo and Both. In Both, center the logo above
the name. Save an optional text color and show it on the actual cover, not just
in the editor. A reset icon clears the override and restores the existing cover
color. Disable the color control without a name; do not add opacity or recolor
the gallery title.

## Motion and delivery

Keep a short button-to-form opening where the source is clear, without an
intermediate ball or waiting to reveal the form. Cancel returns to the source;
success closes in place. Menus, confirmations and the viewer do not need that
effect. Honor reduced motion and keyboard focus. No camera iris or morph
between pages.

Add horizontal swipe only to the client viewer on mobile. Keep controls as an
alternative and keep desktop arrows. Preserve stage-specific actions, playback,
and pausing when a person navigates manually or opens the thread.

Use comparable synthetic before/after screenshots or a video in one PR. Keep
[share and delivery](https://github.com/alekece/photo-proofing/issues/425),
[expiry filtering](https://github.com/alekece/photo-proofing/issues/424),
[cross-gallery ZIPs](https://github.com/alekece/photo-proofing/issues/426),
[notification preferences](https://github.com/alekece/photo-proofing/issues/427),
[final photo conversations](https://github.com/alekece/photo-proofing/issues/419)
and [activity search](https://github.com/alekece/photo-proofing/issues/421)
outside this slice. No release scope or access-policy changes.
