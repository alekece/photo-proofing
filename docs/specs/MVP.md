# Enoam Gallery MVP Specification

## Product statement

Enoam Gallery is a self-hosted, private proofing and delivery workspace for one photographer and their clients. It replaces a costly hosted gallery subscription with an owned system that provides deeper branding and a clear asynchronous workflow from upload to final delivery.

The MVP runs for one photographer on a dedicated server. It is not a multi-tenant SaaS product and does not include billing, studio teams, client accounts, payments, or a freeform site builder.

## Users

### Photographer

The photographer creates galleries, uploads original work, configures the client experience, follows favourite activity, downloads selected source photos for retouching, publishes final images, and controls access.

### Client

The client receives a private gallery link, reviews proof images, saves favourites, and later downloads only the published retouched final photos.

## Product promise

The photographer can prepare a branded private gallery, collect client selections without a meeting, then securely deliver finished files without exposing original sources or relying on a permanent public link.

Watermarking is optional. It deters casual reuse but cannot prevent screenshots or copying a visible image. During proofing, the product must provide neither a client download action nor a high-quality client image endpoint. Proof files are deliberately limited to screen-quality dimensions and compression, unsuitable for normal high-quality printing.

Gallery expiry removes future access. It cannot revoke files a client has already downloaded, saved, screenshotted, or shared.

## Gallery lifecycle

The photographer may pause or revoke a share link in any state. Every client-facing state shows its name and the gallery expiry date or remaining access time.

| State | Photographer action | Client experience |
| --- | --- | --- |
| Draft | Prepare source photos, settings, branding, access and expiry | No access |
| Proofing | Monitor favourite activity and edit gallery | View proof previews and save or remove favourites. No downloads |
| Selection received | Review selections and optionally return gallery to Proofing | View proof previews. No downloads |
| Retouching | Freeze selection snapshot and download a ZIP of selected sources | View proof previews. No downloads |
| Delivery | Publish final assets in a delivery manifest | See and download only published retouched final photos |
| Expired | Retain or archive private material until retention date | No access and no metadata |

The photographer can deliberately move a gallery backward, with a warning whenever that may re-expose proofs after Delivery. The activity history records state transitions, share-link changes, favourites activity, export requests, delivery publishing, and downloads.

## MVP features

### Photographer workspace

- Photographer sign-in with a single-owner workspace.
- Create, edit, archive, and delete galleries.
- Upload original JPEG and RAW files. RAW files are retained privately and never made available to clients.
- Privately download original uploaded files, including RAW files.
- Reorder, hide, restore, replace, and delete photos before delivery.
- Configure one polished template with controlled layout choices, hero image, welcome text, accent colour, and name or logo.
- Optional watermark for proof previews.
- Generate an opaque private share link, with optional passcode, expiry, and immediate revoke or pause controls.
- See current favourites, favourite count, last activity time, gallery expiry, and activity history.
- See one grouped unread notification per gallery, for example: `Sarah and Tom's Wedding: 8 selection changes since you last checked`. Opening the gallery marks that item read. This is photographer-only and does not send email or browser push notifications in the MVP.
- Freeze a timestamped selection snapshot when starting retouching.
- Generate a one-click asynchronous ZIP of the source files included in that frozen snapshot.
- Upload or designate retouched final delivery files after external editing.
- Publish an explicit delivery manifest containing only the retouched final photos the client may see.
- Enable or disable individual final downloads and a whole-delivery ZIP.

### Client gallery

- Responsive proof gallery for desktop and mobile.
- Restricted, screen-quality proof previews, optionally with baked-in watermarking.
- Full-screen viewer with keyboard controls.
- Save and remove favourites automatically in the current private gallery session.
- No account, name, email, or explicit client submission step in the MVP.
- Current gallery state and expiry date or remaining access time.
- No client download action or download endpoint in Proofing, Selection received, or Retouching.
- In Delivery, display only final assets in the published delivery manifest. Proofs and unselected images are not visible.
- Individual final downloads and a delivery ZIP only when the photographer enables them.
- An expired-access page that exposes no photo metadata or previews.

## Asset rules

- Source JPEG and RAW files are private to the photographer.
- A proof is a separately generated, restricted JPEG or WebP with capped dimensions and quality. The exact cap will be determined by visual testing, but it must not be suitable for normal print-quality use.
- When enabled, watermarking is baked into proof files, never rendered as a browser overlay.
- A final is a photographer-uploaded or photographer-designated retouched delivery rendition, normally a high-quality JPEG. Clients do not receive RAW files or original sources.
- GPS and camera metadata are stripped from client proof previews by default.
- Upload, processing, and ZIP generation present clear progress, ready, failed, and retry states.

## Acceptance criteria

1. The photographer can create a gallery, upload JPEG and RAW sources, configure branding and expiry, and create a private link.
2. Without a valid unexpired client session, no visitor can retrieve proofs, originals, RAW files, finals, thumbnails, or export files.
3. A client can use a valid link and optional passcode to view proofs and save or remove favourites without an account or form.
4. Favourites persist through normal browser refreshes and never leak into another gallery or client session.
5. The photographer can see favourite activity through one grouped per-gallery notification and can open the gallery to review the latest state.
6. When Retouching starts, the system freezes a timestamped selection snapshot and allows the photographer to request a ZIP of exactly those source files.
7. Before Delivery, the client UI has no download control and the server denies every original, RAW, final, high-quality, and ZIP request.
8. Delivery exposes only final asset IDs present in the photographer-published delivery manifest. It never exposes proofs, unselected images, or sources.
9. Revocation or expiry blocks new proof and download access at the server.
10. Each client state visibly communicates the state and expiry date or remaining time.
11. Keyboard controls, focus states, screen-reader labels, and mobile layout work on critical gallery paths.

## Explicitly out of scope

- Freeform layers or visual page builder.
- Multiple photographers, studio roles, or granular permissions.
- Client accounts, comments, messages, email, browser push, or formal client selection submission.
- Payments, print sales, bookings, contracts, and invoicing.
- In-app retouching, AI culling, facial recognition, or AI training on client images.
- Client access to source files or RAW files.
- Public portfolios, public search, social sharing, and permanent public links.
- Automated deletion before the full retention, cache, and backup process is tested.

## Success measure

The photographer completes a real project using Enoam Gallery instead of their current proofing and delivery process. The client independently selects favourites, the photographer downloads the frozen selection in one action, and the client later downloads only final retouched files without manual file transfer.