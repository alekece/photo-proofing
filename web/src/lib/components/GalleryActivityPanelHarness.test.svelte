<script lang="ts">
	import { setLiveEvents, type LiveEvents } from '#lib/events.js';
	import GalleryActivityPanel from './GalleryActivityPanel.svelte';

	let { events, galleryId }: { events: LiveEvents; galleryId: string } = $props();

	// svelte-ignore state_referenced_locally
	setLiveEvents(events);
</script>

<GalleryActivityPanel {galleryId} />
