# Comments on proof photos

Status: awaiting maintainer approval. Decisions marked *my call* are the author's and open to change in review.
Issue: [#321](https://github.com/alekece/photo-proofing/issues/321).

## Intent

While choosing favorites, clients want to say something about one photo: "how would this look in black and white?", "can you remove that?", "this one is beautiful". Today that happens in scattered messages. Clients write comments on proof photos in the shared gallery, and the photographer reads them and replies in her photo viewer.

## Goals

- A client writes a comment on any proof photo. Everyone using the same link sees the same thread.
- The photographer sees each photo's thread in her viewer and replies there.
- A photo with comments shows a badge, for both sides.
- Client comments reach the photographer's bell as one more grouped activity.
- The stored shape is generic (photo, author, text, time), so comments can later be enabled on finished photos and a "she replied" email can be sent once client contact details exist.

## Non-goals

- Telling the client about a reply. In this version clients see replies the next time they open the link. Client email/phone and sending are separate later work; this design only avoids blocking it.
- Comments on finished photos. The data allows it; no API or UI path does in this version.
- Client names, accounts or per-person identity. Several people share one client, as for favorites.
- Editing or deleting comments, attachments, reactions, read receipts. *My call:* keep the first version small; a deletion rule can follow once real use shows the need.

## Approaches

1. **One row per message, per photo (chosen).** Fits threads and later features, and matches how favorites and notifications work today.
2. **One note and one reply per photo.** Smallest change, but caps each photo at one exchange and would need redoing.
3. **Gallery-wide chat.** Simpler, but not what was asked and hard to follow with many photos.

## Decisions

- **Storage:** a `photo_comments` table: `id`, `workspace_id`, `gallery_id`, `photo_id` (cascade delete with the photo), `author` (`client` or `photographer`), `body`, `created_at`. It is keyed by photo, not by photo kind, so any kind can carry comments later.
- **Who may write:**
  - *Client:* only on a **proof** photo of a gallery that is shared, not delivered, not expired and not archived. Same 404 as the other client routes otherwise; a non-proof or foreign photo is also a 404. *My call:* delivered galleries show clients finals only, so proofs are out of their reach anyway; the server enforces it too.
  - *Photographer:* replies on proof photos of her own galleries at any time, including after delivery. *My call:* it is harmless and lets her close a conversation.
- **Text:** plain text, trimmed, 1 to 1000 characters. Empty is rejected. Shown as text, never as HTML. *My call:* a fixed limit, not a setting.
- **Client routes:** `GET` and `POST` on the shared gallery's photo comments, using the existing `ClientAccess` extractor and the usual `no-store` / `no-referrer` headers. A gallery read also returns a comment count per photo for the badge.
- **Photographer routes:** `GET` the thread, `POST` a reply, behind the existing session auth, scoped to her workspace.
- **Live updates:** like favorites, an insert announces ids only on a database channel, and open browsers refetch. *My call:* reuse the existing event stream and add a comment event rather than a new mechanism.
- **Bell:** a new `comment` notification kind through `record_notification`, so unread activity still accumulates per gallery. A client comment records it; a photographer reply does not. The count shows how many new client comments.
- **Viewer UI:** `ProofViewer` gets a comments panel with the thread and a text box (client: "add a comment"; photographer: "reply"). The photo grid shows a badge with the comment count. Visual details follow the existing design system in `docs/designs/ui`. Strings go through the existing i18n.
- **Spec:** `docs/SPECIFICATIONS.md` gains a short "Photo comments" requirement and an acceptance criterion. This needs the maintainer's explicit approval as a product change; it is part of this PR.
- **Email later:** the "she replied" email will send from the same insert of a `photographer` comment, once galleries store a client email. Nothing for it is built now.

## Testing

- Database: a client comment records one `comment` notification per gallery and accumulates; photographer replies do not; deleting a photo removes its comments; a migration test checks the table's constraints (author values, body length).
- API: client can post and read on a proof of a shared gallery; rejected on finals, unshared, delivered, expired, archived, unknown or other galleries' photos; empty and over-long bodies rejected; photographer can read and reply, only in her workspace; counts appear in the gallery reads.
- Web: the client viewer posts and shows the thread; the photographer viewer shows it and replies; the badge shows the count; the panel is absent on finals.

## Limitations

- Anyone with the link can comment, as anyone can pick favorites. There is no way to tell people apart or remove a comment in this version.
- Clients only see replies when they come back. The first version has no push or email.
