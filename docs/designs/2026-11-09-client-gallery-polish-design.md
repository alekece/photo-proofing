# Client gallery polish: expiry line and repeated title/download

Issue: [#295](https://github.com/alekece/photo-proofing/issues/295). Small change; decisions marked *my call* are the author's and open to change in review.

## Intent

The client gallery says the same thing twice. The hero already shows the title, the expiry and (once delivered) "Download all"; the sticky bar right below repeats the title and the same download button. The expiry also adds an "N days left" suffix the client does not need.

## Goals

- The expiry line reads "Open until <date>" and nothing else.
- The title and "Download all" are shown once at a time: in the hero while it is on screen, and in the sticky bar only after the hero has scrolled out of view.

## Non-goals

- The photographer's gallery page. `ExpiryBadge` is used only by the client views today (`ClientGallery.svelte`, three places), so the suffix goes away for the client preview too, which is the same component.

## Approaches

1. **Remove the sticky bar's title and download (rejected).** A long delivered gallery would lose its quick download once the hero is scrolled away.
2. **Keep the sticky bar, show its title and download only when the hero is out of view (chosen, maintainer's call).** No repetition while the hero is visible, and the shortcuts are there when the client scrolls down.

## Decisions

- **Expiry:** `ExpiryBadge` shows `expiry_open_until` plus the date only. The "N days left" / "expires today" suffix reads as pressure on the client, so it goes everywhere `ExpiryBadge` is used (client gallery and its preview only). Remove `remaining` and the `expiry_today` / `expiry_days_left` messages once unused. *My call:* no gentler "last days" notice for now; it can be added later if photographers ask.
- **Sticky bar:** the hero's visibility is observed with an `IntersectionObserver` on the hero `header`. While the hero is visible, the bar holds only the All / Favorites switch (not delivered) and is not rendered at all when delivered. Once the hero is out of view, the bar also shows the title and, when delivered, "Download all". The change is animated softly and honours reduced motion. *My call:* the observer is skipped where unsupported, showing the bar's full content.
- Existing messages stay if another view uses them.

## Testing

- `ExpiryBadge.test.ts`: only "Open until <date>".
- `ClientGallery.test.ts`: with the hero visible there is one title and one "Download all"; once the hero is out of view the bar shows the title and the download (the observer is stubbed).

## Left to the builder

Spacing of the sticky filter bar once the title is gone.
