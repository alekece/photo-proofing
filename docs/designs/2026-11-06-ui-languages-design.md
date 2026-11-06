# English and French

Status: approved by @alekece on 2026-11-06. Issue: #187.

## Intent

The photographer and many clients speak French; the tool being replaced offers several languages.
The product should be usable in English and French, with one simple switch.

## Goals

- The whole product is available in English and French: photographer app, client pages and the
  unavailable page.
- Switching is one click, in a place the photographer already looks.

## Non-goals

- Other languages (the design must make adding one cheap, but none is promised).
- A language per gallery or per viewer.
- Translating the photographer's own content: gallery titles, studio name, file names.

## Anti-goals

- A client seeing a mix of two languages.
- A language setting the photographer has to think about for every gallery.

## Approaches

1. **One workspace language (chosen).** One switch sets the app and every client page. Simple, and
   the same product has one voice. It can't serve a French photographer with an English-speaking
   client.
2. **Language per gallery, as Pixieset does.** A workspace default plus a per-gallery override in
   the side nav. More flexible, one more setting to manage per gallery. Can be added later without
   breaking approach 1.
3. **Follow each viewer's browser.** No setting, but the photographer cannot control what the
   client sees, and previews would differ from reality.

## Decisions

- **Switch.** A language button next to the notifications bell in the top bar, showing the current
  language ("EN" or "FR"); a small menu offers English and Français. It saves immediately with the
  usual "Saved" toast. On phones it stays in the top bar.
- **Scope.** The choice belongs to the workspace and applies to the photographer app and every
  client page, so a client always sees the language the photographer works in.
- **Default.** New workspaces start in English. *My call:* the first-run sign-in page uses the
  browser's language (English or French, else English), since no workspace choice exists yet.
- **Unavailable page.** It uses the workspace language. It reveals nothing about the gallery: the
  language is the same for every link of the installation. *My call.*
- **Dates and numbers** follow the chosen language ("12 Sep" and "12 sept.").
- **Layout.** French text is longer; screens must not break with it. The prototype gets a working
  switch, so the translated screens can be checked.

## PRD changes (need approval)

- FR-02: the workspace also owns the language.
- New requirement: the product is available in English and French, chosen once per workspace in
  the top bar, applied to the app and all client pages. Per-gallery or per-viewer languages and
  other languages are listed as future direction.
- MVP release: one acceptance check for switching language and seeing every screen translated.

## UI doc changes

- [`ui/screens/dashboard.md`](ui/screens/dashboard.md): the top bar gets the language button.
- [`ui/DESIGN.md`](ui/DESIGN.md): a rule that all text is translated and layouts tolerate longer
  French.
- New or refreshed pictures: the top bar in both languages, and a French dashboard, gallery and
  client page.

## Left to the builder

- How translations are stored and loaded in SvelteKit, and how the server returns the language to
  client pages and the unavailable page.
- The French wording of every string (reviewed by a French speaker).
