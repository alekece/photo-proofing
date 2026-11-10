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
- A notice to the client that deleted comments stay visible to the photographer (maintainer decision).
- Bold, links or other formatting. Comments are plain text with line breaks. *My call:* formatting needs an editor and sanitizing for little gain.
- Attachments, reactions, read receipts.

## Approaches

1. **One row per message, per photo (chosen).** Fits threads and later features, and matches how favorites and notifications work today.
2. **One note and one reply per photo.** Smallest change, but caps each photo at one exchange and would need redoing.
3. **Gallery-wide chat.** Simpler, but not what was asked and hard to follow with many photos.

## Decisions

- **Storage:** a `photo_comments` table: `id`, `workspace_id`, `gallery_id`, `photo_id` (cascade delete with the photo), `author` (`client` or `photographer`), `body`, `created_at`. It is keyed by photo, not by photo kind, so any kind can carry comments later.
- **Who may write:**
  - *Client:* only on a **proof** photo of a gallery that is shared, not delivered, not expired and not archived. Like favorites: the usual 404 for unavailable links and for non-proof or foreign photos, and 409 `gallery_delivered` once delivered. *My call:* delivered galleries show clients finals only, so proofs are out of their reach anyway; the server enforces it too.
  - *Photographer:* replies on proof photos of her own galleries at any time, including after delivery. *My call:* it is harmless and lets her close a conversation.
- **Edit and delete:** an author can edit (typos) or delete a comment. An edited comment shows "edited" and keeps its place in the thread. *My call:* the client has no identity, so anyone using the link can edit or delete client comments, exactly as anyone can change favorites; the photographer edits and deletes only her own replies. Editing and deleting follow the same availability rules as writing, so they stop once the gallery is delivered, expired or archived. Storage gains a nullable `edited_at` and a nullable `deleted_at`.
- **Deleting keeps the text:** a deleted comment (client or photographer) is marked deleted, not erased. Clients see a "Comment deleted" line in its place and the API never sends them its text. The photographer sees the original text struck through, so she still knows what was said. Deleted comments are not counted in badges or notifications. *My call:* the placeholder keeps the thread coherent. By the maintainer's decision the client is not told that the photographer can still read it (the author flagged the privacy trade-off: comments may hold personal data). The text goes when the photo or gallery is deleted, and the photographer can remove it on request. Edits keep only the latest text, no history.
- **Abuse limits (configuration, with defaults):**
  - *Pace:* client writes (post, edit, delete) are limited to one per `comments.min_interval` (default 2 seconds) per gallery; a faster call gets 429 `busy` with a retry delay. *My call:* per gallery rather than per visitor, because the private link is the credential and the API cannot tell visitors apart (behind a proxy every client shares one address). Several people on one link share the pace, which is still ample for real use. The state is in memory and resets on restart, like the sign-in limiter. The photographer is not limited.
  - *Thread size:* a photo holds at most `comments.max_per_photo` comments (default 100, deleted ones included since they are kept); client posts beyond it get 409 `thread_full`. *My call:* the cap applies to clients only, so the photographer can always answer a full thread.
- **Text:** plain text, trimmed, 1 to 1000 characters. Empty is rejected. Shown as text, never as HTML. *My call:* a fixed limit, not a setting.
- **Client routes:** list the gallery's comment counts per photo (for the badge), list one photo's thread, and post, edit and delete, using the existing `ClientAccess` extractor and the usual `no-store` / `no-referrer` headers.
- **Photographer routes:** the same operations under the gallery, behind the existing session auth and scoped to her workspace. Her thread view includes deleted comments with their text.
- **Live updates:** clients have no live stream (removed in #174; the spec asks for none), so they see changes on reload, like favorites. The photographer's stream already announces her bell changes, and a new client comment changes the bell, so her dashboard refetches counts and any open thread on that event. *My call:* no new event channel.
- **Bell:** a new `comment` notification kind through `record_notification`, so unread activity still accumulates per gallery. A new client comment records it; edits, deletes and photographer replies do not. The count shows how many new client comments.
- **Viewer UI:** the photo stays the star, so the thread is closed by default. The viewer's top bar gets a small comments button with the count; clicking it opens the thread as a side panel on wide screens and a bottom sheet on phones, and clicking again (or Escape) closes it. Moving to another photo keeps it closed. It holds the thread and a text box (client: "add a comment"; photographer: "reply"). The photo grid shows a small badge with the count. Visual details follow the existing design system in `docs/designs/ui`. Strings go through the existing i18n.
- **Spec:** `docs/SPECIFICATIONS.md` gains a short "Photo comments" requirement and an acceptance criterion. This needs the maintainer's explicit approval as a product change; it is part of this PR.
- **Email later:** the "she replied" email will send from the same insert of a `photographer` comment, once galleries store a client email. Nothing for it is built now.

## Testing

- Database: a client comment records one `comment` notification per gallery and accumulates; photographer replies do not; deleting a photo removes its comments; a migration test checks the table's constraints (author values, body length).
- API: client can post, edit, delete and read on a proof of a shared gallery; rejected on finals, unshared, delivered, expired, archived, unknown or other galleries' photos; empty and over-long bodies rejected; photographer can read and reply, only in her workspace; counts appear in the gallery reads.
- Web: the client viewer posts and shows the thread; the photographer viewer shows it and replies; the badge shows the count; the thread is closed until the button is used; the panel is absent on finals.

## Limitations

- A determined holder of the link can still fill threads up to the cap, one comment every 2 seconds per gallery, until the photographer deletes the photos or the gallery; there is no per-comment moderation beyond that.
- Anyone with the link can comment, edit or delete client comments, as anyone can pick favorites. There is no way to tell people apart.
- Clients only see replies when they come back. The first version has no push or email.
