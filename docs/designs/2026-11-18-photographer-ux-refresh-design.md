# Photographer workspace UX refresh — design

Status: proposed.  
Owner: maintainer review required.
Issues: #391, #390, #389.

## Intent

Make the photographer workspace predictable for someone arriving from Pixieset while preserving
PhotoProofing's quieter, image-first Warm Paper identity. Navigation, actions, selection, preview,
settings, and empty states currently use different interaction models. The refresh makes one model
apply across Galleries, All Photos, Contacts, Activity, and each gallery's studio.

## Goals

- Keep workspace navigation and utility actions together on the left.
- Keep gallery identity and gallery-level actions stable while moving between collection sections.
- Add a workspace-wide All Photos index that retains each photo's gallery and Proof/Final context.
- Give every collection the same selection entry and floating action pattern.
- Show client design changes in a large inline preview without nested page scrollbars.
- Simplify gallery and contact flows while preserving the established gallery-sharing model.
- Make status, confirmation, and destructive styling distinguishable without changing product
  meaning.
- Attribute a browser's unlocked state separately to each private gallery link so switching
  personal links on one device requests the access code for the newly used link.

## Non-goals and exclusions

- This is not a rewrite of client-facing favorites, comments, downloads, or delivery behavior.
- No additional client authentication, account, or per-person selection model.
- The access code remains a convenience/protection gate, not proof of identity.
- No animated Activity or decorative empty-state system in this work; motion is deferred.
- No payment, e-commerce, or Pixieset feature parity.
- No automatic gallery reordering while the photographer is interacting with cards.
- No automatic save for bulk edits; selection operations remain explicit.

## Approved visual direction

The approved direction is the modern, quiet workspace:

- A persistent light left rail contains `Galleries`, `All Photos`, `Contacts`, `Activity`, then
  notifications, language, workspace settings, and the account control at the bottom.
- Labels remain visible at desktop widths. The rail may collapse responsively on phone.
- Warm Paper colors remain, with more compact surfaces, modern rounded controls, and photos taking
  the visual lead.
- Gallery pages reuse a stable gallery identity in the rail: cover, favorite, title, status, and
  counts. Preview and Share remain in the same content-header position on every gallery section.
- Active client-facing actions remain centered in the content header rather than repeated in the
  identity rail.

## Navigation and gallery identity

Outside a gallery, the left rail selects workspace pages. Inside a gallery, the rail shows:

1. Back to Galleries and gallery identity.
2. Collection: Photos, Favorites, Comments, Final.
3. Gallery: Activity and Settings, with Design, Sharing/privacy, Contacts, and Details reachable as
   Settings subsections.

The rail is wider than the current gallery rail so titles and counts are readable. The selected
section is clear without requiring a top section pill and a gallery tab system simultaneously.

The gallery identity displays:

- Favorite star before the gallery name.
- Status after or beside the name.
- Counts at the right edge of navigation rows.
- Prepublish as a green outline with green text on white.
- Published as a filled vivid-green chip with white text.
- Draft as neutral grey.
- Archived as dark ink.

Single-gallery favorite changes are immediate. The star and state update in place, but the gallery
card does not move until the user refreshes or explicitly reapplies a sort.

## Selection model

Selection is entered by clicking a selection circle on a gallery or photo card. The card itself
continues to open the gallery or photo viewer. There is no separate Select button.

Selection adds a floating bottom action bar:

- Close selection.
- Selected count.
- Select all/unselect all.
- Only the actions valid for the current collection.

The normal page toolbar remains present and stationary. Entering selection does not replace or
shift the toolbar, header, or grid.

Bulk gallery favorite and quick-edit actions are drafts. The interface previews the intended result,
shows the changed rows, and offers `Cancel changes` and `Apply changes`. No selected gallery is
mutated before Apply.

Photo selection follows the same visual model:

- Photos and Final: Delete.
- Favorites: Download selected originals.
- The action label reflects the selected count.

Deletion remains confirmed. Download and upload behaviors remain otherwise unchanged.

## Dashboard and All Photos

The dashboard keeps its gallery cards and filters, using the modern spacing and status chips above.
Pinning gives immediate feedback without moving cards under the pointer. The page applies
starred-first ordering on refresh or an explicit sort action.

All Photos is a workspace-level page in the left rail. It shows one visual index of all ready proof
and final photos from every gallery, including archived and expired galleries, with:

- Search across gallery and photo identity.
- Gallery and photo-type filters.
- Sort control.
- Proof/Final classification.
- Gallery title on each tile.

Opening a tile opens the photo in its gallery context. Per-photo actions remain source-aware; a
workspace photo view does not expose invalid actions merely because another gallery tab supports
them.

All Photos is for archive-wide maintenance, so a selection may contain any mix of galleries, proof
photos, final photos, and favorite state. Its floating action bar offers Download and Delete for that
mix. Set as cover and other gallery-specific actions remain in the source gallery. Deletion still
requires confirmation.

## Gallery Preview and Design

On wide screens, Design and Settings use approximately one-third width for compact controls and
two-thirds width for the live client preview. The controls and preview grow naturally with the
page. The preview does not have a fixed height or its own visible scrollbar.

`Open full preview` remains at the top-right of the inline preview. On smaller screens, the inline
stage is omitted and Preview opens a full-screen client preview. The preview owns the modal's scroll
and the background does not scroll.

In Design:

- The cover image appears first.
- Change cover and Remove cover appear immediately below it.
- Focal point follows the cover controls.
- Layout, typography, grid, density, and spacing remain compact sections below.

In Settings:

1. Shoot date.
2. Open until.
3. Included photos.
4. Privacy and sharing.
5. Archive and Delete as separate lifecycle actions.

