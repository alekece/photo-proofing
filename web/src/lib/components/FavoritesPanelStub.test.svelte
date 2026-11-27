<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		onheader,
		onfavorites,
		onfulldownload
	}: {
		onheader?: (action: Snippet) => void;
		onfavorites?: (ids: string[] | null) => void;
		onfulldownload?: (ids: string[]) => void;
	} = $props();

	$effect(() => onheader?.(downloadAction));
	let favoriteIds = $state(['p1', 'p2']);
	$effect(() => onfavorites?.([...favoriteIds]));
</script>

{#snippet downloadAction()}
	<button type="button" onclick={() => onfulldownload?.([...favoriteIds])}>Download all</button>
{/snippet}

<p>No favorites yet. The client’s selection will appear here.</p>
{#if !onheader}{@render downloadAction()}{/if}
<button type="button" onclick={() => (favoriteIds = ['p1'])}>Change client favorites</button>
