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
- **Chevron:** toggles an animated details block directly below the row.
- **Details block:** shows Notes and linked galleries. It can be opened independently of Edit.
- **Edit:** replaces that row's actions with Cancel and Save and inserts the fields in the details
  block. The row's identity stays visible.
- **New:** inserts or reveals an editable new-contact block below the page header or at the top of the
  list, with the same fields and Save/Cancel lifecycle.
- **Save:** idle, saving, confirmed and reset states share one reusable action component.
- **Delete:** a second, dedicated countdown confirmation replaces Delete. A second click cancels;
  otherwise the row is deleted after the countdown. The countdown and progress animation remain
  keyboard-accessible and respect reduced motion.
- **Failure:** validation stays inline; API/server errors use existing toast or inline error patterns
  and leave the editor open.

## Open questions

1. **Countdown duration:** should deletion confirm after 3, 5 or 8 seconds?
2. **New contact placement:** should it expand at the top of the list or at the header as a row
   immediately above the list?
3. **Details expansion:** should opening Edit automatically expand details, or keep separate chevron
   and Edit states?
4. **One open row:** should opening another row close the previous details/editor, or can several
   details blocks remain open?
5. **Delete removal:** should a deleted row shrink/fade after confirmation, or disappear immediately?
6. **Mobile:** should right-side actions remain labeled, become icons, or enter an overflow menu?
7. **Countdown versus dialog:** should delete use the requested countdown confirmation, or remain a
   standard destructive dialog for consistency with existing UI rules? If countdown wins, what duration?

## Verification

- One New contact control exists at every list state.
- Chevron, Edit and Delete use keyboard-operable controls and accessible names.
- Edit validation and duplicate-email handling remain unchanged.
- Countdown delete can be canceled by a second click and by keyboard.
- Successful Save shows temporary success feedback and returns to idle.
- Existing linked-gallery links and contact deletion rules remain intact.
