<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onMount, tick } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { cubicIn } from 'svelte/easing';
	import { slide } from 'svelte/transition';
	import { afterNavigate, beforeNavigate } from '$app/navigation';
	import { ArrowUpDown, Check, ChevronDown, Pencil, Plus, Trash2, Users } from '@lucide/svelte';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import ContactIdentity from '#lib/components/ContactIdentity.svelte';
	import ActionButton from '#lib/components/ui/ActionButton/ActionButton.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import Checkbox from '#lib/components/ui/Checkbox/Checkbox.svelte';
	import ConfirmDialog from '#lib/components/ui/ConfirmDialog/ConfirmDialog.svelte';
	import EmptyState from '#lib/components/ui/EmptyState/EmptyState.svelte';
	import Input from '#lib/components/ui/Input/Input.svelte';
	import Label from '#lib/components/ui/Label/Label.svelte';
	import SelectActions from '#lib/components/ui/SelectActions/SelectActions.svelte';
	import ToolbarMenu from '#lib/components/ui/ToolbarMenu/ToolbarMenu.svelte';
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

	type Draft = {
		form: string;
		name: string;
		email: string;
		phone: string;
		notes: string;
		error: string;
		invalid: 'name' | 'email' | null;
		created: boolean;
	};
	type Panel =
		| { mode: 'new'; draft: Draft }
		| { mode: 'details'; id: string }
		| { mode: 'edit'; id: string; draft: Draft; original: Contact };

	let { data }: { data: PageData } = $props();

	const formId = $props.id();
	let draftNumber = 0;
	const writes = new SvelteSet<string | null>();
	const selectedIds = new SvelteSet<string>();
	// svelte-ignore state_referenced_locally
	let contacts = $state<Contact[]>(data.contacts ? [...data.contacts] : []);
	let panel = $state<Panel | null>(null);
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
		matching.filter(({ id }) => !writes.has(id) && !(panel?.mode === 'edit' && panel.id === id))
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

		const current = panel;
		if (!current || current.mode === 'new' || visibleIds.has(current.id)) return;

		const row = document.getElementById(rowId(current.id));
		const restoreFocus = row?.contains(document.activeElement);
		panel = null;
		if (restoreFocus) void focusFallback();
	});

	function rowId(id: string): string {
		return `${formId}-row-${id}`;
	}

	async function focusFallback(): Promise<void> {
		await tick();
		const search = root.querySelector<HTMLInputElement>('input[type=search]');
		(search && !search.disabled ? search : newButton)?.focus();
	}

	function openEditor(contact?: Contact): void {
		if (contact) selectedIds.delete(contact.id);

		const draft: Draft = {
			form: `${formId}-draft-${++draftNumber}`,
			name: contact?.name ?? '',
			email: contact?.email ?? '',
			phone: contact?.phone ?? '',
			notes: contact?.notes ?? '',
			error: '',
			invalid: null,
			created: false
		};

		panel = contact
			? { mode: 'edit', id: contact.id, draft, original: contact }
			: { mode: 'new', draft };
		void tick().then(() => document.getElementById(`${draft.form}-name`)?.focus());
	}

	function toggleDetails(id: string): void {
		panel = panel && panel.mode !== 'new' && panel.id === id ? null : { mode: 'details', id };
	}

	async function cancelEditor(): Promise<void> {
		if (!panel || panel.mode === 'details') return;

		const id = panel.mode === 'edit' ? panel.id : null;
		panel = id === null ? null : { mode: 'details', id };
		await tick();

		if (id === null) newButton?.focus();
		else document.getElementById(`${rowId(id)}-edit`)?.focus();
	}

	function markInvalid(draft: Draft, field: 'name' | 'email', input: HTMLInputElement): void {
		if (input.form?.querySelector(':invalid:not(fieldset)') !== input) return;

		draft.invalid = field;
		draft.error = field === 'name' ? m.contacts_name_required() : m.contacts_email_invalid();
		input.focus();
	}

	function values(draft: Draft) {
		return {
			name: draft.name.trim(),
			email: draft.email.trim(),
			phone: draft.phone.trim() || null,
			notes: draft.notes.trim() || null
		};
	}

	function changes(draft: Draft, original: Contact): ContactChangesInput {
		const { name, email, phone, notes } = values(draft);

		return {
			...(name !== original.name && { name }),
			...(email.toLowerCase() !== original.email && { email, expected_email: original.email }),
			...(phone !== original.phone && { phone }),
			...(notes !== original.notes && { notes })
		};
	}

	async function submitContact(draft: Draft): Promise<boolean> {
		const editor = panel;
		if (!editor || editor.mode === 'details' || editor.draft !== draft) return false;

		const id = editor.mode === 'edit' ? editor.id : null;
		if (writes.has(id)) return false;

		if (!draft.name.trim()) {
			draft.invalid = 'name';
			draft.error = m.contacts_name_required();
			document.getElementById(`${draft.form}-name`)?.focus();

			return false;
		}

		draft.error = '';
		draft.invalid = null;
		writes.add(id);

		try {
			const saved =
				editor.mode === 'new'
					? await createContact(values(draft))
					: await updateContact(editor.id, changes(draft, editor.original));
			if (disposed) return false;

			contacts =
				id === null
					? [...contacts, saved]
					: contacts.map((contact) => (contact.id === id ? saved : contact));

			if (panel !== editor) return false;

			if (editor.mode === 'edit') editor.original = saved;
			else draft.created = true;
			draft.name = saved.name;
			draft.email = saved.email;
			draft.phone = saved.phone ?? '';
			draft.notes = saved.notes ?? '';

			return true;
		} catch (error) {
			if (!disposed && panel === editor) {
				draft.error =
					error instanceof ApiError && (error.status === 409 || error.status === 422)
						? error.message
						: m.contacts_save_failed();
			}

			return false;
		} finally {
			writes.delete(id);
		}
	}

	function finishNew(draft: Draft): void {
		if (panel?.mode !== 'new' || panel.draft !== draft) return;

		const restoreFocus = document.getElementById(`${formId}-new`)?.contains(document.activeElement);
		panel = null;
		if (restoreFocus) void tick().then(() => newButton?.focus());
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

{#snippet fields(draft: Draft, busy: boolean)}
	<form
		id={draft.form}
		onsubmit={(event) => {
			event.preventDefault();
			if (panel?.mode !== 'details' && panel?.draft === draft) saveButton?.activate();
		}}
	>
		<fieldset disabled={busy} class="grid min-w-0 gap-4">
			<div class="grid gap-4 sm:grid-cols-2">
				<div class="grid gap-1.5">
					<Label for="{draft.form}-name">{m.common_name()}</Label>
					<Input
						id="{draft.form}-name"
						bind:value={draft.name}
						maxlength={120}
						required
						aria-invalid={draft.invalid === 'name' || undefined}
						aria-describedby={draft.invalid === 'name' ? `${draft.form}-error` : undefined}
						oninvalid={(event) => {
							event.preventDefault();
							markInvalid(draft, 'name', event.currentTarget);
						}}
					/>
				</div>
				<div class="grid gap-1.5">
					<Label for="{draft.form}-email">{m.login_email()}</Label>
					<Input
						id="{draft.form}-email"
						type="email"
						bind:value={draft.email}
						maxlength={254}
						required
						aria-invalid={draft.invalid === 'email' || undefined}
						aria-describedby={draft.invalid === 'email' ? `${draft.form}-error` : undefined}
						oninvalid={(event) => {
							event.preventDefault();
							markInvalid(draft, 'email', event.currentTarget);
						}}
					/>
				</div>
			</div>
			<div class="grid gap-1.5 sm:max-w-xs">
				<Label for="{draft.form}-phone">{m.contacts_phone()}</Label>
				<Input id="{draft.form}-phone" type="tel" bind:value={draft.phone} maxlength={40} />
			</div>
			<div class="grid gap-1.5">
				<Label for="{draft.form}-notes">{m.contacts_notes()}</Label>
				<textarea
					id="{draft.form}-notes"
					bind:value={draft.notes}
					rows={3}
					maxlength={2000}
					class="cn-input min-h-22 w-full min-w-0 resize-y py-2 focus-visible:border-foreground"
				></textarea>
			</div>
		</fieldset>
		{#if draft.error}
			<p id="{draft.form}-error" role="alert" class="mt-3 text-sm text-danger">{draft.error}</p>
		{/if}
	</form>
{/snippet}

{#snippet actions(draft: Draft, isNew: boolean, busy: boolean, disabled = busy)}
	{#key draft}
		<Button variant="outline" disabled={busy} onclick={() => void cancelEditor()}>
			{m.common_cancel()}
		</Button>
		<ActionButton
			bind:this={saveButton}
			form={draft.form}
			{disabled}
			labels={isNew
				? {
						idle: m.contacts_create(),
						pending: m.contacts_creating(),
						succeeded: m.contacts_created()
					}
				: { idle: m.studio_save(), pending: m.studio_saving(), succeeded: m.studio_saved() }}
			onaction={() => submitContact(draft)}
			onsettled={isNew ? () => finishNew(draft) : undefined}
		/>
	{/key}
{/snippet}

<main bind:this={root} class="flex flex-1 flex-col workspace-inset pt-4 pb-24 sm:pt-6">
	<div class="mb-4 flex flex-wrap items-end justify-between gap-3 sm:mb-6">
		<div>
			<h1 class="text-[1.875rem] leading-[1.08] sm:text-[2.5rem]">{m.contacts_title()}</h1>
			{#if !loadFailed && contacts.length > 0}
				<p class="mt-1 text-sm text-muted-foreground">
					{m.contacts_total({ count: contacts.length })}
				</p>
			{/if}
		</div>
		<Button bind:ref={newButton} onclick={() => openEditor()} disabled={loadFailed}>
			<Plus size={16} aria-hidden="true" />
			{m.contacts_new()}
		</Button>
	</div>

	<WorkspaceToolbar
		class="mb-4 w-full sm:mb-6"
		search={{
			label: m.contacts_search(),
			value: query,
			disabled: loadFailed || contacts.length === 0,
			oninput: (value) => (query = value)
		}}
		resultsHeight={matching.length ? resultsHeight : 0}
		onclear={query.trim() ? () => (query = '') : undefined}
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
		</EmptyState>
	{:else}
		{#if panel?.mode === 'new'}
			<section
				id="{formId}-new"
				aria-labelledby="{formId}-new-heading"
				class="mb-3 grid gap-4 rounded-card border border-border bg-panel p-4 sm:p-5"
				transition:slide={{ duration: duration(200) }}
			>
				<h2 id="{formId}-new-heading" class="text-xl">{m.contacts_new_heading()}</h2>
				{@render fields(panel.draft, writes.has(null) || panel.draft.created)}
				<div class="flex flex-wrap justify-end gap-2">
					{@render actions(panel.draft, true, writes.has(null))}
				</div>
			</section>
		{/if}

		{#if contacts.length === 0 && panel?.mode !== 'new'}
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
			class="grid gap-2"
		>
			{#each matching as item (item.id)}
				{@const active = panel && panel.mode !== 'new' && panel.id === item.id ? panel : null}
				{@const detailsId = `${rowId(item.id)}-details`}
				<li
					id={rowId(item.id)}
					class="min-w-0 rounded-card border border-border bg-panel"
					out:removal={item.id}
				>
					<div
						class="group/contact flex flex-wrap items-start gap-2 p-3 sm:items-center sm:gap-3 sm:p-3.5"
					>
						<label class="relative grid h-9 w-7 shrink-0 cursor-pointer place-items-center">
							<Checkbox
								aria-label={m.tile_select({ name: item.name })}
								checked={selectedIds.has(item.id)}
								disabled={writes.has(item.id) || active?.mode === 'edit'}
								onchange={(event) => {
									if (event.currentTarget.checked) selectedIds.add(item.id);
									else selectedIds.delete(item.id);
								}}
								class="peer size-5 cursor-pointer appearance-none rounded-full border border-border-strong bg-input transition-opacity checked:border-foreground checked:bg-foreground hover:bg-background focus-visible:opacity-100 disabled:cursor-not-allowed disabled:opacity-40 motion-reduce:transition-none {selection.length
									? ''
									: 'group-hover/contact:opacity-100 [@media(hover:hover)]:opacity-0'}"
							/>
							<Check
								size={14}
								aria-hidden="true"
								class="pointer-events-none absolute text-inverse opacity-0 peer-checked:opacity-100"
							/>
						</label>
						<ContactIdentity contact={item}>
							{#snippet details()}
								{#if item.galleries.length > 0}
									<div class="mt-1 flex min-w-0 flex-wrap gap-1">
										{#each item.galleries as linked (linked.id)}
											<span
												class="max-w-full truncate rounded-full border border-border bg-background px-2 py-0.5 text-xs text-muted-foreground"
											>
												{linked.title ?? m.gallery_untitled()}
											</span>
										{/each}
									</div>
								{/if}
							{/snippet}
						</ContactIdentity>
						<div class="ml-auto flex flex-initial flex-wrap justify-end gap-1 sm:gap-2">
							{#if active?.mode !== 'edit'}
								<Button
									id="{rowId(item.id)}-edit"
									variant="ghost"
									size="sm"
									aria-label={m.contacts_edit_label({ name: item.name })}
									disabled={writes.has(item.id)}
									class="min-w-11 gap-2 max-sm:px-0"
									onclick={() => openEditor(item)}
								>
									<Pencil size={16} aria-hidden="true" />
									<span class="max-sm:sr-only">{m.contacts_edit()}</span>
								</Button>
								<Button
									id="{rowId(item.id)}-toggle"
									variant="ghost"
									size="sm"
									class="min-w-11 px-0"
									aria-expanded={active !== null}
									aria-controls={detailsId}
									aria-label={active
										? m.contacts_hide_details({ name: item.name })
										: m.contacts_show_details({ name: item.name })}
									onclick={() => toggleDetails(item.id)}
								>
									<ChevronDown
										size={18}
										aria-hidden="true"
										class="transition-transform motion-reduce:transition-none {active
											? 'rotate-180'
											: ''}"
									/>
								</Button>
							{/if}
						</div>
					</div>
					<div id={detailsId} inert={!active} aria-hidden={!active}>
						{#if active}
							<div
								class="grid gap-4 border-t border-border p-3.5 sm:p-5"
								transition:slide={{ duration: duration(200) }}
							>
								{#if active.mode === 'edit'}
									{@render fields(active.draft, writes.has(item.id))}
									<div class="flex flex-wrap justify-end gap-2">
										{@render actions(
											active.draft,
											false,
											writes.has(item.id),
											writes.has(item.id) ||
												Object.keys(changes(active.draft, active.original)).length === 0
										)}
									</div>
								{:else if item.notes}
									<div class="grid gap-1.5">
										<p class="text-xs font-semibold text-muted-foreground uppercase">
											{m.contacts_notes()}
										</p>
										<p class="text-sm [overflow-wrap:anywhere] whitespace-pre-wrap">
											{item.notes}
										</p>
									</div>
								{/if}
								<div class="grid gap-2">
									<p class="text-xs font-semibold text-muted-foreground uppercase">
										{m.contacts_linked_galleries()}
									</p>
									{#if item.galleries.length === 0}
										<p class="text-sm text-muted-foreground">{m.contacts_no_galleries()}</p>
									{:else}
										<div class="flex flex-wrap gap-2">
											{#each item.galleries as linked (linked.id)}
												<a
													href="/dashboard/{encodeURIComponent(linked.id)}"
													class="inline-flex max-w-full items-center rounded-full border border-border bg-background px-3 py-1 text-xs font-medium text-foreground transition-colors hover:bg-border/60"
												>
													<span class="truncate">{linked.title ?? m.gallery_untitled()}</span>
												</a>
											{/each}
										</div>
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
			(!panel || panel.mode === 'details') &&
			!document.querySelector('[data-slot="popover"] > button[aria-expanded="true"]')}
	>
		{#snippet actions()}
			<Button
				variant="destructive"
				size="sm"
				class="min-h-9! gap-1.5"
				onclick={() => (deleteTargets = selection)}
			>
				<Trash2 size={16} aria-hidden="true" />
				<span class="max-sm:sr-only">{m.action_delete()}</span>
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
