<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import type { Snippet } from 'svelte';
	import { CheckCheck, ListX, X } from '@lucide/svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';

	/**
	 * The contextual bar of select mode: exit and the count on the left; a select-all toggle button
	 * and the actions that fit a selection on the right. The button selects everything, or clears
	 * the selection once everything is selected. On small screens the bar shrinks to a compact
	 * icon toolbar. It sticks to the top of the page while a long gallery scrolls.
	 */
	let {
		count,
		total,
		onselectall,
		onunselectall,
		onexit,
		actions
	}: {
		count: number;
		/** How many items can be selected. */
		total: number;
		onselectall: () => void;
		onunselectall: () => void;
		onexit: () => void;
		actions?: Snippet;
	} = $props();

	const all = $derived(total > 0 && count >= total);

	function toggle() {
		if (all) onunselectall();
		else onselectall();
	}
</script>

<div
	data-slot="select-actions"
	class="sticky top-0 z-20 flex items-center gap-2 px-2 py-1.5 max-sm:ml-auto max-sm:w-fit max-sm:gap-1 max-sm:rounded-full max-sm:border max-sm:border-border max-sm:bg-white max-sm:p-1 max-sm:shadow-sm"
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
