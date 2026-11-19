# Gallery studio polish

Issue: [#407](https://github.com/alekece/photo-proofing/issues/407).
Builds on:
[studio gallery layout](2026-11-12-gallery-studio-layout-design.md),
[cover layouts and focal point](2026-11-13-cover-layouts-and-focal-point-design.md),
[sharing invitations](2026-11-14-gallery-sharing-invitations-design.md),
[gallery sharing](2026-11-14-gallery-sharing-invitations-design.md),
and [gallery screen](../ui/screens/gallery.md).

## Intent

Refine the gallery studio without moving every control or changing gallery lifecycle behavior. The
rail remains the stable place for collection and studio navigation, while the active stage gets full
width.

## Goals

- Keep the gallery rail visible and widen it enough for cover, title and status hierarchy.
- Put the favorite star at cover top-left and status chip at cover top-right.
- Put shoot date at the cover bottom-left.
- Put the title below the cover.
- Center the Journey Wizard and remove Preview and Publish actions from the main action block.
- Move Activity outside Collection.
- Keep font specimen cards in Design.
- Combine cover selection and focal-point positioning in one preview.
- Remove the focal-point percentage readout and reset action.
- Put red destructive Remove cover beside Choose/Change cover.
- Widen and clarify Share editing fields.
- Keep Share limitations inline on the Share page.
- Make Gallery Contacts use full width and remove duplicate add/link controls.
- Keep Download all in Favorites and Final.

## Non-goals

- Changing the client-facing cover or Preview.
- Changing delivery, share-token, watermark or original-download behavior.
- Removing the existing font specimen choice.

## Approaches

1. **Keep the current hero header and add action controls.** Rejected: it consumes stage width and
   separates gallery identity from collection navigation.
2. **A compact rail identity card plus a centered journey and full-width stage.** Chosen.
3. **Separate Cover and Focal point cards.** Rejected: they describe one visual operation.
4. **One combined Cover block with a movable focal-point pointer and adjacent actions.** Chosen.

## Decisions

- The rail is persistent across Collection and Studio views.
- The cover thumbnail, favorite star, status chip and shoot date share the rail identity block.
- The editable title appears below the cover, with existing autosave behavior.
- The Journey is centered in the active stage header. Its stage actions are limited to approved
  existing Share/Deliver/Restore behavior and no duplicate Preview/Publish header action.
- Activity is a Studio-level or account-adjacent view, not a Collection tab.
- Design retains Typography specimen cards and cover/grid controls.
- Cover selection opens ready proofs. The selected cover preview is also the focal-point control.
- Focal point is pointer/drag only; no reset or numeric percentage text.
- Choose/Change cover and Remove cover are adjacent; Remove cover is red and border destructive.
- Share uses a wider Warm Paper sheet. To, Subject and Message are visibly editable; limitations are
  inline, such as empty proofs or unfinished proofs.
- Gallery Contacts is full-width within the stage and exposes one clear Manage or Link control.
- Download all remains available on Favorites and Final.

## Open questions

1. **Rail width:** is 240 px, 256 px or another width best for cover and title hierarchy?
2. **Journey on phones:** should it remain a 4-bar progress row or become a horizontal journey card?
3. **Preview:** remove the preview header action, or keep it accessible elsewhere such as the cover?
4. **Publish/Deliver:** in the current lifecycle, should Publish/Deliver appear only in a view action
   area or remain as a centered primary journey step?
5. **Activity placement:** should it sit as its own Studio navigation item or in a separate rail footer?
6. **Shoot date:** if no shoot date exists, should the block show no text or `Created <date>`?
7. **Combined focal pointer:** should pointer movement save instantly or on pointer release?
8. **Preview and Publish:** should Preview remain an icon elsewhere and should Publish/Deliver remain
   in the Journey/toolbar, or were they both explicitly removed from the main gallery action block?

## Verification

- Rail and cover remain visible when switching Collection/Studio views.
- No duplicate Preview/Publish header controls exist.
- Font specimens remain usable.
- Focal pointer moves without reset or percentage text and survives cover changes.
- Remove cover is destructive but reversible only by replacing the cover.
- Share fields fit desktop and 390 px layouts.
- Gallery Contacts has one primary link/create action and no duplicate icons.
