<script lang="ts">
	import { cn, type WithElementRef } from '#lib/utils.js';
	import type { HTMLAttributes } from 'svelte/elements';

	type Props = WithElementRef<HTMLAttributes<HTMLElement>> & {
		as?: 'div' | 'section' | 'li' | 'form' | 'aside' | 'p';
		surface?: 'panel' | 'glass';
		dataSlot?: string;
	};

	let {
		ref = $bindable(null),
		as = 'div',
		surface = 'panel',
		dataSlot = 'card',
		class: className,
		children,
		...restProps
	}: Props = $props();
</script>

<svelte:element
	this={as}
	bind:this={ref}
	data-slot={dataSlot}
	data-surface={surface}
	class={cn(
		surface === 'glass' ? 'glass-on-image' : 'rounded-card border border-border bg-panel',
		className
	)}
	{...restProps}
>
	{@render children?.()}
</svelte:element>
