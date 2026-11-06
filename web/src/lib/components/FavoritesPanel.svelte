<script lang="ts">
	import { Download, Heart } from '@lucide/svelte';
	import PhotoTile from '#lib/components/PhotoTile.svelte';
	import ProofViewer from '#lib/components/ProofViewer.svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import EmptyState from '#lib/components/ui/empty-state/empty-state.svelte';
	import SelectPill from '#lib/components/ui/select-pill/select-pill.svelte';
	import { getLiveEvents } from '#lib/events.js';
	import { listPhotos, previewUrl, thumbnailUrl, type Photo } from '#lib/gallery/photos.js';
	import { SelectMode } from '#lib/gallery/select-mode.svelte.js';
	import type { Proof } from '#lib/gallery/types.js';

	let {
		galleryId,
		included,
		oncount,
		ondownload
	}: {
		galleryId: string;
		included: number | null;
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

	const extra = $derived(included === null ? 0 : Math.max(0, favorites.length - included));

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
		<span class="mr-auto text-sm text-muted-foreground">
			{#if included === null}
				{favorites.length} selected
			{:else}
				{favorites.length} selected · {included} included
				{#if extra > 0}
					· <strong class="text-danger">{extra} extra</strong>
				{/if}
			{/if}
		</span>
		<Button variant="ghost" size="sm" disabled={favorites.length === 0} onclick={() => mode.enter()}
			>Select</Button
		>
		<Button
			variant="outline"
			size="sm"
			disabled={!ondownload || favorites.length === 0}
			aria-busy={downloading}
			title={ondownload ? undefined : 'Downloads arrive with a later release'}
			onclick={() => download(favorites)}
		>
			<Download size={16} aria-hidden="true" class="mr-1.5" />Download
		</Button>
	{/if}
</div>

{#if failed}
	<p role="alert" class="text-sm text-danger">Couldn’t load the favorites. Please reload.</p>
{:else if loaded && favorites.length === 0}
	<EmptyState class="text-muted-foreground">
		<Heart size={32} aria-hidden="true" />
		<p class="text-lg text-foreground">No favorites yet</p>
		<p class="text-sm">The client’s selection will appear here.</p>
	</EmptyState>
{:else}
	<ul aria-label="Favorites" class="grid grid-cols-[repeat(auto-fill,minmax(8rem,1fr))] gap-2">
		{#each favorites as photo, index (photo.id)}
			<li>
				<PhotoTile
					name={photo.original_name}
					state="ready"
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
	<ProofViewer {proofs} bind:index={viewIndex} onclose={() => (viewing = false)} />
{/if}
