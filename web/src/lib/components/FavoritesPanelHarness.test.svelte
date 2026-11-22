<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { Photo } from '#lib/gallery/photos.js';
	import { setLiveEvents, type LiveEvents } from '#lib/events.js';
	import FavoritesPanel from './FavoritesPanel.svelte';

	let {
		events,
		galleryId = 'g1',
		ondownload,
		active,
		withHeader = true
	}: {
		events: LiveEvents;
		galleryId?: string;
		ondownload?: (photos: Photo[]) => void;
		active?: boolean;
		withHeader?: boolean;
	} = $props();

	// svelte-ignore state_referenced_locally
	setLiveEvents(events);
	let action = $state<Snippet>();

	function registerHeader(next: Snippet) {
		action = next;
	}
</script>

{#if withHeader}<header>{@render action?.()}</header>{/if}
<FavoritesPanel
	{galleryId}
	{ondownload}
	{active}
	onheader={withHeader ? registerHeader : undefined}
/>
