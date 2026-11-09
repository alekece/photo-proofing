<script lang="ts">
	import type { Snippet } from 'svelte';
	import ClickRipple from '#lib/components/ClickRipple.svelte';

	let {
		active,
		onend,
		children
	}: {
		/** Plays the scale-up and ripple while true; reduced motion makes it instant. */
		active: boolean;
		/** Called when the pop finishes, so the owner can re-arm it. */
		onend?: () => void;
		children: Snippet;
	} = $props();
</script>

<span
	class="relative inline-grid {active ? 'animate-heart-pop' : ''}"
	data-active={active || undefined}
>
	{@render children()}
	{#if active}<ClickRipple onend={() => onend?.()} />{/if}
</span>
