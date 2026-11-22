<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onMount } from 'svelte';
	import { SvelteMap, SvelteSet } from 'svelte/reactivity';
	import { afterNavigate, goto } from '$app/navigation';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import PhotoTile, { GRID_CLASSES, ITEM_CLASSES } from '#lib/components/PhotoTile.svelte';
	import PhotoViewer from '#lib/components/PhotoViewer.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import ConfirmDialog from '#lib/components/ui/ConfirmDialog/ConfirmDialog.svelte';
	import EmptyState from '#lib/components/ui/EmptyState/EmptyState.svelte';
	import SelectActions from '#lib/components/ui/SelectActions/SelectActions.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import ToolbarMenu from '#lib/components/ui/ToolbarMenu/ToolbarMenu.svelte';
	import WorkspaceHeading from '#lib/components/ui/WorkspaceHeading/WorkspaceHeading.svelte';
	import WorkspaceToolbar from '#lib/components/ui/WorkspaceToolbar/WorkspaceToolbar.svelte';
	import { ArrowUpDown, Download, Images, ListFilter, Trash2 } from '@lucide/svelte';
	import {
		BULK_DELETE_LIMIT,
		deletePhotos,
		downloadOriginal,
		previewUrl,
		thumbnailUrl,
		type PhotoSet,
		type WorkspacePhoto
	} from '#lib/gallery/photos.js';
	import type { Proof } from '#lib/gallery/types.js';
	import { ApiError } from '#lib/http.js';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	let photos = $derived<WorkspacePhoto[]>(data.photos ?? []);
	let query = $state('');
	let resultsHeight = $state(0);
	let galleryFilter = $state('all');
	let typeFilter = $state<'all' | 'proof' | 'final'>('all');
	let sortOrder = $state<'newest' | 'oldest'>('newest');
	const sorts = $derived([
		{ value: 'newest', label: m.photos_sort_newest() },
		{ value: 'oldest', label: m.photos_sort_oldest() }
	] as const);
	const selectedIds = new SvelteSet<string>();
	let selectionMode = $state(false);
	let deleteConfirmOpen = $state(false);
	let downloadBusy = $state(false);
	let viewerIndex = $state<number | null>(null);

	const loadFailed = $derived(data.photos === null);

	/** The gallery label to show, and to search and build tile names from. */
	function galleryLabel(entry: Pick<WorkspacePhoto, 'gallery_title'>) {
		return entry.gallery_title ?? m.gallery_untitled();
	}

	const galleryOptions = $derived.by(() => {
		const byId = new SvelteMap<string, string>();

		for (const entry of photos) byId.set(entry.gallery_id, galleryLabel(entry));

		return [
			{ value: 'all', label: m.photos_filter_gallery_all() },
			...[...byId.entries()]
				.sort((a, b) => a[1].localeCompare(b[1]))
				.map(([value, label]) => ({ value, label }))
		];
	});
	const types = $derived([
		{ value: 'all', label: m.photos_filter_type_all() },
		{ value: 'proof', label: m.photos_kind_proof() },
		{ value: 'final', label: m.photos_kind_final() }
	] as const);
	const matching = $derived.by(() => {
		const needle = query.trim().toLowerCase();

		return photos.filter((entry) => {
			if (galleryFilter !== 'all' && entry.gallery_id !== galleryFilter) return false;
			if (typeFilter !== 'all' && entry.kind !== typeFilter) return false;
			if (needle === '') return true;

			return galleryLabel(entry).toLowerCase().includes(needle);
		});
	});

	const visible = $derived(
		[...matching].sort((a, b) =>
			sortOrder === 'newest'
				? b.created_at.localeCompare(a.created_at)
				: a.created_at.localeCompare(b.created_at)
		)
	);

	const selection = $derived(visible.filter((entry) => selectedIds.has(entry.id)));
	const viewerPhotos = $derived<Proof[]>(
		viewerIndex === null
			? []
			: visible.map((entry) => ({
					id: entry.id,
					alt: galleryLabel(entry),
					width: entry.width,
					height: entry.height,
					thumbnailUrl: thumbnailUrl(
						entry.gallery_id,
						entry.id,
						photoSet(entry.kind),
						entry.revision
					),
					previewUrl: previewUrl(entry.gallery_id, entry.id, photoSet(entry.kind), entry.revision)
				}))
	);

	// A photo hidden by a filter change is no longer offered among the selection's actions.
	$effect(() => {
		const visibleIds = new Set(visible.map((entry) => entry.id));

		for (const id of [...selectedIds]) {
			if (!visibleIds.has(id)) selectedIds.delete(id);
		}
	});

	function toggleSelect(id: string) {
		selectionMode = true;

		if (!selectedIds.delete(id)) selectedIds.add(id);
	}

	function clearSelection() {
		selectedIds.clear();
		selectionMode = false;
	}

	function clearSelectedOnly() {
		selectedIds.clear();
	}

	function handleWindowKeydown(event: KeyboardEvent) {
		if (
			event.key === 'Escape' &&
			!event.defaultPrevented &&
			selectionMode &&
			!deleteConfirmOpen &&
			!document.querySelector('[data-slot="popover"] > button[aria-expanded="true"]')
		) {
			event.preventDefault();
			clearSelection();
		}
	}

	function clearFilters() {
		query = '';
		galleryFilter = 'all';
		typeFilter = 'all';
	}

	function photoSet(kind: WorkspacePhoto['kind']): PhotoSet {
		return kind === 'final' ? 'finals' : 'photos';
	}

	async function downloadSelected() {
		if (downloadBusy) return;

		const targets = [...selection];

		if (targets.length === 0) return;

		downloadBusy = true;
		let failed = 0;

		try {
			for (const entry of targets) {
				try {
					await downloadOriginal(entry.gallery_id, entry.id, fetch, photoSet(entry.kind));
				} catch (cause) {
					if (cause instanceof ApiError && cause.status === 401) {
						await goto('/login', { refreshAll: true });

						return;
					}

					failed += 1;
				}
			}
		} finally {
			downloadBusy = false;
		}

		if (failed > 0) toasts.error(m.photos_download_some_failed());
	}

	/**
	 * Deletes the selection grouped by the gallery and kind it belongs to, in bounded chunks; on
	 * the first failing chunk it stops, leaving that chunk and every not-yet-processed group
	 * selected so the photographer can retry just what is left.
	 */
	async function confirmBulkDelete() {
		const targets = [...selection];
		const groups = new SvelteMap<string, { galleryId: string; set: PhotoSet; ids: string[] }>();

		for (const entry of targets) {
			const set = photoSet(entry.kind);
			const key = `${entry.gallery_id}:${set}`;
			const group = groups.get(key) ?? { galleryId: entry.gallery_id, set, ids: [] };

			group.ids.push(entry.id);
			groups.set(key, group);
		}

		let deleted = 0;

		for (const group of groups.values()) {
			for (let at = 0; at < group.ids.length; at += BULK_DELETE_LIMIT) {
				const chunk = group.ids.slice(at, at + BULK_DELETE_LIMIT);

				try {
					await deletePhotos(group.galleryId, chunk, fetch, group.set);
				} catch (cause) {
					if (cause instanceof ApiError && cause.status === 401) {
						await goto('/login', { refreshAll: true });

						return;
					}

					throw new Error(
						deleted > 0
							? m.photos_partly_deleted({ deleted, total: targets.length })
							: cause instanceof Error
								? cause.message
								: m.api_error_generic(),
						{ cause }
					);
				}

				for (const id of chunk) {
					selectedIds.delete(id);
					deleted += 1;
				}
				photos = photos.filter((entry) => !chunk.includes(entry.id));
			}
		}

		deleteConfirmOpen = false;
		if (selectedIds.size === 0) selectionMode = false;
	}

	let activity = $derived(createSessionActivity(data.idleTimeoutMs));
	afterNavigate(() => {
		void activity();
	});
	onMount(() => listenForActivity(() => activity()));
