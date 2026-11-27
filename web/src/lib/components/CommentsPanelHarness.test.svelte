<script lang="ts">
	import { setLiveEvents, type LiveEvents } from '#lib/events.js';
	import CommentsPanel from './CommentsPanel.svelte';

	let {
		events,
		oncount,
		openThread
	}: {
		events: LiveEvents;
		oncount?: (count: number | null) => void;
		openThread?: { photoId: string };
	} = $props();

	// svelte-ignore state_referenced_locally
	setLiveEvents(events);
</script>

<CommentsPanel galleryId="g1" {oncount} {openThread} />
