<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { createLiveEvents, setLiveEvents } from '#lib/events.js';
	import { NotificationCenter, setNotifications } from '#lib/gallery/notifications.svelte.js';
	import { UploadQueue, setUploadQueue } from '#lib/gallery/uploads.svelte.js';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	// One stream for the whole signed-in area, opened in the browser only.
	const events = createLiveEvents();
	setLiveEvents(events);

	const notifications = new NotificationCenter();
	setNotifications(notifications);

	// Uploads keep going across dashboard pages; leaving the dashboard (logout) stops them.
	const uploads = new UploadQueue();
	setUploadQueue(uploads);
	onDestroy(() => uploads.clear());

	onMount(() => {
		const stop = notifications.start(events);

		events.start();

		return () => {
			stop();
			events.close();
		};
	});
</script>

<svelte:window onbeforeunload={(event) => uploads.guard(event)} />

{@render children()}
