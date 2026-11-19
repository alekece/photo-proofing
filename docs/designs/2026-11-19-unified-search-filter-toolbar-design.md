# Unified search and filter toolbar

Issue: [#403](https://github.com/alekece/photo-proofing/issues/403).
Existing patterns:
[dashboard](../ui/screens/dashboard.md),
[contacts](../ui/screens/contacts.md),
[activity](../ui/screens/activity.md),
[gallery](../ui/screens/gallery.md).

## Intent

Every signed-in list uses the same toolbar language and behavior: optional search, compact filters
and sort controls in one Warm Paper bar. Page-specific data and filtering logic stay on the page.

## Goals

- Provide one reusable toolbar primitive for Dashboard, Contacts, Activity and gallery collections.
- Optional search appears in the left or leading area.
- Filter and sort controls appear as compact right-side popovers or popover menus.
- Filters remain configurable through Svelte snippets/children and callbacks.
- Sorting uses the same configurable filter/menu component rather than a special visual pattern.
- Keep disabled, keyboard, focus, mobile and localized behavior consistent.

## Non-goals

- A global data store or shared filter state.
- Moving API requests or filtering algorithms into the primitive.
- Changing filter semantics on existing pages.

## Approaches

1. **CSS-only wrapper.** Rejected: it would duplicate control behavior and not provide slots.
2. **A specialized toolbar with props for galleries/contacts/activity.** Rejected: it would grow
   domain-specific branches and make future pages depend on old data shapes.
3. **A generic toolbar shell with search, filter and action snippets.** Chosen. The shell owns layout
   and responsive sizing; pages supply controls and callbacks.

## Decisions

- Search is optional and disabled rather than hidden when no item can be searched.
- The toolbar panel uses existing Warm Paper tokens, border, radius and spacing.
- Filter controls use existing Popover/Select primitives and retain their active summary.
- Sort is represented as a menu of mutually exclusive options, using one generic option model.
- The toolbar exposes an optional clear/reset action but does not decide filter contents.
- On mobile, active filter controls show a badge with the number of active filters.
- Active filter summaries remain stable in width and do not move adjacent controls.
- On narrow screens, search can occupy a full row and controls share the remaining row.
- At widths below `640 px`, search occupies the full row above filter and sort controls.
- The toolbar becomes sticky only when the filtered list is long enough to scroll substantially;
  short lists and empty states keep it in normal flow.

## Open questions

1. ~~**Search row:** when does search move to a full row?~~ **Resolved:** below `640 px`.
2. ~~**Active filter count:** show count badge?~~ **Resolved:** yes, active filter buttons show a
   count badge.
3. ~~**Toolbar stickiness:** sticky or normal flow?~~ **Resolved:** sticky only for long lists.
4. ~~**Clear filters:** reset placement?~~ **Resolved:** in the toolbar and empty-result state.
5. **Sort summary:** for Activity and Contacts, should sorting be omitted or should those pages
   expose an explicit direction/order control?

## Verification

- Every toolbar instance has a search slot, filter slot and optional sort/action slot.
- Page data can change without a toolbar remount or loss of focus.
- Filters remain usable by keyboard and at 390 px.
- Search and active filters have localized accessible names.
- Empty, loading and disabled states follow existing page behavior.
