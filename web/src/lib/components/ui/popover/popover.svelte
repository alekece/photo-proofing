<script lang="ts">
	import { cn } from '#lib/utils.js';
	import type { Snippet } from 'svelte';

	/** A button that opens a small panel; it closes on Escape (focus returns to the button) or a press outside. */
	let {
		open = $bindable(false),
		disabled = false,
		label,
		align = 'start',
		class: className,
		panelClass,
		trigger,
		children
	}: {
		open?: boolean;
		disabled?: boolean;
		/** Accessible name for a button that shows only an icon. */
		label?: string;
		align?: 'start' | 'end';
		class?: string;
		panelClass?: string;
		trigger: Snippet;
		children: Snippet;
	} = $props();

	let root = $state<HTMLElement | null>(null);
	let button = $state<HTMLButtonElement | null>(null);

	function dismiss(event: Event) {
		if (open && root && !root.contains(event.target as Node)) open = false;
	}

	function leave(event: FocusEvent) {
		const next = event.relatedTarget as Node | null;

		if (open && next && root && !root.contains(next)) open = false;
	}

	function escape(event: KeyboardEvent) {
		if (open && event.key === 'Escape') {
			open = false;
			button?.focus();
		}
	}
</script>

<svelte:window onpointerdown={dismiss} onkeydown={escape} />

<div bind:this={root} data-slot="popover" class="relative inline-block" onfocusout={leave}>
	<button
		bind:this={button}
		type="button"
		aria-haspopup="true"
		aria-expanded={open}
		aria-label={label}
		{disabled}
		class={className}
		onclick={() => (open = !open)}
	>
		{@render trigger()}
	</button>
	{#if open}
		<div
			class={cn(
				'absolute top-full z-30 mt-2 min-w-48 animate-dialog-in rounded-card border border-border bg-panel p-1 shadow-viewfinder',
				align === 'end' ? 'right-0' : 'left-0',
				panelClass
			)}
		>
			{@render children()}
		</div>
	{/if}
</div>
