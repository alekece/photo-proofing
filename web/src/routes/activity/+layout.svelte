<script lang="ts">
	import { onMount } from 'svelte';
	import { createLiveEvents, setLiveEvents } from '#lib/events.js';
	import { NotificationCenter, setNotifications } from '#lib/gallery/notifications.svelte.js';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	const events = createLiveEvents();
	setLiveEvents(events);

	const notifications = new NotificationCenter();
	setNotifications(notifications);

	onMount(() => {
		const stop = notifications.start(events);

		events.start();

		return () => {
			stop();
			events.close();
		};
	});
</script>

{@render children()}
