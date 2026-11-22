<script lang="ts">
	import type { Snippet } from 'svelte';
	import { setLiveEvents, type LiveEvents } from '#lib/events.js';
	import FavoritesPanel from './FavoritesPanel.svelte';

	let {
		events,
		galleryId = 'g1',
		active,
		withHeader = true
	}: {
		events: LiveEvents;
		galleryId?: string;
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
<FavoritesPanel {galleryId} {active} onheader={withHeader ? registerHeader : undefined} />
