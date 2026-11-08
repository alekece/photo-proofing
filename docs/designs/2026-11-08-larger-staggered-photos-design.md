# Larger photos in the staggered layout

Issue: [#244](https://github.com/alekece/photo-proofing/issues/244)

## Intent

Staggered proofs are too small to judge at a glance. `ProofGrid` used `columns: 3 240px`, which allows three columns on any wide container.

## Acceptance

- Photos use more width at desktop and tablet sizes.
- Aspect ratios are kept, with no cropping.
- Fewer columns as the container narrows; order stays column-first and predictable.
- One change in `ProofGrid`, so photos, favorites and the client gallery all follow.

## Approaches

1. Keep three columns but cut the side padding: gains little.
2. Cap the count at two columns: large photos, but wastes space on very wide screens.
3. Raise the minimum column width and drop the column cap: the browser picks as many columns as fit.

## Decision

Approach 3: `columns: 24rem` (384px minimum). A 1200px container now shows 2 columns of ~560px instead of 3 of ~370px; three columns return from ~1230px. Containers under 700px keep their fixed two columns. The grid layout (15rem minimum) is unchanged and stays denser.
