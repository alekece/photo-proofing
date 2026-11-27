<script lang="ts">
	import { X } from '@lucide/svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { cn } from '#lib/utils.js';

	let {
		ref = $bindable(null),
		label,
		disabled = false,
		class: className,
		onclick
	}: {
		ref?: HTMLButtonElement | null;
		label?: string;
		disabled?: boolean;
		class?: string;
		onclick: () => void;
	} = $props();
</script>

<button
	bind:this={ref}
	type="button"
	aria-label={label ?? m.action_close()}
	{disabled}
	{onclick}
	class={cn(
		'relative grid size-11 shrink-0 cursor-pointer place-items-center rounded-full text-current transition-colors [--icon-hover-size:2rem] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-current disabled:pointer-events-none disabled:opacity-50 motion-reduce:transition-none max-sm:enabled:hover:bg-[var(--icon-hover-color,var(--studio-hover,color-mix(in_oklab,currentColor_10%,transparent)))] sm:icon-action',
		className
	)}
>
	<X size={16} aria-hidden="true" />
</button>
