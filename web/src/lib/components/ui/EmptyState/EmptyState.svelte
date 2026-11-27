<script lang="ts">
	import { cn, type WithElementRef } from '#lib/utils.js';
	import type { ChevronDown } from '@lucide/svelte';
	import type { HTMLAttributes } from 'svelte/elements';

	type Props = WithElementRef<HTMLAttributes<HTMLDivElement>, HTMLDivElement> & {
		/**
		 * `drop` (default) keeps the striped, dashed drag-and-drop target look for actual upload
		 * zones. `plain` is for read-only empty collections, filtered results, and Activity: no
		 * stripes, no dashes, no motion.
		 */
		variant?: 'drop' | 'plain';
		icon?: typeof ChevronDown;
		title?: string;
		description?: string;
	};

	let {
		ref = $bindable(null),
		variant = 'drop',
		icon: Icon,
		title,
		description,
		class: className,
		children,
		...restProps
	}: Props = $props();
</script>

<div
	bind:this={ref}
	data-slot="empty-state"
	class={cn(
		'grid min-h-[18.75rem] content-center justify-items-center gap-3.5 rounded-xl',
		variant === 'drop'
			? 'border border-border bg-panel bg-[repeating-linear-gradient(135deg,transparent_0_14px,rgb(43_36_32/0.035)_14px_28px)]'
			: 'border border-border bg-panel',
		'p-12 text-center text-muted-foreground sm:min-h-[23.75rem] [&_h3]:font-serif [&_h3]:text-[1.625rem] [&_h3]:font-normal [&_h3]:text-foreground sm:[&_h3]:text-[1.75rem]',
		className
	)}
	{...restProps}
>
	{#if Icon}<Icon size={28} aria-hidden="true" />{/if}
	{#if title}<h3>{title}</h3>{/if}
	{#if description}<p class="max-w-[36ch] text-sm">{description}</p>{/if}
	{@render children?.()}
</div>
