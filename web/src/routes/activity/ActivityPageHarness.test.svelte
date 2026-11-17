<script lang="ts">
	import { setLiveEvents, type LiveEvents } from '#lib/events.js';
	import Page from './+page.svelte';

	let { events, data }: { events: LiveEvents; data: never } = $props();

	// svelte-ignore state_referenced_locally
	setLiveEvents(events);
</script>

<Page {data} />
