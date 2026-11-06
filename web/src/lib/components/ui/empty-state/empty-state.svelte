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
		'grid min-h-[18.75rem] content-center justify-items-center gap-3.5 rounded-xl border-[1.5px] border-dashed border-[#cdbda9] bg-[repeating-linear-gradient(135deg,transparent_0_14px,rgb(43_36_32/0.03)_14px_28px)] p-12 text-center text-muted-foreground sm:min-h-[23.75rem] [&_h3]:font-serif [&_h3]:text-[1.625rem] [&_h3]:font-normal [&_h3]:text-foreground sm:[&_h3]:text-[1.75rem]',
		className
	)}
	{...restProps}
>
	{@render children?.()}
</div>
