<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { CommentsClient } from '#lib/gallery/comments.js';
	import { PhotoComments } from '#lib/gallery/photoComments.svelte.js';
	import CommentThread from './CommentThread.svelte';
	import { Download, Heart, LoaderCircle } from '@lucide/svelte';
	import { tick, untrack, type Snippet } from 'svelte';
	import PhotoTile, { COLLECTION_GRID_CLASSES } from '#lib/components/PhotoTile.svelte';
	import PhotoViewer from '#lib/components/PhotoViewer.svelte';
	import { OwnerExport } from '#lib/gallery/export.svelte.js';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import EmptyState from '#lib/components/ui/EmptyState/EmptyState.svelte';
	import SelectActions from '#lib/components/ui/SelectActions/SelectActions.svelte';
	import WorkspaceToolbar from '#lib/components/ui/WorkspaceToolbar/WorkspaceToolbar.svelte';
	import { getLiveEvents } from '#lib/events.js';
	import { listPhotos, previewUrl, thumbnailUrl, type Photo } from '#lib/gallery/photos.js';
	import { selectionHotkeysAllowed, SelectMode } from '#lib/gallery/select-mode.svelte.js';
	import type { Proof } from '#lib/gallery/types.js';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';

	let {
		galleryId,
		oncount,
		onheader,
		onfavorites,
		onfulldownload,
		active = true
	}: {
		galleryId: string;
		/** How many favorites the client has picked, for the tab. */
		oncount?: (count: number | null) => void;
		onheader?: (action: Snippet) => void;
		onfavorites?: (ids: string[] | null) => void;
		onfulldownload?: (ids: string[]) => void;
		active?: boolean;
	} = $props();

	const events = getLiveEvents();
	const loadedGalleryId = $derived(galleryId);
	const zip = new OwnerExport(
		() => loadedGalleryId,
		'photos',
		() => true,
		false
	);
	const comments = $derived(
		new PhotoComments(new CommentsClient(`/api/v1/galleries/${encodeURIComponent(galleryId)}`))
	);
	$effect(() => comments.start(events, galleryId));

	let favorites = $state<Photo[]>([]);
	let loaded = $state(false);
	let failed = $state(false);
	let loads = 0;
	let viewedId = $state<string | null>(null);
	const viewIndex = $derived(
		viewedId === null ? -1 : favorites.findIndex(({ id }) => id === viewedId)
	);
	let panel = $state<HTMLElement | null>(null);
	let downloadAll = $state<HTMLElement | null>(null);
	const mode = new SelectMode();

	async function load(id: string) {
		const current = ++loads;

		try {
			const photos = await listPhotos(id);

			if (current !== loads) return;

			favorites = photos
				.filter(({ favorited_at }) => favorited_at)
				.sort((a, b) => Date.parse(b.favorited_at!) - Date.parse(a.favorited_at!));
			onfavorites?.(favorites.map(({ id }) => id));
			if (viewIndex < 0) viewedId = null;
			failed = false;
		} catch {
			if (current === loads) {
				failed = true;
				onfavorites?.(null);
			}
		} finally {
			if (current === loads) loaded = true;
		}
	}

	$effect(() => {
		const id = loadedGalleryId;

		favorites = [];
		untrack(() => onfavorites?.(null));
		untrack(() => mode.leave());
		loaded = false;
		viewedId = null;
		void load(id);

		const unsubscribe = [
			events.onFavorites((event) => event.gallery_id === id && void load(id)),
			events.onPhoto(
				(event) =>
					event.gallery_id === id &&
					(event.status === 'deleted' || event.status === 'ready') &&
					void load(id)
			),
			events.onResync(() => void load(id))
		];

		return () => {
			loads++;

			for (const off of unsubscribe) off();
		};
	});

	const proofs = $derived<Proof[]>(
		favorites.map(({ id, original_name, width, height, revision }) => ({
			id,
			alt: original_name,
			thumbnailUrl: thumbnailUrl(galleryId, id, 'photos', revision),
			previewUrl: previewUrl(galleryId, id, 'photos', revision),
			width,
			height
		}))
	);
	$effect(() => onheader?.(headerDownload));

	$effect(() => {
		oncount?.(loaded && !failed ? favorites.length : null);
	});

	let notifiedExportError: string | null = null;
	$effect(() => {
		const error = zip.error ?? zip.statusError;
		if (!error) {
			notifiedExportError = null;
			return;
		}
		if (error === notifiedExportError) return;

		notifiedExportError = error;
		toasts.error(error);
	});

	// Photos that stop being favorites leave a selection with them.
	$effect(() => {
		for (const id of [...mode.ids]) {
			if (!favorites.some((photo) => photo.id === id)) mode.ids.delete(id);
		}
	});

	async function finishSelecting() {
		mode.leave();
		await tick();
		if (active) (downloadAll?.matches(':disabled') ? panel : (downloadAll ?? panel))?.focus();
	}

	function requestDownload(all: boolean) {
		const ids = all
			? favorites.map(({ id }) => id)
			: favorites.filter(({ id }) => mode.has(id)).map(({ id }) => id);

		void zip.request(all ? undefined : ids, all ? () => onfulldownload?.(ids) : undefined);
	}
