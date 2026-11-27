<script lang="ts">
	import type { Snippet } from 'svelte';
	import { setLiveEvents, type LiveEvents } from '#lib/events.js';
	import FavoritesPanel from './FavoritesPanel.svelte';

	let {
		events,
		galleryId = 'g1',
		active,
		withHeader = true,
		onfavorites,
		onfulldownload
	}: {
		events: LiveEvents;
		galleryId?: string;
		active?: boolean;
		withHeader?: boolean;
		onfavorites?: (ids: string[] | null) => void;
		onfulldownload?: (ids: string[]) => void;
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
	{active}
	{onfavorites}
	{onfulldownload}
	onheader={withHeader ? registerHeader : undefined}
/>
