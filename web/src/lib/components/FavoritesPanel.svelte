<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { CommentsClient } from '#lib/gallery/comments.js';
	import { PhotoComments } from '#lib/gallery/photoComments.svelte.js';
	import CommentThread from './CommentThread.svelte';
	import OwnerZipError from './OwnerZipError.svelte';
	import { Download, Heart, LoaderCircle } from '@lucide/svelte';
	import { tick, untrack, type Snippet } from 'svelte';
	import PhotoTile, { COLLECTION_GRID_CLASSES } from '#lib/components/PhotoTile.svelte';
	import PhotoViewer from '#lib/components/PhotoViewer.svelte';
	import ViewerButton from '#lib/components/ViewerButton.svelte';
	import { startDownload } from '#lib/gallery/export.js';
	import { OwnerExport } from '#lib/gallery/export.svelte.js';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import EmptyState from '#lib/components/ui/EmptyState/EmptyState.svelte';
	import SelectActions from '#lib/components/ui/SelectActions/SelectActions.svelte';
	import WorkspaceToolbar from '#lib/components/ui/WorkspaceToolbar/WorkspaceToolbar.svelte';
	import { getLiveEvents } from '#lib/events.js';
	import { ApiError } from '#lib/http.js';
	import { listPhotos, previewUrl, thumbnailUrl, type Photo } from '#lib/gallery/photos.js';
	import { selectionHotkeysAllowed, SelectMode } from '#lib/gallery/select-mode.svelte.js';
	import type { Proof } from '#lib/gallery/types.js';

	let {
		galleryId,
		oncount,
		onheader,
		active = true,
		ondownload = async (photos: Photo[]) => {
			const current = generation;
			const url = `/api/v1/galleries/${encodeURIComponent(galleryId)}/photos/${encodeURIComponent(photos[0].id)}/download`;
			const response = await fetch(url, { method: 'HEAD', credentials: 'same-origin' });
			if (current !== generation) return;
			if (!response.ok) throw new ApiError(response.status, m.favorites_download_failed());
			startDownload(url);
		}
	}: {
		galleryId: string;
		/** How many favorites the client has picked, for the tab. */
		oncount?: (count: number | null) => void;
		onheader?: (action: Snippet) => void;
		active?: boolean;
		/**
		 * Downloads an individual original from a tile or viewer. Selected subsets use ZIP export.
		 */
		ondownload?: (photos: Photo[]) => void | Promise<void>;
	} = $props();

	const events = getLiveEvents();
	const loadedGalleryId = $derived(galleryId);
	const zip = new OwnerExport(() => loadedGalleryId, 'photos');
	const comments = $derived(
		new PhotoComments(new CommentsClient(`/api/v1/galleries/${encodeURIComponent(galleryId)}`))
	);
	$effect(() => comments.start(events, galleryId));

	let favorites = $state<Photo[]>([]);
	let loaded = $state(false);
	let failed = $state(false);
	let loads = 0;
	let viewIndex = $state(0);
	let viewing = $state(false);
	let panel = $state<HTMLElement | null>(null);
	let downloadAll = $state<HTMLElement | null>(null);
	let downloading = $state<string | null>(null);
	let individualError = $state<string | null>(null);
	let generation = 0;
	const mode = new SelectMode();

	async function load(id: string) {
		const current = ++loads;

		try {
			const photos = await listPhotos(id);

			if (current !== loads) return;

			favorites = photos
				.filter(({ favorited_at }) => favorited_at)
				.sort((a, b) => Date.parse(b.favorited_at!) - Date.parse(a.favorited_at!));
			failed = false;
		} catch {
			if (current === loads) failed = true;
		} finally {
			if (current === loads) loaded = true;
		}
	}

	$effect(() => {
		const id = loadedGalleryId;

		generation++;
		favorites = [];
		untrack(() => mode.leave());
		loaded = false;
		downloading = null;
		individualError = null;
		viewing = false;
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
			generation++;
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

	// Photos that stop being favorites leave a selection with them.
	$effect(() => {
		for (const id of [...mode.ids]) {
			if (!favorites.some((photo) => photo.id === id)) mode.ids.delete(id);
		}
	});

	async function download(photos: Photo[]) {
		if (downloading) return;

		const current = generation;
		downloading = photos[0].id;
		individualError = null;

		try {
			await ondownload(photos);
		} catch {
			if (current === generation) individualError = m.favorites_download_failed();
		} finally {
			if (current === generation) downloading = null;
		}
	}

	async function finishSelecting() {
		mode.leave();
		await tick();
		if (active) (downloadAll?.matches(':disabled') ? panel : (downloadAll ?? panel))?.focus();
	}
</script>

{#snippet headerDownload()}
	{#if favorites.length > 0}{@render downloadButton(true)}{/if}
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
		disabled={zip.busy || (!all && mode.count === 0)}
		aria-busy={zip.busy}
		onclick={() =>
			void zip.request(
				!all && mode.count > 0
					? favorites.filter(({ id }) => mode.has(id)).map(({ id }) => id)
					: undefined
			)}
	>
		<Download size={16} aria-hidden="true" />
		<span class="group-data-[rail=gallery]/select-actions:max-xl:sr-only">
			{zip.busy
				? m.favorites_downloading()
				: !all
					? m.favorites_download_count({ count: mode.count })
					: m.favorites_download_all()}
		</span>
	</Button>
{/snippet}

<section
	class="@container grid min-w-0 gap-4"
	aria-label={m.tab_favorites()}
	tabindex="-1"
	bind:this={panel}
>
	{#if !onheader && favorites.length > 0}
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
			onunselectall={() => mode.unselectAll()}
			onexit={() => void finishSelecting()}
			canEscape={() => active && mode.active && !viewing && selectionHotkeysAllowed()}
		>
			{#snippet actions()}{@render downloadButton()}{/snippet}
		</SelectActions>
	{/if}

	<OwnerZipError {zip} />
	{#if individualError}
		<p role="alert" class="text-right text-sm text-danger">{individualError}</p>
	{/if}

	{#if failed}
		<p role="alert" class="text-sm text-danger">{m.favorites_load_failed()}</p>
		<Button variant="outline" onclick={() => void load(galleryId)}>{m.action_retry()}</Button>
	{:else if loaded && favorites.length === 0}
		<EmptyState variant="plain">
			<span class="grid size-14 place-items-center rounded-full bg-background"
				><Heart size={26} aria-hidden="true" /></span
			>
			<h3>{m.favorites_empty()}</h3>
			<p class="text-sm">{m.favorites_empty_body()}</p>
		</EmptyState>
	{:else}
		<ul aria-label={m.tab_favorites()} class={COLLECTION_GRID_CLASSES}>
			{#each favorites as photo, index (photo.id)}
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
						}}
						onopen={() => {
							viewIndex = index;
							viewing = true;
						}}
						ondownload={() => void download([photo])}
						downloadBusy={downloading === photo.id}
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
{#if viewing && proofs[viewIndex]}
	<PhotoViewer
		{proofs}
		bind:index={viewIndex}
		thread={photoThread}
		commentCount={(id) => comments.count(id)}
		readonly
		isSelected={() => true}
		onclose={() => (viewing = false)}
	>
		{#snippet actions(proof)}
			<ViewerButton
				aria-label={m.tile_download({ name: proof.alt })}
				disabled={downloading !== null}
				aria-busy={downloading === proof.id}
				onclick={() => {
					const photo = favorites.find(({ id }) => id === proof.id);
					if (photo) void download([photo]);
				}}
			>
				{#if downloading === proof.id}
					<LoaderCircle
						size={18}
						class="animate-spin motion-reduce:animate-none"
						aria-hidden="true"
					/>
				{:else}
					<Download size={18} aria-hidden="true" />
				{/if}
			</ViewerButton>
		{/snippet}
	</PhotoViewer>
{/if}
