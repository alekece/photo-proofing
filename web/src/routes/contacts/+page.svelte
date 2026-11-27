<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onMount, tick } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { cubicIn } from 'svelte/easing';
	import { slide } from 'svelte/transition';
	import { afterNavigate, beforeNavigate } from '$app/navigation';
	import { ArrowUpDown, Check, Pencil, Trash2, Users } from '@lucide/svelte';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import ContactIdentity from '#lib/components/ContactIdentity.svelte';
	import ContactNotes from '#lib/components/ContactNotes.svelte';
	import ContactNotesToggle from '#lib/components/ContactNotesToggle.svelte';
	import ContactForm, { type ContactDraft } from '#lib/components/ContactForm.svelte';
	import ActionButton from '#lib/components/ui/ActionButton/ActionButton.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import Checkbox from '#lib/components/ui/Checkbox/Checkbox.svelte';
	import ConfirmDialog from '#lib/components/ui/ConfirmDialog/ConfirmDialog.svelte';
	import Dialog from '#lib/components/ui/Dialog/Dialog.svelte';
	import EmptyState from '#lib/components/ui/EmptyState/EmptyState.svelte';
	import Popover from '#lib/components/ui/Popover/Popover.svelte';
	import SelectActions from '#lib/components/ui/SelectActions/SelectActions.svelte';
	import ToolbarMenu from '#lib/components/ui/ToolbarMenu/ToolbarMenu.svelte';
	import WorkspaceHeading from '#lib/components/ui/WorkspaceHeading/WorkspaceHeading.svelte';
	import WorkspaceToolbar from '#lib/components/ui/WorkspaceToolbar/WorkspaceToolbar.svelte';
	import {
		createContact,
		deleteContact,
		updateContact,
		type Contact,
		type ContactChangesInput
	} from '#lib/contacts/api.js';
	import { sortContacts } from '#lib/contacts/format.js';
	import { ApiError } from '#lib/http.js';
	import type { PageData } from './$types';

	type Editor =
		| { mode: 'new'; draft: ContactDraft }
		| { mode: 'edit'; id: string; draft: ContactDraft; original: Contact };
	type EditRow = { original: Contact; draft: ContactDraft };

	let { data }: { data: PageData } = $props();

	const formId = $props.id();
	let draftNumber = 0;
	const writes = new SvelteSet<string | null>();
	const openNotes = new SvelteSet<string>();
	const selectedIds = new SvelteSet<string>();
	// svelte-ignore state_referenced_locally
	let contacts = $state<Contact[]>(data.contacts ? [...data.contacts] : []);
	let editor = $state<Editor | null>(null);
	let editRows = $state<EditRow[]>([]);
	let batchSaving = $state(false);
	let batchSaveButton = $state<ActionButton>();
	let query = $state('');
	let resultsHeight = $state(0);
	let sortOrder = $state<'name_asc' | 'name_desc'>('name_asc');
	let root: HTMLElement;
	let newButton = $state<HTMLButtonElement | null>(null);
	let saveButton = $state<ActionButton>();
	let deleteTargets = $state<Contact[] | null>(null);
	let disposed = false;

	const loadFailed = $derived(data.contacts === null);
	const sorts = $derived([
		{ value: 'name_asc', label: m.contacts_sort_name_asc() },
		{ value: 'name_desc', label: m.contacts_sort_name_desc() }
	] as const);
	const matching = $derived.by(() => {
		const needle = query.trim().toLowerCase();
		const sorted = sortContacts(contacts);

		if (sortOrder === 'name_desc') sorted.reverse();
		if (!needle) return sorted;

		return sorted.filter(
			(item) =>
				item.name.toLowerCase().includes(needle) ||
				item.email.toLowerCase().includes(needle) ||
				(item.phone?.toLowerCase().includes(needle) ?? false)
		);
	});
	const selection = $derived(matching.filter(({ id }) => selectedIds.has(id)));
	const selectable = $derived(
		matching.filter(({ id }) => !writes.has(id) && !(editor?.mode === 'edit' && editor.id === id))
	);

	const activity = $derived(createSessionActivity(data.idleTimeoutMs));

	beforeNavigate(({ to, willUnload }) => {
		if (willUnload || to?.route.id !== '/contacts') deleteTargets = null;
	});
	afterNavigate(() => {
		void activity();
	});
	onMount(() => {
		const stop = listenForActivity(() => activity());

		return () => {
			disposed = true;
			stop();
		};
	});

	$effect.pre(() => {
		const visibleIds = new Set(matching.map(({ id }) => id));
		for (const id of selectedIds) {
			if (!visibleIds.has(id)) selectedIds.delete(id);
		}
	});

	async function focusFallback(): Promise<void> {
		await tick();
		const search = root.querySelector<HTMLInputElement>('input[type=search]');
		(search && !search.disabled ? search : newButton)?.focus();
	}

	function makeDraft(contact?: Contact): ContactDraft {
		return {
			form: `${formId}-draft-${++draftNumber}`,
			name: contact?.name ?? '',
			email: contact?.email ?? '',
			phone: contact?.phone ?? '',
			notes: contact?.notes ?? '',
			error: '',
			invalid: null
		};
	}

	function openEditor(contact?: Contact): void {
		const draft = makeDraft(contact);

		editor = contact
			? { mode: 'edit', id: contact.id, draft, original: contact }
			: { mode: 'new', draft };
		void tick().then(() => document.getElementById(`${draft.form}-name`)?.focus());
	}

	function openQuickEdit(): void {
		if (selection.length === 1) {
			openEditor(selection[0]);
		} else if (selection.length > 1) {
			editRows = selection.map((original) => ({ original, draft: makeDraft(original) }));
			const first = editRows[0].draft;
			void tick().then(() => document.getElementById(`${first.form}-name`)?.focus());
		}
	}

	function closeEditor(): void {
		const previous = editor;
		editor = null;
		if (previous?.mode === 'edit' && !matching.some(({ id }) => id === previous.id)) {
			void focusFallback();
		}
	}

	function values(draft: ContactDraft) {
		return {
			name: draft.name.trim(),
			email: draft.email.trim(),
			phone: draft.phone.trim() || null,
			notes: draft.notes.trim() || null
		};
	}

	function changes(draft: ContactDraft, original: Contact): ContactChangesInput {
		const { name, email, phone, notes } = values(draft);

		return {
			...(name !== original.name && { name }),
			...(email.toLowerCase() !== original.email && { email, expected_email: original.email }),
			...(phone !== original.phone && { phone }),
			...(notes !== original.notes && { notes })
		};
	}

	function validateName(draft: ContactDraft): boolean {
		if (!draft.name.trim()) {
			draft.invalid = 'name';
			draft.error = m.contacts_name_required();
			document.getElementById(`${draft.form}-name`)?.focus();

			return false;
		}

		return true;
	}

	async function saveDraft(draft: ContactDraft, original?: Contact): Promise<Contact | null> {
		const id = original?.id ?? null;
		if (writes.has(id)) return null;

		draft.error = '';
		draft.invalid = null;
		writes.add(id);

		try {
			const saved = original
				? await updateContact(original.id, changes(draft, original))
				: await createContact(values(draft));
			if (disposed) return null;

			contacts =
				id === null
					? [...contacts, saved]
					: contacts.map((contact) => (contact.id === id ? saved : contact));

			return saved;
		} catch (error) {
			if (!disposed) {
				draft.error =
					error instanceof ApiError && (error.status === 409 || error.status === 422)
						? error.message
						: m.contacts_save_failed();
			}

			return null;
		} finally {
			writes.delete(id);
		}
	}

	async function submitContact(draft: ContactDraft): Promise<boolean> {
		const current = editor;
		if (!current || current.draft !== draft || !validateName(draft)) return false;

		const saved = await saveDraft(draft, current.mode === 'edit' ? current.original : undefined);

		return saved !== null && editor === current;
	}

	const pendingEdits = $derived(
		editRows.filter(({ draft, original }) => Object.keys(changes(draft, original)).length > 0)
	);

	async function submitBatch(): Promise<boolean> {
		const pending = pendingEdits;
		if (batchSaving || !pending.length || !pending.every(({ draft }) => validateName(draft))) {
			return false;
		}

		batchSaving = true;
		let allSaved = true;

		try {
			for (const row of pending) {
				if (disposed) return false;

				const saved = await saveDraft(row.draft, row.original);
				if (saved) {
					row.original = saved;
					Object.assign(row.draft, {
						name: saved.name,
						email: saved.email,
						phone: saved.phone ?? '',
						notes: saved.notes ?? ''
					});
				} else {
					allSaved = false;
				}
			}

			return allSaved;
		} finally {
			batchSaving = false;
		}
	}

	function clearSelection(): void {
		selectedIds.clear();
		void focusFallback();
	}

	async function removeSelection() {
		const targets = deleteTargets;
		if (!targets) return;

		for (const [index, { id }] of targets.entries()) {
			if (disposed || deleteTargets !== targets) return;

			writes.add(id);
			try {
				await deleteContact(id);
				if (disposed) return;

				selectedIds.delete(id);
				contacts = contacts.filter((item) => item.id !== id);
			} catch {
				if (disposed || deleteTargets !== targets) return;

				deleteTargets = targets.slice(index);
				throw new Error(
					index
						? m.contacts_delete_partial({ deleted: index, total: targets.length })
						: m.contacts_delete_failed()
				);
			} finally {
				writes.delete(id);
			}
		}
	}

	function removal(node: HTMLElement, id: string) {
		// Search results leave immediately; only a committed deletion animates.
		return contacts.some((item) => item.id === id)
			? { duration: 0 }
			: slide(node, { duration: duration(280), easing: cubicIn });
	}

	function duration(ms: number): number {
		return window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ? 0 : ms;
	}
