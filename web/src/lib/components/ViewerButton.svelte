<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { HTMLButtonAttributes } from 'svelte/elements';
	import { cn } from '#lib/utils.js';

	/**
	 * A pill control for the dark photo viewer. `pressed` fills it light; `tone="warn"` colors a
	 * destructive action.
	 */
	type Props = {
		pressed?: boolean;
		tone?: 'default' | 'warn';
		class?: string;
		children: Snippet;
	} & Omit<HTMLButtonAttributes, 'class' | 'children'>;

	let {
		pressed = undefined,
		tone = 'default',
		class: className,
		children,
		...rest
	}: Props = $props();

	const classes = $derived(
		cn(
			'inline-flex h-10 min-w-10 cursor-pointer items-center justify-center gap-2 rounded-full px-3.5 text-[0.84375rem] font-semibold whitespace-nowrap transition-colors focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-viewer-focus disabled:cursor-default disabled:opacity-30 aria-disabled:cursor-not-allowed aria-disabled:opacity-50 max-[700px]:px-2.5',
			pressed
				? 'bg-panel text-foreground'
				: 'bg-white/8 text-[#efe6da] hover:bg-white/14 aria-disabled:hover:bg-white/8',
			tone === 'warn' && !pressed && 'text-warning-on-dark',
			className
		)
	);
</script>

<button type="button" aria-pressed={pressed} class={classes} {...rest}>{@render children()}</button>
