# Contacts

Workspace-wide client directory. Route `/contacts`, reached from **Contacts** in `AppHeader`.

## Layout

1. **Header and toolbar:** the **Contacts** heading shows the total and the only **+ New contact**
   button. The shared directory toolbar filters by name, email or phone and sorts by name
   without clearing the search. An empty directory keeps the toolbar visible but disabled.
2. **Compact rows:** name and email stack beside the monogram. Optional phone, linked galleries, note and Edit share the row and wrap on narrow screens.
3. **Forms:** Edit and New open a dialog. Name and email are required; phone and notes are optional.
   The form actions follow the fields in keyboard order. Validation errors leave the draft intact.
   Edits save only changed fields. Changing an address checks that the original address still
   matches; if not, reload the directory before retrying. Successful saves briefly show confirmation.
4. **Deletion:** select contacts by checkbox, then choose **Delete** in the shared selection
   bar. Confirm after the warning that personal gallery links will stop working. Cancel leaves
   the selection intact. Filtering out a row or editing it clears that row's selection.
   After a partial failure, the dialog reports the error and retries only the remaining contacts.
   A contact already deleted in another session does not block the rest of the batch.

Details and forms expand or collapse in 200 ms; a successfully deleted row shrinks and fades in
280 ms. Filtering does not use the deletion animation. Reduced motion skips those transitions
without affecting confirmation. New and Edit focus the name field, and Cancel restores focus
to the opening action. After deletion, focus returns to search, or **+ New contact** if the
directory is empty.
