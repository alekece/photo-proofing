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

<!-- Equal slots either side keep the label centred whether or not its icon is drawn. -->
<span
	class="col-start-1 row-start-1 grid grid-cols-[1rem_auto_1rem] items-center justify-center gap-2 transition-opacity duration-300 {shown
		? ''
		: 'opacity-0'}"
	aria-hidden={!shown}
>
	<span class="grid place-items-center">{@render icon?.()}</span>
	{@render children()}
	<span></span>
</span>
