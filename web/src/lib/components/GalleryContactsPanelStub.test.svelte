<script lang="ts">
	import type { Snippet } from 'svelte';

	let { onheader }: { onheader?: (action: Snippet) => void } = $props();

	$effect(() => onheader?.(newContactAction));
</script>

{#snippet newContactAction()}
	<button type="button">New contact</button>
{/snippet}

<p>No contacts linked yet.</p>
{#if !onheader}{@render newContactAction()}{/if}
