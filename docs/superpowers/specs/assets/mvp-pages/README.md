# MVP page design references

Visual references for the MVP pages, produced during brainstorming. Each page is a standalone HTML file (open it in a browser) with a PNG screenshot of its default state.

**The HTML is the source of truth.** The PNG is a quick preview and may not show interactive states. The mockups use throwaway inline CSS and JS: reproduce the look and behavior in SvelteKit, Tailwind and shadcn-svelte, not the markup. "PhotoProofing" is a placeholder brand.

| Page | HTML | Preview | Covers |
|------|------|---------|--------|
| Sign in and first run | `auth.html` | `auth.png` | First-run account setup with password confirmation, sign in, wrong-password error |
| Dashboard | `dashboard.html` | `dashboard.png` | Gallery cards, search, filter (including Archived), sort, notifications panel, golden-hour empty state, no-results state, New gallery dialog (title only) |
| Gallery | `gallery.html` | `gallery.png` | Settings panel, Photos / Favorites / Finals tabs, drop-zone empty states, Favorites download, offline, no-photos and export-failure simulations |
| Client gallery | `client-gallery.html` | `client-gallery.png` | Selecting (hearts, filter pill, lightbox) and delivered (download arrows, Download all ZIP) |
| Client 404 | `client-404.html` | `client-404.png` | Viewfinder "Focus not locked"; used for every unavailable link |
| Full flow | `prototype.html` | `prototype.png` | Clickable prototype: first run, sign in, create gallery, upload, share, Preview (Desktop/Phone), deliver, notifications. It is the reference for how the pages connect |

## Design tokens

Direction: "Warm paper". Georgia serif for headings and brand, system sans for UI text, Lucide line icons.

| Token | Value |
|-------|-------|
| Ink | `#2b2420` |
| Muted text | `#7a6c5f` |
| Page background | `#f4efe8` |
| Panel background | `#fbf8f3` |
| Borders | `#e0d6c8`, `#d6c9b8` |
| Danger / accent | `#b5381f` |
| Info | `#6b7f8e` |

## Behavior to preserve

- **Header action** is a progress button: Share gallery, then Deliver to client, then "Delivered". Each step uses press-again confirmation (second press within 5 seconds, with a draining fill; Esc or clicking away cancels). Delete uses the same pattern.
- **Share** is disabled until at least one photo is ready. **Deliver** is disabled with no finals, or while finals are uploading.
- **After delivery** the gallery is live: finals changes reach the client silently. There is no publish step.
- **Preview** opens the real client view in an overlay with a Desktop/Phone toggle. Delivered galleries preview the delivered view.
- **Favorites** shows only the client's selection, with a plain "Download" button. Selection is never blocked by the included-photos allowance.
- **Client 404** never reveals why a link is unavailable (unshared, expired, archived, deleted or unknown).

## Not covered by the mockups

Failed uploads with Retry, hero picker, watermark logo handling and photo ordering. The prototype omits the export-failure and offline simulations that `gallery.html` has. The client "photos coming soon" page is dropped from the MVP; confirm in the design doc.
