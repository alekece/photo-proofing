<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onMount } from 'svelte';
	import { SvelteMap, SvelteSet } from 'svelte/reactivity';
	import { afterNavigate, goto } from '$app/navigation';
	import { navigating } from '$app/state';
	import { authRequest } from '#lib/auth.js';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import Placeholder from '#lib/components/Placeholder.svelte';
	import AppHeader from '#lib/components/AppHeader.svelte';
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
	import { Pencil, Plus, Search, Star, Trash2 } from '@lucide/svelte';
	import GalleryStateFilter from '#lib/components/GalleryStateFilter.svelte';
	import GallerySortMenu from '#lib/components/GallerySortMenu.svelte';
	import {
		deleteGallery,
		updateGallery,
		type GalleryChanges,
		type GalleryOverview
	} from '#lib/gallery/api.js';
	import { ApiError } from '#lib/http.js';
	import type { GallerySort, GalleryState } from '#lib/gallery/schemas.js';
	import { ALL_STATES, DEFAULT_STATES } from '#lib/gallery/states.js';
	import { galleryStatus } from '#lib/gallery/format.js';
	import { getNotifications, notificationBadge } from '#lib/gallery/notifications.svelte.js';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	const notifications = getNotifications();
	let pending = $state(false);
	let bulkBusy = $state(false);
	let message = $state('');

	const gridClass =
		'grid grid-cols-2 gap-2.5 px-2.5 sm:grid-cols-[repeat(auto-fill,minmax(230px,1fr))] sm:gap-5 sm:px-8';

	let dialogOpen = $state(false);
	let quickEditOpen = $state(false);
	let deleteConfirmOpen = $state(false);
	let query = $state('');
	let selectionMode = $state(false);
	// Only a same-page navigation (changing the sort) shows the loading cards.
	let loading = $derived(navigating.to?.route.id === '/dashboard');
	let selected = $state<GalleryState[]>([...DEFAULT_STATES]);
	let localGalleries = $derived<GalleryOverview[] | null>(data.galleries);

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
	// Staged bulk favorite/quick-edit changes: previewed on the card but never sent until Apply.
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
	// Archived galleries stay out of the totals, as in the prototype.
	let totals = $derived.by(() => {
		const live = all.filter((gallery) => galleryStatus(gallery.states) !== 'archived');

		return {
			galleries: live.length,
			photos: live.reduce((sum, gallery) => sum + gallery.photo_count, 0)
		};
	});
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

	// Merges staged changes for a gallery, dropping any key that now matches the server value so an
	// undone edit (e.g. toggling favorite back) clears the draft instead of leaving a no-op entry.
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
		if (selectedGalleries.length === 0) return;

		const target = !allSelectedFavorited;

		for (const gallery of selectedGalleries) {
			stageChanges(gallery.id, { favorite: target });
		}
	}

	// The dialog remains the editor; it only hands back edits for the page to stage as a preview.
	async function saveQuickEdits(edits: QuickGalleryEdit[]): Promise<string | null> {
		for (const edit of edits) {
			stageChanges(edit.id, edit.changes);
		}

		return null;
	}

	function cancelChanges() {
		for (const id of selectedIds) draftById.delete(id);
	}

	async function applyChanges() {
		const entries = [...draftById.entries()].filter(([id]) => selectedIds.has(id));

		if (bulkBusy || entries.length === 0) return;

		bulkBusy = true;

		try {
			let firstError: string | null = null;

			for (const [id, changes] of entries) {
				try {
					const updated = await updateGallery(id, changes);

					mergeUpdated(updated);
					draftById.delete(id);
					selectedIds.delete(id);
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
	let galleriesWithNews = $derived(
		new Set(
			(notifications?.items ?? [])
				.filter((item) => item.read_at === null)
				.map((item) => item.gallery_id)
		).size
	);
	let greeting = $derived.by(() => {
		const hour = new Date().getHours();

		return hour < 12
			? m.dashboard_morning()
			: hour < 18
				? m.dashboard_afternoon()
				: m.dashboard_evening();
	});
	let activity = $derived(createSessionActivity(data.idleTimeoutMs));
	afterNavigate(() => {
		void activity();
	});
	onMount(() => listenForActivity(() => activity()));
	async function logout() {
		if (pending) return;
		pending = true;
		message = '';
		try {
			await authRequest('logout');
			await goto('/login', { refreshAll: true });
		} catch (error) {
			message = error instanceof Error ? error.message : m.sign_out_failed();
		} finally {
			pending = false;
		}
	}
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
<AppHeader onsignout={logout} studioName={data.settings?.studioName} />
<main class="flex flex-1 flex-col">
	<div
		class="grid items-end gap-3 px-3.5 pt-[1.125rem] pb-[1.125rem] sm:grid-cols-[1fr_auto] sm:gap-6 sm:px-8 sm:pt-6 sm:pb-[1.625rem]"
	>
		<div>
			<h1 class="text-[1.875rem] leading-[1.05] sm:text-[2.75rem]">
				{greeting}.
				{#if galleriesWithNews > 0}
					<em class="text-danger">{m.dashboard_news({ count: galleriesWithNews })}</em>
				{:else}
					<em class="text-danger">{m.dashboard_quiet()}</em>
					{m.dashboard_quiet_rest()}
				{/if}
			</h1>
		</div>
		<Button class="justify-self-start sm:justify-self-auto" onclick={() => (dialogOpen = true)}
			>{m.dashboard_new_gallery()}</Button
		>
	</div>
	{#if message}<p role="alert" class="px-3.5 pb-3 text-danger sm:px-8">{message}</p>{/if}

	{#if data.galleries !== null}
		{@const none = all.length === 0}
		<div
			class="sticky top-2 z-[6] mx-2.5 mb-4 flex flex-wrap items-center gap-2 rounded-[0.875rem] border border-border bg-panel p-2 sm:mx-8 sm:mb-[1.375rem] sm:gap-3.5 sm:py-2.5 sm:pr-3 sm:pl-4 {none
				? 'opacity-55'
				: ''}"
		>
			<label
				class="flex min-w-40 flex-1 items-center gap-2 rounded-md has-focus-visible:outline-2 has-focus-visible:outline-offset-4 has-focus-visible:outline-focus/40 max-sm:basis-full"
			>
				<Search size={16} aria-hidden="true" class="shrink-0" />
				<input
					type="search"
					aria-label={m.dashboard_search()}
					placeholder={m.dashboard_search()}
					bind:value={query}
					disabled={none}
					class="w-full border-0 bg-transparent text-[0.9375rem] outline-none placeholder:text-muted-foreground focus-visible:shadow-none focus-visible:outline-none [&::-webkit-search-cancel-button]:hidden"
				/>
			</label>
			<GalleryStateFilter
				{selected}
				{counts}
				disabled={none}
				onchange={(next) => (selected = next)}
			/>
			<GallerySortMenu value={data.sort} disabled={none} onchange={changeSort} />
		</div>
	{/if}
	{#if loading}
		<p class="sr-only" role="status">{m.dashboard_loading()}</p>
		<ul class={gridClass} aria-busy="true">
			{#each [1, 2, 3, 4] as n (n)}
				<li class="aspect-[3/4] overflow-hidden rounded-card border border-border sm:aspect-[4/5]">
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
		<ul class="{gridClass} mb-12">
			{#each visible as gallery (gallery.id)}
				{@const news = [
					...new Set(notifications?.unreadFor(gallery.id).map((note) => notificationBadge(note)))
				]}
				<GalleryCard
					{gallery}
					{query}
					{news}
					selected={selectedIds.has(gallery.id)}
					selecting={selectedIds.size > 0}
					pending={draftById.has(gallery.id)}
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
				<li class="aspect-[3/4] sm:aspect-[4/5]">
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
		<p
			class="mt-auto px-3.5 pt-6 {selectedGalleries.length > 0
				? 'pb-24'
				: 'pb-6'} text-[0.8125rem] text-muted-foreground sm:px-8"
		>
			{m.dashboard_total_galleries({ count: totals.galleries })} · {m.summary_photos({
				count: totals.photos
			})}
		</p>
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
			{#if hasDraft}
				<Button
					variant="outline"
					size="sm"
					class="min-h-9! px-3!"
					disabled={bulkBusy}
					onclick={cancelChanges}>{m.dashboard_bulk_cancel()}</Button
				>
				<Button
					size="sm"
					class="min-h-9! px-3!"
					disabled={bulkBusy}
					onclick={() => void applyChanges()}>{m.dashboard_bulk_apply()}</Button
				>
			{/if}
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
