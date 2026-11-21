<script lang="ts">
	import { onDestroy } from 'svelte';
	import { UploadQueue, setUploadQueue } from '#lib/gallery/uploads.svelte.js';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	// Uploads keep going across dashboard pages; leaving the dashboard (logout) stops them.
	const uploads = new UploadQueue();
	setUploadQueue(uploads);
	onDestroy(() => uploads.clear());
</script>

<svelte:window onbeforeunload={(event) => uploads.guard(event)} />

{@render children()}
