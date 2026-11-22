# Photographer studio: from the gallery list to delivery

Approved 2026-11-22 for [issue #434](https://github.com/alekece/photo-proofing/issues/434). One implementation PR. This design changes the photographer's workspace, not the presentation or permissions of a shared gallery.

## What the photographer sees

Recognize the series by its cover, open its Photos, recover the client's favorites for retouching, then see which finished photos the client requested. Keep the two desktop rails on the left. Keep all four journey steps visible, highlight the current one in terracotta, and label the last one **Delivered**. Put the current tab's action to the right of the steps; wrap it rather than truncate a step. Opening a gallery normally goes to its own Photos, never Favorites or the workspace library. Preserve intentional deep links and the return to the filtered gallery list.

Two independent **workspace** choices apply to Galleries, Photos, Favorites, Finals and the workspace photo library:

| Choice | Behavior |
|---|---|
| Full image (default) / Print | In Full image, place useful metadata over the photo, with enough contrast and no white surround. In Print, put the caption on paper below. Do not invent a caption or add an empty strip. Preserve grid proportions and cover focal points. |
| Warm board / Neutral board | Keep the current warm palette by default. Page, panel, borders and hover colors follow the saved palette. On light surfaces, use a soft grey hover for Neutral and a soft earth tint for Warm, not the darker navigation highlight. |

Save these choices explicitly in Settings. Changing only presentation must not regenerate assets. Leave existing independent language behavior alone. Neither choice changes the owner viewer, public gallery nor its preview: gallery cover layout, focal point, typography and workspace signature still belong to the photographer.

## Gallery list and bulk actions

- Square corners belong to photographs; keep controls lightly rounded. Search fills the space up to the filters, with the magnifier inside the placeholder and a thin visible focus border. Status is **one choice or All statuses**; the chosen item shows a warm-black check, not a tinted background. Keep existing sort choices and access indicators.
- Clicking the cover opens the gallery. The separate selection control appears on hover or keyboard focus; on touch use an explicit **Select** control. Keep its 44 px hit area and inset 26 px fine-ringed disc. A checked card recedes slightly (97.5%); neither the disc nor the hit area shrinks. No black selection outline. Hide the circle again after deselection without removing keyboard focus visibility.
- Put unread activity at the upper right, the yellow favorite star at the lower right, and status above the title in both modes. Keep the client's heart, link protection and access-closed icons distinct. No backing pill or shadow on the star. Animate only a newly confirmed favorite and pulse an arriving unread dot once; never replay on initial load. Honor reduced motion.
- In the selection bar, separate the count from the actions. Neutral buttons inherit the bar's surface until hovered; exit is a borderless cross. Delete is lighter brick red at rest and darker on hover. Its confirmation stacks the actual selected covers next to the count instead of listing every name.
- **Quick edit saves once.** Validate in the dialog; disable the submit button when invalid, unchanged or reverted. Do not stage a preview and ask for Apply later. Form dialogs use the page surface, white fields and a thin focus border. Preserve failed edits to retry; when only part of a batch succeeds, reflect successes and leave only failed targets selected.
- Start an API write immediately and guard duplicate activation. Show **only a spinner** if still pending after the action's threshold: 150 ms for favorite or quick edit, 200 ms for delete. Do not delay a fast success, flash a spinner, dim photos or show a success toast for these actions. Use one contextual error toast at the upper right; keep it readable and dismissible even over a dialog. Preserve the button's accessible name and announce pending state.

## Inside the gallery

- Opening a gallery from a card, a notification or a direct URL clears **that gallery's** unread indicator, not its activity log or counts. Opening the bell alone clears nothing; keep the explicit mark-all action. The notifications panel has a small accessible close cross at bottom right. Hide Last opened entirely until a client visit exists.
- **Favorites:** most recent selection first, with its date; one contextual Download action for the working selection, no duplicate toolbar or unread filter. Leave the tab readable without a decorative padlock after publication. This does not unlock client favorites. Neither a quota nor exporting a ZIP signals that the client has finished.
- **Finals:** keep Import and management actions. Show a distinct-photo download count, not clicks or proof that a file was saved. Only requested photos get a non-interactive dark download mark and first request date, with a legible surface over photographs. No photographer download action on the tab, in a bulk selection, on a tile or in its viewer. No download date in the viewer. Client individual and ZIP downloads are unchanged.

## The rest of the studio

The photo library shows the photo count near the title and reuses Galleries' filter menus without losing gallery/type filtering, sorting or selection. Keep the source gallery in the print caption or the full-image overlay. Contacts uses one compact desktop row: only name and email stack; phone, linked galleries, note and Edit sit alongside. Missing fields do not add empty rows; wrap when space runs out. Preserve contact details, editing and associations.

Activity remains a timeline and Settings a form. Give Settings a discreet navigation divider; put version, year and brand under workspace identity rather than below each studio page. Preserve phone usability without redesigning mobile navigation. Keep both locales legible, including long sort labels, and ensure the settings signature preview remains legible without recoloring the signature. Studio load errors should offer recovery, not tell the photographer to contact their photographer.

## Visual anchors and boundaries

Approved workshop examples, with synthetic photos: [Print and selection](ui/screens/studio-workshop/interactions-desktop.jpg), [Full image and neutral palette](ui/screens/studio-workshop/interactions-pleine-image.jpg), [selection disc at native size](ui/screens/studio-workshop/interaction-galerie-cochee.png), [quick edit](ui/screens/studio-workshop/interactions-modification-rapide.png), [Contacts](ui/screens/studio-workshop/contacts-chaud.jpg). **Test settings** and simulated requests in the workshop are not product controls. Compare the real interactions as well as the pictures: keyboard and touch, fast and slow requests, failure and retry, light and dark photos.

No redesign of public covers, new mobile navigation, ZIP engine changes, notification subscriptions, retouching-readiness signal, dark mode, or architectural refactor. Use existing primitives and semantics; inspect shared components in Storybook when available and verify the actual journey on phone and desktop. No extensive snapshot suite.
