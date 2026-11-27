<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Plus, Search, Users, X } from '@lucide/svelte';
	import Checkbox from '#lib/components/ui/Checkbox/Checkbox.svelte';
	import type { Gallery, InvitationRecipientInput } from '#lib/gallery/api.js';
	import type { InvitationDraft } from '#lib/gallery/types.js';
	import { previewUrl } from '#lib/gallery/photos.js';
	import CoverBackdrop from '#lib/components/CoverBackdrop.svelte';
	import StudioSignature from '#lib/components/StudioSignature.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import Dialog from '#lib/components/ui/Dialog/Dialog.svelte';
	import Input from '#lib/components/ui/Input/Input.svelte';
	import Label from '#lib/components/ui/Label/Label.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import { listContacts, type Contact } from '#lib/contacts/api.js';
	import { contactInitials, sortContacts } from '#lib/contacts/format.js';
	import { listGalleryContacts } from '#lib/gallery/contacts.js';
	import { defaultInvitationSubject, formatLongDate } from '#lib/gallery/format.js';
	import { DEFAULT_SETTINGS, type WorkspaceSettings } from '#lib/settings/settings.js';

	let {
		gallery,
		stage,
		settings = DEFAULT_SETTINGS,
		heroRevision,
		visibleCount = null,
		sending = false,
		expanded = $bindable(false),
		blocked = $bindable(false),
		loadLinkedContacts = listGalleryContacts,
		loadWorkspaceContacts = listContacts
	}: {
		gallery: Gallery;
		stage: 'proofing' | 'finals';
		settings?: WorkspaceSettings;
		heroRevision?: number;
		visibleCount?: number | null;
		sending?: boolean;
		expanded?: boolean;
		blocked?: boolean;
		loadLinkedContacts?: typeof listGalleryContacts;
		loadWorkspaceContacts?: typeof listContacts;
	} = $props();
	const delivered = $derived(stage === 'finals');
	const coverId = $derived(gallery.hero_id);
	const focalPoint = $derived(gallery.focal_point);
	const accessCode = $derived(gallery.access_code);
	const emailEnabled = $derived(!!settings.emailEnabled && !gallery.states.includes('expired'));
	let contactsState = $state<'loading' | 'ready' | 'failed'>('loading');
	let lookup = $state(0);
	let optedOut = $state(false);
	let copySelf = $state(false);
	let pickerOpen = $state(false);
	let pickerLoading = $state(false);
	let pickerQuery = $state('');
	let directoryContacts = $state<Contact[]>([]);
	const id = $props.id();
	const pickerDialogId = `${id}-picker`;
	const subjectInputId = `${id}-subject`;
	const messageInputId = `${id}-message`;

	const displayTitle = $derived(gallery.title?.trim() || m.gallery_untitled());

	const defaultMessage = $derived(
		delivered ? m.share_default_message_finals() : m.share_default_message_proofing()
	);

	let toRecipients = $state<InvitationRecipientInput[]>([]);

	let customSubject = $state<string>();
	const subject = $derived(customSubject ?? defaultInvitationSubject(stage, gallery.title));
	// svelte-ignore state_referenced_locally
	let message = $state<string>(defaultMessage);
	const galleryId = $derived(gallery.id);
	$effect(() => {
		void lookup;
		if (!emailEnabled) return;
		contactsState = 'loading';
		let cancelled = false;

		void loadLinkedContacts(galleryId)
			.then((contacts) => {
				if (cancelled) return;
				contactsState = 'ready';
				toRecipients = contacts.map((contact) => ({
					link_id: contact.link_id,
					email: contact.email.toLowerCase(),
					name: contact.name
				}));
			})
			.catch(() => {
				if (!cancelled) contactsState = 'failed';
			});

		return () => {
			cancelled = true;
		};
	});

	const toEmailSet = $derived(new Set(toRecipients.map((item) => item.email.toLowerCase())));
	const effectiveRecipientCount = $derived(toRecipients.length);

	// Customizing stays open when the last recipient is removed; Share is held back until one is added.
	const available = $derived(
		emailEnabled && contactsState === 'ready' && (effectiveRecipientCount > 0 || expanded)
	);
	const checked = $derived(available && !optedOut);
	const editing = $derived(expanded && checked);
	/** Who the invitation goes to, for a summary line outside the customization form. */
	export function recipientNames() {
		return checked ? toRecipients.map((r) => r.name || r.email).join(', ') : '';
	}
	export function canEmail() {
		return available;
	}
	export function canCustomize() {
		return checked && !sending;
	}
	export async function toggleCustomization() {
		if (!canCustomize()) return;

		expanded = !editing;
	}
	const shootDateText = $derived(gallery.shoot_date ? formatLongDate(gallery.shoot_date) : null);
	$effect(() => {
		blocked =
			emailEnabled &&
			!optedOut &&
			(contactsState !== 'ready' || (expanded && toRecipients.length === 0));
		if (!checked) expanded = false;
	});

	export function draft(): InvitationDraft | undefined {
		if (blocked) throw new Error(m.contacts_load_failed());
		if (!checked) return;
		if (!subject.trim()) throw new Error(m.share_subject_required());
		if (!message.trim()) throw new Error(m.share_message_required());
		return {
			stage,
			copy_self: copySelf,
			to: [...toRecipients],
			bcc: [],
			subject: customSubject?.trim(),
			message: message.trim()
		};
	}
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

	function removeTo(email: string) {
		toRecipients = toRecipients.filter((item) => item.email !== email);
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

		if (!toEmailSet.has(normalized)) {
			toRecipients = [...toRecipients, { email: normalized, name: contact.name }];
		}

		if (availableContacts.length <= 1) {
			pickerOpen = false;
		}
	}
