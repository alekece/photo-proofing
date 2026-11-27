<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import {
		ArrowUpDown,
		Check,
		LoaderCircle,
		Pencil,
		Plus,
		Search,
		UserMinus,
		Users
	} from '@lucide/svelte';
	import { onDestroy, type Snippet } from 'svelte';
	import { SvelteMap, SvelteSet } from 'svelte/reactivity';
	import { slide } from 'svelte/transition';
	import ContactForm, { type ContactDraft } from './ContactForm.svelte';
	import ContactIdentity from './ContactIdentity.svelte';
	import ContactNotes from './ContactNotes.svelte';
	import ContactNotesToggle from './ContactNotesToggle.svelte';
	import ActionButton from './ui/ActionButton/ActionButton.svelte';
	import Button from './ui/Button/Button.svelte';
	import Checkbox from './ui/Checkbox/Checkbox.svelte';
	import Dialog from './ui/Dialog/Dialog.svelte';
	import EmptyState from './ui/EmptyState/EmptyState.svelte';
	import Input from './ui/Input/Input.svelte';
	import Switch from './ui/Switch/Switch.svelte';
	import SelectActions from './ui/SelectActions/SelectActions.svelte';
	import ToolbarMenu from './ui/ToolbarMenu/ToolbarMenu.svelte';
	import WorkspaceToolbar from './ui/WorkspaceToolbar/WorkspaceToolbar.svelte';
	import { toasts } from './ui/Toast/toasts.svelte.js';
	import {
		createContact,
		listContacts,
		updateContact,
		type Contact,
		type ContactChangesInput
	} from '#lib/contacts/api.js';
	import { sortContacts } from '#lib/contacts/format.js';
	import {
		linkGalleryContact,
		listGalleryContacts,
		unlinkGalleryContact
	} from '#lib/gallery/contacts.js';
	import { ApiError } from '#lib/http.js';

	type Editor =
		| { mode: 'new'; draft: ContactDraft; created?: Contact }
		| { mode: 'edit'; id: string; draft: ContactDraft; original: Contact };
	type EditRow = { original: Contact; draft: ContactDraft };

	let {
		galleryId,
		revision = 0,
		oncount,
		onheader
	}: {
		galleryId: string;
		revision?: number;
		oncount?: (count: number) => void;
		onheader?: (action: Snippet) => void;
	} = $props();

	$effect(() => onheader?.(newContactAction));

	const formId = $props.id();
	const linkedIds = new SvelteSet<string>();
	const writes = new SvelteSet<string | null>();
	const openNotes = new SvelteSet<string>();
	/** Associations toggled in Manage contacts, applied only once it is confirmed. */
	const wanted = new SvelteMap<string, boolean>();
	const selectedIds = new SvelteSet<string>();
	let draftNumber = 0;
	let contacts = $state<Contact[]>([]);
	let loading = $state(true);
	let loadError = $state('');
	let query = $state('');
	let resultsHeight = $state(0);
	let sortOrder = $state<'name_asc' | 'name_desc'>('name_asc');
	let editor = $state<Editor | null>(null);
	let editRows = $state<EditRow[]>([]);
	let managing = $state(false);
	let applying = $state(false);
	let pickerQuery = $state('');
	let batchSaving = $state(false);
	let batchAssociation = $state(false);
	let saveButton = $state<ActionButton>();
	let batchSaveButton = $state<ActionButton>();
	let newButton = $state<HTMLButtonElement | null>(null);
	let disposed = false;
	let loadNumber = 0;
	onDestroy(() => {
		disposed = true;
		loadNumber++;
	});

	const sorts = $derived([
		{ value: 'name_asc', label: m.contacts_sort_name_asc() },
		{ value: 'name_desc', label: m.contacts_sort_name_desc() }
	] as const);
	const linkedContacts = $derived(contacts.filter(({ id }) => linkedIds.has(id)));
	const matching = $derived.by(() => {
		const needle = query.trim().toLowerCase();
		const sorted = sortContacts(linkedContacts);

		if (sortOrder === 'name_desc') sorted.reverse();
		if (!needle) return sorted;

		return sorted.filter(
			(contact) =>
				contact.name.toLowerCase().includes(needle) ||
				contact.email.toLowerCase().includes(needle) ||
				(contact.phone?.toLowerCase().includes(needle) ?? false)
		);
	});
	const directory = $derived.by(() => {
		const needle = pickerQuery.trim().toLowerCase();

		return sortContacts(contacts).filter(
			({ name, email }) =>
				!needle || name.toLowerCase().includes(needle) || email.toLowerCase().includes(needle)
		);
	});
	const selection = $derived(matching.filter(({ id }) => selectedIds.has(id)));
	const selectable = $derived(matching.filter(({ id }) => !writes.has(id)));

	$effect(() => {
		const id = galleryId;
		void revision;
		void load(id);

		return () => {
			loadNumber++;
		};
	});

	$effect(() => {
		if (!loading && !loadError) oncount?.(linkedIds.size);
	});

	$effect.pre(() => {
		const visibleIds = new Set(matching.map(({ id }) => id));
		for (const id of selectedIds) {
			if (!visibleIds.has(id)) selectedIds.delete(id);
		}
	});

	async function load(id = galleryId) {
		const current = ++loadNumber;

		loading = true;
		loadError = '';
		try {
			const [directory, linked] = await Promise.all([listContacts(), listGalleryContacts(id)]);
			if (current !== loadNumber || disposed || id !== galleryId) return;

			contacts = directory;
			linkedIds.clear();
			for (const contact of linked) linkedIds.add(contact.id);
			selectedIds.clear();
			editor = null;
			editRows = [];
		} catch {
			if (current === loadNumber && !disposed && id === galleryId) {
				loadError = m.gallery_contacts_load_failed();
			}
		} finally {
			if (current === loadNumber && !disposed && id === galleryId) loading = false;
		}
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

	function openEditor(contact?: Contact) {
		const draft = makeDraft(contact);

		editor = contact
			? { mode: 'edit', id: contact.id, draft, original: contact }
			: { mode: 'new', draft };
		queueMicrotask(() => document.getElementById(`${draft.form}-name`)?.focus());
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
		const value = values(draft);

		return {
			...(value.name !== original.name && { name: value.name }),
			...(value.email.toLowerCase() !== original.email && {
				email: value.email,
				expected_email: original.email
			}),
			...(value.phone !== original.phone && { phone: value.phone }),
			...(value.notes !== original.notes && { notes: value.notes })
		};
	}

	function validate(draft: ContactDraft) {
		if (draft.name.trim()) return true;

		draft.invalid = 'name';
		draft.error = m.contacts_name_required();
		document.getElementById(`${draft.form}-name`)?.focus();
		return false;
	}

	function upsert(contact: Contact) {
		const remaining = contacts.filter(({ id }) => id !== contact.id);

		contacts = sortContacts([...remaining, contact]);
	}

	async function saveDraft(draft: ContactDraft, original?: Contact) {
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

			upsert(saved);
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

	async function setAssociation(contactId: string, linked: boolean) {
		if (writes.has(contactId)) return false;

		writes.add(contactId);
		try {
			if (linked) {
				const contact = await linkGalleryContact(galleryId, contactId);
				linkedIds.add(contactId);
				upsert(contact);
			} else {
				await unlinkGalleryContact(galleryId, contactId);
				linkedIds.delete(contactId);
			}

			return true;
		} catch (error) {
			toasts.error(
				error instanceof ApiError && (error.status === 409 || error.status === 422)
					? error.message
					: linked
						? m.gallery_contacts_link_failed()
						: m.gallery_contacts_unlink_failed()
			);
			return false;
		} finally {
			writes.delete(contactId);
		}
	}

	async function submitEditor(draft: ContactDraft) {
		const current = editor;
		if (!current || current.draft !== draft || !validate(draft)) return false;

		if (current.mode === 'edit') {
			return (await saveDraft(draft, current.original)) !== null;
		}

		current.created ??= (await saveDraft(draft)) ?? undefined;
		if (!current.created) return false;

		if (Object.keys(changes(draft, current.created)).length > 0) {
			const saved = await saveDraft(draft, current.created);
			if (!saved) return false;

			current.created = saved;
		}

		const linked = await setAssociation(current.created.id, true);
		if (!linked) draft.error = m.gallery_contacts_link_failed();

		return linked;
	}

	const pendingEdits = $derived(
		editRows.filter(({ draft, original }) => Object.keys(changes(draft, original)).length > 0)
	);

	function openQuickEdit() {
		if (selection.length === 1) {
			openEditor(selection[0]);
			return;
		}

		editRows = selection.map((original) => ({ original, draft: makeDraft(original) }));
		const first = editRows[0]?.draft;
		if (first) queueMicrotask(() => document.getElementById(`${first.form}-name`)?.focus());
	}

	async function submitBatch() {
		const pending = pendingEdits;
		if (batchSaving || !pending.length || !pending.every(({ draft }) => validate(draft))) {
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

	async function unlinkSelection() {
		const targets = [...selection];
		if (batchAssociation || targets.length === 0) return;

		batchAssociation = true;
		try {
			for (const target of targets) {
				if (disposed) return;
				if (await setAssociation(target.id, false)) selectedIds.delete(target.id);
			}
		} finally {
			batchAssociation = false;
		}
	}

	function motion(ms: number) {
		const reduced = window.matchMedia?.('(prefers-reduced-motion: reduce)');

		return reduced && !reduced.matches ? ms : 0;
	}

	function want(contactId: string, linked: boolean) {
		if (linked === linkedIds.has(contactId)) wanted.delete(contactId);
		else wanted.set(contactId, linked);
	}

	/** Applies the toggled associations; failures stay toggled so they can be retried. */
	async function applyWanted(dismiss: () => void) {
		if (applying) return;

		applying = true;
		try {
			for (const [contactId, linked] of [...wanted]) {
				if (disposed) return;
				if (await setAssociation(contactId, linked)) wanted.delete(contactId);
			}
		} finally {
			applying = false;
		}

		if (wanted.size === 0) dismiss();
	}

	function clearSelection() {
		selectedIds.clear();
	}
</script>

{#snippet newContactAction()}
	<div class="flex flex-wrap items-center justify-end gap-2">
		<Button
			variant="outline"
			size="sm"
			onclick={() => {
				pickerQuery = '';
				wanted.clear();
				managing = true;
			}}
		>
			<Users size={15} aria-hidden="true" />
			{m.gallery_contacts_manage()}
		</Button>
		<Button size="sm" bind:ref={newButton} onclick={() => openEditor()}>
			<Plus size={15} aria-hidden="true" />
			{m.contacts_new()}
		</Button>
	</div>
{/snippet}

<section class="grid min-w-0 gap-4" aria-label={m.contacts_title()}>
	{#if !onheader}
		<div class="flex flex-wrap justify-end gap-3">
			{@render newContactAction()}
		</div>
	{/if}

	<WorkspaceToolbar
		search={{
			label: m.contacts_search(),
			value: query,
			disabled: loading || Boolean(loadError) || linkedContacts.length === 0,
			oninput: (value) => (query = value)
		}}
		resultsHeight={matching.length ? resultsHeight : 0}
		resultCount={!loadError && query.trim()
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
				disabled={loading || Boolean(loadError) || linkedContacts.length === 0}
				onchange={(value) => (sortOrder = value)}
			/>
		{/snippet}
	</WorkspaceToolbar>

	{#if loading}
		<div class="flex items-center justify-center py-10">
			<LoaderCircle
				size={20}
				class="animate-spin text-muted-foreground motion-reduce:animate-none"
				aria-hidden="true"
			/>
		</div>
	{:else if loadError}
		<div
			class="flex flex-wrap items-center justify-between gap-3 rounded-card border border-border bg-panel p-4"
		>
			<p role="alert" class="text-sm text-danger">{loadError}</p>
			<Button variant="outline" size="sm" onclick={() => void load()}>
				{m.action_retry()}
			</Button>
		</div>
	{:else if linkedContacts.length === 0}
		<EmptyState variant="plain" class="rounded-card border border-border bg-panel py-10">
			<Users size={26} aria-hidden="true" class="text-muted-foreground" />
			<h3>{m.gallery_contacts_empty()}</h3>
			<p class="max-w-sm text-sm">{m.gallery_contacts_empty_body()}</p>
		</EmptyState>
	{:else}
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
		{/if}

		<ul
			bind:clientHeight={resultsHeight}
			aria-label={m.contacts_list_label()}
			aria-hidden={matching.length === 0}
			class="@container grid gap-2.5"
		>
			{#each matching as item (item.id)}
				{@const busy = writes.has(item.id)}
				<li
					class="min-w-0 rounded-xl border border-border bg-panel"
					in:slide={{ duration: motion(260) }}
					out:slide={{ duration: motion(220) }}
				>
					<div
						class="group/contact grid min-h-16 items-center gap-2 p-2.5 @xl:grid-cols-[clamp(14rem,34%,22rem)_minmax(0,1fr)] @xl:gap-4"
					>
						<div class="flex min-w-0 items-center gap-2 @xl:gap-3">
							<label class="relative grid h-9 w-7 shrink-0 cursor-pointer place-items-center">
								<Checkbox
									aria-label={m.tile_select({ name: item.name })}
									checked={selectedIds.has(item.id)}
									disabled={busy}
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
							{#if item.notes}
								<ContactNotesToggle
									open={openNotes.has(item.id)}
									onclick={() => {
										if (!openNotes.delete(item.id)) openNotes.add(item.id);
									}}
								/>
							{/if}
						</div>
						{#if item.notes}
							<div class="min-w-0 {openNotes.has(item.id) ? 'pl-9 @xl:pl-0' : 'hidden @xl:block'}">
								<ContactNotes notes={item.notes} />
							</div>
						{/if}
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</section>

{#if editor}
	{@const current = editor}
	{@const draft = current.draft}
	{@const busy =
		(current.mode === 'edit'
			? writes.has(current.id)
			: writes.has(null) || (current.created ? writes.has(current.created.id) : false)) ||
		batchAssociation}
	<Dialog
		formMotion
		dismissible={!busy}
		labelledby="{draft.form}-title"
		onclose={() => (editor = null)}
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
							? { idle: m.gallery_contacts_create_and_link(), pending: m.contacts_creating() }
							: { idle: m.studio_save(), pending: m.studio_saving() }}
						onaction={() => submitEditor(draft)}
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
		onclose={() => (editRows = [])}
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
		rail="gallery"
		onselectall={() => {
			for (const { id } of selectable) selectedIds.add(id);
		}}
		onunselectall={clearSelection}
		onexit={clearSelection}
		canEscape={() => !editor && !editRows.length && !batchAssociation}
	>
		{#snippet actions()}
			<Button
				variant="outline"
				size="sm"
				class="min-h-9! gap-1.5 px-3! group-data-[rail=gallery]/select-actions:max-xl:px-2.5!"
				aria-label={m.dashboard_bulk_quick_edit()}
				disabled={batchAssociation}
				onclick={openQuickEdit}
			>
				<Pencil size={15} aria-hidden="true" />
				<span class="group-data-[rail=gallery]/select-actions:max-xl:sr-only">
					{m.dashboard_bulk_quick_edit()}
				</span>
			</Button>
			<Button
				variant="destructive"
				size="sm"
				class="min-h-9! gap-1.5 group-data-[rail=gallery]/select-actions:max-xl:px-2.5!"
				aria-label={m.gallery_contacts_unlink_selected({ count: selection.length })}
				disabled={batchAssociation}
				onclick={() => void unlinkSelection()}
			>
				<UserMinus size={15} aria-hidden="true" />
				<span class="group-data-[rail=gallery]/select-actions:max-xl:sr-only">
					{m.gallery_contacts_unlink_selected({ count: selection.length })}
				</span>
			</Button>
		{/snippet}
	</SelectActions>
{/if}

{#if managing}
	<Dialog
		dismissible={!applying}
		onclose={() => {
			managing = false;
			wanted.clear();
		}}
		labelledby="{formId}-manage-title"
		class="m-auto h-[min(34rem,calc(100dvh-2rem))] w-[min(30rem,calc(100vw-2rem))] rounded-dialog border border-border bg-panel text-foreground shadow-xl"
	>
		{#snippet header()}
			<div class="grid gap-3 pb-3">
				<h2 id="{formId}-manage-title" class="pr-8 text-xl">{m.gallery_contacts_dialog_title()}</h2>
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
			</div>
		{/snippet}
		{#snippet content()}
			{#if directory.length === 0}
				<p class="py-8 text-center text-sm text-muted-foreground">
					{pickerQuery.trim()
						? m.contacts_no_match({ query: pickerQuery.trim() })
						: m.contacts_empty()}
				</p>
			{:else}
				<ul class="grid gap-1.5">
					{#each directory as item (item.id)}
						<li
							class="flex min-w-0 items-center gap-3 rounded-xl border border-border bg-white p-2.5"
						>
							<ContactIdentity contact={item} showPhone={false} />
							<Switch
								checked={wanted.get(item.id) ?? linkedIds.has(item.id)}
								disabled={applying}
								aria-label={m.gallery_contacts_link_named({ name: item.name })}
								onchange={(checked) => want(item.id, checked)}
							/>
						</li>
					{/each}
				</ul>
			{/if}
		{/snippet}
		{#snippet actions(dismiss)}
			<div class="flex flex-wrap justify-end gap-2 border-t border-border pt-3">
				<Button variant="outline" disabled={applying} onclick={dismiss}>{m.common_cancel()}</Button>
				<Button
					disabled={applying}
					onclick={() => (wanted.size === 0 ? dismiss() : void applyWanted(dismiss))}
				>
					{#if applying}
						<LoaderCircle
							size={16}
							class="animate-spin motion-reduce:animate-none"
							aria-hidden="true"
						/>
					{/if}
					{m.action_done()}
				</Button>
			</div>
		{/snippet}
	</Dialog>
{/if}
