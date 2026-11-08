<script lang="ts">
	import type { Snippet } from 'svelte';

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
	onanimationend={(e) => e.currentTarget === e.target && onend?.()}
>
	{@render children()}
	{#if active}
		<span
			aria-hidden="true"
			data-testid="click-ripple"
			class="pointer-events-none absolute inset-0 animate-click-ripple rounded-full border-2 border-current"
		></span>
	{/if}
</span>
