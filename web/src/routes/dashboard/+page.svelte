<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onMount } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
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
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import { Pencil, Plus, Search, Star, Trash2, X } from '@lucide/svelte';
	import GalleryStateFilter from '#lib/components/GalleryStateFilter.svelte';
	import GallerySortMenu from '#lib/components/GallerySortMenu.svelte';
	import {
		deleteGallery,
		GalleryApiError,
		updateGallery,
		type GalleryOverview
	} from '#lib/gallery/api.js';
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

	let all = $derived.by(() => {
		if (!localGalleries) return [];

		return [...localGalleries].sort((a, b) => compareGalleries(a, b, data.sort));
	});

	const selectedIds = new SvelteSet<string>();

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

	// Prune selected IDs whenever search, status filters, or deletions hide/remove cards.
	$effect(() => {
		const visibleSet = new Set(visible.map((g) => g.id));

		for (const id of [...selectedIds]) {
			if (!visibleSet.has(id)) selectedIds.delete(id);
		}
	});

	let selectedGalleries = $derived(visible.filter((g) => selectedIds.has(g.id)));
	let allSelectedFavorited = $derived(
		selectedGalleries.length > 0 && selectedGalleries.every((g) => g.favorite)
	);

	function toggleSelect(id: string) {
		if (selectedIds.has(id)) selectedIds.delete(id);
		else selectedIds.add(id);
	}

	function clearSelection() {
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
			selectedIds.size > 0 &&
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
			item.id === updated.id
				? {
						...item,
						...updated,
						access_code_protected: updated.access_code_protected ?? item.access_code_protected
					}
				: item
		);
	}

	async function toggleBulkFavorite() {
		if (bulkBusy || selectedGalleries.length === 0) return;

		const targetFavorite = !allSelectedFavorited;
		const targets = selectedGalleries.filter((g) => Boolean(g.favorite) !== targetFavorite);

		if (targets.length === 0) return;

		bulkBusy = true;

		try {
			let firstError: string | null = null;

			for (const item of targets) {
				try {
					const updated = await updateGallery(item.id, { favorite: targetFavorite });

					mergeUpdated(updated);
				} catch (cause) {
					if (cause instanceof GalleryApiError && cause.status === 401) {
						await goto('/login', { refreshAll: true });

						return;
					}

					firstError ??= cause instanceof Error ? cause.message : m.api_error_generic();
				}
			}

			if (firstError) toasts.error(firstError);
		} finally {
			bulkBusy = false;
		}
	}

	async function saveQuickEdits(edits: QuickGalleryEdit[]): Promise<string | null> {
		let firstError: string | null = null;
		const failedIds: string[] = [];

		for (const edit of edits) {
			try {
				const updated = await updateGallery(edit.id, edit.changes);

				mergeUpdated(updated);
			} catch (cause) {
				if (cause instanceof GalleryApiError && cause.status === 401) {
					await goto('/login', { refreshAll: true });

					return null;
				}

				failedIds.push(edit.id);
				firstError ??= cause instanceof Error ? cause.message : m.api_error_generic();
			}
		}

		if (failedIds.length > 0) {
			for (const id of [...selectedIds]) {
				if (!failedIds.includes(id)) selectedIds.delete(id);
			}
		}

		return firstError;
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
				if (cause instanceof GalleryApiError && cause.status === 401) {
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
<AppHeader onsignout={logout} />
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
{#if selectedGalleries.length > 0}
	<div
		role="region"
		aria-label={m.dashboard_bulk_selected({ count: selectedGalleries.length })}
		class="fixed inset-x-2.5 bottom-4 z-20 mx-auto flex w-fit max-w-[calc(100vw-1.25rem)] items-center gap-1 rounded-full border border-white/12 bg-foreground px-2 py-1.5 text-[0.8125rem] font-semibold text-panel shadow-[0_20px_50px_-12px_rgb(0_0_0/0.55)] sm:bottom-6 sm:gap-1.5 sm:px-3 sm:py-2"
	>
		<button
			type="button"
			aria-label={m.dashboard_bulk_clear()}
			title={m.dashboard_bulk_clear()}
			onclick={clearSelection}
			class="inline-grid size-8 shrink-0 cursor-pointer place-items-center rounded-full text-panel/80 transition-colors hover:bg-white/12 hover:text-panel focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-viewer-focus"
		>
			<X size={16} aria-hidden="true" />
		</button>
		<span class="px-1 whitespace-nowrap tabular-nums sm:px-1.5">
			{m.dashboard_bulk_selected({ count: selectedGalleries.length })}
		</span>
		<span aria-hidden="true" class="mx-0.5 h-4 w-px shrink-0 bg-white/18"></span>
		<button
			type="button"
			disabled={bulkBusy}
			aria-label={allSelectedFavorited
				? m.dashboard_bulk_unfavorite()
				: m.dashboard_bulk_favorite()}
			onclick={() => void toggleBulkFavorite()}
			class="inline-flex min-h-8 shrink-0 cursor-pointer items-center gap-1.5 rounded-full px-2.5 py-1 text-panel transition-colors hover:bg-white/12 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-viewer-focus disabled:cursor-not-allowed disabled:opacity-50 sm:px-3"
		>
			<Star
				size={15}
				fill={allSelectedFavorited ? 'currentColor' : 'none'}
				class={allSelectedFavorited ? 'text-[#f2c063]' : ''}
				aria-hidden="true"
			/>
			<span class="max-[460px]:sr-only">
				{allSelectedFavorited ? m.dashboard_bulk_unfavorite() : m.dashboard_bulk_favorite()}
			</span>
		</button>
		<button
			type="button"
			disabled={bulkBusy}
			aria-label={m.dashboard_bulk_quick_edit()}
			onclick={() => (quickEditOpen = true)}
			class="inline-flex min-h-8 shrink-0 cursor-pointer items-center gap-1.5 rounded-full px-2.5 py-1 text-panel transition-colors hover:bg-white/12 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-viewer-focus disabled:cursor-not-allowed disabled:opacity-50 sm:px-3"
		>
			<Pencil size={15} aria-hidden="true" />
			<span class="max-[460px]:sr-only">{m.dashboard_bulk_quick_edit()}</span>
		</button>
		<button
			type="button"
			disabled={bulkBusy}
			aria-label={m.action_delete()}
			onclick={() => (deleteConfirmOpen = true)}
			class="inline-flex min-h-8 shrink-0 cursor-pointer items-center gap-1.5 rounded-full px-2.5 py-1 text-warning-on-dark transition-colors hover:bg-white/12 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-viewer-focus disabled:cursor-not-allowed disabled:opacity-50 sm:px-3"
		>
			<Trash2 size={15} aria-hidden="true" />
			<span class="max-[460px]:sr-only">{m.action_delete()}</span>
		</button>
	</div>
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
