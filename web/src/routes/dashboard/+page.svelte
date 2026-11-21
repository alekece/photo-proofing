<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onMount, tick } from 'svelte';
	import { SvelteMap, SvelteSet } from 'svelte/reactivity';
	import { afterNavigate, beforeNavigate, goto } from '$app/navigation';
	import { navigating } from '$app/state';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import Placeholder from '#lib/components/Placeholder.svelte';
	import EmptyWorkspace from '#lib/components/EmptyWorkspace.svelte';
	import GalleryCard from '#lib/components/GalleryCard.svelte';
	import NewGalleryDialog from '#lib/components/NewGalleryDialog.svelte';
	import QuickEditGalleriesDialog, {
		type QuickGalleryEdit
	} from '#lib/components/QuickEditGalleriesDialog.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import ConfirmDialog from '#lib/components/ui/ConfirmDialog/ConfirmDialog.svelte';
	import SelectActions from '#lib/components/ui/SelectActions/SelectActions.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import ToolbarMenu from '#lib/components/ui/ToolbarMenu/ToolbarMenu.svelte';
	import WorkspaceHeading from '#lib/components/ui/WorkspaceHeading/WorkspaceHeading.svelte';
	import WorkspaceToolbar from '#lib/components/ui/WorkspaceToolbar/WorkspaceToolbar.svelte';
	import { ArrowUpDown, Pencil, Plus, Star, Trash2 } from '@lucide/svelte';
	import GalleryStateFilter from '#lib/components/GalleryStateFilter.svelte';
	import {
		deleteGallery,
		updateGallery,
		type GalleryChanges,
		type GalleryOverview
	} from '#lib/gallery/api.js';
	import { ApiError } from '#lib/http.js';
	import {
		getGalleryGridReturn,
		type GalleryGridSnapshot
	} from '#lib/gallery/dashboardNavigation.svelte.js';
	import type { GallerySort, GalleryState } from '#lib/gallery/schemas.js';
	import { ALL_STATES, DEFAULT_STATES, sameStates } from '#lib/gallery/states.js';
	import { galleryStatus } from '#lib/gallery/format.js';
	import { getNotifications } from '#lib/gallery/notifications.svelte.js';
	import type { PageData, Snapshot } from './$types';

	let { data }: { data: PageData } = $props();
	const notifications = getNotifications();
	const gridReturn = getGalleryGridReturn();
	let bulkBusy = $state(false);

	const gridClass =
		'workspace-inset grid grid-cols-2 gap-2.5 sm:grid-cols-[repeat(auto-fill,minmax(230px,1fr))] sm:gap-5';

	let dialogOpen = $state(false);
	let quickEditOpen = $state(false);
	let deleteConfirmOpen = $state(false);
	let query = $state('');
	let resultsHeight = $state(0);
	let selectionMode = $state(false);
	// Only a same-page navigation (changing the sort) shows the loading cards.
	let loading = $derived(
		navigating.from?.route.id === '/dashboard' && navigating.to?.route.id === '/dashboard'
	);
	let selected = $state<GalleryState[]>([...DEFAULT_STATES]);
	let localGalleries = $derived<GalleryOverview[] | null>(data.galleries);
	export const snapshot: Snapshot<GalleryGridSnapshot> = {
		capture: () => ({ query, selected: [...selected], scrollY: window.scrollY }),
		restore: (view) => {
			query = view.query;
			selected = view.selected;
			void tick().then(() => window.scrollTo(0, view.scrollY));
		}
	};
	const sorts = $derived<{ value: GallerySort; label: string }[]>([
		{ value: 'updated', label: m.sort_updated() },
		{ value: 'created', label: m.sort_created() },
		{ value: 'oldest', label: m.sort_oldest() },
		{ value: 'title', label: m.sort_title() }
	]);

	function compareGalleries(
		left: GalleryOverview,
		right: GalleryOverview,
		sort: GallerySort
	): number {
		if (Boolean(left.favorite) !== Boolean(right.favorite)) {
			return left.favorite ? -1 : 1;
		}

		switch (sort) {
			case 'created': {
				const diff = right.created_at.localeCompare(left.created_at);
				if (diff !== 0) return diff;
				break;
			}
			case 'oldest': {
				const diff = left.created_at.localeCompare(right.created_at);
				if (diff !== 0) return diff;
				break;
			}
			case 'title': {
				const leftTitle = left.title?.toLowerCase() ?? null;
				const rightTitle = right.title?.toLowerCase() ?? null;
				if (leftTitle === null && rightTitle !== null) return 1;
				if (leftTitle !== null && rightTitle === null) return -1;
				if (leftTitle !== null && rightTitle !== null) {
					const diff = leftTitle.localeCompare(rightTitle);
					if (diff !== 0) return diff;
				}
				break;
			}
			default: {
				const diff = right.updated_at.localeCompare(left.updated_at);
				if (diff !== 0) return diff;
				break;
			}
		}

		return left.id.localeCompare(right.id);
	}

	// A stable sort snapshot captured once per navigation (a fresh `data.galleries`/`data.sort`):
	// pinning and draft edits never move a card under the pointer. Only a refresh or an explicit
	// sort action re-sorts, because those are the only actions that change this derivation's inputs.
	let sortedIds = $derived.by(() => {
		if (!data.galleries) return [];

		return [...data.galleries].sort((a, b) => compareGalleries(a, b, data.sort)).map((g) => g.id);
	});

	const selectedIds = new SvelteSet<string>();
	// Quick edits are previewed on the card but never sent until Apply.
	const draftById = new SvelteMap<string, GalleryChanges>();

	function withDraft(gallery: GalleryOverview): GalleryOverview {
		const changes = draftById.get(gallery.id);

		return changes ? { ...gallery, ...changes } : gallery;
	}

	let all = $derived.by(() => {
		if (!localGalleries) return [];

		const byId = new Map(localGalleries.map((g) => [g.id, g]));

		return sortedIds
			.map((id) => byId.get(id))
			.filter((g): g is GalleryOverview => g !== undefined)
			.map(withDraft);
	});

	let counts = $derived(
		Object.fromEntries(
			ALL_STATES.map((state) => [
				state,
				all.filter((g) => galleryStatus(g.states) === state).length
			])
		) as Record<GalleryState, number>
	);
	// Plain substring match on the displayed title; never a regular expression.
	let matching = $derived.by(() => {
		const needle = query.trim().toLowerCase();

		return all.filter((gallery) =>
			(gallery.title ?? m.gallery_untitled()).toLowerCase().includes(needle)
		);
	});
	let visible = $derived(
		matching.filter((gallery) => selected.includes(galleryStatus(gallery.states)))
	);

	// Prune selected IDs (and their drafts) whenever search, status filters, or deletions hide or
	// remove cards.
	$effect(() => {
		const visibleSet = new Set(visible.map((g) => g.id));

		for (const id of [...selectedIds]) {
			if (!visibleSet.has(id)) {
				selectedIds.delete(id);
				draftById.delete(id);
			}
		}
	});

	let selectedGalleries = $derived(visible.filter((g) => selectedIds.has(g.id)));
	let allSelectedFavorited = $derived(
		selectedGalleries.length > 0 && selectedGalleries.every((g) => g.favorite)
	);
	let hasDraft = $derived([...selectedIds].some((id) => draftById.has(id)));

	function toggleSelect(id: string) {
		selectionMode = true;

		if (selectedIds.has(id)) {
			// Deselecting a card discards its draft too: a card that isn't selected has no
			// Apply/Cancel path, so leaving its staged change behind would orphan it.
			selectedIds.delete(id);
			draftById.delete(id);
		} else selectedIds.add(id);
	}

	function clearSelection() {
		for (const id of selectedIds) draftById.delete(id);
		selectedIds.clear();
		selectionMode = false;
	}

	function clearSelectedOnly() {
		for (const id of selectedIds) draftById.delete(id);
		selectedIds.clear();
	}

	let hasOpenDialog = $derived(
		dialogOpen ||
			(quickEditOpen && selectedGalleries.length > 0) ||
			(deleteConfirmOpen && selectedGalleries.length > 0)
	);

	function handleWindowKeydown(event: KeyboardEvent) {
		if (
			event.key === 'Escape' &&
			!event.defaultPrevented &&
			selectionMode &&
			!hasOpenDialog &&
			!document.querySelector('[data-slot="popover"] > button[aria-expanded="true"]')
		) {
			event.preventDefault();
			clearSelection();
		}
	}

	function mergeUpdated(
		updated: Omit<
			GalleryOverview,
			'photo_count' | 'favorite_count' | 'final_count' | 'downloaded_count'
		>
	) {
		if (!localGalleries) return;

		localGalleries = localGalleries.map((item) =>
			item.id === updated.id ? { ...item, ...updated } : item
		);
	}

	// Drop unchanged fields so undoing a quick edit does not leave a no-op draft.
	function stageChanges(id: string, changes: GalleryChanges) {
		const applied = localGalleries?.find((g) => g.id === id);

		if (!applied) return;

		const merged: GalleryChanges = { ...draftById.get(id), ...changes };

		for (const key of Object.keys(merged) as (keyof GalleryChanges)[]) {
			if (merged[key] === applied[key]) delete merged[key];
		}

		if (Object.keys(merged).length === 0) draftById.delete(id);
		else draftById.set(id, merged);
	}

	function toggleBulkFavorite() {
		const favorite = !allSelectedFavorited;
		const targets = selectedGalleries.filter((gallery) => Boolean(gallery.favorite) !== favorite);

		void persistChanges(
			targets.map(({ id }) => [id, { favorite }]),
			false
		);
	}

	// The dialog remains the editor; it only hands back edits for the page to stage as a preview.
	async function saveQuickEdits(edits: QuickGalleryEdit[]): Promise<string | null> {
		for (const edit of edits) {
			stageChanges(edit.id, edit.changes);
		}

		return null;
	}

	function applyChanges() {
		void persistChanges(
			[...draftById.entries()].filter(([id]) => selectedIds.has(id)),
			true
		);
	}

	async function persistChanges(entries: [string, GalleryChanges][], clearDraft: boolean) {
		if (bulkBusy || entries.length === 0) return;

		bulkBusy = true;

		try {
			let firstError: string | null = null;

			for (const [id, changes] of entries) {
				try {
					const updated = await updateGallery(id, changes);

					mergeUpdated(updated);
					if (clearDraft) {
						draftById.delete(id);
						selectedIds.delete(id);
					}
				} catch (cause) {
					if (cause instanceof ApiError && cause.status === 401) {
						await goto('/login', { refreshAll: true });

						return;
					}

					// Keep the draft and the selection for the failed id so the photographer can retry.
					firstError ??= cause instanceof Error ? cause.message : m.api_error_generic();
				}
			}

			if (firstError) toasts.error(firstError);
			if (selectedIds.size === 0) selectionMode = false;
		} finally {
			bulkBusy = false;
		}
	}

	async function confirmBulkDelete() {
		const targets = [...selectedGalleries];
		let firstError: string | null = null;

		for (const item of targets) {
			try {
				await deleteGallery(item.id);
				selectedIds.delete(item.id);
				draftById.delete(item.id);
				if (localGalleries) {
					localGalleries = localGalleries.filter((g) => g.id !== item.id);
				}
			} catch (cause) {
				if (cause instanceof ApiError && cause.status === 401) {
					await goto('/login', { refreshAll: true });

					return;
				}

				firstError ??= cause instanceof Error ? cause.message : m.api_error_generic();
			}
		}

		if (firstError) {
			throw new Error(firstError);
		}

		deleteConfirmOpen = false;
		if (selectedIds.size === 0) selectionMode = false;
	}

	let hiddenMatches = $derived(matching.length - visible.length);
	let activity = $derived(createSessionActivity(data.idleTimeoutMs));
	beforeNavigate(({ to }) => {
		if (gridReturn && to?.route.id === '/dashboard/[id]') {
			gridReturn.view = { snapshot: snapshot.capture(), sort: data.sort };
		}
	});
	afterNavigate(({ from, type }) => {
		void activity();
		const view = gridReturn?.view;

		if (type === 'link' && from?.route.id === '/dashboard/[id]' && view) {
			snapshot.restore(view.snapshot);
		}
	});
	onMount(() => listenForActivity(() => activity()));
	function changeSort(sort: GallerySort) {
		void goto(`?sort=${sort}`, { reset: false });
	}
	function clearAll() {
		query = '';
		selected = [...DEFAULT_STATES];
	}
