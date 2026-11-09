<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import type { Snippet } from 'svelte';
	import { X } from '@lucide/svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import Checkbox from '#lib/components/ui/Checkbox/Checkbox.svelte';

	/**
	 * The contextual bar of select mode: exit, count, a tri-state "Select all" and the actions that
	 * fit a selection. It sticks to the top of the page while a long gallery scrolls.
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
	const some = $derived(count > 0 && !all);
</script>

<div
	data-slot="select-actions"
	class="sticky top-0 z-20 flex items-center gap-3 rounded-md border border-hero/40 bg-golden-hour px-2 py-1.5 shadow-sm"
>
	<Button
		variant="ghost"
		size="sm"
		class="min-h-9! px-2.5!"
		aria-label={m.selection_exit()}
		onclick={onexit}><X size={18} aria-hidden="true" /></Button
	>
	<strong class="text-sm font-semibold">{m.notification_selected({ count })}</strong>
	<label class="flex cursor-pointer items-center gap-2 text-sm">
		<Checkbox
			class="size-4 cursor-pointer accent-foreground"
			checked={all}
			indeterminate={some}
			disabled={total === 0}
			onchange={() => (all ? onunselectall() : onselectall())}
		/>
		<span class="max-sm:sr-only">{m.action_select_all()}</span>
	</label>
	<div class="ml-auto flex items-center gap-2">{@render actions?.()}</div>
</div>
