<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Check, LoaderCircle, Plus, Search, UserMinus, Users, X } from '@lucide/svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import Dialog from '#lib/components/ui/Dialog/Dialog.svelte';
	import EmptyState from '#lib/components/ui/EmptyState/EmptyState.svelte';
	import Input from '#lib/components/ui/Input/Input.svelte';
	import Label from '#lib/components/ui/Label/Label.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import { createContact, listContacts, type Contact } from '#lib/contacts/api.js';
	import { contactInitials, sortContacts } from '#lib/contacts/format.js';
	import {
		linkGalleryContact,
		listGalleryContacts,
		unlinkGalleryContact
	} from '#lib/gallery/contacts.js';
	import { ApiError } from '#lib/http.js';

	let {
		galleryId,
		revision = 0,
		oncount
	}: {
		galleryId: string;
		revision?: number;
		oncount?: (count: number) => void;
	} = $props();

	const dialogId = $props.id();

	/** How long the row shows an animated "Linked" confirmation before settling to Unlink. */
	const LINKED_FEEDBACK_MS = 600;

	let linked = $state<Contact[]>([]);
	let loading = $state(true);
	let loadError = $state('');

	let dialogOpen = $state(false);
	let directoryLoading = $state(false);
	let directoryContacts = $state<Contact[]>([]);
	let dialogMode = $state<'existing' | 'create'>('existing');
	let pickerQuery = $state('');

	// Per-contact transient action state, keyed by contact id, so every row's Link/Linked/Unlink
	// button transitions independently without the dialog (or the always-visible list) closing.
	let pending = $state<Record<string, 'linking' | 'linked' | 'unlinking'>>({});
	// Per-contact request tokens: guard against an in-flight response landing after a newer
	// action (e.g. a quick unlink-then-relink) has already changed that same row's state.
	const requestTokens = new SvelteMap<string, number>();

	let newName = $state('');
	let newEmail = $state('');
	let newPhone = $state('');
	let newNotes = $state('');
	let inlineError = $state('');
	let creatingAndLinking = $state(false);

	function nextToken(contactId: string) {
		const token = (requestTokens.get(contactId) ?? 0) + 1;

		requestTokens.set(contactId, token);

		return token;
	}

	function isCurrentToken(contactId: string, token: number) {
		return requestTokens.get(contactId) === token;
	}

	function clearPending(contactId: string) {
		if (!(contactId in pending)) return;

		const rest = { ...pending };

		delete rest[contactId];
		pending = rest;
	}

	function isBusy(contactId: string) {
		return pending[contactId] === 'linking' || pending[contactId] === 'unlinking';
	}

	/** Applies a successful link response and starts the brief "Linked" confirmation. */
	function applyLinked(contactId: string, added: Contact, token: number) {
		const next = sortContacts([...linked.filter((item) => item.id !== added.id), added]);

		linked = next;
		oncount?.(next.length);
		pending = { ...pending, [contactId]: 'linked' };
		setTimeout(() => {
			if (!isCurrentToken(contactId, token)) return;

			clearPending(contactId);
		}, LINKED_FEEDBACK_MS);
	}

	$effect(() => {
		const currentGalleryId = galleryId;
		void revision;
		let cancelled = false;

		loading = true;
		loadError = '';

		void listGalleryContacts(currentGalleryId)
			.then((items) => {
				if (cancelled) return;

				linked = items;
				oncount?.(items.length);
			})
			.catch(() => {
				if (cancelled) return;

				loadError = m.gallery_contacts_load_failed();
			})
			.finally(() => {
				if (!cancelled) loading = false;
			});

		return () => {
			cancelled = true;
		};
	});

	const linkedIds = $derived(new Set(linked.map((item) => item.id)));
	// The dialog is a merged directory: every contact shows its own Link/Linked/Unlink state,
	// so linking or unlinking never removes a row from the list the user is looking at.
	const filteredDirectory = $derived.by(() => {
		const needle = pickerQuery.trim().toLowerCase();

		if (!needle) return directoryContacts;

		return directoryContacts.filter(
			(item) =>
				item.name.toLowerCase().includes(needle) ||
				item.email.toLowerCase().includes(needle) ||
				(item.phone?.toLowerCase().includes(needle) ?? false)
		);
	});

	async function openPicker() {
		dialogOpen = true;
		pickerQuery = '';
		newName = '';
		newEmail = '';
		newPhone = '';
		newNotes = '';
		inlineError = '';
		directoryLoading = true;

		try {
			const all = await listContacts();

			directoryContacts = all;
			dialogMode = all.length > 0 ? 'existing' : 'create';
		} catch {
			dialogMode = 'create';
		} finally {
			directoryLoading = false;
		}
	}

	async function linkRow(contactId: string) {
		if (isBusy(contactId)) return;

		const token = nextToken(contactId);

		pending = { ...pending, [contactId]: 'linking' };
		inlineError = '';

		try {
			const added = await linkGalleryContact(galleryId, contactId);

			if (!isCurrentToken(contactId, token)) return;

			applyLinked(contactId, added, token);
		} catch (error) {
			if (!isCurrentToken(contactId, token)) return;

			clearPending(contactId);
			inlineError = error instanceof ApiError ? error.message : m.gallery_contacts_link_failed();
		}
	}

	async function unlinkRow(contactId: string) {
		if (isBusy(contactId)) return;

		const token = nextToken(contactId);

		pending = { ...pending, [contactId]: 'unlinking' };

		try {
			await unlinkGalleryContact(galleryId, contactId);

			if (!isCurrentToken(contactId, token)) return;

			const next = linked.filter((item) => item.id !== contactId);

			linked = next;
			oncount?.(next.length);
			clearPending(contactId);
		} catch {
			if (!isCurrentToken(contactId, token)) return;

			clearPending(contactId);
			toasts.error(m.gallery_contacts_unlink_failed(), () => void unlinkRow(contactId));
		}
	}

	async function createInlineAndLink() {
		if (creatingAndLinking) return;

		const name = newName.trim();
		const email = newEmail.trim();
		const phone = newPhone.trim() || null;
		const notes = newNotes.trim() || null;

		if (!name) {
			inlineError = m.contacts_name_required();

			return;
		}

		if (!email || !email.includes('@')) {
			inlineError = m.contacts_email_invalid();

			return;
		}

		creatingAndLinking = true;
		inlineError = '';

		try {
			const created = await createContact({ name, email, phone, notes });

			directoryContacts = sortContacts([...directoryContacts, created]);
			pickerQuery = '';
			dialogMode = 'existing';

			await linkRow(created.id);
		} catch (error) {
			inlineError = error instanceof ApiError ? error.message : m.gallery_contacts_link_failed();
		} finally {
			creatingAndLinking = false;
		}
	}
