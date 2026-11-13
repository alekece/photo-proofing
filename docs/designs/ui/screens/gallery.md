# Gallery

Where the photographer runs one shoot from upload to delivery. Route `/dashboard/{id}`
(prototype: `#/g/<id>/photos|favorites|final`).

| | Desktop | Phone |
|---|---|---|
| Photos | ![](gallery/photos.desktop.png) | ![](gallery/photos.phone.png) |
| No hero | ![](gallery/no-hero.desktop.png) | ![](gallery/no-hero.phone.png) |
| Uploading | ![](gallery/upload.desktop.png) | ![](gallery/upload.phone.png) |
| Select | ![](gallery/select.desktop.png) | ![](gallery/select.phone.png) |
| Updating watermarks | ![](gallery/watermark-updating.desktop.png) | ![](gallery/watermark-updating.phone.png) |
| Favorites | ![](gallery/favorites.desktop.png) | ![](gallery/favorites.phone.png) |
| Favorites, select | ![](gallery/favorites-select.desktop.png) | ![](gallery/favorites-select.phone.png) |
| Final | ![](gallery/final.desktop.png) | ![](gallery/final.phone.png) |
| Final, select | ![](gallery/final-select.desktop.png) | ![](gallery/final-select.phone.png) |
| Share | ![](gallery/share-confirm.desktop.png) | ![](gallery/share-confirm.phone.png) |
| Deliver | ![](gallery/deliver-confirm.desktop.png) | ![](gallery/deliver-confirm.phone.png) |
| Delete | ![](gallery/delete-confirm.desktop.png) | ![](gallery/delete-confirm.phone.png) |
| Side nav editing | ![](gallery/side-nav-edit.desktop.png) | |
| Photos in French | ![](gallery/photos-fr.desktop.png) | ![](gallery/photos-fr.phone.png) |

## Studio layout: left rail and top journey bar

The gallery page uses the standard top `AppHeader` above a two-column Warm Paper studio workspace on
desktop (`md:`), collapsing to a single-DOM compact stack on `390 px` phones:

- **Left studio rail (`identity` + `nav`):**
  - `← Back to galleries` link and a `3:2` framed cover thumbnail button (`44 × 34 px` on phones)
    showing the hero preview or a compact `CoverBackdrop` stand-in; clicking it opens **Design**.
  - Inline-editable Georgia gallery title paired on its right with the read-only status chip
    (**Draft**, **Prepublish**, **Publish**, **Expired**, **Archived**), plus `Shot` (or `Created`)
    and, once shared, `Last opened` (`Not yet` until first opened).
  - **Collection** navigation with counts: **Photos**, **Favorites** (accented when over allowance,
    padlock once published), **Comments**, and **Final** (`Favorites`, `Comments`, and `Final` stay
    visible and disabled before the first share, with tooltip `"Available once the gallery is shared"`).
  - **Studio** navigation (always enabled): **Design**, **Contacts** (with linked-contact count), and **Settings**.
  - On phones (`< md`), `nav` becomes a horizontally scrollable pill strip below the Journey bar.
- **Top Journey Wizard & action bar (`journey`):**
  - Actionable 4-step wizard on Warm Paper: **Upload → Share → Collect favorites → Publish** (hidden
    when archived; collapses to four progress bars + the active step label and fact on phones).
  - Right-aligned actions: **Preview** (outline) and the stage CTA (**Share** on a draft,
    **Publish** during Prepublish, nothing once published, **Restore** when archived).

## Studio views (`Design`, `Contacts`, and `Settings`)

| View / Card | Content |
|---|---|
| **Design** | Two-column workspace (stacking on `390 px` phones): **Cover** (**Select cover** / **Change cover** modal over ready proofs + **Remove cover**, four visual cover-template mini-cards — **Full bleed**, **Split editorial**, **Framed paper**, **Minimal banner** — and an interactive rule-of-thirds **Focal point** crosshair pad with `Reset`, disabled without a cover photo), **Typography** (four self-hosted heading-font specimen cards — **Editorial**, **Fine Art**, **Timeless**, **Modern** — rendered in their own typefaces), **Photo Grid & Spacing** (**Layout** icon buttons for staggered columns vs even rows, **Grid density** segmented **Compact** / **Large**, and **Spacing** segmented **Compact** / **Generous**), and a live scrollable **Client preview** with **Desktop** / **Phone** toggle |
| **Contacts** | Linked contacts list (monogram, **Name**, **Email**, optional **Phone**, **Unlink**), **Manage directory** (`/contacts`), and **+ Link contact** dialog (search unlinked workspace contacts or create a new contact inline and link it in one step) |
| **Settings → General** | **Shoot date**, **Included photos** (number input, disabled once published), and **Open until** (date picker with one-click relative presets from 1 week to 1 year); all optional |
| **Settings → Privacy & Protection** | **Client link** (Draft: `"Not shared yet"`; once shared: link and **Copy**) and **Enable watermarks** switch |
| **Settings → Actions** | **Archive** (or **Restore**) and destructive **Delete**, each with a short consequence hint and confirmation dialog |

