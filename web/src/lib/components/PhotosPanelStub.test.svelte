<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		galleryId,
		onheader
	}: {
		galleryId: string;
		onheader?: (action: Snippet) => void;
	} = $props();

	$effect(() => onheader?.(uploadAction));
</script>

{#snippet uploadAction()}
	<button type="button">Upload</button>
{/snippet}

<p>Photos of {galleryId}</p>
{#if !onheader}{@render uploadAction()}{/if}
