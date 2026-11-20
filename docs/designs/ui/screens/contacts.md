# Contacts

Workspace-wide client directory. Route `/contacts`, reached from **Contacts** in `AppHeader`.

## Layout

1. **Header and toolbar:** the **Contacts** heading shows the total and the only **+ New contact**
   button. The shared directory toolbar filters by name, email or phone and sorts by name
   without clearing the search. An empty directory keeps the toolbar visible but disabled.
2. **Full-width rows:** each contact shows a monogram, name, email, optional phone and gallery
   summaries. **Edit**, **Delete** and the details chevron sit at the right and use named icon
   buttons on phones. The chevron reveals Notes and gallery links directly below that row.
3. **Inline forms:** **Edit** opens the same row, preserves its identity and swaps its actions
   for **Cancel** and **Save changes**. **+ New contact** inserts the form above the list.
   Only one form or details block is open. Name and email are required; phone and notes are optional.
   The form actions follow the fields in keyboard order. Validation errors leave the draft intact.
   Edits save only changed fields. Changing an address checks that the original address still
   matches; if not, reload the directory before retrying. Successful saves briefly show confirmation.
4. **Deletion:** the row action shows a three-second countdown and warns that personal gallery
   links will stop working. Activating it again or pressing Escape while focused cancels it.
   Filtering out the row, editing it or leaving the page also cancels it. A failed request leaves
   the row and its links in place with an inline error and a fresh countdown available.

Details and forms expand or collapse in 200 ms; a successfully deleted row shrinks and fades in
280 ms. Filtering does not use the deletion animation. Reduced motion skips those transitions
without changing the confirmation time. New and Edit focus the name field, and Cancel restores
focus to the opening action. If a deleted row held focus, focus moves to the next visible
chevron, the previous one if last, or **+ New contact** if none remain.