</script>

<svelte:head><title>{m.photos_head()} — PhotoProofing</title></svelte:head>
<svelte:window onkeydown={handleWindowKeydown} />
<main class="flex flex-1 flex-col workspace-inset pt-4 pb-24 sm:pt-6">
	<div class="mb-4 sm:mb-6">
		<WorkspaceHeading
			title={m.photos_head()}
			count={loadFailed ? undefined : m.summary_photos({ count: visible.length })}
		/>
	</div>

	{#if !loadFailed}
		{@const none = photos.length === 0}
		<WorkspaceToolbar
			class="mb-4 sm:mb-[1.375rem]"
			search={{
				label: m.photos_search(),
				value: query,
				disabled: none,
				oninput: (value) => (query = value)
			}}
			resultsHeight={visible.length ? resultsHeight : 0}
			onclear={query.trim() || galleryFilter !== 'all' || typeFilter !== 'all'
				? clearFilters
				: undefined}
		>
			{#snippet filters()}
				<ToolbarMenu
					label={m.photos_filter_gallery()}
					summary={galleryOptions.find((o) => o.value === galleryFilter)?.label ??
						m.photos_filter_gallery_all()}
					icon={Images}
					value={galleryFilter}
					options={galleryOptions}
					disabled={none}
					onchange={(v) => (galleryFilter = v)}
				/>
				<ToolbarMenu
					label={m.photos_filter_type()}
					summary={types.find((o) => o.value === typeFilter)!.label}
					icon={ListFilter}
					value={typeFilter}
					options={types}
					disabled={none}
					onchange={(v) => (typeFilter = v)}
				/>
			{/snippet}
			{#snippet sort()}
				<ToolbarMenu
					label={m.photos_sort()}
					summary={sorts[sortOrder === 'newest' ? 0 : 1].label}
					icon={ArrowUpDown}
					value={sortOrder}
					options={sorts}
					disabled={none}
					onchange={(value) => (sortOrder = value)}
				/>
			{/snippet}
		</WorkspaceToolbar>
	{/if}

	{#if loadFailed}
		<p role="alert" class="px-3.5 sm:px-0">{m.photos_load_failed()}</p>
		<Button variant="outline" class="w-fit" onclick={() => location.reload()}
			>{m.action_retry()}</Button
		>
	{:else if photos.length === 0}
		<EmptyState variant="plain" class="rounded-card py-14">
			<Images size={28} aria-hidden="true" class="text-muted-foreground" />
			<h2 class="text-[1.625rem] text-foreground sm:text-[1.75rem]">{m.photos_empty()}</h2>
			<p class="max-w-sm text-sm">{m.photos_empty_body()}</p>
		</EmptyState>
	{:else if visible.length === 0}
		<EmptyState variant="plain" class="rounded-card py-14">
			<h2 class="text-[1.625rem] text-foreground sm:text-[1.75rem]">{m.photos_no_match()}</h2>
			<Button variant="outline" onclick={clearFilters}>{m.photos_clear_filters()}</Button>
		</EmptyState>
	{:else}
		<ul bind:clientHeight={resultsHeight} aria-label={m.photos_head()} class={GRID_CLASSES.masonry}>
			{#each visible as entry, index (entry.id)}
				<li class={ITEM_CLASSES.masonry}>
					<PhotoTile
						name={`${galleryLabel(entry)} — ${entry.original_name}`}
						state="ready"
						cleanReady
						persistentSelect
						ratio={entry.width / entry.height}
						thumbnailUrl={thumbnailUrl(
							entry.gallery_id,
							entry.id,
							photoSet(entry.kind),
							entry.revision
						)}
						selected={selectedIds.has(entry.id)}
						onopen={() => (viewerIndex = index)}
						onselect={() => toggleSelect(entry.id)}
						source={galleryLabel(entry)}
					/>
				</li>
			{/each}
		</ul>
	{/if}
</main>

{#if viewerIndex !== null}
	<PhotoViewer
		proofs={viewerPhotos}
		bind:index={() => viewerIndex ?? 0, (index) => (viewerIndex = index)}
		mobileArrows
		onclose={() => (viewerIndex = null)}
	/>
{/if}

{#if selectionMode}
	<SelectActions
		count={selection.length}
		total={visible.length}
		rail="dashboard"
		onselectall={() => {
			for (const entry of visible) selectedIds.add(entry.id);
		}}
		onunselectall={clearSelectedOnly}
		onexit={clearSelection}
	>
		{#snippet actions()}
			<Button
				variant="outline"
				size="sm"
				class="min-h-9! gap-1.5"
				disabled={selection.length === 0 || downloadBusy}
				onclick={() => void downloadSelected()}
				><Download size={16} aria-hidden="true" /><span class="max-sm:sr-only"
					>{m.action_download()}</span
				></Button
			>
			<Button
				variant="destructive"
				size="sm"
				class="min-h-9! gap-1.5"
				disabled={selection.length === 0}
				onclick={() => (deleteConfirmOpen = true)}
				><Trash2 size={16} aria-hidden="true" /><span class="max-sm:sr-only"
					>{m.action_delete()}</span
				></Button
			>
		{/snippet}
	</SelectActions>
{/if}

{#if deleteConfirmOpen}
	<ConfirmDialog
		title={m.photos_delete_many({ count: selection.length })}
		confirmLabel={m.action_delete()}
		destructive
		onconfirm={confirmBulkDelete}
		onclose={() => (deleteConfirmOpen = false)}
	>
		{#snippet description()}
			<p class="text-sm text-muted-foreground">{m.photos_delete_undone()}</p>
		{/snippet}
	</ConfirmDialog>
{/if}
