<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { AlertCircle, Mail, Search, Users, X } from '@lucide/svelte';
	import CoverBackdrop from '#lib/components/CoverBackdrop.svelte';
	import StudioSignature from '#lib/components/StudioSignature.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import ConfirmDialog from '#lib/components/ui/ConfirmDialog/ConfirmDialog.svelte';
	import Dialog from '#lib/components/ui/Dialog/Dialog.svelte';
	import Input from '#lib/components/ui/Input/Input.svelte';
	import Label from '#lib/components/ui/Label/Label.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import {
		contactInitials,
		listContacts,
		listGalleryContacts,
		sortContacts,
		type Contact
	} from '#lib/contacts/api.js';
	import {
		GalleryApiError,
		sendGalleryInvitations,
		type FocalPoint,
		type Gallery,
		type InvitationRecipientInput,
		type QueuedInvitations
	} from '#lib/gallery/api.js';
	import { formatLongDate } from '#lib/gallery/format.js';
	import { listPhotos, previewUrl } from '#lib/gallery/photos.js';
	import { DEFAULT_FOCAL_POINT } from '#lib/gallery/schemas.js';
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
		recipientCount = $bindable(0),
		canSend = $bindable(false),
		sending = $bindable(false),
		loadLinkedContacts = listGalleryContacts,
		loadWorkspaceContacts = listContacts,
		loadPhotos = listPhotos,
		sendInvitations = sendGalleryInvitations,
		onbeforesend,
		onsent,
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
		recipientCount?: number;
		canSend?: boolean;
		sending?: boolean;
		loadLinkedContacts?: typeof listGalleryContacts;
		loadWorkspaceContacts?: typeof listContacts;
		loadPhotos?: typeof listPhotos;
		sendInvitations?: typeof sendGalleryInvitations;
		onbeforesend?: () => Promise<boolean>;
		onsent?: (result: QueuedInvitations) => void;
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
	let subject = $state(defaultSubject);
	let subjectDirty = $state(false);
	// svelte-ignore state_referenced_locally
	let message = $state(defaultMessage);
	let messageDirty = $state(false);
	// svelte-ignore state_referenced_locally
	let previousGalleryId = gallery.id;
	// svelte-ignore state_referenced_locally
	let previousDelivered = delivered;

	let messageEl = $state<HTMLTextAreaElement | null>(null);
	let pickerOpen = $state(false);
	let pickerLoading = $state(false);
	let pickerQuery = $state('');
	let directoryContacts = $state<Contact[]>([]);
	let readinessPrompt = $state<'empty' | 'unfinished' | null>(null);

	$effect(() => {
		if (gallery.id !== previousGalleryId) {
			previousGalleryId = gallery.id;
			previousDelivered = delivered;
			recipientsDirty = false;
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

	$effect(() => {
		const total = toRecipients.length + bccRecipients.length;

		recipientCount = total;
		canSend =
			emailEnabled &&
			!archived &&
			!expired &&
			total > 0 &&
			subject.trim().length > 0 &&
			message.trim().length > 0;
	});

	$effect(() => {
		void message;

		if (!messageEl) return;

		messageEl.style.height = 'auto';
		messageEl.style.height = `${Math.max(messageEl.scrollHeight, 96)}px`;
	});

	const shootDateText = $derived(
		gallery.shoot_date ? formatLongDate(`${gallery.shoot_date}T12:00`) : null
	);
	const visibleCount = $derived(delivered ? finalCount : photoCount);
	const countText = $derived(
		visibleCount !== null ? m.summary_photos({ count: visibleCount }) : null
	);
	const metaLine = $derived([shootDateText, countText].filter(Boolean).join(' · '));
	const expiresText = $derived(gallery.expires_at ? formatLongDate(gallery.expires_at) : null);

	const toEmailSet = $derived(new Set(toRecipients.map((item) => item.email.toLowerCase())));
	const bccEmailSet = $derived(new Set(bccRecipients.map((email) => email.toLowerCase())));
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

	async function executeSend(): Promise<void> {
		const trimmedSubject = subject.trim();
		const trimmedMessage = message.trim();

		if (!trimmedSubject) {
			toasts.error(m.share_subject_required());
			return;
		}
		if (!trimmedMessage) {
			toasts.error(m.share_message_required());
			return;
		}

		sending = true;
		try {
			const result = await sendInvitations(gallery.id, {
				to: toRecipients,
				bcc: bccRecipients,
				subject: trimmedSubject,
				message: trimmedMessage
			});

			bccRecipients = [];
			bccDraft = '';
			onsent?.(result);
		} catch (err) {
			const detail = err instanceof GalleryApiError ? err.message : m.share_send_failed();

			toasts.error(detail);
			throw err;
		} finally {
			sending = false;
		}
	}

	export async function requestSend(): Promise<void> {
		if (sending) return;
		if (!commitToDraft() || !commitBccDraft()) return;
		if (!emailEnabled || archived || expired) return;
		if (toRecipients.length + bccRecipients.length === 0) return;

		if (onbeforesend && !(await onbeforesend())) {
			return;
		}

		if (gallery.share_token === null) {
			try {
				const photos = await loadPhotos(gallery.id, fetch, 'photos');
				const readyCount = photos.filter((photo) => photo.status === 'ready').length;
				const inFlightCount =
					uploading +
					photos.filter((photo) => photo.status === 'waiting' || photo.status === 'processing')
						.length;
				const failedCount = photos.filter((photo) => typeof photo.status === 'object').length;

				if (readyCount === 0) {
					readinessPrompt = 'empty';
					return;
				}
				if (inFlightCount > 0 || failedCount > 0) {
					readinessPrompt = 'unfinished';
					return;
				}
			} catch {
				// Proceed with sending if the client-side photo precheck fails.
			}
		}

		try {
			await executeSend();
		} catch {
			// Error toast already reported in executeSend.
		}
	}
</script>

<div class="mx-auto grid w-full max-w-[41.25rem] gap-4">
	{#if !emailEnabled}
		<div
			role="status"
			class="flex flex-wrap items-center justify-between gap-3 rounded-[0.875rem] border border-border bg-panel p-4 text-sm"
		>
			<div class="flex items-start gap-2.5">
				<Mail size={18} aria-hidden="true" class="mt-0.5 shrink-0 text-muted-foreground" />
				<div class="grid gap-0.5">
					<p class="font-semibold text-foreground">{m.share_smtp_unavailable_title()}</p>
					<p class="text-muted-foreground">{m.share_smtp_unavailable_body()}</p>
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
	{:else if expired}
		<div
			role="status"
			class="flex flex-wrap items-center justify-between gap-3 rounded-[0.875rem] border border-border bg-panel p-4 text-sm"
		>
			<div class="flex items-start gap-2.5">
				<AlertCircle size={18} aria-hidden="true" class="mt-0.5 shrink-0 text-danger" />
				<div class="grid gap-0.5">
					<p class="font-semibold text-foreground">{m.share_expired_title()}</p>
					<p class="text-muted-foreground">{m.share_expired_body()}</p>
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

	<div class="overflow-hidden rounded-[0.875rem] border border-border shadow-xs">
		<!-- Sheet 1: Envelope -->
		<section aria-label={m.share_envelope_label()} class="bg-panel px-4 py-3.5 sm:px-6 sm:py-4">
			<!-- To row -->
			<div class="grid gap-1.5 border-b border-[#ece3d6] pb-3">
				<div class="flex flex-wrap items-center gap-2">
					<Label
						for={toInputId}
						class="w-14 shrink-0 text-xs font-semibold tracking-[0.06em] text-muted-foreground uppercase"
					>
						{m.share_to_label()}
					</Label>
					<div class="flex min-w-0 flex-1 flex-wrap items-center gap-1.5">
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
							class="min-w-40 flex-1 bg-transparent py-1 text-sm text-foreground outline-none placeholder:text-muted-foreground/55 disabled:cursor-not-allowed disabled:opacity-50"
						/>
					</div>
					<div class="ml-auto flex shrink-0 items-center gap-1.5">
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
					<p role="alert" class="pl-16 text-xs text-danger">{toError}</p>
				{/if}
			</div>

			<!-- Optional BCC row -->
			{#if bccOpen || bccRecipients.length > 0}
				<div class="grid gap-1.5 border-b border-[#ece3d6] py-3">
					<div class="flex flex-wrap items-center gap-2">
						<Label
							for={bccInputId}
							class="w-14 shrink-0 text-xs font-semibold tracking-[0.06em] text-muted-foreground uppercase"
						>
							{m.share_bcc_label()}
						</Label>
						<div class="flex min-w-0 flex-1 flex-wrap items-center gap-1.5">
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
								class="min-w-40 flex-1 bg-transparent py-1 text-sm text-foreground outline-none placeholder:text-muted-foreground/55 disabled:cursor-not-allowed disabled:opacity-50"
							/>
						</div>
					</div>
					{#if bccError}
						<p role="alert" class="pl-16 text-xs text-danger">{bccError}</p>
					{/if}
				</div>
			{/if}

			<!-- Subject row -->
			<div class="flex items-center gap-2 pt-3">
				<Label
					for={subjectInputId}
					class="w-14 shrink-0 text-xs font-semibold tracking-[0.06em] text-muted-foreground uppercase"
				>
					{m.share_subject_label()}
				</Label>
				<input
					id={subjectInputId}
					type="text"
					maxlength={200}
					disabled={!emailEnabled || sending}
					bind:value={subject}
					oninput={() => {
						subjectDirty = true;
					}}
					class="min-w-0 flex-1 bg-transparent py-1 text-sm font-medium text-foreground outline-none placeholder:text-muted-foreground/55 disabled:cursor-not-allowed disabled:opacity-50"
				/>
			</div>
		</section>

		<!-- Sheet 2: Live Warm Paper Email Preview (flush below Envelope) -->
		<section
			aria-label={m.share_preview_label()}
			class="border-t border-border bg-white text-foreground"
		>
			<div class="px-6 pt-7 pb-5 text-center sm:px-10">
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

			<div class="grid gap-5 px-6 py-8 sm:px-10">
				<div class="grid gap-1.5 text-center">
					<p
						class="text-[0.6875rem] font-semibold tracking-[0.16em] text-muted-foreground uppercase"
					>
						{delivered ? m.share_eyebrow_finals() : m.share_eyebrow_proofing()}
					</p>
					<h3 class="text-2xl leading-tight sm:text-[1.75rem]">{displayTitle}</h3>
					{#if metaLine}
						<p class="text-xs text-muted-foreground">{metaLine}</p>
					{/if}
				</div>

				<div>
					<label for={messageInputId} class="sr-only">{m.share_message_label()}</label>
					<textarea
						id={messageInputId}
						bind:this={messageEl}
						rows={4}
						maxlength={5000}
						disabled={!emailEnabled || sending}
						placeholder={m.share_message_placeholder()}
						bind:value={message}
						oninput={() => {
							messageDirty = true;
						}}
						class="w-full resize-none overflow-hidden rounded-lg border border-transparent bg-transparent px-2.5 py-2 text-[0.9375rem] leading-relaxed text-foreground transition-colors outline-none hover:border-border focus:border-foreground focus:bg-[#faf8f5]/60 disabled:cursor-not-allowed disabled:opacity-70"
					></textarea>
				</div>

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
	<ConfirmDialog
		title={readinessPrompt === 'empty'
			? m.share_warn_empty_title()
			: m.share_warn_unfinished_title()}
		confirmLabel={m.share_send_anyway()}
		onconfirm={executeSend}
		onclose={() => (readinessPrompt = null)}
	>
		{#snippet description()}
			<p class="text-sm text-muted-foreground">{m.share_warn_body()}</p>
		{/snippet}
	</ConfirmDialog>
{/if}
