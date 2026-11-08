# Toast countdown and duplicate suppression

Issue: [#262](https://github.com/alekece/photo-proofing/issues/262)

## Intent

Auto-dismissing toasts give no hint of how long they stay, and repeated triggers
stack identical toasts. Show the remaining time and suppress repeats, centrally.

## Behavior

- **Countdown:** an auto-dismissing toast shows a thin bar along its bottom edge that
  shrinks over the dismiss duration (a CSS animation sized from the toast's `duration`,
  4 s today). Persistent toasts (errors) show no bar. The close button stays.
- **Same toast:** same `kind` and `message`, only while it is still visible. A repeat
  after it disappeared shows a fresh toast.
- **On duplicate:** the existing toast is left alone (timer not restarted, no counter), so
  the bar always matches the real dismissal time. A newer `retry` is ignored.
- **Reduced motion:** the bar still shrinks, in four discrete steps instead of a smooth slide,
  so time is conveyed without continuous motion.
- **No pause on hover/focus:** out of scope for the MVP.
- **`saved()`:** keeps its quiet-period debounce (`SAVED_QUIET_MS`, see the debounced Saved
  toast design). Central suppression does not replace it: it would show "Saved" immediately
  and again once the first toast is gone, whereas the debounce shows one toast after
  editing stops.

## Rejected

- Restarting the timer on duplicates: the bar would need to restart too, and a spammed
  toast could never go away.
- JS-driven progress: more code and harder tests than a CSS animation, with no benefit
  without hover-pause.

## Tests

Bar only on auto-dismissing toasts, close button works, duplicates are not stacked
(different kind is distinct, original timer unchanged, repeat after dismissal allowed).
