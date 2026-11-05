<script lang="ts">
	import EmptyState from '#lib/components/ui/empty-state/empty-state.svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import { getLiveEvents } from '#lib/events.js';
	import { listPhotos, thumbnailUrl, type Photo } from '#lib/gallery/photos.js';

	let { galleryId, included }: { galleryId: string; included: number | null } = $props();

	const events = getLiveEvents();

	let favorites = $state<Photo[]>([]);
	let loaded = $state(false);
	let failed = $state(false);
	let loads = 0;

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
</script>

<div class="flex flex-wrap items-center justify-between gap-2">
	<span class="text-sm text-muted-foreground">
		{#if included === null}
			{favorites.length} selected
		{:else}
			{favorites.length} selected · {included} included
			{#if extra > 0}
				· <strong class="text-warning">{extra} extra</strong>
			{/if}
		{/if}
	</span>
	<Button size="sm" disabled title="Downloads arrive with a later release">Download</Button>
</div>

{#if failed}
	<p role="alert" class="text-destructive text-sm">Couldn’t load the favorites. Please reload.</p>
{:else if loaded && favorites.length === 0}
	<EmptyState class="text-muted-foreground">
		No favorites yet. The client’s selection will appear here.
	</EmptyState>
{:else}
	<ul class="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-4">
		{#each favorites as photo (photo.id)}
			<li class="bg-muted overflow-hidden rounded-md">
				<img
					src={thumbnailUrl(galleryId, photo.id)}
					alt={photo.original_name}
					loading="lazy"
					class="aspect-[3/2] w-full object-cover"
				/>
			</li>
		{/each}
	</ul>
{/if}
