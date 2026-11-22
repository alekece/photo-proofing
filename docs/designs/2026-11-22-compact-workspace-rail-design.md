# Compact desktop workspace navigation

Proposal for [#446](https://github.com/alekece/photo-proofing/issues/446).
The [photographer studio design](2026-11-22-photographer-studio-design.md)
remains the reference for the expanded workspace and the separate gallery rail.

Put a desktop-only **Collapse navigation** button immediately above the
account area of the global rail. This avoids crowding the brand and notification
bell at the narrowest desktop width. In compact mode, an **Expand navigation**
icon button stays in the same place. Keep keyboard focus on the button and
announce its state. Do not navigate or remount the current view.

Use the same links, bell and account menu in both modes. In compact mode, center
the icons in a 4rem rail; retain accessible names, hover/focus labels, the
current-page treatment and the unread indicator. Keep legal and build information
available from the account menu when it cannot fit in the rail.

Start expanded. Store only an explicit compact preference in browser-local
storage; expanding removes it. If storage is unavailable, the toggle still
works for the current visit. No account setting or server persistence.

Give the global rail its own effective width, defaulting to the existing gallery
rail width. Content and fixed selection bars use the effective width; the
gallery panel keeps its existing width and breakpoint. Below the desktop rail
breakpoint the preference has no visual effect: no toggle, no gutter and no
change to phone navigation. The public gallery is outside this change.

Verify keyboard collapse and expansion with a gallery selection active, the
bell and account popovers in compact mode, and selection-bar clearance with
zero, one and two rails. Check that reloading restores the preference and
expanding restores the original layout. Coordinate the offset formulas with
[#444](https://github.com/alekece/photo-proofing/issues/444); keep its expanded
width rather than copying a clamp into the global rail or action bar.
