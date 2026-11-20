<script lang="ts">
	import { cn } from '#lib/utils.js';
	import type { Snippet } from 'svelte';

	/** A button that opens a small panel; it closes on Escape (focus returns to the button) or a press outside. */
	let {
		id,
		open = $bindable(false),
		disabled = false,
		label,
		align = 'start',
		placement = 'below',
		class: className,
		rootClass,
		panelClass,
		trigger,
		children,
		'aria-describedby': ariaDescribedby
	}: {
		/** The button's id, so a `<label for>` names it. */
		id?: string;
		open?: boolean;
		disabled?: boolean;
		/** Accessible name for a button that shows only an icon. */
		label?: string;
		align?: 'start' | 'end';
		placement?: 'below' | 'above' | 'right' | 'responsive';
		class?: string;
		rootClass?: string;
		panelClass?: string;
		trigger: Snippet;
		children: Snippet<[{ close: () => void }]>;
		'aria-describedby'?: string;
	} = $props();

	let root = $state<HTMLElement | null>(null);
	let button = $state<HTMLButtonElement | null>(null);

	function close() {
		open = false;
		button?.focus();
	}

	function dismiss(event: Event) {
		if (open && root && !root.contains(event.target as Node)) open = false;
	}

	function leave(event: FocusEvent) {
		const next = event.relatedTarget as Node | null;

		if (open && next && root && !root.contains(next)) open = false;
	}

	function escape(event: KeyboardEvent) {
		if (open && event.key === 'Escape') close();
	}

	const placementClass = $derived(
		placement === 'above'
			? 'bottom-full mb-2'
			: placement === 'right'
				? 'top-0 left-full ml-2'
				: placement === 'responsive'
					? 'fixed top-14 right-3 left-3 bottom-auto sm:absolute sm:right-0 sm:bottom-full sm:left-auto sm:mb-2'
					: 'top-full mt-2'
	);
</script>

<svelte:window onpointerdown={dismiss} onkeydown={escape} />

<div
	bind:this={root}
	data-slot="popover"
	class={cn('relative inline-block', rootClass)}
	onfocusout={leave}
>
	<button
		bind:this={button}
		{id}
		type="button"
		aria-haspopup="true"
		aria-expanded={open}
		aria-label={label}
		aria-describedby={ariaDescribedby}
		{disabled}
		class={className}
		onclick={() => (open = !open)}
	>
		{@render trigger()}
	</button>
	{#if open}
		<div
			data-slot="popover-panel"
			class={cn(
				'absolute z-30 min-w-48 animate-dialog-in rounded-card border border-border bg-panel p-1 shadow-viewfinder',
				placementClass,
				placement === 'right' || placement === 'responsive'
					? ''
					: align === 'end'
						? 'right-0'
						: 'left-0',
				panelClass
			)}
		>
			{@render children({ close })}
		</div>
	{/if}
</div>
