<script lang="ts">
	import { setLiveEvents, type LiveEvents } from '#lib/events.js';
	import CommentsPanel from './CommentsPanel.svelte';

	let { events, oncount }: { events: LiveEvents; oncount?: (count: number | null) => void } =
		$props();

	// svelte-ignore state_referenced_locally
	setLiveEvents(events);
</script>

<CommentsPanel galleryId="g1" {oncount} />
