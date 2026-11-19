# Inline contact list and editor

Issue: [#405](https://github.com/alekece/photo-proofing/issues/405).
Extends:
[workspace contact directory](2026-11-13-contacts-directory-design.md).

## Intent

The workspace Contacts page becomes a full-width, scan-friendly list. Viewing and editing stay close
to the selected contact; no right-side master-detail card is required.

## Goals

- Replace the contacts table/master-detail split with a full-width list.
- Keep one New contact action in the page header.
- Add right-side Edit/Delete actions and a chevron to expand details.
- Expand details and edit forms inline below the contact row.
- Reuse a countdown-confirmation and action-state component.
- Keep existing contact validation and linked-gallery behavior.

## Non-goals

- Changing contact storage, duplicate-email rules or gallery associations.
- Opening a modal for New contact or Edit.
- A separate detail route or page.

## Approaches

1. **Keep the right-side inspector and add row actions.** Rejected: it leaves unused width and moves
   editing away from the selected row.
2. **Open a dialog for New/Edit.** Rejected for this audit: the requested inline flow is faster and
   keeps list context.
3. **Replace the row with inline details and fields.** Chosen. One row has a chevron and actions;
   expansion/in-place edit occurs beneath it.

## Decisions

- **List:** ordered by name, each row showing monogram, Name, Email, optional Phone and linked-gallery
  summaries. Right-side actions remain visible or appear on hover/focus/touch consistently.
- On mobile, right-side row actions use icon buttons with accessible names.
- **Chevron:** toggles an animated details block directly below the row.
- **Details block:** shows Notes and linked galleries. It can be opened independently of Edit.
- Only one contact details/editor block is open at a time.
- **Edit:** replaces that row's actions with Cancel and Save and inserts the fields in the details
  block; clicking Edit automatically expands details. The row's identity stays visible.
- **New:** inserts or reveals an editable new-contact block below the page header or at the top of the
  list, with the same fields and Save/Cancel lifecycle. **Resolved:** place it at the top of the list.
- **Save:** idle, saving, confirmed and reset states share one reusable action component.
- **Delete:** a second, dedicated countdown confirmation replaces Delete. A second click cancels;
  otherwise the row is deleted after the countdown. The countdown and progress animation remain
  keyboard-accessible and respect reduced motion. The countdown lasts `3 seconds`.
- **Failure:** validation stays inline; API/server errors use existing toast or inline error patterns
  and leave the editor open.
- **Motion:** details/edit blocks expand and collapse with height/fade animation; row actions crossfade
  between states; deleting rows shrink and fade; Save/Delete lifecycle states use the shared motion
  rules in `ui/DESIGN.md`.

## Open questions

1. ~~**Countdown duration:** 3, 5 or 8 seconds?~~ **Resolved:** `3 seconds`.
2. ~~**New contact placement:** at the top or header?~~ **Resolved:** at the top of the list.
3. ~~**Details expansion:** open details on Edit?~~ **Resolved:** Edit automatically expands details.
4. ~~**One open row:** close previous or allow several?~~ **Resolved:** only one row open at a time.
5. ~~**Delete removal:** shrink/fade or immediate?~~ **Resolved:** shrink and fade.
6. ~~**Mobile:** row actions?~~ **Resolved:** icon buttons.
7. ~~**Countdown versus dialog:** dialog or countdown?~~ **Resolved:** countdown confirmation
   for this inline contact flow, using the shared action component.

## Verification

- One New contact control exists at every list state.
- Chevron, Edit and Delete use keyboard-operable controls and accessible names.
- Edit validation and duplicate-email handling remain unchanged.
- Countdown delete can be canceled by a second click and by keyboard.
- Successful Save shows temporary success feedback and returns to idle.
- Existing linked-gallery links and contact deletion rules remain intact.
