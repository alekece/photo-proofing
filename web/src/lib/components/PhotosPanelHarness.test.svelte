<script lang="ts">
	import { setLiveEvents, type LiveEvents } from '#lib/events.js';
	import { setUploadQueue, type UploadQueue } from '#lib/gallery/uploads.svelte.js';
	import PhotosPanel from './PhotosPanel.svelte';

	let {
		galleryId,
		queue,
		events,
		heroId = $bindable(null)
	}: {
		galleryId: string;
		queue: UploadQueue;
		events: LiveEvents;
		heroId?: string | null;
	} = $props();

	// svelte-ignore state_referenced_locally
	setUploadQueue(queue);
	// svelte-ignore state_referenced_locally
	setLiveEvents(events);
</script>

<PhotosPanel {galleryId} bind:heroId />