</script>

<svelte:window onkeydown={handleWindowKeydown} />
<svelte:head><title>{m.dashboard_head()} — PhotoProofing</title></svelte:head>
<main class="flex flex-1 flex-col">
	<div class="grid items-end gap-3 workspace-inset py-4 sm:grid-cols-[1fr_auto] sm:gap-6 sm:py-6">
		<WorkspaceHeading
			title={m.nav_galleries()}
			count={data.galleries === null
				? undefined
				: m.dashboard_total_galleries({ count: visible.length })}
		/>
		<Button class="justify-self-start sm:justify-self-auto" onclick={() => (dialogOpen = true)}
			>{m.dashboard_new_gallery()}</Button
		>
	</div>

	{#if data.galleries !== null}
		{@const none = all.length === 0}
		<WorkspaceToolbar
			class="mx-2.5 mb-4 sm:mx-8 sm:mb-[1.375rem]"
			search={{
				label: m.dashboard_search(),
				value: query,
				disabled: none,
				oninput: (value) => (query = value)
			}}
			resultsHeight={loading || visible.length === 0 ? 0 : resultsHeight}
			onclear={query.trim() || !sameStates(selected, DEFAULT_STATES) ? clearAll : undefined}
		>
			{#snippet filters()}
				<GalleryStateFilter
					{selected}
					{counts}
					disabled={none}
					onchange={(next) => (selected = next)}
				/>
			{/snippet}
			{#snippet sort()}
				<ToolbarMenu
					label={m.sort_by()}
					summary={sorts.find(({ value }) => value === data.sort)?.label ?? m.sort_by()}
					icon={ArrowUpDown}
					value={data.sort}
					options={sorts}
					disabled={none}
					onchange={changeSort}
				/>
			{/snippet}
		</WorkspaceToolbar>
	{/if}
	{#if loading}
		<p class="sr-only" role="status">{m.dashboard_loading()}</p>
		<ul class={gridClass} aria-busy="true">
			{#each [1, 2, 3, 4] as n (n)}
				<li class="aspect-[4/3] overflow-hidden rounded-card border border-border">
					<Placeholder background="sheet" loading />
				</li>
			{/each}
		</ul>
	{:else if localGalleries === null}
		<p role="alert" class="px-3.5 sm:px-8">
			{m.dashboard_load_failed()}
		</p>
	{:else if all.length === 0}
		<EmptyWorkspace />
	{:else}
		<ul bind:clientHeight={resultsHeight} class="{gridClass} mb-12">
			{#each visible as gallery (gallery.id)}
				<GalleryCard
					{gallery}
					{query}
					unread={(notifications?.unreadFor(gallery.id).length ?? 0) > 0}
					selected={selectedIds.has(gallery.id)}
					selecting={selectedIds.size > 0}
					onselect={() => toggleSelect(gallery.id)}
				/>
			{:else}
				<li class="col-span-full px-0 pt-14 pb-10 text-center text-muted-foreground">
					<h2 class="mb-1.5 text-2xl leading-[1.2] text-foreground">
						{query.trim() !== '' && matching.length === 0
							? m.dashboard_no_match({ query: query.trim() })
							: m.dashboard_no_status_match()}
					</h2>
					<p>
						{#if hiddenMatches > 0}
							{m.dashboard_hidden_matches({ count: hiddenMatches })}
						{:else}
							{m.dashboard_check_spelling()}
						{/if}
					</p>
					<Button variant="outline" class="mt-3.5" onclick={clearAll}
						>{m.dashboard_clear_filters()}</Button
					>
				</li>
			{/each}
			{#if visible.length > 0}
				<li class="min-h-36">
					<button
						type="button"
						class="group grid size-full cursor-pointer content-center justify-items-center gap-2.5 rounded-card border-[1.5px] border-dashed border-[#cdbda9] bg-[radial-gradient(120%_80%_at_50%_30%,#faf5ee,transparent_70%)] text-center text-muted-foreground transition-colors duration-250 hover:border-danger hover:bg-[#f8f1e8]"
						onclick={() => (dialogOpen = true)}
					>
						<span
							class="relative mb-2 grid h-16 w-[5.375rem] place-items-center text-[#a8977f] transition duration-300 group-hover:scale-[0.88] group-hover:text-danger"
						>
							<i class="absolute top-0 left-0 size-3.5 border-t-2 border-l-2"></i>
							<i class="absolute top-0 right-0 size-3.5 border-t-2 border-r-2"></i>
							<i class="absolute bottom-0 left-0 size-3.5 border-b-2 border-l-2"></i>
							<i class="absolute right-0 bottom-0 size-3.5 border-r-2 border-b-2"></i>
							<Plus size={30} strokeWidth={1.4} aria-hidden="true" />
						</span>
						<strong
							class="px-2 font-serif text-[1.0625rem] leading-[1.2] font-normal text-foreground italic sm:text-[1.375rem]"
							>{m.dashboard_next_client_line1()} <br />{m.dashboard_next_client_line2()}</strong
						>
					</button>
				</li>
			{/if}
		</ul>
	{/if}
</main>
{#if selectionMode}
	<SelectActions
		count={selectedGalleries.length}
		total={visible.length}
		rail="dashboard"
		onselectall={() => {
			for (const gallery of visible) selectedIds.add(gallery.id);
		}}
		onunselectall={clearSelectedOnly}
		onexit={clearSelection}
	>
		{#snippet actions()}
			<Button
				variant="outline"
				size="sm"
				class="min-h-9! gap-1.5 px-3!"
				disabled={bulkBusy}
				aria-label={allSelectedFavorited
					? m.dashboard_bulk_unfavorite()
					: m.dashboard_bulk_favorite()}
				onclick={toggleBulkFavorite}
			>
				<Star
					size={15}
					fill={allSelectedFavorited ? 'currentColor' : 'none'}
					class={allSelectedFavorited ? 'text-[#b8792b]' : ''}
					aria-hidden="true"
				/>
				<span class="max-sm:sr-only"
					>{allSelectedFavorited
						? m.dashboard_bulk_unfavorite()
						: m.dashboard_bulk_favorite()}</span
				>
			</Button>
			<Button
				variant="outline"
				size="sm"
				class="min-h-9! gap-1.5 px-3!"
				disabled={bulkBusy}
				aria-label={m.dashboard_bulk_quick_edit()}
				onclick={() => (quickEditOpen = true)}
			>
				<Pencil size={15} aria-hidden="true" />
				<span class="max-sm:sr-only">{m.dashboard_bulk_quick_edit()}</span>
			</Button>
			<Button
				size="sm"
				class="min-h-9! px-3!"
				disabled={bulkBusy || !hasDraft}
				onclick={applyChanges}>{m.dashboard_bulk_apply()}</Button
			>
			<Button
				variant="destructive"
				size="sm"
				class="min-h-9! gap-1.5 px-3!"
				disabled={bulkBusy}
				aria-label={m.action_delete()}
				onclick={() => (deleteConfirmOpen = true)}
			>
				<Trash2 size={15} aria-hidden="true" />
				<span class="max-sm:sr-only">{m.action_delete()}</span>
			</Button>
		{/snippet}
	</SelectActions>
{/if}
{#if dialogOpen}
	<NewGalleryDialog onclose={() => (dialogOpen = false)} />
{/if}
{#if quickEditOpen && selectedGalleries.length > 0}
	<QuickEditGalleriesDialog
		galleries={selectedGalleries}
		onsave={saveQuickEdits}
		onclose={() => (quickEditOpen = false)}
	/>
{/if}
{#if deleteConfirmOpen && selectedGalleries.length > 0}
	<ConfirmDialog
		title={m.dashboard_bulk_delete_title({ count: selectedGalleries.length })}
		confirmLabel={m.action_delete()}
		destructive
		onconfirm={confirmBulkDelete}
		onclose={() => (deleteConfirmOpen = false)}
	>
		{#snippet description()}
			<p class="text-sm text-muted-foreground">{m.dashboard_bulk_delete_body()}</p>
		{/snippet}
	</ConfirmDialog>
{/if}