</script>

{#snippet headerDownload()}
	{@render downloadButton(true)}
{/snippet}

{#snippet downloadButton(all = false)}
	<Button
		bind:ref={
			() => (all ? downloadAll : null),
			(element) => {
				if (all) downloadAll = element;
			}
		}
		size="sm"
		class="min-h-9! gap-1.5 group-data-[rail=gallery]/select-actions:max-xl:px-2.5"
		disabled={zip.busy || (all ? favorites.length === 0 : mode.count === 0)}
		aria-busy={zip.busy}
		onclick={() => requestDownload(all)}
	>
		{#if zip.busy}
			<LoaderCircle size={16} class="animate-spin motion-reduce:animate-none" aria-hidden="true" />
		{:else}
			<Download size={16} aria-hidden="true" />
		{/if}
		<span class="group-data-[rail=gallery]/select-actions:max-xl:sr-only">
			{all ? m.favorites_download_all() : m.favorites_download_count({ count: mode.count })}
		</span>
		{#if zip.busy}<span class="sr-only">{m.favorites_downloading()}</span>{/if}
	</Button>
{/snippet}

<section
	class="@container grid min-w-0 gap-4"
	aria-label={m.tab_favorites()}
	tabindex="-1"
	bind:this={panel}
>
	{#if !onheader}
		<WorkspaceToolbar>
			{#snippet actions()}{@render downloadButton(true)}{/snippet}
		</WorkspaceToolbar>
	{/if}
	{#if mode.active}
		<SelectActions
			count={mode.count}
			total={favorites.length}
			rail="gallery"
			onselectall={() => mode.selectAll(favorites.map(({ id }) => id))}
			onunselectall={() => void finishSelecting()}
			onexit={() => void finishSelecting()}
			canEscape={() => active && mode.active && viewedId === null && selectionHotkeysAllowed()}
		>
			{#snippet actions()}{@render downloadButton()}{/snippet}
		</SelectActions>
	{/if}

	{#if failed}
		<p role="alert" class="text-sm text-danger">{m.favorites_load_failed()}</p>
		<Button variant="outline" onclick={() => void load(galleryId)}>{m.action_retry()}</Button>
	{:else if loaded && favorites.length === 0}
		<EmptyState
			variant="plain"
			icon={Heart}
			title={m.favorites_empty()}
			description={m.favorites_empty_body()}
		/>
	{:else}
		<ul aria-label={m.tab_favorites()} class={COLLECTION_GRID_CLASSES}>
			{#each favorites as photo (photo.id)}
				<li class="min-w-0">
					<PhotoTile
						name={photo.original_name}
						state="ready"
						ratio={3 / 2}
						thumbnailUrl={thumbnailUrl(galleryId, photo.id, 'photos', photo.revision)}
						caption={photo.favorited_at ? { kind: 'favorited', at: photo.favorited_at } : undefined}
						selected={mode.has(photo.id)}
						collectionSelect={mode.active ? 'active' : 'idle'}
						onselect={() => {
							mode.enter();
							mode.toggle(photo.id);
							if (mode.count === 0) mode.leave();
						}}
						onopen={() => (viewedId = photo.id)}
					/>
				</li>
			{/each}
		</ul>
	{/if}
</section>

{#if comments.error}<p role="alert" class="text-sm text-danger">{comments.error}</p>{/if}
{#snippet photoThread(proof: Proof)}
	<CommentThread
		client={comments.client}
		photoId={proof.id}
		viewer="photographer"
		refresh={comments.revision}
		onchange={() => void comments.refresh()}
	/>
{/snippet}
{#if viewIndex >= 0}
	<PhotoViewer
		{proofs}
		bind:index={() => viewIndex, (index) => (viewedId = proofs[index].id)}
		thread={photoThread}
		commentCount={(id) => comments.count(id)}
		readonly
		isSelected={() => true}
		onclose={() => (viewedId = null)}
	/>
{/if}
