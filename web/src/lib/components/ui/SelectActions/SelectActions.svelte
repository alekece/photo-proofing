<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import type { Snippet } from 'svelte';
	import { ListChecks, ListX, X } from '@lucide/svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';

	/**
	 * The contextual bar of select mode: exit and count on the left; Select all, Unselect all and the
	 * actions that fit a selection on the right. It sticks to the top of the page while a long gallery
	 * scrolls.
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
</script>

<div
	data-slot="select-actions"
	class="sticky top-0 z-20 flex flex-wrap items-center gap-2 rounded-md border border-border bg-panel px-2 py-1.5 shadow-sm"
>
	<Button
		variant="ghost"
		size="sm"
		class="min-h-9! px-2.5!"
		aria-label={m.selection_exit()}
		onclick={onexit}><X size={18} aria-hidden="true" /></Button
	>
	<strong class="text-sm font-semibold">{m.notification_selected({ count })}</strong>
	<div class="ml-auto flex flex-wrap items-center justify-end gap-2">
		<Button
			variant="outline"
			size="sm"
			class="min-h-9! gap-1.5"
			disabled={total === 0 || count >= total}
			onclick={onselectall}
			><ListChecks size={16} aria-hidden="true" /><span class="max-sm:sr-only"
				>{m.action_select_all()}</span
			></Button
		>
		<Button
			variant="outline"
			size="sm"
			class="min-h-9! gap-1.5"
			disabled={count === 0}
			onclick={onunselectall}
			><ListX size={16} aria-hidden="true" /><span class="max-sm:sr-only"
				>{m.action_unselect_all()}</span
			></Button
		>
		{@render actions?.()}
	</div>
</div>
