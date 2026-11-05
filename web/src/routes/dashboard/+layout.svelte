<script lang="ts">
	import { onMount } from 'svelte';
	import { createLiveEvents, setLiveEvents } from '#lib/events.js';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	// One stream for the whole signed-in area, opened in the browser only.
	const events = createLiveEvents();
	setLiveEvents(events);

	onMount(() => {
		events.start();
		return () => events.close();
	});
</script>

{@render children()}
