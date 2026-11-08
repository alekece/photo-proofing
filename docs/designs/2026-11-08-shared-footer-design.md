# Shared footer

Issue: [#248](https://github.com/alekece/photo-proofing/issues/248). The
maintainer approved going straight to implementation and overrode the issue's
client exclusion: every page, including the client gallery, gets the footer.

## Behaviour

One footer closes every page: brand name, copyright, a slot for future
mandatory or legal links, and the build version. It sits at the bottom of short
pages and follows the page after longer content. It replaces the dashboard
footer, the Settings `BuildInfo` and the client gallery's own signature
footer. The dashboard keeps its gallery and photo totals as a plain line above
the shared footer; the studio signature stays on the client cover.

The build version is one generic value (`web/src/lib/build.ts`). Today it is
the full commit SHA embedded by the build (`PP_BUILD_SHA`, see
[build identity](2026-11-07-build-identity-design.md)), displayed as seven
characters; a release version later replaces the value and is displayed as is.
Unstamped local builds show "Development build".

Photographer pages offer a copy action for the full version. The client gallery
shows it small, with no copy action, since clients have no use for it.

Legal information is a list in `web/src/lib/footer.ts`, empty until the product
defines real notices; the footer renders links only when entries exist. No
legal text is invented here.

## Approaches

- **Footer in the root layout (chosen).** One place, covers login, error
  pages, dashboard, settings and client pages. The layout is a full-height
  column, so the footer sticks to the bottom; the layout reads the route id to
  turn copying off for `/g/*`.
- Per-area layouts: repeats the footer and misses login, settings and errors.
- Footer inside each page: the current situation, many sources of truth.
