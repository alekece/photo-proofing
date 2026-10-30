# MVP — Local-network workflow trial

Status: Planned. This is a release scope and acceptance record, not a claim that the current scaffold implements the workflow.

Product authority: [SPECIFICATIONS.md](../SPECIFICATIONS.md). Architecture: [ADR 0001](../adr/0001-technology-baseline.md).

At release completion, record the release identifier/date, the accepted PRD commit or tag, and acceptance evidence here. Until then, this record may be refined with the PRD. After completion, preserve it as history; later release scope belongs in a new release document.

## Goal

A photographer can prepare and share JPEG proofs, receive a shared client selection, export selected originals for external retouching, and deliver finished photos for download through the same link. Try the complete journey on the local network with replaceable data and representative galleries of 50–100 photos.

## Included scope

- One photographer account/workspace per installation; one-time setup, email/password, logout, persistent sliding sessions ([FR-01–FR-02](../SPECIFICATIONS.md#fr-01)).
- Title, hero, two layouts, optional expiry, editable optional included photo allowance, and watermark enablement ([FR-03](../SPECIFICATIONS.md#fr-03)).
- Progressively populated proofing shares through one private link and one client selection, once a photo is ready ([FR-04](../SPECIFICATIONS.md#fr-04)).
- JPEG uploads, separate transfer/processing progress, failed-file retry, reduced previews, and optional workspace text/logo watermark ([FR-05–FR-07](../SPECIFICATIONS.md#fr-05)).
- Editable favorites with informational overflow, and one-click selection ZIP preparation/download ([FR-08–FR-09](../SPECIFICATIONS.md#fr-08)).
- Private preparation and explicit delivery of independent finished-photo sets, later updates, individual/ZIP client downloads ([FR-10](../SPECIFICATIONS.md#fr-10)).
- Gallery-card activity and top-bar grouped selection/download notifications with explicit reads ([FR-11–FR-12](../SPECIFICATIONS.md#fr-11)).
- Expiry, archive/restore, confirmed deletion, persistent local storage, durable processing, and SSE updates ([FR-13](../SPECIFICATIONS.md#fr-13), [NF-01–NF-04](../SPECIFICATIONS.md#nf-01), [TC-01–TC-05](../SPECIFICATIONS.md#tc-01)).
- Responsive photographer and client journeys in mobile and desktop browsers.

## Excluded and deferred

No Google sign-in, password recovery, collaboration/account management, roles, workspace switching, email/SMS sending, access codes, native apps, RAW/other photo inputs, custom preview-quality controls, configurable metadata policy, resumable uploads, S3-compatible storage, backup/restore tooling, public-internet deployment, or SaaS provisioning.

No client submission button or retouching phase. No individual-original export interface, separate download-management screen, or return from finished delivery to proofing. No proof-to-final matching algorithm. Billing is outside the product entirely, not merely deferred from this release.

Logo input formats, resource limits, session lifetimes, and processing/export mechanisms must be settled by the corresponding implementation designs. JPEG-only photo support does not prohibit a separately validated logo format. These design choices must preserve the PRD guarantees rather than silently expanding scope.

## Acceptance checklist

Run with synthetic/replaceable photos whose originals are retained elsewhere. The checks below remain unchecked until verified against the implemented product. Record failures and evidence rather than treating a healthy container or unit tests alone as end-to-end acceptance.

### Setup and access

- [ ] On an empty installation, create the photographer account through the login URL; subsequent and concurrent setup attempts cannot create another initial account. Verify email/password login and logout ([FR-01](../SPECIFICATIONS.md#fr-01)).
- [ ] A persistent session survives browser restart. Verify the configurable inactivity timeout (default 30 days), activity-driven renewal with no absolute lifetime cap, and that background traffic does not renew it. Server expiry and logout invalidate access, including copied session tokens and protected event/media endpoints ([FR-01](../SPECIFICATIONS.md#fr-01), [NF-01](../SPECIFICATIONS.md#nf-01)).
- [ ] Reach the application from a supported mobile browser and another desktop browser on the local network after the deployment design establishes LAN configuration; do not treat localhost access as proof of this ([NF-03](../SPECIFICATIONS.md#nf-03), [TC-02](../SPECIFICATIONS.md#tc-02)).

### Prepare, process, and share

- [ ] Create a gallery from the title-only New gallery dialog, then set hero, switch between default uniform grid and masonry, and set/clear expiry and included photo count ([FR-03](../SPECIFICATIONS.md#fr-03)).
- [ ] Share gallery is disabled while the gallery has no ready photo; once one is ready, sharing opens the private link with no client account or code required ([FR-04](../SPECIFICATIONS.md#fr-04)).
- [ ] Upload a representative 50–100-JPEG gallery; observe transfer progress separately from processing. Navigate away and return to the current processing state. Ready previews appear progressively at the shared link; pending/failed photos never appear as ready ([FR-04–FR-05](../SPECIFICATIONS.md#fr-04), [NF-04](../SPECIFICATIONS.md#nf-04)).
- [ ] Reject non-JPEG, malformed, and over-limit photo inputs. Simulate interrupted upload and processing failure: retain successful files, clean incomplete files, and allow retry without duplicates ([FR-05](../SPECIFICATIONS.md#fr-05), [NF-01–NF-02](../SPECIFICATIONS.md#nf-01)).
- [ ] Save watermark text, logo, and both; verify gallery enable/disable behavior. Check previews preserve orientation/color, reduce usable resolution/quality, and omit source identifying/GPS metadata. Original bytes remain unchanged ([FR-06–FR-07](../SPECIFICATIONS.md#fr-06)).
- [ ] Test direct media/download requests as well as the UI: client proofing access cannot fetch uploaded originals or another inaccessible gallery's assets ([AC-01](../SPECIFICATIONS.md#acceptance-criteria)).

### Select and export

- [ ] Use the same link from multiple browsers: favorites form one shared persistent selection across visits ([FR-08](../SPECIFICATIONS.md#fr-08)).
- [ ] Configure 10 included photos and select 13: show 13 selected/3 extra and permit more. Change or remove the allowance without erasing choices or blocking selection. With no allowance, show the selected count without implying completion ([FR-03](../SPECIFICATIONS.md#fr-03), [FR-08](../SPECIFICATIONS.md#fr-08)).
- [ ] Remove a selected proofing photo as photographer: require confirmation and remove its favorite. Other photos and favorites remain intact ([FR-04](../SPECIFICATIONS.md#fr-04)).
- [ ] Request Download selection, see preparation status, and receive a ZIP containing byte-identical selected originals. Change favorites during preparation: the requested ZIP remains consistent with the request-time selection, and live selection stays editable ([FR-09](../SPECIFICATIONS.md#fr-09), [AC-02–AC-03](../SPECIFICATIONS.md#acceptance-criteria)).

### Deliver and update

- [ ] Upload finished JPEGs individually and in batches, using a smaller, equal, or larger set than the selection without mapping requirements. Undelivered files remain inaccessible to clients ([FR-10](../SPECIFICATIONS.md#fr-10), [NF-01](../SPECIFICATIONS.md#nf-01)).
- [ ] Prevent Deliver for an empty set or a set containing unfinished/failed photos. After retrying/removing those files, deliver the complete intended set ([FR-10](../SPECIFICATIONS.md#fr-10)).
- [ ] At the same private link, show only the delivered finished photos; reject client favorite changes. Individual downloads and the all-photo ZIP preserve the uploaded finished-file bytes, including metadata ([FR-06](../SPECIFICATIONS.md#fr-06), [FR-10](../SPECIFICATIONS.md#fr-10)).
- [ ] Add/remove finished photos after delivery: clients see the changes without a publish step, with no intermediate set or incomplete file exposed. Removing every finished photo leaves clients an empty delivered gallery ([FR-10](../SPECIFICATIONS.md#fr-10), [NF-02](../SPECIFICATIONS.md#nf-02)).

### Activity and notifications

- [ ] Selection edits and finished-photo download requests update gallery-card activity and accumulate in one unread top-bar notification per gallery. Repeated downloads do not inflate distinct-photo counts; an all-photo request covers its included files ([FR-11–FR-12](../SPECIFICATIONS.md#fr-11)).
- [ ] Opening the panel or navigating independently to the gallery does not clear unread status. Clicking the notification navigates and marks that gallery's notification read; the single Mark all as read link clears the rest without navigation. Subsequent activity creates a new unread notification ([FR-12](../SPECIFICATIONS.md#fr-12)).
- [ ] Read state belongs to the workspace, not a browser. Delivered-set updates recalculate relevant counts: newly added photos start undownloaded and removed photos no longer contribute. All downloaded is never inferred from an empty set or claimed as proof of a saved file ([FR-02](../SPECIFICATIONS.md#fr-02), [FR-11](../SPECIFICATIONS.md#fr-11)).

### Lifecycle and recovery

- [ ] Expire a proofing or delivered gallery: deny client page/media/download/event access but retain photographer access and all data. Extend expiry and restore permitted access without regeneration. No-expiry galleries remain available until access is otherwise blocked ([FR-13](../SPECIFICATIONS.md#fr-13), [NF-01](../SPECIFICATIONS.md#nf-01)).
- [ ] Archive and restore a gallery: preserve assets, selection, activity, delivery state, and private link. Restoring an expired gallery does not bypass expiry ([FR-13](../SPECIFICATIONS.md#fr-13)).
- [ ] Confirm deletion while work is queued/in progress: deny subsequent access and eventually remove stored assets/temporary outputs; late workers cannot recreate accessible deleted content ([FR-13](../SPECIFICATIONS.md#fr-13), [NF-02](../SPECIFICATIONS.md#nf-02)).
- [ ] Restart API/worker and recreate containers with persistent storage: accepted data and jobs survive. Interrupted work resumes eligibility, retries do not duplicate/corrupt results, and repeated failures become visible and manually retryable ([NF-02](../SPECIFICATIONS.md#nf-02), [TC-03–TC-04](../SPECIFICATIONS.md#tc-03)).
- [ ] Disconnect and reconnect SSE during activity/processing: refresh authoritative state and recover missed changes without exposing data after access is revoked. Disk-full and bounded processing failures are visible and never reported as successful completion ([TC-05](../SPECIFICATIONS.md#tc-05), [NF-01–NF-04](../SPECIFICATIONS.md#nf-01)).
- [ ] Complete the main photographer/client journey with keyboard navigation and mobile/desktop layouts, including dialogs, viewer, favorites, notifications, upload errors, and download-preparation status ([NF-03](../SPECIFICATIONS.md#nf-03)).
