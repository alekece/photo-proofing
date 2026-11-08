# Input focus style

Issue: [#243](https://github.com/alekece/photo-proofing/issues/243)

## Intent

Focused inputs show a heavy 3px near-black outline that clashes with the soft warm-paper fields. The cause is an unlayered global `:focus-visible` rule that beats the layered `outline-none` utility on the input component.

## Acceptance

- Text, password, email, search and select fields show a subtle focus style with no heavy outline.
- Focus stays clearly visible (WCAG 2.4.7, 1.4.11).
- Buttons and links keep an outline indicator.
- Focus has one source of truth.

## Approaches

1. Remove the global outline and style each component. Rejected: scatters focus rules and risks controls with none.
2. Keep one global rule but make `outline-none` win. Does not change the look by itself.
3. **Chosen:** one global rule in the `base` layer, split by control kind, so utilities can still override it.

## Chosen style

- Buttons and links: 2px `--color-focus` outline, 2px offset (thinner than before).
- Fields (`input`, `select`, `textarea`): no outline; the border darkens to `--color-foreground` (`focus-visible:border-foreground` on the input and select components) plus a 3px ring of `--color-focus` at 18%.
- Component-specific overrides (inline title inputs, viewer controls) are unchanged and keep winning, as the rule is in `base`.

## Contrast

The border goes from `#d6c9b8` to `#2b2420` on white, about 14:1 against the field background and far above the 3:1 required by 1.4.11. The ring is decorative reinforcement. Outlines on buttons and links use the same `#2b2420`, about 12:1 on the page background.
