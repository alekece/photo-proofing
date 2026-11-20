<script lang="ts">
	import { goto } from '$app/navigation';
	import { m } from '#lib/paraglide/messages.js';
	import { onDestroy } from 'svelte';
	import { AlertCircle, Mail, Search, Users, X } from '@lucide/svelte';
	import CoverBackdrop from '#lib/components/CoverBackdrop.svelte';
	import StudioSignature from '#lib/components/StudioSignature.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import ConfirmDialog from '#lib/components/ui/ConfirmDialog/ConfirmDialog.svelte';
	import Dialog from '#lib/components/ui/Dialog/Dialog.svelte';
	import Input from '#lib/components/ui/Input/Input.svelte';
	import Label from '#lib/components/ui/Label/Label.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import { getLiveEvents } from '#lib/events.js';
	import { listContacts, type Contact } from '#lib/contacts/api.js';
	import { contactInitials, sortContacts } from '#lib/contacts/format.js';
	import {
		sendGalleryInvitations,
		type FocalPoint,
		type Gallery,
		type InvitationRecipientInput,
		type QueuedInvitations,
		type SendInvitationsInput
	} from '#lib/gallery/api.js';
	import { listGalleryContacts } from '#lib/gallery/contacts.js';
	import { formatLongDate } from '#lib/gallery/format.js';
	import { listPhotos, previewUrl, summarizePhotoReadiness } from '#lib/gallery/photos.js';
	import { DEFAULT_FOCAL_POINT } from '#lib/gallery/schemas.js';
	import { ApiError } from '#lib/http.js';
	import { DEFAULT_SETTINGS, type WorkspaceSettings } from '#lib/settings/settings.js';

	const EMAIL_RE = /^[^\s@<>]+@[^\s@<>]+\.[^\s@<>]+$/;
	const NAMED_EMAIL_RE = /^([^<>]+?)\s*<\s*([^\s@<>]+@[^\s@<>]+\.[^\s@<>]+)\s*>$/;

	let {
		gallery,
		settings = DEFAULT_SETTINGS,
		coverId = gallery.hero_id,
		heroRevision,
		focalPoint = gallery.focal_point ?? DEFAULT_FOCAL_POINT,
		photoCount = null,
		finalCount = null,
		uploading = 0,
		accessCode = gallery.access_code ?? null,
		contactsRevision = 0,
		active = true,
		loadLinkedContacts = listGalleryContacts,
		loadWorkspaceContacts = listContacts,
		loadPhotos = listPhotos,
		sendInvitations = sendGalleryInvitations,
		onbeforesend,
		onsent,
		oncancel,
		onpublish,
		onopensettings
	}: {
		gallery: Gallery;
		settings?: WorkspaceSettings;
		coverId?: string | null;
		heroRevision?: number;
		focalPoint?: FocalPoint;
		photoCount?: number | null;
		finalCount?: number | null;
		uploading?: number;
		accessCode?: string | null;
		contactsRevision?: number;
		active?: boolean;
		loadLinkedContacts?: typeof listGalleryContacts;
		loadWorkspaceContacts?: typeof listContacts;
		loadPhotos?: typeof listPhotos;
		sendInvitations?: typeof sendGalleryInvitations;
		onbeforesend?: () => Promise<boolean>;
		onsent?: (result: QueuedInvitations) => void;
		oncancel?: () => void;
		onpublish?: () => void;
		onopensettings?: () => void;
	} = $props();

	const id = $props.id();
	const pickerDialogId = `${id}-picker`;
	const toInputId = `${id}-to`;
	const bccInputId = `${id}-bcc`;
	const subjectInputId = `${id}-subject`;
	const messageInputId = `${id}-message`;

	const delivered = $derived(gallery.states.includes('delivered'));
	const expired = $derived(gallery.states.includes('expired'));
	const archived = $derived(gallery.states.includes('archived'));
	const emailEnabled = $derived(Boolean(settings.emailEnabled));
	const displayTitle = $derived(gallery.title?.trim() || m.gallery_untitled());
	const shared = $derived(gallery.share_token !== null);
	const canInvite = $derived(active && emailEnabled && !archived && !expired);
	const events = getLiveEvents();

	const defaultSubject = $derived(
		delivered
			? m.share_default_subject_finals({ title: displayTitle })
			: m.share_default_subject_proofing({ title: displayTitle })
	);
	const defaultMessage = $derived(
		delivered ? m.share_default_message_finals() : m.share_default_message_proofing()
	);

	let toRecipients = $state<InvitationRecipientInput[]>([]);
	let bccRecipients = $state<string[]>([]);
	let toDraft = $state('');
	let bccDraft = $state('');
	let bccOpen = $state(false);
	let toError = $state('');
	let bccError = $state('');
	let recipientsDirty = $state(false);

	// svelte-ignore state_referenced_locally
	let subject = $state<string>(defaultSubject);
	let subjectDirty = $state(false);
	// svelte-ignore state_referenced_locally
	let message = $state<string>(defaultMessage);
	let messageDirty = $state(false);
	// svelte-ignore state_referenced_locally
	let previousGalleryId = gallery.id;
	let galleryRevision = 0;
	let disposed = false;
	onDestroy(() => (disposed = true));
	// svelte-ignore state_referenced_locally
	let previousDelivered = delivered;

	let pickerOpen = $state(false);
	let pickerLoading = $state(false);
	let pickerQuery = $state('');
	let directoryContacts = $state<Contact[]>([]);
	type Warning = 'empty' | 'unfinished';
	type Invitation = {
		galleryId: string;
		revision: number;
		wasShared: boolean;
		wasDelivered: boolean;
		payload: SendInvitationsInput;
	};
	let readinessPrompt = $state.raw<{ invitation: Invitation; warning: Warning } | null>(null);
	let sending = $state(false);
	let readinessPhotos = $state<Awaited<ReturnType<typeof listPhotos>> | null>(null);
	let readinessRevision = $state(0);
	const proofWarning = $derived(
		active && !shared && readinessPhotos
			? readinessWarning(summarizePhotoReadiness(readinessPhotos, uploading))
			: null
	);

	function readinessWarning(readiness: ReturnType<typeof summarizePhotoReadiness>): Warning | null {
		return readiness.empty ? 'empty' : readiness.warned ? 'unfinished' : null;
	}

	$effect(() => {
		if (readinessPrompt && !eligible(readinessPrompt.invitation)) readinessPrompt = null;
	});

	$effect(() => {
		const galleryId = gallery.id;
		void readinessRevision;
		if (!active || shared) {
			readinessPhotos = null;
			return;
		}

		let cancelled = false;
		readinessPhotos = null;
		void loadPhotos(galleryId, fetch, 'photos')
			.then((photos) => {
				if (!cancelled) readinessPhotos = photos;
			})
			.catch(() => {
				// The fresh send precheck still runs; a failed advisory read says nothing about readiness.
			});
		return () => {
			cancelled = true;
		};
	});

	$effect(() => {
		if (!active || shared || !events) return;

		const galleryId = gallery.id;
		const refresh = () => readinessRevision++;
		const stopPhoto = events.onPhoto((event) => {
			if (event.gallery_id === galleryId && event.kind !== 'final') refresh();
		});
		const stopResync = events.onResync(refresh);
		return () => {
			stopPhoto();
			stopResync();
		};
	});

	$effect(() => {
		if (gallery.id !== previousGalleryId) {
			previousGalleryId = gallery.id;
			galleryRevision++;
			previousDelivered = delivered;
			recipientsDirty = false;
			toRecipients = [];
			subjectDirty = false;
			messageDirty = false;
			toDraft = '';
			bccDraft = '';
			bccRecipients = [];
			bccOpen = false;
			toError = '';
			bccError = '';
			subject = defaultSubject;
			message = defaultMessage;
			return;
		}

		if (delivered !== previousDelivered) {
			previousDelivered = delivered;
			if (!subjectDirty) subject = defaultSubject;
			if (!messageDirty) message = defaultMessage;
			return;
		}

		if (!subjectDirty && subject !== defaultSubject) {
			subject = defaultSubject;
		}
		if (!messageDirty && message !== defaultMessage) {
			message = defaultMessage;
		}
	});

	$effect(() => {
		const currentGalleryId = gallery.id;
		void contactsRevision;
		let cancelled = false;

		void loadLinkedContacts(currentGalleryId)
			.then((contacts) => {
				if (cancelled || recipientsDirty) return;

				toRecipients = contacts.map((contact) => ({
					email: contact.email.toLowerCase(),
					name: contact.name
				}));
			})
			.catch(() => {
				// Leave To editable manually even if preloading linked contacts fails.
			});

		return () => {
			cancelled = true;
		};
	});

	const toEmailSet = $derived(new Set(toRecipients.map((item) => item.email.toLowerCase())));
	const bccEmailSet = $derived(new Set(bccRecipients.map((email) => email.toLowerCase())));
	const effectiveRecipientCount = $derived.by(() => {
		const draftTo = parseRecipientEntry(toDraft);
		const draftBcc = parseRecipientEntry(bccDraft);

		return new Set<string>([
			...toEmailSet,
			...bccEmailSet,
			...(draftTo ? [draftTo.email] : []),
			...(draftBcc ? [draftBcc.email] : [])
		]).size;
	});

	const canSend = $derived(
		canInvite &&
			effectiveRecipientCount > 0 &&
			subject.trim().length > 0 &&
			message.trim().length > 0
	);

	const shootDateText = $derived(gallery.shoot_date ? formatLongDate(gallery.shoot_date) : null);
	const visibleCount = $derived(delivered ? finalCount : photoCount);
	const countText = $derived(
		visibleCount !== null ? m.summary_photos({ count: visibleCount }) : null
	);
	const metaLine = $derived([shootDateText, countText].filter(Boolean).join(' · '));
	const expiresText = $derived(gallery.expires_at ? formatLongDate(gallery.expires_at) : null);

	const availableContacts = $derived.by(() => {
		const query = pickerQuery.trim().toLowerCase();

		return sortContacts(directoryContacts).filter((contact) => {
			if (toEmailSet.has(contact.email.toLowerCase())) return false;
			if (!query) return true;

			return (
				contact.name.toLowerCase().includes(query) || contact.email.toLowerCase().includes(query)
			);
		});
	});

	function parseRecipientEntry(raw: string): InvitationRecipientInput | null {
		const trimmed = raw.trim();

		if (!trimmed) return null;

		const named = NAMED_EMAIL_RE.exec(trimmed);

		if (named) {
			const name = named[1]!.trim();
			const email = named[2]!.trim().toLowerCase();

			return { email, name: name || null };
		}

		if (EMAIL_RE.test(trimmed)) {
			return { email: trimmed.toLowerCase(), name: null };
		}

		return null;
	}

	function commitToDraft(): boolean {
		const raw = toDraft.trim();

		if (!raw) {
			toError = '';
			return true;
		}

		const parsed = parseRecipientEntry(raw);

		if (!parsed) {
			toError = m.share_invalid_email();
			return false;
		}

		toError = '';
		toDraft = '';
		recipientsDirty = true;

		if (!toEmailSet.has(parsed.email)) {
			toRecipients = [...toRecipients, parsed];
			bccRecipients = bccRecipients.filter((item) => item.toLowerCase() !== parsed.email);
		}

		return true;
	}

	function commitBccDraft(): boolean {
		const raw = bccDraft.trim();

		if (!raw) {
			bccError = '';
			return true;
		}

		const parsed = parseRecipientEntry(raw);

		if (!parsed) {
			bccError = m.share_invalid_email();
			return false;
		}

		bccError = '';
		bccDraft = '';
		recipientsDirty = true;

		if (!toEmailSet.has(parsed.email) && !bccEmailSet.has(parsed.email)) {
			bccRecipients = [...bccRecipients, parsed.email];
		}

		return true;
	}

	function removeTo(email: string) {
		recipientsDirty = true;
		toRecipients = toRecipients.filter((item) => item.email !== email);
	}

	function removeBcc(email: string) {
		recipientsDirty = true;
		bccRecipients = bccRecipients.filter((item) => item !== email);
	}

	function openContactsPicker() {
		pickerQuery = '';
		pickerOpen = true;
		pickerLoading = true;

		void loadWorkspaceContacts()
			.then((items) => {
				directoryContacts = items;
			})
			.catch(() => {
				toasts.error(m.contacts_load_failed());
			})
			.finally(() => {
				pickerLoading = false;
			});
	}

	function pickContact(contact: Contact) {
		const normalized = contact.email.toLowerCase();

		recipientsDirty = true;

		if (!toEmailSet.has(normalized)) {
			toRecipients = [...toRecipients, { email: normalized, name: contact.name }];
			bccRecipients = bccRecipients.filter((item) => item.toLowerCase() !== normalized);
		}

		if (availableContacts.length <= 1) {
			pickerOpen = false;
		}
	}

	function isCurrent(invitation: Invitation) {
		return (
			!disposed && invitation.galleryId === gallery.id && invitation.revision === galleryRevision
		);
	}

	function eligible(invitation: Invitation) {
		return (
			isCurrent(invitation) &&
			canInvite &&
			shared === invitation.wasShared &&
			delivered === invitation.wasDelivered
		);
	}

	async function executeSend(invitation: Invitation, fromConfirm: boolean): Promise<void> {
		try {
			const result = await sendInvitations(invitation.galleryId, invitation.payload);

			if (!isCurrent(invitation)) return;
			recipientsDirty = false;
			bccRecipients = [];
			bccDraft = '';
			onsent?.(result);
		} catch (err) {
			if (!isCurrent(invitation)) return;

			if (err instanceof ApiError && err.status === 401) {
				readinessPrompt = null;
				await goto('/login?expired=1', { refreshAll: true });
				return;
			}

			const detail = err instanceof ApiError ? err.message : m.share_send_failed();

			if (fromConfirm) {
				throw new Error(detail, { cause: err });
			}

			toasts.error(detail);
		}
	}

	export async function requestSend(): Promise<void> {
		if (disposed || sending || readinessPrompt || !canInvite) return;
		if (!commitToDraft() || !commitBccDraft()) return;
		if (toRecipients.length + bccRecipients.length === 0) return;

		const invitation: Invitation = {
			galleryId: gallery.id,
			revision: galleryRevision,
			wasShared: shared,
			wasDelivered: delivered,
			payload: {
				to: toRecipients.map((recipient) => ({ ...recipient })),
				bcc: [...bccRecipients],
				subject: subject.trim(),
				message: message.trim()
			}
		};
		if (!invitation.payload.subject) {
			toasts.error(m.share_subject_required());
			return;
		}
		if (!invitation.payload.message) {
			toasts.error(m.share_message_required());
			return;
		}

		sending = true;
		try {
			// Capture pending settings saves before the photo read can outlast their results.
			if ((onbeforesend && !(await onbeforesend())) || !eligible(invitation)) return;

			let warning: Warning | null = null;
			if (!invitation.wasShared) {
				try {
					const photos = await loadPhotos(invitation.galleryId, fetch, 'photos');
					warning = readinessWarning(summarizePhotoReadiness(photos, uploading));
				} catch {
					// A failed precheck does not prevent an explicit send.
				}
			}

			if (!eligible(invitation)) return;

			if (warning) {
				readinessPrompt = { invitation, warning };
			} else {
				if (!invitation.wasShared && onbeforesend && !(await onbeforesend())) return;
				if (eligible(invitation)) await executeSend(invitation, false);
			}
		} finally {
			sending = false;
		}
	}

	async function confirmSend(invitation: Invitation): Promise<void> {
		if (sending || !eligible(invitation)) throw new Error(m.share_send_failed());

		sending = true;
		try {
			if ((onbeforesend && !(await onbeforesend())) || !eligible(invitation)) {
				throw new Error(m.share_send_failed());
			}
			await executeSend(invitation, true);
		} finally {
			sending = false;
		}
	}
