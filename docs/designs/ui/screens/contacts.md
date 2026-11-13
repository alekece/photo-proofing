# Contacts

Workspace-wide client directory. Route `/contacts`, reached from the top `AppHeader` section
pills (**Galleries**, **Contacts**, and disabled **Activity** with tooltip `"Coming soon"`).

## Layout

1. **Top navigation (`AppHeader`):** beside the `PhotoProofing` brand sit three pills:
   **Galleries** (`/dashboard`), **Contacts** (`/contacts`), and disabled **Activity**. Active
   section uses ink fill (`#2b2420`); on `390 px` phones the section pills wrap cleanly onto a
   second row below the brand and account controls so nothing clips or scrolls horizontally.
2. **Page header & search:**
   - Georgia heading **Contacts**, total count (`1 contact` / `N contacts`), and primary
     **+ New contact** pill button on the right.
   - Rounded search box filtering immediately by name, email, or phone. Follows *Disable, don't
     hide*: stays visible and disabled when the directory is empty.
3. **Empty workspace:** Warm Paper striped empty state (`"No contacts yet."`, short explanation,
   and **+ New contact**).
4. **Two-column directory (`md:`):**
   - **Left column (list):** selectable Warm Paper contact cards ordered alphabetically, each with
     a 1–2 letter monogram disc, **Name**, **Email**, optional **Phone**, and linked-gallery pills.
     When search matches nothing, offers **Clear search**.
   - **Right column (inspector / creator):** Warm Paper card showing monogram + heading, **Name**
     (required), **Email** (required, normalized and unique per workspace), **Phone** (optional),
     **Notes** (optional), and—when inspecting an existing contact—**Linked galleries** as
     clickable pills to `/dashboard/{id}` (or `"Not linked to any gallery yet."`).
   - **Actions:** **Cancel** and **Save changes** (`SaveButton` with `"✓ Saved"` confirmation, both
     disabled until dirty) or **Create contact** when adding, plus destructive **Delete** opening a
     `ConfirmDialog` warning that personal gallery links for that contact will stop working.
5. **Phone (`< md`, `390 px`):** collapses to one column at a time: selecting a contact or
   pressing **+ New contact** opens the inspector/creator card with a **← Back to contacts**
   control at the top.
