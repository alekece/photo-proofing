<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import type { Snippet } from 'svelte';
	import { X } from '@lucide/svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';

	/**
	 * The contextual bar of select mode: exit, a select-all toggle button and the count on the left;
	 * the actions that fit a selection on the right. The button selects everything, or clears the
	 * selection once everything is selected. The bar sticks to the top of the page while a long
	 * gallery scrolls.
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
	class="sticky top-0 z-20 flex flex-wrap items-center gap-2 px-2 py-1.5"
>
	<Button
		variant="ghost"
		size="sm"
		class="min-h-9! px-2.5!"
		aria-label={m.selection_exit()}
		onclick={onexit}><X size={18} aria-hidden="true" /></Button
	>
	<Button variant="outline" size="sm" class="min-h-9! px-3!" disabled={total === 0} onclick={toggle}
		>{all ? m.action_unselect_all() : m.action_select_all()}</Button
	>
	<strong class="text-sm font-semibold">{m.notification_selected({ count })}</strong>
	<div class="ml-auto flex flex-wrap items-center justify-end gap-2">
		{@render actions?.()}
	</div>
</div>
