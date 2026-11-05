<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { createLiveEvents, setLiveEvents } from '#lib/events.js';
	import { UploadQueue, setUploadQueue } from '#lib/gallery/uploads.svelte.js';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	// One stream for the whole signed-in area, opened in the browser only.
	const events = createLiveEvents();
	setLiveEvents(events);

	// Uploads keep going across dashboard pages; leaving the dashboard (logout) stops them.
	const uploads = new UploadQueue();
	setUploadQueue(uploads);
	onDestroy(() => uploads.clear());

	onMount(() => {
		events.start();
		return () => events.close();
	});
</script>

<svelte:window onbeforeunload={(event) => uploads.guard(event)} />

{@render children()}