The first two date fields are separate rows, not side-by-side. Cards are compact and use the
available column width without long rows or excess padding.

The preview button is removed from the left identity rail; stable Preview and Share actions belong
to the gallery content header.

## Contacts

The workspace Contacts page is a full-width directory:

- One New contact action.
- Search and sort controls use the full page width.
- Contact list on the left, inline details on the right.
- Linked galleries are visible directly in the list and in the editor.
- The editor supports edit-in-place and standard Cancel/Save.

There is only one New contact entry point on the page.

Inside gallery Settings, contact linking uses a persistent side panel or dialog that stays open
after an action:

1. The contact row starts with Link.
2. Clicking Link briefly shows an animated `Linked` confirmation in the same button.
3. The button then becomes Unlink in the same position.
4. A user may immediately undo by clicking Unlink.
5. The row stays visible throughout.

Linked and hover styling use the normal ink/success theme, not red. Red remains reserved for
destructive, non-routine outcomes.

Creating a contact is available inside this same panel and links the newly created contact without
leaving the gallery flow.

## Activity and empty states

Activity keeps the established filters and grouped timeline. With no events, it uses a simple
centered icon, title, and sentence on the page. It has no striped drop-zone panel, dashed border, or
motion.

The same distinction applies elsewhere:

- Stripes are reserved for actual upload or drop targets.
- Read-only empty collections, filtered results, and Activity use a plain empty state.
- Motion for empty states is future work.

## Client link authorization

The current gallery unlock cookie authorizes a gallery, regardless of which share token reached it.
That allows two personal links used in one browser to inherit the same unlock authorization.

Change the authorization entry to identify the **gallery share link** in addition to the gallery and
code proof:

- One HttpOnly cookie can contain multiple signed link authorizations.
- Each personal or general link is authorized independently.
- Entering an access code records authorization for the link currently in use.
- Reusing the same link in that browser continues to work without another prompt.
- Switching to a different personal or general link for the same gallery requests the code once for
  that link.
- The access code itself is never stored in the cookie.
- Client responses never expose contact identifiers.
- Activity is attributed from the private link after unlock, exactly as today.

This is convenience and activity attribution, not identity verification. If a personal link is
shared, everyone using it is attributed to that linked contact.

The existing bounded cookie capacity remains. When the authorization list is full, the oldest link
authorization is evicted; opening that link later asks for the code again.

## Responsive behavior

- Phone uses one primary content column and collapses the workspace rail into its existing compact
  navigation pattern.
- The gallery rail becomes the compact identity bar and section navigation.
- Selection circles remain visible on touch; hover is never the only way to select.
- Floating selection actions remain reachable above the safe area and use icon-only actions if
  space is constrained.
- Dialogs and full client preview remain centred and viewport-bound.

## Delivery phases

### Phase 1 — workspace shell and selection

- Replace the top workspace pills with the modern left rail.
- Add All Photos.
- Apply status chips and card styling.
- Replace Select buttons with card selection circles.
- Add the shared floating selection bar.
- Delay dashboard gallery reordering after pin changes.
- Require Apply for bulk gallery favorite/quick edits.
- Introduce plain informational empty states and remove stripes from Activity.

### Phase 2 — gallery workspace and contacts

- Stabilize gallery identity and content-header Preview/Share actions.
- Group gallery Settings subsections.
- Move cover controls to the approved order.
- Add the responsive one-third/two-thirds Design preview.
- Use the one-scroll full preview on small screens.
- Reorder gallery Settings rows.
- Redesign the Contacts directory and inline editor.
- Keep gallery contact linking open with Link → Linked → Unlink behavior.
- Align photo/viewer actions with the approved contextual model.

### Phase 3 — link-scoped unlock authorization

- Extend unlock cookie records to bind authorization to the share link.
- Preserve multiple authorizations in one cookie.
- Validate general and personal links independently in the same browser.
- Keep rate limiting, 30-day cookie lifetime, HttpOnly/SameSite/Secure behavior, and client payload
  secrecy.
- Add regression coverage for switching links on one browser.
- Update the product specification and access-gate design records.

## Constraints

- Keep SvelteKit, Tailwind, existing shared UI primitives, and the Warm Paper design system.
- User-visible strings remain Paraglide messages in English and French.
- Existing accessibility behavior must remain: focus return, keyboard navigation, visible focus,
  labelled controls, and reduced-motion behavior.
- No raw share tokens or access codes in logs, activity, client JSON, or public UI.
- Existing specs are authoritative. Product changes in this design, especially All Photos and
  link-scoped authorization, require an explicit specification update before implementation.

## Verification

Each phase must verify:

- Relevant frontend checks with `pnpm --dir web verify`.
- Rust checks for the authorization phase as required by `crates/AGENTS.md`.
- Phone and desktop screenshots for every changed screen.
- Keyboard and reduced-motion behavior for selection, dialogs, preview, and linking.
- No duplicate Select buttons, nested page scrollbars, or striped non-drop empty states.
- Regression tests for delayed sorting, staged bulk edits, contact link/unlink, and per-link unlock.

## Consequences

The photographer gains one predictable left-to-right workflow and less layout movement during
selection. The tradeoff is a larger structural frontend phase before the more local refinements.

The contact picker gains recoverability but remains open longer, so its close and keyboard behavior
must be deliberate. All Photos adds a new workspace query surface and requires efficient photo
listing rather than client-side aggregation of every original.

Link-scoped unlock changes the cookie payload format. Existing cookies may be treated as invalid and
ask for the code again once, which is acceptable because the cookie is convenience state.
