<script lang="ts">
	import { CircleCheck, Download, Heart } from '@lucide/svelte';
	import PhotoTile, { GRID_CLASSES, ITEM_CLASSES } from '#lib/components/PhotoTile.svelte';
	import ProofViewer from '#lib/components/ProofViewer.svelte';
	import type { GalleryLayout } from '#lib/gallery/api.js';
	import Button from '#lib/components/ui/button/button.svelte';
	import EmptyState from '#lib/components/ui/empty-state/empty-state.svelte';
	import SelectPill from '#lib/components/ui/select-pill/select-pill.svelte';
	import { getLiveEvents } from '#lib/events.js';
	import { listPhotos, previewUrl, thumbnailUrl, type Photo } from '#lib/gallery/photos.js';
	import { SelectMode } from '#lib/gallery/select-mode.svelte.js';
	import type { Proof } from '#lib/gallery/types.js';

	let {
		galleryId,
		layout = 'masonry',
		oncount,
		ondownload
	}: {
		galleryId: string;
		/** How the tiles are laid out: the gallery's setting. */
		layout?: GalleryLayout;
		/** How many favorites the client has picked, for the tab. */
		oncount?: (count: number | null) => void;
		/**
		 * Downloads these photos, a ZIP when there are several; the seam for the download endpoint
		 * (#69). Without it, downloading is shown but unavailable.
		 */
		ondownload?: (photos: Photo[]) => void | Promise<void>;
	} = $props();

	const events = getLiveEvents();

	let favorites = $state<Photo[]>([]);
	let loaded = $state(false);
	let failed = $state(false);
	let loads = 0;
	let viewIndex = $state(0);
	let viewing = $state(false);
	let downloading = $state(false);
	const mode = new SelectMode();

	async function load(id: string) {
		const current = ++loads;

		try {
			const photos = await listPhotos(id);

			if (current !== loads) return;

			favorites = photos.filter(({ favorite }) => favorite);
			failed = false;
		} catch {
			if (current === loads) failed = true;
		} finally {
			if (current === loads) loaded = true;
		}
	}

	$effect(() => {
		const id = galleryId;

		favorites = [];
		loaded = false;
		void load(id);

		const unsubscribe = [
			events.onFavorites((event) => event.gallery_id === id && void load(id)),
			events.onPhoto(
				(event) => event.gallery_id === id && event.status === 'deleted' && void load(id)
			),
			events.onResync(() => void load(id))
		];

		return () => {
			loads++;

			for (const off of unsubscribe) off();
		};
	});

	const proofs = $derived<Proof[]>(
		favorites.map(({ id, original_name, width, height }) => ({
			id,
			alt: original_name,
			thumbnailUrl: thumbnailUrl(galleryId, id),
			previewUrl: previewUrl(galleryId, id),
			width,
			height
		}))
	);

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
		if (!ondownload || downloading) return;

		downloading = true;

		try {
			await ondownload(photos);
		} finally {
			downloading = false;
		}
	}

	function onkeydown(event: KeyboardEvent) {
		if (mode.active && event.key === 'Escape' && !event.defaultPrevented) mode.leave();
	}
</script>

<svelte:window {onkeydown} />

<div class="flex flex-wrap items-center justify-end gap-2">
	{#if mode.active}
		<SelectPill
			count={mode.count}
			actionLabel="Download"
			actionDisabled={!ondownload || mode.count === 0}
			actionBusy={downloading}
			onaction={() => download(favorites.filter(({ id }) => mode.has(id)))}
			onselectall={() => mode.selectAll(favorites.map(({ id }) => id))}
			onclose={() => mode.leave()}
		>
			{#snippet icon()}<Download size={16} aria-hidden="true" />{/snippet}
		</SelectPill>
	{:else}
		<Button
			variant="outline"
			size="sm"
			class="min-h-9! gap-1.5"
			disabled={favorites.length === 0}
			onclick={() => mode.enter()}><CircleCheck size={16} aria-hidden="true" />Select</Button
		>
		<Button
			size="sm"
			class="min-h-9! gap-1.5"
			disabled={!ondownload || favorites.length === 0}
			aria-busy={downloading}
			title={ondownload ? undefined : 'Downloads arrive with a later release'}
			onclick={() => download(favorites)}
		>
			<Download size={16} aria-hidden="true" />Download
		</Button>
	{/if}
</div>

{#if failed}
	<p role="alert" class="text-sm text-danger">Couldn’t load the favorites. Please reload.</p>
{:else if loaded && favorites.length === 0}
	<EmptyState class="border-solid bg-panel">
		<span class="grid size-14 place-items-center rounded-full bg-background"
			><Heart size={26} aria-hidden="true" /></span
		>
		<h3>No favorites yet</h3>
		<p class="text-sm">The client’s selection will appear here.</p>
	</EmptyState>
{:else}
	<ul aria-label="Favorites" class={GRID_CLASSES[layout]}>
		{#each favorites as photo, index (photo.id)}
			<li class={ITEM_CLASSES[layout]}>
				<PhotoTile
					name={photo.original_name}
					state="ready"
					ratio={layout === 'masonry' ? photo.width / photo.height : 3 / 2}
					thumbnailUrl={thumbnailUrl(galleryId, photo.id)}
					selected={mode.has(photo.id)}
					onselect={mode.active ? () => mode.toggle(photo.id) : undefined}
					onopen={() => {
						viewIndex = index;
						viewing = true;
					}}
					ondownload={ondownload && (() => void download([photo]))}
				/>
			</li>
		{/each}
	</ul>
{/if}

{#if viewing && proofs[viewIndex]}
	<ProofViewer
		{proofs}
		bind:index={viewIndex}
		readonly
		isSelected={() => true}
		onclose={() => (viewing = false)}
	/>
{/if}
