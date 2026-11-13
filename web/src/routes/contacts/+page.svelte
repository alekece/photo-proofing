<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onMount } from 'svelte';
	import { afterNavigate, goto } from '$app/navigation';
	import { ArrowLeft, LoaderCircle, Plus, Search, Trash2, Users } from '@lucide/svelte';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import { authRequest } from '#lib/auth.js';
	import AppHeader from '#lib/components/AppHeader.svelte';
	import SaveButton, { type SaveStatus } from '#lib/components/SaveButton.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import ConfirmDialog from '#lib/components/ui/ConfirmDialog/ConfirmDialog.svelte';
	import EmptyState from '#lib/components/ui/EmptyState/EmptyState.svelte';
	import Input from '#lib/components/ui/Input/Input.svelte';
	import Label from '#lib/components/ui/Label/Label.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import {
		ContactApiError,
		contactInitials,
		createContact,
		deleteContact,
		sortContacts,
		updateContact,
		type Contact
	} from '#lib/contacts/api.js';
	import type { PageData } from './$types';

	const SAVED_MS = 1600;

	let { data }: { data: PageData } = $props();

	const formId = $props.id();

	// svelte-ignore state_referenced_locally
	let contacts = $state<Contact[]>(data.contacts ? [...data.contacts] : []);
	// svelte-ignore state_referenced_locally
	let selectedId = $state<string | null>(data.contacts?.[0]?.id ?? null);
	let creating = $state(false);
	let mobileDetailOpen = $state(false);
	let query = $state('');
	let signingOut = $state(false);
	let signOutError = $state('');

	let draftName = $state('');
	let draftEmail = $state('');
	let draftPhone = $state('');
	let draftNotes = $state('');
	let formError = $state('');
	let saveStatus = $state<SaveStatus>('idle');
	let creatingBusy = $state(false);
	let confirmDeleteOpen = $state(false);
	let savedTimer: ReturnType<typeof setTimeout> | undefined;

	const loadFailed = $derived(data.contacts === null);
	const selectedContact = $derived(
		creating ? null : (contacts.find((item) => item.id === selectedId) ?? null)
	);

	// Sync draft fields whenever the active mode or selected contact changes.
	$effect(() => {
		if (creating) {
			return;
		}

		const current = selectedContact;

		if (!current) {
			draftName = '';
			draftEmail = '';
			draftPhone = '';
			draftNotes = '';

			return;
		}

		draftName = current.name;
		draftEmail = current.email;
		draftPhone = current.phone ?? '';
		draftNotes = current.notes ?? '';
		formError = '';
	});

	const matching = $derived.by(() => {
		const needle = query.trim().toLowerCase();

		if (!needle) return contacts;

		return contacts.filter(
			(item) =>
				item.name.toLowerCase().includes(needle) ||
				item.email.toLowerCase().includes(needle) ||
				(item.phone?.toLowerCase().includes(needle) ?? false)
		);
	});

	const dirty = $derived.by(() => {
		if (!selectedContact) return false;

		return (
			draftName.trim() !== selectedContact.name ||
			draftEmail.trim().toLowerCase() !== selectedContact.email ||
			draftPhone.trim() !== (selectedContact.phone ?? '') ||
			draftNotes.trim() !== (selectedContact.notes ?? '')
		);
	});

	const shownSaveStatus = $derived<SaveStatus>(
		saveStatus === 'saved' && dirty ? 'idle' : saveStatus
	);

	const activity = $derived(createSessionActivity(data.idleTimeoutMs));

	afterNavigate(() => {
		void activity();
	});
	onMount(() => {
		const stop = listenForActivity(() => activity());

		return () => {
			stop();
			clearTimeout(savedTimer);
		};
	});

	function selectContact(id: string) {
		creating = false;
		selectedId = id;
		mobileDetailOpen = true;
		formError = '';
		clearTimeout(savedTimer);
		saveStatus = 'idle';
	}

	function startNewContact() {
		creating = true;
		mobileDetailOpen = true;
		draftName = '';
		draftEmail = '';
		draftPhone = '';
		draftNotes = '';
		formError = '';
		clearTimeout(savedTimer);
		saveStatus = 'idle';
	}

	function cancelCreate() {
		creating = false;
		formError = '';

		if (!selectedId && contacts.length > 0) {
			selectedId = contacts[0].id;
		}

		if (contacts.length === 0) {
			mobileDetailOpen = false;
		}
	}

	function revertDraft() {
		if (!selectedContact) return;

		draftName = selectedContact.name;
		draftEmail = selectedContact.email;
		draftPhone = selectedContact.phone ?? '';
		draftNotes = selectedContact.notes ?? '';
		formError = '';
	}

	function validateFields(): {
		name: string;
		email: string;
		phone: string | null;
		notes: string | null;
	} | null {
		const name = draftName.trim();
		const email = draftEmail.trim();
		const phone = draftPhone.trim() || null;
		const notes = draftNotes.trim() || null;

		if (!name) {
			formError = m.contacts_name_required();

			return null;
		}

		if (!email || !email.includes('@')) {
			formError = m.contacts_email_invalid();

			return null;
		}

		formError = '';

		return { name, email, phone, notes };
	}

	async function submitCreate() {
		if (creatingBusy) return;

		const parsed = validateFields();

		if (!parsed) return;

		creatingBusy = true;

		try {
			const created = await createContact(parsed);

			contacts = sortContacts([...contacts, created]);
			creating = false;
			selectedId = created.id;
			mobileDetailOpen = true;
		} catch (error) {
			if (error instanceof ContactApiError && (error.status === 409 || error.status === 422)) {
				formError = error.message;
			} else {
				toasts.error(m.contacts_save_failed(), () => void submitCreate());
			}
		} finally {
			creatingBusy = false;
		}
	}

	async function submitSave() {
		if (!selectedContact || !dirty || shownSaveStatus !== 'idle') return;

		const parsed = validateFields();

		if (!parsed) return;

		const targetId = selectedContact.id;

		saveStatus = 'saving';
		clearTimeout(savedTimer);

		try {
			const updated = await updateContact(targetId, parsed);

			contacts = sortContacts(contacts.map((item) => (item.id === targetId ? updated : item)));
			saveStatus = 'saved';
			savedTimer = setTimeout(() => {
				saveStatus = 'idle';
			}, SAVED_MS);
		} catch (error) {
			saveStatus = 'idle';

			if (error instanceof ContactApiError && (error.status === 409 || error.status === 422)) {
				formError = error.message;
			} else {
				toasts.error(m.contacts_save_failed(), () => void submitSave());
			}
		}
	}

	async function confirmDelete() {
		if (!selectedContact) return;

		const targetId = selectedContact.id;

		try {
			await deleteContact(targetId);
		} catch (error) {
			throw new Error(
				error instanceof ContactApiError ? error.message : m.contacts_delete_failed(),
				{ cause: error }
			);
		}

		const remaining = contacts.filter((item) => item.id !== targetId);

		contacts = remaining;
		selectedId = remaining[0]?.id ?? null;
		mobileDetailOpen = false;
	}

	async function logout() {
		if (signingOut) return;

		signingOut = true;
		signOutError = '';

		try {
			await authRequest('logout');
			await goto('/login', { refreshAll: true });
		} catch (error) {
			signOutError = error instanceof Error ? error.message : m.sign_out_failed();
		} finally {
			signingOut = false;
		}
	}
