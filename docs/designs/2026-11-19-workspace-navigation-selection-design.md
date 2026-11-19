# Workspace navigation and bulk selection

Issues: [#391](https://github.com/alekece/photo-proofing/issues/391),
[#404](https://github.com/alekece/photo-proofing/issues/404).
Related designs:
[#342](2026-11-12-gallery-studio-layout-design.md),
[#241](2026-11-08-select-mode-toolbar-design.md),
and [UI redesign](2026-11-06-ui-redesign-design.md).

## Intent

The signed-in photographer needs a stable workspace navigation and a selection interaction that is
consistent in Galleries, Photos, Favorites, Final and Contacts. This design changes placement and
interaction structure only; it does not replace the Warm Paper visual identity.

## Goals

- Keep the workspace side navigation mounted and usable everywhere: Dashboard, Contacts, Activity,
  Settings and Gallery studio.
- Keep the footer outside the side navigation area on desktop and avoid overlapping it.
- Put Settings below Activity.
- Place Language and Notifications in the side navigation near the account section, separated from
  the main section links by a divider.
- Put the photographer avatar, name and Sign out at the bottom of the side navigation.
- Start selection from a small circle at the top-left of a card or photo.
- Use one reusable selection bar with a close control, selected count, Select all and
  page-specific actions.
- Keep staged gallery bulk changes until Apply and reuse the existing cancel/save lifecycle.

## Non-goals

- A final redraw of the brand, typography, cards or sidebar styling.
- Persisting a desktop/mobile sidebar mode in the backend.
- Changing privacy, upload, delete, favorite or download behavior.

## Approaches

1. **Keep the top navigation and add a sidebar only in detail pages.** Rejected: navigation would
   disappear or move between routes and the audit goal of predictability would remain unmet.
2. **Replace the top workspace navigation with one persistent sidebar.** Chosen. Language,
   notifications and the account identity may move into the sidebar without changing route URLs or
   gallery data.
3. **Build separate selection bars for each page.** Rejected: close/count/select-all behavior would
   drift and duplicate accessibility behavior.
4. **A shared selection bar with an actions snippet and per-page callbacks.** Chosen. The shared
   component owns layout and lifecycle; each page supplies Delete, Download or future actions.

## Decisions

- **Desktop sidebar:** a persistent left rail with brand, Galleries, Contacts, Activity, Settings,
  account controls and footer-safe spacing.
- **Sidebar width:** use `16vw`, clamped between `240 px` and `280 px`
  (`w-[clamp(15rem,16vw,17.5rem)]`).
- **Mobile sidebar:** maintain one compact navigation DOM; do not duplicate desktop/mobile account
  controls. The exact compact breakpoint and drawer behavior remain open to review.
- **Account section:** Language and Notifications are account-adjacent controls. A divider separates
  them from section navigation. Photographer identity and Sign out remain together at the sidebar
  bottom.
- **Selection entry:** remove Select buttons from gallery and photo top toolbars. A selection circle
  appears on hover/focus and remains visible on touch. Clicking one circle selects that item and
  enters selection mode.
- **Selection bar:** close control and count stay left, Select all remains next to the count, and
  actions render in a right-side slot. Disabled actions stay visible.
- **Context actions:** Photos and Final supply Delete; Favorites supplies Download; Galleries can
  supply staged Favorite, Quick edit and Delete actions.
- **Save lifecycle:** the shared action control has idle, processing, confirmed and reset states.
  Confirmed feedback is temporary and returns to idle without hiding a newer unsaved edit.
- **Page padding:** shared workspace root layout owns the main-content margin/padding used by
  Dashboard, Contacts, Activity, Settings and Gallery routes.

## Open questions

1. ~~**Sidebar width:** use 240 px, 256 px or another existing-width value?~~ **Resolved:** `16vw`,
   clamped from 240 px to 280 px.
2. **Phone navigation:** at 390 px, should account controls remain in a compact sidebar, collapse
   into the top bar, or open from the photographer avatar?
3. **Footer:** should the sidebar extend to the viewport edge with the footer outside it, or should
   the rail stop above the footer?
4. **Selection bar placement:** should it float at the viewport bottom or be sticky inside the page
   content container?
5. **Gallery staged changes:** should pinning/favoriting update immediately while status and Quick
   edit remain staged, or should every gallery bulk change wait for Apply?
6. **Selection bar style:** should the floating bar use the dashboard accent-tinted style or the
   existing gallery sticky bar style, and should it remain one component across both contexts?

## Verification

- Navigation remains visible and does not overlap the footer on desktop and 390 px phone layouts.
- Language, notifications, identity and Sign out follow the approved placement.
- Selection starts from one card/photo circle and never restyles or moves the underlying grid.
- Escape and the close control exit selection mode.
- Context actions can be replaced without changing count/select-all behavior.
- No duplicate IDs or inaccessible hidden navigation exist across breakpoints.
