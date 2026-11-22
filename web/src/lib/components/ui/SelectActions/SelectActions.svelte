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
		canEscape,
		disabled = false,
		rail,
		actions
	}: {
		count: number;
		/** How many items can be selected. */
		total: number;
		onselectall: () => void;
		onunselectall: () => void;
		onexit: () => void;
		/** Opts in to Escape; the panel can let a dialog or field own the key instead. */
		canEscape?: () => boolean;
		disabled?: boolean;
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
			'sm:left-[calc(50%+clamp(7.5rem,8vw,8.75rem))] sm:max-w-[calc(100vw-clamp(15rem,16vw,17.5rem)-1.25rem)] md:left-[calc(50%+clamp(7.5rem,8vw,8.75rem)+var(--gallery-rail-width)/2)] md:max-w-[calc(100vw-clamp(15rem,16vw,17.5rem)-var(--gallery-rail-width)-1.5rem)]'
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

	function onkeydown(event: KeyboardEvent) {
		if (event.key !== 'Escape' || event.defaultPrevented || !canEscape?.()) return;

		event.preventDefault();
		onexit();
	}
</script>

<svelte:window {onkeydown} />

<div
	data-slot="select-actions"
	data-rail={rail}
	out:fly={{ y: 8, duration: leaveDuration() }}
	class={[
		'group/select-actions fixed bottom-4 left-1/2 z-20 flex w-max max-w-[calc(100vw-1.25rem)] -translate-x-1/2 animate-selection-in flex-wrap items-center justify-center gap-2 rounded-[1.5rem] border border-border bg-panel px-2 py-1.5 shadow-[0_20px_50px_-12px_rgb(0_0_0/0.35)] max-sm:gap-1 max-sm:p-1 sm:bottom-6',
		RAIL_CLEARANCE_CLASS[rail],
		rail === 'gallery' &&
			'max-xl:w-max max-xl:flex-wrap max-xl:justify-center max-xl:gap-1 max-xl:px-1'
	]}
>
	<Button
		variant="ghost"
		size="sm"
		class="min-h-9! px-2.5!"
		aria-label={m.selection_exit()}
		{disabled}
		onclick={onexit}><X size={18} aria-hidden="true" /></Button
	>
	<strong class="text-sm font-semibold whitespace-nowrap sm:mr-6"
		>{m.notification_selected({ count })}</strong
	>
	<div
		class={[
			'flex flex-wrap items-center justify-center gap-2 max-sm:gap-1',
			rail === 'gallery' && 'max-xl:ml-0 max-xl:gap-1'
		]}
	>
		<Button
			variant="outline"
			size="sm"
			class="min-h-9! gap-1.5 px-3! group-data-[rail=gallery]/select-actions:max-xl:px-2.5! max-sm:border-transparent! max-sm:px-2.5!"
			disabled={disabled || total === 0}
			onclick={toggle}
		>
			{#if all}
				<ListX
					size={16}
					aria-hidden="true"
					class="group-data-[rail=gallery]/select-actions:max-xl:inline sm:hidden"
				/>
			{:else}
				<CheckCheck
					size={16}
					aria-hidden="true"
					class="group-data-[rail=gallery]/select-actions:max-xl:inline sm:hidden"
				/>
			{/if}
			<span class="group-data-[rail=gallery]/select-actions:max-xl:sr-only max-sm:sr-only"
				>{all ? m.action_unselect_all() : m.action_select_all()}</span
			>
		</Button>
		{@render actions?.()}
	</div>
</div>
