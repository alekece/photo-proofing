# Deliver dialog thumbnails

Issue: [#247](https://github.com/alekece/photo-proofing/issues/247)

## Intent

The Deliver confirmation lists the gallery's finals but draws proof thumbnails, so the tiles are broken images.

## Acceptance

- Deliver shows final-photo thumbnails in the confirmation and the success animation.
- Share keeps showing proof thumbnails.
- A regression test covers both.

## Approaches

- Pass the set to `PhotoStack` from `PublishDialog`: one line, uses the existing `set` prop.
- Change `PhotoStack`'s default: wrong, it would silently affect other callers.

## Chosen

Pass `set={kind === 'deliver' ? 'finals' : 'photos'}`, the same mapping already used to load the list.
