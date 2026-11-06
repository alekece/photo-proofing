<script lang="ts">
	import { X } from '@lucide/svelte';
	import type { Snippet } from 'svelte';

	let {
		count,
		actionLabel,
		actionDisabled = false,
		actionBusy = false,
		onaction,
		onselectall,
		onclose,
		icon
	}: {
		count: number;
		/** The tab's one action: Delete on Photos and Final, Download on Favorites. */
		actionLabel: string;
		actionDisabled?: boolean;
		/** Shows the action as working, e.g. while a ZIP is prepared. */
		actionBusy?: boolean;
		onaction: () => void;
		onselectall: () => void;
		onclose: () => void;
		icon?: Snippet;
	} = $props();

	const pill =
		'inline-flex min-h-9 cursor-pointer items-center gap-1.5 rounded-full px-3 text-sm focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-foreground disabled:cursor-default disabled:opacity-50';
</script>

<div
	data-slot="select-pill"
	class="mr-auto inline-flex max-w-full flex-wrap items-center gap-1 rounded-full bg-foreground p-1 text-inverse"
>
	<button
		type="button"
		class="{pill} grid size-9 place-items-center px-0 hover:bg-white/15"
		aria-label="Cancel selection"
		onclick={onclose}
	>
		<X size={16} aria-hidden="true" />
	</button>
	<span class="px-1 text-sm">{count} selected</span>
	<button type="button" class="{pill} hover:bg-white/15" onclick={onselectall}>Select all</button>
	<button
		type="button"
		class="{pill} bg-inverse font-semibold text-foreground"
		disabled={actionDisabled || actionBusy}
		aria-busy={actionBusy}
		onclick={onaction}
	>
		{@render icon?.()}
		{actionLabel}
	</button>
</div>
