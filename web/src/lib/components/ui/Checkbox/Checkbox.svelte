<script lang="ts">
	import { cn, type WithElementRef } from '#lib/utils.js';
	import type { HTMLInputAttributes } from 'svelte/elements';

	type Props = WithElementRef<Omit<HTMLInputAttributes, 'type' | 'value'>>;

	let {
		ref = $bindable(null),
		checked = $bindable(false),
		class: className,
		'data-slot': dataSlot = 'checkbox',
		...restProps
	}: Props = $props();
</script>

<input
	bind:this={ref}
	data-slot={dataSlot}
	class={cn(
		'size-4 shrink-0 cursor-pointer accent-foreground focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50',
		className
	)}
	type="checkbox"
	bind:checked
	{...restProps}
/>
