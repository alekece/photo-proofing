# Share a gallery and deliver final photos

Approved 2026-11-22 for [#425](https://github.com/alekece/photo-proofing/issues/425).

## The photographer's journey

Importing photos never sends an email. The photographer decides when to **Share gallery** (invite the client to select proofs) and when to **Deliver final photos** (**Livrer les photos retouchées** in French). Keep the four studio steps, but remove the Share tab. Each unfinished sharing or delivery step opens its confirmation from the journey bar. A completed step shows what happened; it does not offer to send again. An old link to the Share tab lands on the gallery's Photos without taking any action.

Use the same compact dialog for both stages. Lead with the gallery cover (or its existing fallback), title and ready photo count for this stage. Keep the existing readiness warnings **inside** this dialog: the photographer can confirm even with no ready photos or an upload in progress. Show a warning instead of a misleading zero if readiness cannot be checked.

The single primary button is **Share** or **Deliver** (**Partager** or **Livrer**), whether or not an email will be sent. Cancel changes nothing. After the server accepts, return to the originating screen and confirm the stage change. Mention that emails are queued only if they were requested. Do not wait for the worker or imply that an email reached an inbox. A rejected request leaves the dialog, recipients and edits intact.

## An email checkbox for this action

The dialog opens compact. **Send an email** is checked by default when mail is configured and at least one valid recipient is selected. Prefill linked contacts and show who will receive the email while the checkbox is checked. **Customize** (**Personnaliser**) is enabled only while it is checked. The photographer can confirm with the default email without ever opening the editor.

**Customize** expands the same dialog; it neither publishes the gallery nor queues mail. On desktop, put the editable recipients (To and optional BCC), subject and message on the left and the live email preview on the right. On a phone, stack the preview below the editor. At the bottom of the editor, offer **Send me a copy** (**M’envoyer une copie**), unchecked by default. This sends one blind copy to the photographer's account address using the general gallery link, never a client's personal link. Reuse recipient deduplication: no extra message if the address is already included, no new contact for a blind copy, and no copy when email is unchecked.

Unchecking **Send an email** returns the dialog to its compact state without losing edits or setting a preference for future actions. Confirming with email unchecked changes the stage only: no contact creation or mail jobs, even with recipients selected.

The click fixes the email choice, recipients, blind copies and message. Only an untouched default subject follows a pending gallery title save. If a pending save makes a requested email ineligible, keep the dialog open without sharing or delivering.

With no recipients, leave the checkbox unchecked and disabled, not the stage button. The label beside it is simply **“No recipient”** (**« Aucun destinataire »** in French). The photographer can still add a recipient in the dialog. With SMTP unavailable, show the corresponding reason instead. Do not mistake a pending or failed contact lookup for a known empty list. An explicit opt-out is never undone by editing the recipients.

The general link and, when configured, the access code are copied **separately** in **Gallery settings → Privacy & protection**, not in the dialog, its success message or the studio header. Copying never changes the stage or sends anything. Email recipients still receive personal links for To and the general link for BCC; attribution and access rules are unchanged.

## One transition, then one background job per recipient

Confirming the email path commits the intended stage and its email jobs together. Sharing activates the link; delivery switches that same link to ready finals, closes client favorite editing and preserves existing watermark-job handling. Validate saved gallery edits, including the access code, before sharing. Leave existing manual endpoints and the client experience intact.

The server checks the intended stage under the same transaction as the change. A repeated click, lost response or second browser must not enqueue a second batch or send a proofing invitation as a delivery email. If a response was lost, refresh the gallery before offering to submit again; a completed step is not evidence that anyone received an email. A failure to queue rolls back the stage change. A later SMTP failure does **not** roll it back.

At confirmation, a prefilled address must still match its original gallery link. An explicitly added address can create a link; a stale selection cannot. At the end of preparation, immediately before SMTP, the worker checks the stage, link and destination again. Skip an obsolete invitation without recording a send. Delivery does not wait for, or recall, mail already handed to SMTP.

## When an email cannot be sent

Use **Activity**, both workspace-wide and inside the gallery. The existing worker retries automatically. Only when a recipient's job reaches terminal failure does the photographer see a line such as **“Camille's invitation could not be sent · Retry”**. No line for transient failures, no error banner to maintain in the gallery, no new bell notification in #425. Whether failures notify belongs to [#427](https://github.com/alekece/photo-proofing/issues/427).

- Keep a failure line tied to its individual job, not grouped by gallery or contact. Record it durably with the terminal failure, including when a worker's last lease expires. Do not copy the job payload, SMTP error, email body or link into activity. Use the same recipient privacy as sent-email activity, including for BCC.
- **Retry** requeues only that existing failed email for the authenticated workspace and gallery. An expired gallery keeps its history but has no Retry button. Recheck expiry on the server when clicked, so a stale Activity screen cannot requeue it. Extending expiry never sends mail automatically; retry is possible again only if the failed job, link, recipient, stage and activity line are still eligible. A double click cannot requeue it twice. Never share, deliver or notify successful recipients again as part of this action.
- After retry is accepted, remove the button; the line remains historical. If the job exhausts its attempts again, the same line offers retry again. A successful send uses the existing sent-email activity; a revoked or obsolete invitation is not actionable. Reuse existing queue and activity updates, not a second retry engine or email tracking service.
- Keep the normal Activity retention period (90 days by default). Once the line ages out, that in-studio retry expires with it; this does not change the queue's retention or introduce a permanent retry inbox.

## Alternatives and boundaries

Keeping a permanent Share tab duplicates the decision. Two submit buttons make email look like a second stage; a workspace preference hides its effect when the photographer confirms. One button names the stage and a checkbox says who will be emailed for this action. A gallery-only failure banner hides problems from photographers working in other galleries; Activity gives them one place to act.

No voluntary resend, invitation to a newly added contact after sharing, new notification preferences, changes to client permissions, or Storybook integration. Only a failed email can be retried from Activity.

## Acceptance

Demonstrate both stages with the checkbox checked, explicitly unchecked, and unavailable (no recipient or no SMTP). Compare the default email with a customized email: edits update the preview but nothing is queued until confirmation; the optional self-copy adds at most one blind copy. Verify that only checked submissions queue email, that the link and optional code can be copied separately in settings, and that a lost response does not duplicate jobs. With two recipients, make one succeed and the other exhaust retries: only the latter offers **Retry** in both Activity views; another workspace cannot see or trigger it. Expire the gallery with Activity open: the stale action must not enqueue anything. Verify the link serves the right stage before mail runs, existing readiness warnings still work and the bell stays unchanged. Inspect the compact and expanded dialog and actionable Activity row in French and English, on phone and desktop, including keyboard focus and status announcements.
