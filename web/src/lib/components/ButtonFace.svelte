<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		shown,
		icon,
		children
	}: {
		/** Faces stack in one grid cell; only the shown one is visible and read. */
		shown: boolean;
		icon?: Snippet;
		children: Snippet;
	} = $props();
</script>

<span
	class="col-start-1 row-start-1 flex items-center justify-center gap-2 transition-opacity duration-300 {shown
		? ''
		: 'opacity-0'}"
	aria-hidden={!shown}
>
	{#if icon}{@render icon()}{/if}
	{@render children()}
</span>