</script>

<div class="grid gap-4">
	<div class="flex flex-wrap items-center justify-between gap-3">
		<h2 class="text-xl">{m.contacts_title()}</h2>
		<Button size="sm" onclick={() => void openPicker()}>
			{m.gallery_contacts_manage()}
		</Button>
	</div>

	{#if loadError}
		<div
			class="flex flex-wrap items-center justify-between gap-3 rounded-[0.875rem] border border-border bg-panel p-4"
		>
			<p role="alert" class="text-sm text-danger">{loadError}</p>
		</div>
	{:else if !loading && linked.length === 0}
		<EmptyState class="min-h-60 rounded-[0.875rem] border border-border bg-panel py-10 sm:min-h-64">
			<Users size={26} aria-hidden="true" class="text-muted-foreground" />
			<h3>{m.gallery_contacts_empty()}</h3>
			<p class="max-w-sm text-sm">{m.gallery_contacts_empty_body()}</p>
		</EmptyState>
	{:else if linked.length > 0}
		<ul class="divide-y divide-[#ece3d6] rounded-[0.875rem] border border-border bg-panel">
			{#each linked as item (item.id)}
				<li class="flex flex-wrap items-center justify-between gap-3 p-4">
					<div class="flex min-w-0 items-center gap-3">
						<span
							aria-hidden="true"
							class="grid size-9 shrink-0 place-items-center rounded-full bg-border-strong/55 text-xs font-semibold text-foreground"
						>
							{contactInitials(item.name)}
						</span>
						<div class="grid min-w-0 gap-0.5">
							<span class="break-anywhere text-sm font-semibold text-foreground">{item.name}</span>
							<span class="break-anywhere text-xs text-muted-foreground">
								{item.email}{#if item.phone}
									· {item.phone}{/if}
							</span>
						</div>
					</div>
					<Button
						variant="outline"
						size="sm"
						disabled={isBusy(item.id)}
						onclick={() => void unlinkRow(item.id)}
					>
						{#if pending[item.id] === 'unlinking'}
							<LoaderCircle
								size={14}
								class="animate-spin motion-reduce:animate-none"
								aria-hidden="true"
							/>
						{:else}
							<UserMinus size={14} aria-hidden="true" />
						{/if}
						{m.gallery_contacts_unlink()}
					</Button>
				</li>
			{/each}
		</ul>
	{/if}
</div>

{#if dialogOpen}
	<Dialog
		labelledby="{dialogId}-title"
		dismissible={!creatingAndLinking}
		onclose={() => (dialogOpen = false)}
		class="m-auto w-[min(29rem,calc(100vw-2rem))] rounded-dialog border border-border bg-panel p-5 text-foreground shadow-xl sm:p-6"
	>
		{#snippet children(close)}
			<div class="grid gap-4">
				<div class="flex items-center justify-between gap-2">
					<h2 id="{dialogId}-title" class="text-xl">{m.gallery_contacts_dialog_title()}</h2>
					<Button
						variant="ghost"
						size="sm"
						aria-label={m.action_close()}
						disabled={creatingAndLinking}
						onclick={close}
						class="min-h-8! px-2!"
					>
						<X size={18} aria-hidden="true" />
					</Button>
				</div>
				<a
					href="/contacts"
					class="w-fit rounded-sm text-sm font-medium underline underline-offset-2 hover:text-muted-foreground"
				>
					{m.gallery_contacts_open_directory()}
				</a>

				{#if directoryLoading}
					<div class="flex items-center justify-center py-8">
						<LoaderCircle
							size={20}
							class="animate-spin text-muted-foreground motion-reduce:animate-none"
							aria-hidden="true"
						/>
					</div>
				{:else if dialogMode === 'existing'}
					<div class="grid gap-3">
						{#if directoryContacts.length > 0}
							<div class="relative">
								<Search
									size={15}
									aria-hidden="true"
									class="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-muted-foreground"
								/>
								<Input
									type="search"
									bind:value={pickerQuery}
									aria-label={m.contacts_search()}
									placeholder={m.contacts_search()}
									class="rounded-full pl-8"
								/>
							</div>

							{#if filteredDirectory.length === 0}
								<p class="py-4 text-center text-sm text-muted-foreground">
									{m.contacts_no_match({ query: pickerQuery.trim() })}
								</p>
							{:else}
								<ul class="grid max-h-64 gap-1.5 overflow-y-auto pr-0.5">
									{#each filteredDirectory as item (item.id)}
										{@const rowState = pending[item.id]}
										{@const isLinked = linkedIds.has(item.id)}
										<li
											class="flex items-center justify-between gap-2.5 rounded-control border border-border bg-background p-2.5"
										>
											<div class="flex min-w-0 items-center gap-2.5">
												<span
													aria-hidden="true"
													class="grid size-8 shrink-0 place-items-center rounded-full bg-border-strong/55 text-xs font-semibold text-foreground"
												>
													{contactInitials(item.name)}
												</span>
												<div class="grid min-w-0">
													<span class="truncate text-sm font-medium text-foreground">
														{item.name}
													</span>
													<span class="truncate text-xs text-muted-foreground">{item.email}</span>
												</div>
											</div>
											<Button
												size="sm"
												variant={isLinked || rowState === 'linked' ? 'outline' : 'default'}
												disabled={isBusy(item.id)}
												onclick={() => void (isLinked ? unlinkRow(item.id) : linkRow(item.id))}
											>
												{#if rowState === 'linking'}
													<LoaderCircle
														size={14}
														class="animate-spin motion-reduce:animate-none"
														aria-hidden="true"
													/>
													{m.gallery_contacts_link_action()}
												{:else if rowState === 'linked'}
													<Check size={14} aria-hidden="true" />
													{m.gallery_contacts_linked_state()}
												{:else if rowState === 'unlinking'}
													<LoaderCircle
														size={14}
														class="animate-spin motion-reduce:animate-none"
														aria-hidden="true"
													/>
													{m.gallery_contacts_unlink()}
												{:else if isLinked}
													<UserMinus size={14} aria-hidden="true" />
													{m.gallery_contacts_unlink()}
												{:else}
													{m.gallery_contacts_link_action()}
												{/if}
											</Button>
										</li>
									{/each}
								</ul>
							{/if}
						{:else}
							<p class="py-3 text-sm text-muted-foreground">{m.gallery_contacts_all_linked()}</p>
						{/if}

						{#if inlineError}
							<p role="alert" class="text-sm text-danger">{inlineError}</p>
						{/if}

						<div
							class="flex flex-wrap items-center justify-between gap-2 border-t border-border pt-3"
						>
							<Button
								variant="outline"
								size="sm"
								onclick={() => {
									inlineError = '';
									dialogMode = 'create';
								}}
							>
								<Plus size={15} aria-hidden="true" />
								{m.gallery_contacts_new_inline()}
							</Button>
							<Button variant="ghost" size="sm" onclick={close}>{m.common_cancel()}</Button>
						</div>
					</div>
				{:else}
					<form
						class="grid gap-3.5"
						onsubmit={(event) => {
							event.preventDefault();
							void createInlineAndLink();
						}}
					>
						<div class="grid gap-1.5">
							<Label for="{dialogId}-name">{m.common_name()}</Label>
							<Input id="{dialogId}-name" bind:value={newName} maxlength={120} required />
						</div>

						<div class="grid gap-1.5">
							<Label for="{dialogId}-email">{m.login_email()}</Label>
							<Input
								id="{dialogId}-email"
								type="email"
								bind:value={newEmail}
								maxlength={254}
								required
							/>
						</div>

						<div class="grid gap-1.5">
							<Label for="{dialogId}-phone">{m.contacts_phone()}</Label>
							<Input id="{dialogId}-phone" type="tel" bind:value={newPhone} maxlength={40} />
						</div>

						<div class="grid gap-1.5">
							<Label for="{dialogId}-notes">{m.contacts_notes()}</Label>
							<textarea
								id="{dialogId}-notes"
								bind:value={newNotes}
								rows={2}
								maxlength={2000}
								class="cn-input min-h-18 w-full min-w-0 resize-y py-2 placeholder:text-muted-foreground focus-visible:border-foreground"
							></textarea>
						</div>

						{#if inlineError}
							<p role="alert" class="text-sm text-danger">{inlineError}</p>
						{/if}

						<div class="flex flex-wrap items-center justify-between gap-2 pt-1">
							{#if directoryContacts.length > 0}
								<Button
									variant="ghost"
									size="sm"
									disabled={creatingAndLinking}
									onclick={() => {
										inlineError = '';
										dialogMode = 'existing';
									}}
								>
									{m.gallery_contacts_choose_existing()}
								</Button>
							{:else}
								<span></span>
							{/if}
							<div class="flex items-center gap-2">
								<Button variant="outline" onclick={close} disabled={creatingAndLinking}>
									{m.common_cancel()}
								</Button>
								<Button type="submit" disabled={creatingAndLinking}>
									{#if creatingAndLinking}
										<LoaderCircle
											size={15}
											class="animate-spin motion-reduce:animate-none"
											aria-hidden="true"
										/>
									{/if}
									{m.gallery_contacts_create_and_link()}
								</Button>
							</div>
						</div>
					</form>
				{/if}
			</div>
		{/snippet}
	</Dialog>
{/if}
