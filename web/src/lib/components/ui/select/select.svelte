<script lang="ts">
	import { cn, type WithElementRef } from '#lib/utils.js';
	import type { HTMLSelectAttributes } from 'svelte/elements';

	type Props = WithElementRef<HTMLSelectAttributes>;

	let {
		ref = $bindable(null),
		value = $bindable(),
		class: className,
		children,
		'data-slot': dataSlot = 'select',
		...restProps
	}: Props = $props();
</script>

<select
	bind:this={ref}
	data-slot={dataSlot}
	class={cn(
		'rounded-full border border-border-strong bg-panel px-3 py-1.5 text-sm focus-visible:border-foreground',
		className
	)}
	bind:value
	{...restProps}
>
	{@render children?.()}
</select>
