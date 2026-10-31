<script lang="ts">
	import { cn, type WithElementRef } from '#lib/utils.js';
	import type { HTMLAttributes } from 'svelte/elements';

	type Props = WithElementRef<HTMLAttributes<HTMLElement>> & {
		as?: 'div' | 'section' | 'li' | 'form' | 'aside';
	};

	let {
		ref = $bindable(null),
		as = 'div',
		class: className,
		children,
		...restProps
	}: Props = $props();
</script>

<svelte:element
	this={as}
	bind:this={ref}
	data-slot="card"
	class={cn('rounded-card border border-border bg-panel', className)}
	{...restProps}
>
	{@render children?.()}
</svelte:element>
