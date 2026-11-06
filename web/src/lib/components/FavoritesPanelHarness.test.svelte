<script lang="ts">
	import type { Photo } from '#lib/gallery/photos.js';
	import { setLiveEvents, type LiveEvents } from '#lib/events.js';
	import FavoritesPanel from './FavoritesPanel.svelte';

	let {
		events,
		ondownload
	}: {
		events: LiveEvents;
		ondownload?: (photos: Photo[]) => void;
	} = $props();

	// svelte-ignore state_referenced_locally
	setLiveEvents(events);
</script>

<FavoritesPanel galleryId="g1" {ondownload} />
