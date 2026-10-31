<script lang="ts">
	import { cn, type WithElementRef } from '#lib/utils.js';
	import type { HTMLInputAttributes } from 'svelte/elements';

	type Props = WithElementRef<Omit<HTMLInputAttributes, 'type'>>;

	let {
		ref = $bindable(null),
		class: className,
		'data-slot': dataSlot = 'radio',
		...restProps
	}: Props = $props();
</script>

<input bind:this={ref} data-slot={dataSlot} class={cn(className)} type="radio" {...restProps} />
