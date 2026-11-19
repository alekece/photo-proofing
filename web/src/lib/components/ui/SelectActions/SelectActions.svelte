<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import type { Snippet } from 'svelte';
	import { CheckCheck, ListX, X } from '@lucide/svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import { fly } from 'svelte/transition';

	/**
	 * The contextual bar of select mode: exit and the count on the left; a select-all toggle button
	 * and the actions that fit a selection on the right. The button selects everything, or clears
	 * the selection once everything is selected. On small screens the bar shrinks to a compact
	 * icon toolbar. It floats above the bottom of the viewport on every page that uses it, shifted
	 * clear of whichever fixed left workspace rail the consumer renders behind it.
	 */
	let {
		count,
		total,
		onselectall,
		onunselectall,
		onexit,
		rail,
		actions
	}: {
		count: number;
		/** How many items can be selected. */
		total: number;
		onselectall: () => void;
		onunselectall: () => void;
		onexit: () => void;
		/**
		 * Which fixed left workspace rail this consumer renders behind the bar, so it can shift
		 * clear of the clamped workspace rail or both workspace and gallery rails.
		 */
		rail: 'dashboard' | 'gallery';
		actions?: Snippet;
	} = $props();

	const all = $derived(total > 0 && count >= total);

	const RAIL_CLEARANCE_CLASS = {
		dashboard:
			'sm:left-[calc(50%+clamp(7.5rem,8vw,8.75rem))] sm:max-w-[calc(100vw-clamp(15rem,16vw,17.5rem)-1.25rem)]',
		gallery:
			'md:left-[calc(50%+clamp(7.5rem,8vw,8.75rem)+8.25rem)] md:max-w-[calc(100vw-clamp(15rem,16vw,17.5rem)-18rem)]'
	} as const;

	function leaveDuration() {
		return typeof HTMLElement === 'undefined' ||
			typeof HTMLElement.prototype.animate !== 'function' ||
			(typeof window !== 'undefined' &&
				typeof window.matchMedia === 'function' &&
				window.matchMedia('(prefers-reduced-motion: reduce)').matches)
			? 0
			: 180;
	}

	function toggle() {
		if (all) onunselectall();
		else onselectall();
	}
</script>

<div
	data-slot="select-actions"
	out:fly={{ y: 8, duration: leaveDuration() }}
	class="fixed bottom-4 left-1/2 z-20 flex w-fit max-w-[calc(100vw-1.25rem)] -translate-x-1/2 animate-selection-in items-center gap-2 rounded-full border border-border bg-panel px-2 py-1.5 shadow-[0_20px_50px_-12px_rgb(0_0_0/0.35)] max-sm:gap-1 max-sm:p-1 sm:bottom-6 {RAIL_CLEARANCE_CLASS[
		rail
	]}"
>
	<Button
		variant="ghost"
		size="sm"
		class="min-h-9! px-2.5!"
		aria-label={m.selection_exit()}
		onclick={onexit}><X size={18} aria-hidden="true" /></Button
	>
	<strong class="text-sm font-semibold">{m.notification_selected({ count })}</strong>
	<div class="ml-auto flex items-center justify-end gap-2 max-sm:ml-0 max-sm:gap-1">
		<Button
			variant="outline"
			size="sm"
			class="min-h-9! gap-1.5 px-3! max-sm:border-transparent! max-sm:px-2.5!"
			disabled={total === 0}
			onclick={toggle}
		>
			{#if all}
				<ListX size={16} aria-hidden="true" class="sm:hidden" />
			{:else}
				<CheckCheck size={16} aria-hidden="true" class="sm:hidden" />
			{/if}
			<span class="max-sm:sr-only">{all ? m.action_unselect_all() : m.action_select_all()}</span>
		</Button>
		{@render actions?.()}
	</div>
</div>