</script>

<div class="@container mx-auto grid w-full max-w-[72rem] gap-4">
	{#if shared && !delivered && !archived && onpublish}
		<div
			class="flex flex-wrap items-center justify-between gap-3 rounded-card border border-border bg-panel p-4 text-sm"
		>
			<p class="text-muted-foreground">{m.share_publish_hint()}</p>
			<Button size="sm" disabled={sending} onclick={onpublish}>{m.action_deliver()}</Button>
		</div>
	{/if}

	{#if !emailEnabled || expired}
		<div
			role="status"
			class="flex flex-wrap items-center justify-between gap-3 rounded-[0.875rem] border border-border bg-panel p-4 text-sm"
		>
			<div class="flex items-start gap-2.5">
				{#if expired}
					<AlertCircle size={18} aria-hidden="true" class="mt-0.5 shrink-0 text-danger" />
				{:else}
					<Mail size={18} aria-hidden="true" class="mt-0.5 shrink-0 text-muted-foreground" />
				{/if}
				<div class="grid gap-2">
					{#if !emailEnabled}
						<div>
							<p class="font-semibold text-foreground">{m.share_smtp_unavailable_title()}</p>
							<p class="text-muted-foreground">{m.share_smtp_unavailable_body()}</p>
						</div>
					{/if}
					{#if expired}
						<div>
							<p class="font-semibold text-foreground">{m.share_expired_title()}</p>
							<p class="text-muted-foreground">{m.share_expired_body()}</p>
						</div>
					{/if}
				</div>
			</div>
			{#if onopensettings}
				<Button
					variant="outline"
					size="sm"
					class="min-h-9! shrink-0 bg-white!"
					onclick={onopensettings}
				>
					{m.share_open_settings()}
				</Button>
			{/if}
		</div>
	{/if}
	{#if proofWarning}
		<p
			role="status"
			class="flex items-start gap-2 rounded-card border border-border bg-panel p-4 text-sm font-medium text-warning"
		>
			<AlertCircle size={18} aria-hidden="true" class="shrink-0" />
			{proofWarning === 'empty' ? m.share_proofs_empty() : m.share_proofs_unfinished()}
		</p>
	{/if}

	<div
		class="grid overflow-hidden rounded-dialog border border-border shadow-xs @min-[44rem]:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]"
	>
		<section
			aria-label={m.share_envelope_label()}
			class="grid min-w-0 content-start gap-4 bg-panel p-4 sm:p-5"
		>
			<h2 class="text-xl">{m.share_envelope_label()}</h2>
			<div class="grid gap-1.5">
				<div class="grid gap-2">
					<Label for={toInputId} class="text-sm font-semibold">
						{m.share_to_label()}
					</Label>
					<div
						class="flex min-w-0 flex-wrap items-center gap-1.5 rounded-control border border-border-strong bg-input p-1.5 focus-within:border-foreground"
					>
						{#each toRecipients as recipient (recipient.email)}
							<span
								class="inline-flex max-w-full items-center gap-1 rounded-full border border-border bg-white px-2.5 py-1 text-xs font-medium text-foreground"
							>
								<span class="truncate">
									{recipient.name ? `${recipient.name} <${recipient.email}>` : recipient.email}
								</span>
								<button
									type="button"
									disabled={!emailEnabled || sending}
									aria-label={m.share_remove_recipient({ email: recipient.email })}
									onclick={() => removeTo(recipient.email)}
									class="rounded-full p-0.5 text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-2 focus-visible:outline-foreground disabled:pointer-events-none"
								>
									<X size={13} aria-hidden="true" />
								</button>
							</span>
						{/each}
						<input
							id={toInputId}
							type="text"
							disabled={!emailEnabled || sending}
							placeholder={toRecipients.length === 0 ? m.share_to_placeholder() : ''}
							bind:value={toDraft}
							onkeydown={(event) => {
								if (event.key === 'Enter' || event.key === ',') {
									event.preventDefault();
									commitToDraft();
								} else if (
									event.key === 'Backspace' &&
									toDraft.length === 0 &&
									toRecipients.length > 0
								) {
									removeTo(toRecipients[toRecipients.length - 1]!.email);
								}
							}}
							onblur={() => {
								commitToDraft();
							}}
							class="min-w-24 flex-1 bg-transparent px-1.5 py-1 text-sm text-foreground outline-none placeholder:text-muted-foreground disabled:cursor-not-allowed disabled:opacity-50"
						/>
					</div>
					<div class="flex flex-wrap items-center gap-1.5">
						<Button
							variant="outline"
							size="sm"
							class="min-h-8! bg-white! px-2.5! text-xs"
							disabled={!emailEnabled || sending}
							onclick={openContactsPicker}
						>
							{m.share_add_contacts()}
						</Button>
						{#if !bccOpen}
							<Button
								variant="ghost"
								size="sm"
								class="min-h-8! px-2! text-xs text-muted-foreground"
								disabled={!emailEnabled || sending}
								onclick={() => (bccOpen = true)}
							>
								{m.share_add_bcc()}
							</Button>
						{/if}
					</div>
				</div>
				{#if toError}
					<p role="alert" class="text-xs text-danger">{toError}</p>
				{/if}
			</div>

			{#if bccOpen || bccRecipients.length > 0}
				<div class="grid gap-1.5">
					<div class="grid gap-2">
						<Label for={bccInputId} class="text-sm font-semibold">
							{m.share_bcc_label()}
						</Label>
						<div
							class="flex min-w-0 flex-wrap items-center gap-1.5 rounded-control border border-border-strong bg-input p-1.5 focus-within:border-foreground"
						>
							{#each bccRecipients as email (email)}
								<span
									class="inline-flex max-w-full items-center gap-1 rounded-full border border-border bg-white px-2.5 py-1 text-xs font-medium text-foreground"
								>
									<span class="truncate">{email}</span>
									<button
										type="button"
										disabled={!emailEnabled || sending}
										aria-label={m.share_remove_recipient({ email })}
										onclick={() => removeBcc(email)}
										class="rounded-full p-0.5 text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-2 focus-visible:outline-foreground disabled:pointer-events-none"
									>
										<X size={13} aria-hidden="true" />
									</button>
								</span>
							{/each}
							<input
								id={bccInputId}
								type="text"
								disabled={!emailEnabled || sending}
								placeholder={bccRecipients.length === 0 ? m.share_bcc_placeholder() : ''}
								bind:value={bccDraft}
								onkeydown={(event) => {
									if (event.key === 'Enter' || event.key === ',') {
										event.preventDefault();
										commitBccDraft();
									} else if (
										event.key === 'Backspace' &&
										bccDraft.length === 0 &&
										bccRecipients.length > 0
									) {
										removeBcc(bccRecipients[bccRecipients.length - 1]!);
									}
								}}
								onblur={() => {
									commitBccDraft();
								}}
								class="min-w-24 flex-1 bg-transparent px-1.5 py-1 text-sm text-foreground outline-none placeholder:text-muted-foreground disabled:cursor-not-allowed disabled:opacity-50"
							/>
						</div>
					</div>
					{#if bccError}
						<p role="alert" class="text-xs text-danger">{bccError}</p>
					{/if}
				</div>
			{/if}

			<div class="grid gap-2">
				<Label for={subjectInputId} class="text-sm font-semibold">
					{m.share_subject_label()}
				</Label>
				<Input
					id={subjectInputId}
					type="text"
					maxlength={200}
					disabled={!emailEnabled || sending}
					value={subject}
					oninput={(event) => {
						subjectDirty = true;
						subject = event.currentTarget.value;
					}}
					class="border-border-strong bg-input text-sm"
				/>
			</div>
			<div class="grid gap-2">
				<Label for={messageInputId} class="text-sm font-semibold">{m.share_message_label()}</Label>
				<textarea
					id={messageInputId}
					rows={5}
					maxlength={5000}
					disabled={!emailEnabled || sending}
					placeholder={m.share_message_placeholder()}
					value={message}
					oninput={(event) => {
						messageDirty = true;
						message = event.currentTarget.value;
					}}
					class="w-full min-w-0 resize-y rounded-control border border-border-strong bg-input px-3 py-2 text-sm leading-relaxed text-foreground placeholder:text-muted-foreground focus:border-foreground focus:outline-1 focus:outline-focus disabled:cursor-not-allowed disabled:opacity-50"
				></textarea>
			</div>
			<div class="flex flex-wrap justify-end gap-2 pt-1">
				{#if oncancel}
					<Button variant="outline" size="sm" disabled={sending} onclick={oncancel}>
						{m.common_cancel()}
					</Button>
				{/if}
				<Button size="sm" disabled={!canSend || sending} onclick={() => void requestSend()}>
					<Mail size={16} aria-hidden="true" class="mr-1.5" />
					{sending
						? m.share_sending()
						: effectiveRecipientCount > 0
							? m.share_send_invitation_count({ count: effectiveRecipientCount })
							: m.share_send_invitation()}
				</Button>
			</div>
		</section>

		<section
			aria-label={m.share_preview_label()}
			class="min-w-0 border-t border-border bg-input text-foreground @min-[44rem]:border-t-0 @min-[44rem]:border-l"
		>
			<div class="grid gap-2 border-b border-border bg-panel/60 px-4 py-3 sm:px-5">
				<h2 class="text-xl">{m.share_preview_label()}</h2>
				<p class="text-sm font-semibold wrap-anywhere whitespace-pre-wrap">{subject}</p>
			</div>
			<div class="px-6 py-4 text-center">
				<StudioSignature
					studio={settings.studioName}
					logo={settings.logo}
					signature={settings.signature}
					size={settings.signatureSize}
					stacked
				/>
			</div>

			{#if coverId}
				<div class="aspect-[3/2] w-full overflow-hidden bg-[#efe8dc]">
					<img
						src={previewUrl(gallery.id, coverId, 'photos', heroRevision)}
						alt={m.client_cover_alt()}
						style:object-position="{focalPoint.x}% {focalPoint.y}%"
						class="h-full w-full object-cover"
					/>
				</div>
			{:else}
				<div class="relative aspect-[3/2] w-full overflow-hidden bg-dark">
					<CoverBackdrop title={gallery.title} compact />
				</div>
			{/if}

			<div class="grid gap-4 p-5 sm:px-6">
				<div class="grid gap-1.5 text-center">
					<p
						class="text-[0.6875rem] font-semibold tracking-[0.16em] text-muted-foreground uppercase"
					>
						{delivered ? m.share_eyebrow_finals() : m.share_eyebrow_proofing()}
					</p>
					<h3 class="text-2xl leading-tight wrap-anywhere">{displayTitle}</h3>
					{#if metaLine}
						<p class="text-xs text-muted-foreground">{metaLine}</p>
					{/if}
				</div>

				<p class="text-sm leading-relaxed wrap-anywhere whitespace-pre-wrap">{message}</p>

				{#if accessCode}
					<div
						class="mx-auto grid w-full max-w-xs gap-1 rounded-xl border border-border bg-[#faf8f5] px-5 py-3.5 text-center"
					>
						<span
							class="text-[0.6875rem] font-semibold tracking-[0.14em] text-muted-foreground uppercase"
						>
							{m.share_access_code_heading()}
						</span>
						<span class="font-mono text-xl font-semibold tracking-[0.28em] text-foreground">
							{accessCode}
						</span>
					</div>
				{/if}

				<div class="grid justify-items-center gap-3 pt-1">
					<span
						aria-hidden="true"
						class="inline-flex items-center justify-center rounded-full bg-foreground px-7 py-3 text-sm font-semibold text-background select-none"
					>
						{delivered ? m.share_cta_finals() : m.share_cta_proofing()}
					</span>

					{#if expiresText}
						<p class="text-xs text-muted-foreground">
							{m.share_available_until({ date: expiresText })}
						</p>
					{/if}
				</div>
			</div>
		</section>
	</div>
</div>

{#if pickerOpen}
	<Dialog
		onclose={() => (pickerOpen = false)}
		labelledby="{pickerDialogId}-title"
		class="m-auto w-[min(28rem,calc(100vw-2rem))] rounded-dialog border border-border bg-panel p-6 text-foreground shadow-xl"
	>
		{#snippet children(close)}
			<div class="grid gap-4">
				<div class="flex items-center justify-between gap-2">
					<h2 id="{pickerDialogId}-title" class="text-xl">{m.share_picker_title()}</h2>
					<Button
						variant="ghost"
						size="sm"
						class="min-h-9! px-2.5!"
						aria-label={m.action_close()}
						onclick={close}
					>
						<X size={18} aria-hidden="true" />
					</Button>
				</div>

				<div class="relative">
					<Search
						size={16}
						aria-hidden="true"
						class="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-muted-foreground"
					/>
					<Input
						type="search"
						placeholder={m.contacts_search()}
						aria-label={m.contacts_search()}
						bind:value={pickerQuery}
						class="bg-white pl-9"
					/>
				</div>

				{#if pickerLoading}
					<p class="py-6 text-center text-sm text-muted-foreground">{m.dialog_working()}</p>
				{:else if availableContacts.length === 0}
					<div
						class="grid justify-items-center gap-2 py-6 text-center text-sm text-muted-foreground"
					>
						<Users size={22} aria-hidden="true" />
						<p>
							{pickerQuery.trim()
								? m.contacts_no_match({ query: pickerQuery.trim() })
								: m.share_picker_empty()}
						</p>
					</div>
				{:else}
					<div class="grid max-h-64 gap-1.5 overflow-y-auto pr-1">
						{#each availableContacts as contact (contact.id)}
							<button
								type="button"
								onclick={() => pickContact(contact)}
								class="flex w-full items-center gap-3 rounded-xl border border-border bg-white px-3 py-2.5 text-left transition-colors hover:border-foreground focus-visible:outline-2 focus-visible:outline-foreground"
							>
								<span
									aria-hidden="true"
									class="flex size-8 shrink-0 items-center justify-center rounded-full bg-[#efe8dc] text-xs font-semibold text-foreground"
								>
									{contactInitials(contact.name)}
								</span>
								<span class="min-w-0 flex-1">
									<span class="block truncate text-sm font-semibold text-foreground">
										{contact.name}
									</span>
									<span class="block truncate text-xs text-muted-foreground">{contact.email}</span>
								</span>
							</button>
						{/each}
					</div>
				{/if}
			</div>
		{/snippet}
	</Dialog>
{/if}

{#if readinessPrompt !== null}
	{@const { invitation, warning } = readinessPrompt}
	<ConfirmDialog
		title={warning === 'empty' ? m.share_warn_empty_title() : m.share_warn_unfinished_title()}
		confirmLabel={m.share_send_anyway()}
		onconfirm={() => confirmSend(invitation)}
		onclose={() => (readinessPrompt = null)}
	>
		{#snippet description()}
			<p class="text-sm text-muted-foreground">{m.share_warn_body()}</p>
		{/snippet}
	</ConfirmDialog>
{/if}
