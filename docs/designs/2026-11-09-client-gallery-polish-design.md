# Client gallery polish: expiry line and repeated title/download

Issue: [#295](https://github.com/alekece/photo-proofing/issues/295). Small change; decisions marked *my call* are the author's and open to change in review.

## Intent

The client gallery says the same thing twice. The hero already shows the title, the expiry and (once delivered) "Download all"; the sticky bar right below repeats the title and the same download button. The expiry also adds an "N days left" suffix the client does not need.

## Goals

- The expiry line reads "Open until <date>" and nothing else.
- The title and "Download all" appear once, in the hero.

## Non-goals

- The photographer's gallery page. `ExpiryBadge` is used only by the client views today (`ClientGallery.svelte`, three places), so the suffix goes away for the client preview too, which is the same component.

## Approaches

1. **Hide the sticky bar's duplicates, keep the bar (chosen).** Drop the title (`h2`) from the bar. When delivered, drop its download button too, and since the bar is then empty, do not render it. When not delivered, the bar keeps the All / Favorites switch.
2. **Remove the hero's download button instead (rejected).** The hero is the first thing the client sees, and the issue keeps it there.

## Decisions

- **Expiry:** `ExpiryBadge` shows `expiry_open_until` plus the date only. Remove `remaining` and the `expiry_today` / `expiry_days_left` messages (both languages) once nothing uses them.
- **Sticky bar:** remove the `h2` and the delivered "Download all" button. Render the bar only when not delivered, where it holds the filter switch alone. *My call:* a delivered gallery has no favorites switch to show, so no bar rather than an empty one. A delivered client therefore loses the sticky download while scrolling a long gallery; the hero button is one scroll up. Say so in review if you want it back as an icon-only button.
- Existing messages stay if another view uses them.

## Testing

- `ExpiryBadge.test.ts`: only "Open until <date>".
- `ClientGallery.test.ts`: delivered has one "Download all" and one title; not delivered shows the filter without a repeated title.

## Left to the builder

Spacing of the sticky filter bar once the title is gone.
