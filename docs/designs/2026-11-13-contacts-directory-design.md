# Workspace contact directory and gallery contact associations

Issue: [#339](https://github.com/alekece/photo-proofing/issues/339). Builds on `GalleryStudio` (#342) and prepares per-contact link resolution for invitations (#348), reminders (#349), and activity logs (#350).

## Intent

To send personal gallery invitations, automate expiration reminders, and attribute client visits, favorites, comments, and downloads to specific people, the studio needs a workspace-wide contact directory (`/contacts`), a way to link contacts to individual galleries (`Studio → Contacts`), and a dedicated unguessable link token per `(gallery, contact)` pair while keeping the gallery's general direct link working anonymously.

## Goals

- **`AppHeader` navigation pills:** Add `Galleries` (`/dashboard`), `Contacts` (`/contacts`), and a disabled `Activity` placeholder pill (with tooltip `"Coming soon"` / enabled in #350) beside the `PhotoProofing` brand, highlighting the active section and fitting cleanly on `390 px` phones and desktop.
- **Workspace `/contacts` directory:**
  - Warm Paper header (`Contacts` + `+ New contact` primary pill) and client-side/query search bar filtering by name, email, or phone.
  - Two-column desktop layout: selectable contact list on the left (monogram avatar, **Name**, **Email**, optional **Phone**, and linked-gallery count/tags) paired with a right-side inspector/editor card (`Name`, validated unique `Email`, optional `Phone`, optional `Notes`, clickable linked-gallery tags linking to `/dashboard/{id}`, and `Delete` with `ConfirmDialog`). Collapses cleanly on phones (selecting a contact opens/scrolls to the editor or sheet, and `+ New contact` opens creation cleanly).
- **Gallery `Contacts` studio view (`Studio → Contacts` in `GalleryStudio`):**
  - Lists contacts linked to the gallery (showing contact count in the rail badge), `+ Link contact` (pick an existing workspace contact or create a new contact inline and link it in one step), and `Unlink` (removes the `gallery_contacts` association without deleting the workspace contact).
- **Separate random 22-char per-contact tokens:**
  - Every `gallery_contacts` row stores its own stable 22-character `ShareToken` (`Token<16>`, redacted from `Debug` and logs per `crates/AGENTS.md`), completely independent of `galleries.share_token` so clients cannot guess or strip tokens.
  - A single database lookup in `GalleryRepository::find_shared` resolves `/g/{token}` and `/api/v1/galleries/shared/{token}...`:
    - `galleries.share_token = $1` → `(gallery, contact_id: None)` (anonymous general link).
    - `gallery_contacts.share_token = $1` (when the parent gallery has `shared_at IS NOT NULL`, `archived_at IS NULL`, and is not expired) → `(gallery, contact_id: Some(ContactId))`.
  - Both links share the gallery's selection, comments, downloads, and access rules; `contact_id` and contact details are **never** serialized in any client-facing response.

## Non-goals

- Sending emails (#348 / #349) or recording per-contact activity timelines (#350); this issue establishes the directory, associations, and `SharedAccess.contact_id: Option<ContactId>` plumbing they consume.
- Exposing per-contact link copy buttons in the UI (#348 owns invitation sending; photographer API responses for gallery contacts omit raw tokens).

## Constraints

- **Database schema (`0031_contacts.sql` — numbered `0031` to coordinate cleanly alongside #345's `0030`, renumbered before merge if #339 lands first):**
  - `contacts`: `id uuid PRIMARY KEY`, `workspace_id uuid NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE`, `name text NOT NULL CHECK (char_length(btrim(name)) BETWEEN 1 AND 120)`, `email text NOT NULL CHECK (char_length(email) BETWEEN 3 AND 254)`, `phone text CHECK (phone IS NULL OR char_length(phone) BETWEEN 1 AND 40)`, `notes text CHECK (notes IS NULL OR char_length(notes) <= 2000)`, `created_at timestamptz NOT NULL DEFAULT now()`, `updated_at timestamptz NOT NULL DEFAULT now()`, `UNIQUE (workspace_id, email)`.
  - `gallery_contacts`: `gallery_id uuid NOT NULL REFERENCES galleries(id) ON DELETE CASCADE`, `contact_id uuid NOT NULL REFERENCES contacts(id) ON DELETE CASCADE`, `share_token text NOT NULL UNIQUE CHECK (share_token ~ '^[A-Za-z0-9_-]{22}$')`, `created_at timestamptz NOT NULL DEFAULT now()`, `PRIMARY KEY (gallery_id, contact_id)`.
- **Rust rules (`crates/AGENTS.md`):**
  - Reuse `pp_core::Email` (trimmed, lowercased, validated) for contact emails; define validated domain types (`ContactId`, `ContactName`, `ContactPhone`, `ContactNotes`, `Contact`, `ContactChanges`) with no free functions and redacting `Debug` on token types (`ShareToken` already wraps `Token<16>` whose `Debug` prints `Token(..)`).
  - Scope every `ContactRepository` query by `workspace_id` so cross-workspace IDs return `404` (and linking a foreign contact to a gallery returns `404`). Duplicate email within a workspace returns `409 Conflict` with a dedicated `ErrorResponse` code localized in EN/FR.
  - Before a gallery is shared (`shared_at IS NULL`), a `gallery_contacts.share_token` must **not** grant client access (`find_shared` requires `g.shared_at IS NOT NULL`, `g.archived_at IS NULL`, and `g.expires_at IS NULL OR g.expires_at > now()`).

## Approaches

- **A. Single SQL query resolving either `galleries.share_token` or `gallery_contacts.share_token` + dedicated `ContactRepository` (chosen).** One round-trip per client request (`LEFT JOIN gallery_contacts gc ON gc.gallery_id = g.id AND gc.share_token = $1 WHERE (g.share_token = $1 OR gc.contact_id IS NOT NULL) AND g.shared_at IS NOT NULL ...`), returning `SharedAccess { location, gallery, watermark_enabled, contact_id: Option<ContactId> }`.
- **B. Two sequential queries on every client request.** Extra latency on every thumbnail/preview load; rejected.

## Decisions

- Authenticated API endpoints under `/api/v1`:
  - `GET /contacts` (lists workspace contacts ordered alphabetically by `lower(name), id`, each including its linked galleries `[{ id, title }]`).
  - `POST /contacts` (creates a contact; `409` on duplicate email in workspace).
  - `PATCH /contacts/{id}` (updates `name`, `email`, `phone`, `notes`; `409` on duplicate email).
  - `DELETE /contacts/{id}` (`204`, cascades `gallery_contacts`).
  - `GET /galleries/{id}/contacts` (lists contacts linked to the gallery, without exposing `share_token` over JSON).
  - `PUT /galleries/{id}/contacts/{contact_id}` (idempotently links an existing workspace contact to the gallery, generating `ShareToken` on insert; returns linked contact).
  - `DELETE /galleries/{id}/contacts/{contact_id}` (unlinks; `204`).
- Update `docs/SPECIFICATIONS.md` (`FR-02`, `FR-04`) and `docs/releases/MVP.md`, plus screen docs (`docs/designs/ui/screens/gallery.md` and new `docs/designs/ui/screens/contacts.md` linked from `DESIGN.md`).

## Left to the builder

- Exact responsive mobile behavior on `/contacts` (master-detail stack or back-to-list toggle at `< md`) so `390 px` screens feel effortless.
- Ensuring generated `ShareToken` never collides across `galleries.share_token` and `gallery_contacts.share_token`.
- Comprehensive unit, migration, API integration, and `vitest` / `vitest-axe` tests in EN and FR.