</script>

<svelte:head><title>{m.contacts_title()} — PhotoProofing</title></svelte:head>

<main bind:this={root} class="flex flex-1 flex-col workspace-inset pb-24">
	<WorkspaceHeading
		title={m.contacts_title()}
		create={{ label: m.contacts_new(), onclick: () => openEditor(), disabled: loadFailed }}
		bind:createRef={newButton}
	/>

	<WorkspaceToolbar
		class="mb-4 w-full sm:mb-6"
		search={{
			label: m.contacts_search(),
			value: query,
			disabled: loadFailed || contacts.length === 0,
			oninput: (value) => (query = value)
		}}
		resultsHeight={matching.length ? resultsHeight : 0}
		resultCount={!loadFailed && query.trim()
			? m.contacts_total({ count: matching.length })
			: undefined}
	>
		{#snippet sort()}
			<ToolbarMenu
				label={m.sort_by()}
				summary={sorts[sortOrder === 'name_asc' ? 0 : 1].label}
				icon={ArrowUpDown}
				value={sortOrder}
				options={sorts}
				disabled={loadFailed || contacts.length === 0}
				onchange={(value) => (sortOrder = value)}
			/>
		{/snippet}
	</WorkspaceToolbar>

	{#if loadFailed}
		<EmptyState variant="plain" class="rounded-card">
			<h2 class="text-[1.625rem] text-foreground sm:text-[1.75rem]">{m.contacts_load_failed()}</h2>
			<Button variant="outline" onclick={() => location.reload()}>{m.action_retry()}</Button>
		</EmptyState>
	{:else}
		{#if contacts.length === 0}
			<EmptyState variant="plain" class="rounded-card py-14">
				<Users size={28} aria-hidden="true" class="text-muted-foreground" />
				<h2 class="text-[1.625rem] text-foreground sm:text-[1.75rem]">{m.contacts_empty()}</h2>
				<p class="max-w-sm text-sm">{m.contacts_empty_body()}</p>
			</EmptyState>
		{:else if contacts.length > 0 && matching.length === 0}
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
		{/if}
		<ul
			bind:clientHeight={resultsHeight}
			aria-label={m.contacts_list_label()}
			aria-hidden={matching.length === 0}
			class="@container grid gap-2.5"
		>
			{#each matching as item (item.id)}
				<li class="min-w-0 rounded-xl border border-border bg-panel" out:removal={item.id}>
					<div
						class="group/contact grid grid-rows-[3.5rem] items-center gap-2 p-2.5 @xl:min-h-20 @xl:grid-cols-[clamp(14rem,34%,22rem)_minmax(0,1fr)_clamp(11rem,24%,15rem)] @xl:grid-rows-1 @xl:gap-3"
					>
						<div class="flex min-w-0 items-center gap-2 @xl:gap-3">
							<label class="relative grid h-9 w-7 shrink-0 cursor-pointer place-items-center">
								<Checkbox
									aria-label={m.tile_select({ name: item.name })}
									checked={selectedIds.has(item.id)}
									disabled={writes.has(item.id)}
									onchange={(event) => {
										if (event.currentTarget.checked) selectedIds.add(item.id);
										else selectedIds.delete(item.id);
									}}
									class="peer size-5 cursor-pointer appearance-none rounded-full border border-border-strong bg-input transition-opacity checked:border-foreground checked:bg-foreground hover:bg-background checked:hover:bg-foreground focus-visible:opacity-100 disabled:cursor-not-allowed disabled:opacity-40 motion-reduce:transition-none {selection.length
										? ''
										: 'group-hover/contact:opacity-100 [@media(hover:hover)]:opacity-0'}"
								/>
								<Check
									size={14}
									aria-hidden="true"
									class="pointer-events-none absolute text-inverse opacity-0 peer-checked:opacity-100"
								/>
							</label>
							<ContactIdentity contact={item} />
							{#if item.notes || item.galleries.length > 0}
								<ContactNotesToggle
									open={openNotes.has(item.id)}
									onclick={() => {
										if (!openNotes.delete(item.id)) openNotes.add(item.id);
									}}
								/>
							{/if}
						</div>
						{#if item.notes}
							<div
								class="min-w-0 @xl:col-start-2 @xl:row-start-1 {openNotes.has(item.id)
									? 'pl-9 @xl:pl-0'
									: 'hidden @xl:block'}"
							>
								<p class="mb-0.5 text-xs font-semibold text-foreground @xl:hidden">
									{m.contacts_notes()}
								</p>
								<ContactNotes notes={item.notes} />
							</div>
						{/if}
						{#if item.galleries.length > 0}
							{@const [latest, ...remaining] = item.galleries}
							{@const title = latest.title ?? m.gallery_untitled()}
							<div
								class="min-w-0 @xl:col-start-3 @xl:row-start-1 {openNotes.has(item.id)
									? 'pl-9 @xl:pl-0'
									: 'hidden @xl:block'}"
							>
								<p class="mb-1 text-xs font-semibold text-foreground @xl:hidden">
									{m.contacts_linked_galleries()}
								</p>
								<div class="flex min-h-10 min-w-0 items-center gap-1 @xl:justify-end">
									<a
										href="/dashboard/{encodeURIComponent(latest.id)}"
										{title}
										class="max-w-[24ch] min-w-0 truncate pill border border-border bg-background text-xs text-foreground transition-colors hover:bg-border/60"
									>
										{title}
									</a>
									{#if remaining.length > 0}
										<Popover
											align="end"
											placement="auto"
											label={m.contacts_more_galleries({ count: remaining.length })}
											class="overflow-count sm:icon-action"
											rootClass="shrink-0"
											panelClass="w-max min-w-0 max-w-[min(18rem,calc(100vw-3.5rem))] max-h-[var(--popover-space,50dvh)] overflow-auto p-2"
										>
											{#snippet trigger()}+{remaining.length}{/snippet}
											<ul aria-label={m.contacts_linked_galleries()} class="flex flex-wrap gap-1">
												{#each remaining as linked (linked.id)}
													<li class="max-w-full">
														<a
															href="/dashboard/{encodeURIComponent(linked.id)}"
															class="block pill border border-border bg-background text-xs wrap-anywhere text-foreground hover:bg-border/60"
														>
															{linked.title ?? m.gallery_untitled()}
														</a>
													</li>
												{/each}
											</ul>
										</Popover>
									{/if}
								</div>
							</div>
						{/if}
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</main>

{#if editor}
	{@const current = editor}
	{@const draft = current.draft}
	{@const busy = writes.has(current.mode === 'edit' ? current.id : null)}
	<Dialog
		formMotion
		dismissible={!busy}
		labelledby="{draft.form}-title"
		onclose={closeEditor}
		class="m-auto max-h-[calc(100dvh-2rem)] w-[min(30rem,calc(100vw-2rem))] overflow-auto rounded-dialog border border-border bg-panel p-4 text-foreground shadow-xl sm:p-6"
	>
		{#snippet children(dismiss, complete)}
			<div class="grid gap-4">
				<h2 id="{draft.form}-title" class="pr-10 text-xl">
					{current.mode === 'new'
						? m.contacts_new_heading()
						: m.contacts_edit_label({ name: current.original.name })}
				</h2>
				<form
					id={draft.form}
					onsubmit={(event) => {
						event.preventDefault();
						if (editor?.draft === draft) saveButton?.activate();
					}}
				>
					<fieldset disabled={busy} class="min-w-0">
						<ContactForm {draft} onchange={(change) => Object.assign(draft, change)} />
					</fieldset>
				</form>
				<div class="flex flex-wrap justify-end gap-2">
					<Button variant="outline" disabled={busy} onclick={dismiss}>{m.common_cancel()}</Button>
					<ActionButton
						bind:this={saveButton}
						form={draft.form}
						disabled={busy ||
							(current.mode === 'edit' &&
								Object.keys(changes(draft, current.original)).length === 0)}
						labels={current.mode === 'new'
							? { idle: m.contacts_create(), pending: m.contacts_creating() }
							: { idle: m.studio_save(), pending: m.studio_saving() }}
						onaction={() => submitContact(draft)}
						onsettled={complete}
					/>
				</div>
			</div>
		{/snippet}
	</Dialog>
{/if}

{#if editRows.length}
	<Dialog
		formMotion
		dismissible={!batchSaving}
		labelledby="{formId}-batch-title"
		onclose={() => {
			editRows = [];
			if (!selection.length) void focusFallback();
		}}
		class="m-auto max-h-[calc(100dvh-2rem)] w-[min(60rem,calc(100vw-2rem))] overflow-auto rounded-dialog border border-border bg-background p-4 text-foreground shadow-xl sm:p-6"
	>
		{#snippet children(dismiss, complete)}
			<div class="grid gap-4">
				<h2 id="{formId}-batch-title" class="border-b border-border pr-10 pb-3 text-2xl">
					{m.contacts_quick_edit_title()}
				</h2>
				<form
					id="{formId}-batch"
					aria-busy={batchSaving}
					class="grid gap-2.5"
					onsubmit={(event) => {
						event.preventDefault();
						batchSaveButton?.activate();
					}}
				>
					{#each editRows as { original, draft } (original.id)}
						<fieldset
							disabled={batchSaving}
							class="min-w-0 rounded-control border border-border bg-panel p-3"
						>
							<legend class="sr-only">{original.name}</legend>
							<ContactForm {draft} compact onchange={(change) => Object.assign(draft, change)} />
						</fieldset>
					{/each}
				</form>
				<div class="flex flex-wrap justify-end gap-2">
					<Button variant="outline" disabled={batchSaving} onclick={dismiss}>
						{m.common_cancel()}
					</Button>
					<ActionButton
						bind:this={batchSaveButton}
						form="{formId}-batch"
						disabled={batchSaving || !pendingEdits.length}
						labels={{ idle: m.studio_save(), pending: m.studio_saving() }}
						onaction={submitBatch}
						onsettled={complete}
					/>
				</div>
			</div>
		{/snippet}
	</Dialog>
{/if}

{#if selection.length > 0}
	<SelectActions
		count={selection.length}
		total={selectable.length}
		rail="dashboard"
		onselectall={() => {
			for (const { id } of selectable) selectedIds.add(id);
		}}
		onunselectall={clearSelection}
		onexit={clearSelection}
		canEscape={() =>
			!deleteTargets &&
			!editor &&
			!editRows.length &&
			!document.querySelector('[data-slot="popover"] > button[aria-expanded="true"]')}
	>
		{#snippet actions()}
			<Button
				variant="outline"
				size="sm"
				class="min-h-9! gap-1.5 px-3! group-data-[rail=dashboard]/select-actions:max-lg:px-2.5!"
				aria-label={m.dashboard_bulk_quick_edit()}
				onclick={openQuickEdit}
			>
				<Pencil size={15} aria-hidden="true" />
				<span class="group-data-[rail=dashboard]/select-actions:max-lg:sr-only">
					{m.dashboard_bulk_quick_edit()}
				</span>
			</Button>
			<Button
				variant="destructive"
				size="sm"
				class="min-h-9! gap-1.5 group-data-[rail=dashboard]/select-actions:max-lg:px-2.5!"
				onclick={() => (deleteTargets = selection)}
			>
				<Trash2 size={16} aria-hidden="true" />
				<span class="group-data-[rail=dashboard]/select-actions:max-lg:sr-only">
					{m.action_delete()}
				</span>
			</Button>
		{/snippet}
	</SelectActions>
{/if}

{#if deleteTargets}
	<ConfirmDialog
		title={deleteTargets.length === 1
			? m.contacts_delete_title({ name: deleteTargets[0].name })
			: m.contacts_delete_many({ count: deleteTargets.length })}
		confirmLabel={m.action_delete()}
		destructive
		onconfirm={removeSelection}
		onclose={() => {
			deleteTargets = null;
			if (!selection.length) void focusFallback();
		}}
	>
		{#snippet description()}
			<p class="text-sm text-muted-foreground">
				{deleteTargets?.length === 1 ? m.contacts_delete_body() : m.contacts_delete_many_body()}
			</p>
		{/snippet}
	</ConfirmDialog>
{/if}
