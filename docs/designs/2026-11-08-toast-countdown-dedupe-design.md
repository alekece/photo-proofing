# Toast countdown and duplicate merging

Issue: [#262](https://github.com/alekece/photo-proofing/issues/262)

## Intent

Auto-dismissing toasts give no hint of how long they stay, and repeated triggers
stack identical toasts. Make the remaining time visible and merge repeats.

## Goals

- Auto-dismissing toasts show a thin bar along the bottom edge that shrinks
  slowly over the dismiss duration. No percentage or text.
- The close button stays.
- Identical toasts merge while one is visible: the countdown restarts and an
  optional `×N` counter increases.

## Non-goals

- Pause on hover/focus.
- A reusable progress component (this is the app's first bar; extract it when a
  second use appears).
- Changing the 4s duration or the default persistence of errors.

## Behavior

- **Same toast:** same `kind` and `message`, and only while it is still visible.
  A duplicate arriving after it disappeared creates a fresh toast (count 1).
- **On duplicate:** the existing toast's timer restarts (auto-dismissing toasts
  only), its count increments, and a newer `retry` replaces the old one.
- **Auto-dismiss:** each toast is either auto-closable or persistent. Defaults
  stay as today (success auto-closes after 4s, errors persist); a call can
  override either way.
- **Bar:** only on auto-closable toasts. A persistent toast shows no bar at all,
  just the close button. Honors `prefers-reduced-motion` by not animating (the toast
  still dismisses on schedule).
- **Counter:** shown as `×N` from the second occurrence, only when enabled.
  Default on for errors, off for success; a call can override it, e.g.
  `toasts.success('Saved', { count: true })`.
- **`saved()`:** the 3s quiet-period debounce (`SAVED_QUIET_MS`) is removed.
  "Saved" appears immediately and repeated saves merge into it.

## Approaches

- **A. CSS animation (chosen).** The store keeps a per-toast timer; a restart
  bumps a `generation` number. The toaster renders the bar with a CSS animation
  lasting the dismiss duration, re-keyed on `generation` so it restarts. Small
  and smooth; bar and timer are separate clocks (drift is invisible at 4s).
- **B. JS-driven progress** (`expiresAt` plus `requestAnimationFrame`/`Tween`).
  Guaranteed sync and easy hover-pause later, but more code and harder tests.
  Rejected: hover-pause is out of scope.

## Constraints

- Changes stay within `web/src/lib/components/ui/toast/` and call sites of
  `saved()`; `pnpm --dir web verify` must pass.
- Tests cover merge rules, restart, counter defaults/override, bar presence,
  and the removed debounce.

## Left to the builder

Bar color/height, exact counter styling, keyframe name, and how timers are
tracked per toast.
