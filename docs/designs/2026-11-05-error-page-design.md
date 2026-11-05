# App error page

Issue: #152. Follows the [frontend design system](2026-10-31-frontend-design-system-design.md).

## Intent

When a page fails to load, the photographer or client sees a short explanation and a way back,
not SvelteKit's bare default page.

## Goals

- One root `web/src/routes/+error.svelte` used by every page.
- A not-found (404) and a server failure (5xx and anything else) read differently.
- A heading that receives focus when the page appears.
- A link back to a safe place.

## Non-goals

- Visual design beyond existing design-system tokens and components; the real mockup comes later.
- Error logging or reporting (`handleError` hooks).
- Changing how pages already show expected states (expired links, empty galleries).

## Approach

A single root error page reads `page.status` and `page.error` from `$app/state`.

- **404:** "We couldn't find this page."
- **Everything else:** "Something went wrong on our side. Please try again in a moment."
- **Raw error text:** never shown, so internal details don't leak. SvelteKit already replaces
  unexpected errors with a generic message, and the page doesn't display it either.
- **Link back, chosen from the failing URL:**
  - Under `/dashboard`: "Back to your galleries" (`/dashboard`).
  - Under `/g/` (a client's private link): a "Try again" link to the same URL, because the
    client has no account and sign-in would mislead them.
  - Elsewhere: "Go to sign in" (`/login`).
- **Focus:** the `h1` has `tabindex="-1"` and is focused on mount, matching how other pages
  handle focus.

The root page also covers `/dashboard`, so the dashboard header isn't shown on errors. That keeps
one simple page that works even when the dashboard layout itself failed.

## Testing

Component tests with a mocked `$app/state` cover:
- 404 and 500 wording;
- the link chosen for each URL kind;
- focus on the heading.

A route-level test checks that a failing `load` produces SvelteKit's error state, using the
existing dashboard page test setup.
