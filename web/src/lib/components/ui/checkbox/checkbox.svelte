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
	class={cn(className)}
	type="checkbox"
	bind:checked
	{...restProps}
/>
