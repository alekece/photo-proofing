<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onMount, tick } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { afterNavigate, beforeNavigate, goto, invalidateAll } from '$app/navigation';
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
	import ActionButton from '#lib/components/ui/ActionButton/ActionButton.svelte';
	import CoverBackdrop from '#lib/components/CoverBackdrop.svelte';
	import { thumbnailUrl } from '#lib/gallery/photos.js';
	import ConfirmDialog from '#lib/components/ui/ConfirmDialog/ConfirmDialog.svelte';
	import SelectActions from '#lib/components/ui/SelectActions/SelectActions.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import ToolbarMenu from '#lib/components/ui/ToolbarMenu/ToolbarMenu.svelte';
	import WorkspaceHeading from '#lib/components/ui/WorkspaceHeading/WorkspaceHeading.svelte';
	import WorkspaceToolbar from '#lib/components/ui/WorkspaceToolbar/WorkspaceToolbar.svelte';
	import { ArrowUpDown, Pencil, Plus, Star, Trash2 } from '@lucide/svelte';
	import GalleryStateFilter from '#lib/components/GalleryStateFilter.svelte';
	import { deleteGallery, updateGallery, type GalleryOverview } from '#lib/gallery/api.js';
	import { ApiError } from '#lib/http.js';
	import {
		getGalleryGridReturn,
		type GalleryGridSnapshot
	} from '#lib/gallery/dashboardNavigation.svelte.js';
	import {
		galleryStateSchema,
		parseOrNull,
		type GallerySort,
		type GalleryState
	} from '#lib/gallery/schemas.js';
	import { ALL_STATES } from '#lib/gallery/states.js';
	import { galleryStatus } from '#lib/gallery/format.js';
	import { getNotifications } from '#lib/gallery/notifications.svelte.js';
	import type { PageData, Snapshot } from './$types';

	let { data }: { data: PageData } = $props();
	const notifications = getNotifications();
	const gridReturn = getGalleryGridReturn();
	let bulkBusy = $state(false);

	const gridClass =
		'workspace-inset grid grid-cols-2 gap-2.5 sm:grid-cols-[repeat(auto-fill,minmax(min(100%,18rem),1fr))] sm:gap-5';

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
	let selected = $state<GalleryState | null>(null);
	let localGalleries = $derived<GalleryOverview[] | null>(data.galleries);
	export const snapshot: Snapshot<GalleryGridSnapshot> = {
		capture: () => ({ query, selected, scrollY: window.scrollY }),
		restore: (view) => {
			query = view.query;
			// Older history entries can contain several statuses; there is no single equivalent.
			const status = Array.isArray(view.selected)
				? view.selected.length === 1
					? view.selected[0]
					: null
				: view.selected;
			selected = parseOrNull(galleryStateSchema, status);
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
	// pinning and quick edits never move a card under the pointer. Only a refresh or an explicit
	// sort action re-sorts, because those are the only actions that change this derivation's inputs.
	let sortedIds = $derived.by(() => {
		if (!data.galleries) return [];

		return [...data.galleries].sort((a, b) => compareGalleries(a, b, data.sort)).map((g) => g.id);
	});

	const selectedIds = new SvelteSet<string>();

	let all = $derived.by(() => {
		if (!localGalleries) return [];

		const byId = new Map(localGalleries.map((g) => [g.id, g]));

		return sortedIds.map((id) => byId.get(id)).filter((g): g is GalleryOverview => g !== undefined);
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
		matching.filter((gallery) => !selected || selected === galleryStatus(gallery.states))
	);

	// Hidden cards are never targets of a bulk action.
	$effect(() => {
		const visibleSet = new Set(visible.map((g) => g.id));

		for (const id of [...selectedIds]) {
			if (!visibleSet.has(id)) {
				selectedIds.delete(id);
			}
		}
	});

	let selectedGalleries = $derived(visible.filter((g) => selectedIds.has(g.id)));
	$effect(() => {
		if (selectedGalleries.length === 0) {
			quickEditOpen = false;
			deleteConfirmOpen = false;
		}
	});
	let allSelectedFavorited = $derived(
		selectedGalleries.length > 0 && selectedGalleries.every((g) => g.favorite)
	);

	function toggleSelect(id: string) {
		if (bulkBusy) return;
		selectionMode = true;

		if (selectedIds.has(id)) {
			selectedIds.delete(id);
		} else selectedIds.add(id);
	}

	function clearSelection() {
		selectedIds.clear();
		selectionMode = false;
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
			!bulkBusy &&
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

	async function toggleBulkFavorite() {
		const favorite = !allSelectedFavorited;
		const targets = selectedGalleries.filter((gallery) => Boolean(gallery.favorite) !== favorite);
		const error = await persistChanges(targets.map(({ id }) => ({ id, changes: { favorite } })));
		if (error) toasts.error(error);
		return !error;
	}

	async function persistChanges(entries: QuickGalleryEdit[]) {
		if (bulkBusy || entries.length === 0) return null;

		bulkBusy = true;

		try {
			let firstError: string | null = null;
			const failed = new SvelteSet<string>();

			for (const { id, changes } of entries) {
				try {
					const updated = await updateGallery(id, changes);

					mergeUpdated(updated);
				} catch (cause) {
					if (cause instanceof ApiError && cause.status === 401) {
						await goto('/login', { refreshAll: true });

						return null;
					}

					failed.add(id);
					firstError ??= cause instanceof Error ? cause.message : m.api_error_generic();
				}
			}

			if (firstError) {
				for (const id of selectedIds) if (!failed.has(id)) selectedIds.delete(id);
			}
			return firstError;
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
		selected = null;
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
			onclear={query.trim() || selected ? clearAll : undefined}
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
			<Button variant="outline" onclick={() => void invalidateAll()}>{m.action_retry()}</Button>
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
					liveUpdate={notifications?.lastRefresh === 'live'}
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
		disabled={bulkBusy}
		onunselectall={() => selectedIds.clear()}
		onexit={clearSelection}
	>
		{#snippet actions()}
			<ActionButton
				variant="outline"
				class="min-h-9! px-3!"
				disabled={bulkBusy || !selectedGalleries.length}
				compactOnMobile
				pendingDelayMs={150}
				labels={{
					idle: allSelectedFavorited ? m.dashboard_bulk_unfavorite() : m.dashboard_bulk_favorite(),
					pending: m.dialog_working()
				}}
				onaction={toggleBulkFavorite}
			>
				{#snippet icon()}
					<Star
						size={15}
						fill={allSelectedFavorited ? 'currentColor' : 'none'}
						class={allSelectedFavorited ? 'text-[#b8792b]' : ''}
						aria-hidden="true"
					/>
				{/snippet}
			</ActionButton>
			<Button
				variant="outline"
				size="sm"
				class="min-h-9! gap-1.5 px-3!"
				disabled={bulkBusy || !selectedGalleries.length}
				aria-label={m.dashboard_bulk_quick_edit()}
				onclick={() => (quickEditOpen = true)}
			>
				<Pencil size={15} aria-hidden="true" />
				<span class="max-sm:sr-only">{m.dashboard_bulk_quick_edit()}</span>
			</Button>
			<Button
				variant="destructive"
				size="sm"
				class="min-h-9! gap-1.5 px-3!"
				disabled={bulkBusy || !selectedGalleries.length}
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
		onsave={persistChanges}
		onclose={() => (quickEditOpen = false)}
	/>
{/if}
{#if deleteConfirmOpen && selectedGalleries.length > 0}
	<ConfirmDialog
		title={m.dashboard_bulk_delete_title({ count: selectedGalleries.length })}
		confirmLabel={m.action_delete()}
		destructive
		pendingDelayMs={200}
		onconfirm={confirmBulkDelete}
		onclose={() => (deleteConfirmOpen = false)}
	>
		{#snippet visual()}
			<div class="relative h-20 w-24 self-center" aria-hidden="true">
				{#each selectedGalleries as gallery, i (gallery.id)}
					{@const spread =
						selectedGalleries.length > 1 ? i / (selectedGalleries.length - 1) - 0.5 : 0}
					<div
						class="absolute inset-2 overflow-hidden border-2 border-white bg-panel shadow-sm"
						style:transform="translateX({spread * 12}px) rotate({spread * 18}deg)"
					>
						{#if gallery.hero_id}
							<img
								src={thumbnailUrl(gallery.id, gallery.hero_id)}
								alt=""
								class="size-full object-cover"
							/>
						{:else}
							<CoverBackdrop title={gallery.title} compact />
						{/if}
					</div>
				{/each}
			</div>
		{/snippet}
		{#snippet description()}
			<p class="text-sm text-muted-foreground">{m.dashboard_bulk_delete_body()}</p>
		{/snippet}
	</ConfirmDialog>
{/if}
