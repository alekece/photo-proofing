<script lang="ts">
	import type { Photo } from '#lib/gallery/photos.js';
	import { setLiveEvents, type LiveEvents } from '#lib/events.js';
	import FavoritesPanel from './FavoritesPanel.svelte';

	let {
		events,
		galleryId = 'g1',
		ondownload,
		active
	}: {
		events: LiveEvents;
		galleryId?: string;
		ondownload?: (photos: Photo[]) => void;
		active?: boolean;
	} = $props();

	// svelte-ignore state_referenced_locally
	setLiveEvents(events);
</script>

<FavoritesPanel {galleryId} {ondownload} {active} />