</script>

<svelte:head><title>{m.contacts_title()} — PhotoProofing</title></svelte:head>

<AppHeader onsignout={logout} />

<main class="flex flex-1 flex-col px-3.5 pt-4 pb-12 sm:px-8 sm:pt-6">
	{#if signOutError}
		<p role="alert" class="mb-3 text-sm text-danger">{signOutError}</p>
	{/if}

	<div class="mb-4 flex flex-wrap items-end justify-between gap-3 sm:mb-6">
		<div>
			<h1 class="text-[1.875rem] leading-[1.08] sm:text-[2.5rem]">{m.contacts_title()}</h1>
			{#if !loadFailed && contacts.length > 0}
				<p class="mt-1 text-sm text-muted-foreground">
					{m.contacts_total({ count: contacts.length })}
				</p>
			{/if}
		</div>
		<Button onclick={startNewContact} disabled={loadFailed}>
			<Plus size={16} aria-hidden="true" />
			{m.contacts_new()}
		</Button>
	</div>

	<div class="relative mb-4 max-w-md sm:mb-6">
		<Search
			size={16}
			aria-hidden="true"
			class="pointer-events-none absolute top-1/2 left-3.5 -translate-y-1/2 text-muted-foreground"
		/>
		<Input
			type="search"
			bind:value={query}
			disabled={loadFailed || contacts.length === 0}
			aria-label={m.contacts_search()}
			placeholder={m.contacts_search()}
			class="rounded-full pl-9"
		/>
	</div>

	{#if loadFailed}
		<EmptyState class="rounded-card border border-border bg-panel">
			<h2 class="text-[1.625rem] text-foreground sm:text-[1.75rem]">{m.contacts_load_failed()}</h2>
		</EmptyState>
	{:else if contacts.length === 0 && !creating}
		<EmptyState class="rounded-card border border-border bg-panel py-14">
			<Users size={28} aria-hidden="true" class="text-muted-foreground" />
			<h2 class="text-[1.625rem] text-foreground sm:text-[1.75rem]">{m.contacts_empty()}</h2>
			<p class="max-w-sm text-sm">{m.contacts_empty_body()}</p>
			<Button onclick={startNewContact}>{m.contacts_new()}</Button>
		</EmptyState>
	{:else}
		<div class="grid grid-cols-1 gap-5 md:grid-cols-[minmax(18rem,23rem)_1fr] md:items-start">
			{#if contacts.length > 0}
				<div class={mobileDetailOpen ? 'max-md:hidden' : ''}>
					{#if matching.length === 0}
						<div
							class="grid justify-items-center gap-3 rounded-card border border-border bg-panel p-6 text-center"
						>
							<p class="text-sm font-medium text-foreground">
								{m.contacts_no_match({ query: query.trim() })}
							</p>
							<Button variant="outline" size="sm" onclick={() => (query = '')}>
								{m.contacts_clear_search()}
							</Button>
						</div>
					{:else}
						<ul aria-label={m.contacts_list_label()} class="grid gap-2">
							{#each matching as item (item.id)}
								{@const active = !creating && selectedContact?.id === item.id}
								<li>
									<button
										type="button"
										aria-pressed={active}
										onclick={() => selectContact(item.id)}
										class="flex w-full cursor-pointer items-start gap-3 rounded-card border p-3.5 text-left transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus {active
											? 'border-foreground bg-panel shadow-sm'
											: 'border-border bg-panel/75 hover:bg-panel'}"
									>
										<span
											aria-hidden="true"
											class="grid size-9 shrink-0 place-items-center rounded-full text-xs font-semibold tracking-wide {active
												? 'bg-foreground text-background'
												: 'bg-border-strong/55 text-foreground'}"
										>
											{contactInitials(item.name)}
										</span>
										<span class="grid min-w-0 flex-1 gap-0.5">
											<span class="truncate text-sm font-semibold text-foreground">{item.name}</span
											>
											<span class="truncate text-xs text-muted-foreground">{item.email}</span>
											{#if item.phone}
												<span class="truncate text-xs text-muted-foreground">{item.phone}</span>
											{/if}
											{#if item.galleries.length > 0}
												<span class="mt-1.5 flex flex-wrap gap-1">
													{#each item.galleries as linked (linked.id)}
														<span
															class="max-w-44 truncate rounded-full border border-border bg-background px-2 py-0.5 text-[0.6875rem] font-medium text-muted-foreground"
														>
															{linked.title ?? m.gallery_untitled()}
														</span>
													{/each}
												</span>
											{/if}
										</span>
									</button>
								</li>
							{/each}
						</ul>
					{/if}
				</div>
			{/if}

			{#if creating || selectedContact}
				<section
					aria-labelledby="{formId}-heading"
					class="rounded-card border border-border bg-panel p-4 sm:p-6 {contacts.length > 0 &&
					!mobileDetailOpen
						? 'max-md:hidden'
						: ''}"
				>
					{#if contacts.length > 0}
						<button
							type="button"
							onclick={() => {
								if (creating) cancelCreate();
								else mobileDetailOpen = false;
							}}
							class="mb-3 inline-flex cursor-pointer items-center gap-1.5 text-xs font-medium text-muted-foreground hover:text-foreground md:hidden"
						>
							<ArrowLeft size={14} aria-hidden="true" />
							{m.contacts_back_to_list()}
						</button>
					{/if}

					<div class="mb-5 flex flex-wrap items-center justify-between gap-3">
						<div class="flex min-w-0 items-center gap-3">
							<span
								aria-hidden="true"
								class="grid size-10 shrink-0 place-items-center rounded-full bg-foreground text-sm font-semibold text-background"
							>
								{contactInitials(draftName || selectedContact?.name || '?')}
							</span>
							<h2 id="{formId}-heading" class="truncate text-xl">
								{creating ? m.contacts_new_heading() : selectedContact?.name}
							</h2>
						</div>

						{#if !creating && selectedContact}
							<Button
								variant="outline"
								size="sm"
								class="text-danger hover:border-danger/40 hover:bg-danger/10"
								onclick={() => (confirmDeleteOpen = true)}
							>
								<Trash2 size={15} aria-hidden="true" />
								{m.action_delete()}
							</Button>
						{/if}
					</div>

					<form
						class="grid gap-4"
						onsubmit={(event) => {
							event.preventDefault();
							if (creating) void submitCreate();
							else void submitSave();
						}}
					>
						<div class="grid gap-1.5 sm:grid-cols-2 sm:gap-4">
							<div class="grid gap-1.5">
								<Label for="{formId}-name">{m.common_name()}</Label>
								<Input id="{formId}-name" bind:value={draftName} maxlength={120} required />
							</div>

							<div class="grid gap-1.5">
								<Label for="{formId}-email">{m.login_email()}</Label>
								<Input
									id="{formId}-email"
									type="email"
									bind:value={draftEmail}
									maxlength={254}
									required
								/>
							</div>
						</div>

						<div class="grid gap-1.5 sm:max-w-xs">
							<Label for="{formId}-phone">{m.contacts_phone()}</Label>
							<Input id="{formId}-phone" type="tel" bind:value={draftPhone} maxlength={40} />
						</div>

						<div class="grid gap-1.5">
							<Label for="{formId}-notes">{m.contacts_notes()}</Label>
							<textarea
								id="{formId}-notes"
								bind:value={draftNotes}
								rows={3}
								maxlength={2000}
								class="cn-input min-h-22 w-full min-w-0 resize-y py-2 placeholder:text-muted-foreground focus-visible:border-foreground"
							></textarea>
						</div>

						{#if !creating && selectedContact}
							<div class="grid gap-2 border-t border-border pt-4">
								<span
									class="text-xs font-semibold tracking-[0.06em] text-muted-foreground uppercase"
								>
									{m.contacts_linked_galleries()}
								</span>
								{#if selectedContact.galleries.length === 0}
									<p class="text-sm text-muted-foreground">{m.contacts_no_galleries()}</p>
								{:else}
									<div class="flex flex-wrap gap-2">
										{#each selectedContact.galleries as linked (linked.id)}
											<a
												href="/dashboard/{linked.id}"
												class="inline-flex max-w-full items-center rounded-full border border-border bg-background px-3 py-1 text-xs font-medium text-foreground transition-colors hover:bg-border/60"
											>
												<span class="truncate">{linked.title ?? m.gallery_untitled()}</span>
											</a>
										{/each}
									</div>
								{/if}
							</div>
						{/if}

						{#if formError}
							<p role="alert" class="text-sm text-danger">{formError}</p>
						{/if}

						<div class="flex flex-wrap items-center justify-end gap-2.5 pt-2">
							{#if creating}
								<Button variant="outline" onclick={cancelCreate} disabled={creatingBusy}>
									{m.common_cancel()}
								</Button>
								<Button type="submit" disabled={creatingBusy}>
									{#if creatingBusy}
										<LoaderCircle
											size={16}
											class="animate-spin motion-reduce:animate-none"
											aria-hidden="true"
										/>
										{m.contacts_creating()}
									{:else}
										{m.contacts_create()}
									{/if}
								</Button>
							{:else}
								<Button
									variant="outline"
									disabled={!dirty || shownSaveStatus === 'saving'}
									onclick={revertDraft}
								>
									{m.common_cancel()}
								</Button>
								<SaveButton
									status={shownSaveStatus}
									disabled={shownSaveStatus === 'idle' && !dirty}
									onclick={() => void submitSave()}
								/>
							{/if}
						</div>
					</form>
				</section>
			{/if}
		</div>
	{/if}
</main>

{#if confirmDeleteOpen && selectedContact}
	<ConfirmDialog
		title={m.contacts_delete_title({ name: selectedContact.name })}
		confirmLabel={m.action_delete()}
		destructive
		onconfirm={confirmDelete}
		onclose={() => (confirmDeleteOpen = false)}
	>
		{#snippet description()}
			<p class="text-sm text-muted-foreground">{m.contacts_delete_body()}</p>
		{/snippet}
	</ConfirmDialog>
{/if}
