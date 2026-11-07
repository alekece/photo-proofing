<script lang="ts">
	import { setLiveEvents, type LiveEvents } from '#lib/events.js';
	import { setUploadQueue, type UploadQueue } from '#lib/gallery/uploads.svelte.js';
	import type { PhotoSet } from '#lib/gallery/photos.js';
	import PhotosPanel from './PhotosPanel.svelte';

	let {
		galleryId,
		queue,
		events,
		heroId = $bindable(null),
		watermarked = false,
		set = 'photos'
	}: {
		galleryId: string;
		queue: UploadQueue;
		events: LiveEvents;
		heroId?: string | null;
		watermarked?: boolean;
		set?: PhotoSet;
	} = $props();

	// svelte-ignore state_referenced_locally
	setUploadQueue(queue);
	// svelte-ignore state_referenced_locally
	setLiveEvents(events);
</script>

<PhotosPanel {galleryId} bind:heroId {watermarked} {set} />