- All seven panels (**Photos**, **Favorites**, **Comments**, **Final**, **Design**, **Contacts**,
  **Settings**) stay mounted so counts load on open and in-place edits, uploads, or open viewers
  survive section switches and autosaves.
- In-place inputs save on change without success rings or repeated `"Saved"` toasts. An invalid
  value shows a red toast and reverts. A past `"Open until"` marks the gallery Expired; a future or
  cleared one lifts it.
- The watermark switch is the only watermark control here. Mark, repeat, position and size live in
  [Settings](settings.md) and apply to every gallery.

## Collection views (`Photos`, `Favorites`, `Comments`, `Final`)

### Photos

The proofs. Toolbar: **Select** and **Upload**. Tiles show only:

- a star at the top right: **Set as hero** / **Remove hero** (a toggle). On hover and focus only,
  except the hero's own star, which stays visible as a gold star with no text (its tooltip and
  screen-reader label are "Remove hero"). On touch screens use the viewer instead;
- a × to delete, which always asks first.

No hearts or favorite marks here: favorites belong to the Favorites tab. Clicking a photo opens
the [viewer](viewer.md).

**Empty:** a drop zone, "Drop the shoot here", **Upload**.

**Upload:** each new tile goes Waiting → Uploading → Processing → ready, each with a label and an
indeterminate bar (no percentages, no overall counter). The status stays centered inside its
tile, above the bottom progress bar. A failed tile turns red, "Upload failed"
with the reason, a **Retry** button and ×; a red toast says "1 photo failed to upload."

![Upload motion](gallery/upload-motion.gif)
![Upload frames](gallery/upload-motion.frames.png)

**Updating watermarks:** after watermark settings change, all galleries show a slim
"Updating watermarks…" line with a small spinner in the toolbar until the marked proofs are
regenerated, even with watermarking off. Clean owner tiles, the open viewer and the hero remain
usable and unwatermarked. Unlike initial upload processing, regeneration keeps the photo visible
with a small bottom status badge; a failed regeneration includes **Retry**. Published render
revisions refresh client images without reloading the gallery; originals and finals stay
unwatermarked. Switching watermarking on or off only selects existing files.

### Favorites

Only the client's selection, read-only. Toolbar: **Select** and **Download all** (every favorite as a ZIP, or **Download X** for the X selected photos; hidden when there are no favorites, with a busy state while it's prepared). Hovering a photo shows a small download button
for that photo (always visible on touch). Empty: the same striped drop-area look as the other
empty tabs, with a heart and "No favorites yet". Opening a photo
shows the viewer with the favorite heart and the same individual-original download action.
The whole or selected subset uses one ZIP preparation slot: **Downloading…** is disabled from
the initial request until download initiation, with errors left visible and no separate success
message. Select-mode subsets always use a ZIP, even for one selected photo;
tile and viewer downloads stream the individual original with a busy download control.

### Final

The finished photos, private until delivered. Same toolbar as Photos (**Select**, **Upload**),
× to delete, drop zone "Drop the finished photos here" when empty. After delivery, a small
download mark shows which photos the client has downloaded.

### Select mode

Select mode replaces the toolbar with one sticky, accent-tinted bar: an icon-only **✕** (Exit
selection) on the left, "3 selected", a tri-state **Select all** checkbox (clicking it when all are
selected unselects all) and, on the right, **Delete** on Photos and Final or **Download** on
Favorites (both disabled at 0; Upload is hidden). On small screens it collapses to `[✕] 3 selected`
with icon-only actions. **✕** and Escape leave select mode. Tiles show a check; clicking toggles.
Select all only picks the current tab.

All three tabs use the same striped empty-state background. Confirming a photo deletion shows
a disabled **Deleting…** button with a spinner and retains the confirmed count.
Cancel and Escape cannot dismiss it while deletion runs; removed tiles shrink and fade unless
reduced motion is requested. Desktop journey steps are centered within the banner.

## Dialogs

Share, Deliver and Delete always confirm. Share and Deliver never refuse: when nothing is ready
they warn and the button says "anyway". The "until 12 Sep" in the Share text only appears when
an Open until date is set.

Deliver counts and warns about finals only; proofs do not contribute to that dialog.

| Dialog | Normal | Something missing |
|---|---|---|
| Share | "Share 48 photos?" "This creates a private link. Anyone with it can see the proofs and pick favorites until 12 Sep." **Cancel** / **Share** | "Share with no photos?" warning "Clients see an empty gallery until you upload." **Share anyway** |
| Deliver | "Deliver 20 photos?" "Camille & Hugo see them at the same link and can download them. Favorites are locked from now on." **Cancel** / **Deliver** | "Deliver with no photos?" **Deliver anyway** |
| Delete photos | Stack of the photos, count; mentions client favorites only if one is a favorite | — |

Photos still uploading or failed are mentioned as a warning line ("2 are still uploading and 1
failed."). After Share the photo stack flies out, a green check pops and the dialog becomes "Your
gallery is shared" with **Copy link**. The prototype also shows Email and SMS; those are future
scope, not MVP.

![Dialog motion](gallery/confirm-motion.gif)
![Dialog frames](gallery/confirm-motion.frames.png)
