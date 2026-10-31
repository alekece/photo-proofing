<script lang="ts">
	import { cn, type WithElementRef } from '#lib/utils.js';
	import type { HTMLAttributes } from 'svelte/elements';

	type Props = WithElementRef<HTMLAttributes<HTMLDivElement>, HTMLDivElement>;

	let { ref = $bindable(null), class: className, children, ...restProps }: Props = $props();
</script>

<div
	bind:this={ref}
	data-slot="empty-state"
	class={cn(
		'grid justify-items-center gap-3 rounded-card border border-dashed border-border-strong p-12 text-center',
		className
	)}
	{...restProps}
>
	{@render children?.()}
</div>
