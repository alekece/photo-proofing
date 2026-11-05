<script lang="ts">
	import { setLiveEvents, type LiveEvents } from '#lib/events.js';
	import FavoritesPanel from './FavoritesPanel.svelte';

	let { events, included }: { events: LiveEvents; included: number | null } = $props();

	// svelte-ignore state_referenced_locally
	setLiveEvents(events);
</script>

<FavoritesPanel galleryId="g1" {included} />