</script>

<div class="@container mx-auto grid w-full max-w-[72rem] gap-4">
	{#if emailEnabled && contactsState === 'loading'}
		<p role="status">{m.dialog_working()}</p>
	{:else if emailEnabled && contactsState === 'failed'}
		<p role="status">{m.contacts_load_failed()}</p>
		<div class="flex flex-wrap gap-2">
			<Button size="sm" variant="outline" onclick={() => lookup++}>{m.action_retry()}</Button>
			{#if !optedOut}<Button size="sm" variant="ghost" onclick={() => (optedOut = true)}>
					{m.stage_without_email()}
				</Button>{/if}
		</div>
	{/if}

	{#if editing}
		<div
			class="grid overflow-hidden rounded-dialog border border-border shadow-xs @min-[44rem]:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]"
		>
			<section
				aria-label={m.share_envelope_label()}
				class="grid min-w-0 content-start gap-4 bg-panel p-4 sm:p-5"
			>
				<div class="grid gap-2">
					<span class="text-sm font-semibold">{m.share_to_label()}</span>
					<div
						class="flex min-h-11 min-w-0 flex-wrap items-center gap-1.5 rounded-control border border-border-strong bg-input p-1.5"
					>
						{#each toRecipients as recipient (recipient.email)}
							<span
								class="inline-flex max-w-full items-center gap-1 pill border border-border bg-white text-xs font-medium text-foreground"
							>
								<span class="truncate">{recipient.name || recipient.email}</span>
								<button
									type="button"
									disabled={!emailEnabled || sending}
									aria-label={m.share_remove_recipient({ email: recipient.email })}
									onclick={() => removeTo(recipient.email)}
									class="relative rounded-full p-0.5 text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-2 focus-visible:outline-foreground disabled:pointer-events-none sm:icon-action"
								>
									<X size={13} aria-hidden="true" />
								</button>
							</span>
						{/each}
						{#if toRecipients.length === 0}
							<button
								type="button"
								disabled={!emailEnabled || sending}
								onclick={openContactsPicker}
								class="min-w-0 flex-1 cursor-pointer rounded-control px-1.5 py-1 text-left text-sm text-muted-foreground hover:text-foreground focus-visible:outline-2 focus-visible:outline-foreground disabled:pointer-events-none"
							>
								{m.share_to_empty()}
							</button>
						{/if}
						<button
							type="button"
							disabled={!emailEnabled || sending}
							aria-label={m.share_add_contacts()}
							title={m.share_add_contacts()}
							onclick={openContactsPicker}
							class="icon-action relative ml-auto inline-flex size-6 items-center justify-center rounded-full text-muted-foreground transition-colors [--icon-hover-size:100%] hover:text-foreground focus-visible:outline-2 focus-visible:outline-foreground disabled:pointer-events-none disabled:opacity-50"
						>
							<Plus size={14} aria-hidden="true" />
						</button>
					</div>
				</div>

				{#if editing}
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
								customSubject = event.currentTarget.value;
							}}
							class="border-border-strong bg-input text-sm"
						/>
					</div>
					<div class="grid gap-2">
						<Label for={messageInputId} class="text-sm font-semibold">
							{m.share_message_label()}
						</Label>
						<textarea
							id={messageInputId}
							rows={5}
							maxlength={5000}
							disabled={!emailEnabled || sending}
							placeholder={m.share_message_placeholder()}
							value={message}
							oninput={(event) => {
								message = event.currentTarget.value;
							}}
							class="w-full min-w-0 resize-y rounded-control border border-border-strong bg-input px-3 py-2 text-sm leading-relaxed text-foreground placeholder:text-muted-foreground focus:border-foreground focus:outline-1 focus:outline-focus disabled:cursor-not-allowed disabled:opacity-50"
						></textarea>
					</div>
					<label class="flex items-center gap-2 text-sm">
						<Checkbox bind:checked={copySelf} disabled={sending} />{m.stage_copy_self()}
					</label>
				{/if}
			</section>

			{#if editing}
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
							textColor={settings.signatureTextColor}
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
			{/if}
		</div>
	{/if}
</div>

{#if pickerOpen}
	<Dialog
		onclose={() => (pickerOpen = false)}
		focusClose
		labelledby="{pickerDialogId}-title"
		class="m-auto w-[min(28rem,calc(100vw-2rem))] rounded-dialog border border-border bg-panel p-6 text-foreground shadow-xl"
	>
		<div class="grid gap-4">
			<h2 id="{pickerDialogId}-title" class="pr-10 text-xl">{m.share_picker_title()}</h2>

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
				<div class="grid justify-items-center gap-2 py-6 text-center text-sm text-muted-foreground">
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
	</Dialog>
{/if}
