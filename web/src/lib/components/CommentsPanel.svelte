<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { CommentsClient } from '#lib/gallery/comments.js';
	import { PhotoComments } from '#lib/gallery/photoComments.svelte.js';
	import CommentThread from './CommentThread.svelte';
	import { MessageSquare } from '@lucide/svelte';
	import PhotoTile, { GRID_CLASSES, ITEM_CLASSES } from '#lib/components/PhotoTile.svelte';
	import PhotoViewer from '#lib/components/PhotoViewer.svelte';
	import EmptyState from '#lib/components/ui/EmptyState/EmptyState.svelte';
	import { getLiveEvents } from '#lib/events.js';
	import type { GalleryLayout } from '#lib/gallery/api.js';
	import { listPhotos, previewUrl, thumbnailUrl, type Photo } from '#lib/gallery/photos.js';
	import type { Proof } from '#lib/gallery/types.js';

	let {
		galleryId,
		layout = 'masonry',
		oncount
	}: {
		galleryId: string;
		/** How the tiles are laid out: the gallery's setting. */
		layout?: GalleryLayout;
		/** How many photos have comments, for the tab. */
		oncount?: (count: number | null) => void;
	} = $props();

	const events = getLiveEvents();
	const comments = $derived(
		new PhotoComments(new CommentsClient(`/api/v1/galleries/${encodeURIComponent(galleryId)}`))
	);
	$effect(() => comments.start(events, galleryId));

	let photos = $state<Photo[]>([]);
	let loaded = $state(false);
	let failed = $state(false);
	let loads = 0;
	let viewIndex = $state(0);
	let viewing = $state(false);

	async function load(id: string) {
		const current = ++loads;

		try {
			const all = await listPhotos(id);

			if (current !== loads) return;

			photos = all;
			failed = false;
		} catch {
			if (current === loads) failed = true;
		} finally {
			if (current === loads) loaded = true;
		}
	}

	$effect(() => {
		const id = galleryId;

		photos = [];
		loaded = false;
		viewing = false;
		void load(id);

		const unsubscribe = [
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

	const commented = $derived(photos.filter(({ id }) => comments.count(id) > 0));
	const proofs = $derived<Proof[]>(
		commented.map(({ id, original_name, width, height, revision }) => ({
			id,
			alt: original_name,
			thumbnailUrl: thumbnailUrl(galleryId, id, 'photos', revision),
			previewUrl: previewUrl(galleryId, id, 'photos', revision),
			width,
			height
		}))
	);

	$effect(() => {
		oncount?.(loaded && !failed && !comments.error ? commented.length : null);
	});
</script>

{#snippet photoThread(proof: Proof)}
	<CommentThread
		client={comments.client}
		photoId={proof.id}
		viewer="photographer"
		refresh={comments.revision}
		onchange={() => void comments.refresh()}
	/>
{/snippet}

{#if failed}
	<p role="alert" class="text-sm text-danger">{m.comments_tab_load_failed()}</p>
{:else if comments.error}
	<p role="alert" class="text-sm text-danger">{comments.error}</p>
{:else if loaded && commented.length === 0}
	<EmptyState>
		<span class="grid size-14 place-items-center rounded-full bg-background"
			><MessageSquare size={26} aria-hidden="true" /></span
		>
		<h3>{m.comments_tab_empty()}</h3>
		<p class="text-sm">{m.comments_tab_empty_body()}</p>
	</EmptyState>
{:else}
	<ul aria-label={m.tab_comments()} class={GRID_CLASSES[layout]}>
		{#each commented as photo, index (photo.id)}
			<li class={ITEM_CLASSES[layout]}>
				<PhotoTile
					name={photo.original_name}
					commentCount={comments.count(photo.id)}
					state="ready"
					ratio={layout === 'masonry' ? photo.width / photo.height : 3 / 2}
					thumbnailUrl={thumbnailUrl(galleryId, photo.id, 'photos', photo.revision)}
					onopen={() => {
						viewIndex = index;
						viewing = true;
					}}
				/>
			</li>
		{/each}
	</ul>
{/if}

{#if viewing && proofs[viewIndex]}
	<PhotoViewer
		{proofs}
		bind:index={viewIndex}
		thread={photoThread}
		commentCount={(id) => comments.count(id)}
		readonly
		isSelected={() => true}
		onclose={() => (viewing = false)}
	/>
{/if}
