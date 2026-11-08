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
3. Lower the column cap with a width-based step up: two columns by default, three on very wide containers.

## Decision

Approach 3, with a count cap so the layout never drops to one column: `columns: 2 19rem`, and `3 19rem` from a 1400px container. A 1200px container shows 2 columns of ~560px instead of 3 of ~370px; 1400px shows 3 of ~430px. Two 19rem columns fit down to ~680px, so the fixed two columns under 700px stay continuous. The grid layout (15rem minimum) is unchanged and stays denser.
