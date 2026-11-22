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
		'cn-input w-full min-w-0 focus-visible:border-foreground disabled:cursor-not-allowed disabled:opacity-50',
		className
	)}
	bind:value
	{...restProps}
>
	{@render children?.()}
</select>
