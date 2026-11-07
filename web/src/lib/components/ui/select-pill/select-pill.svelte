<script lang="ts">
	import { X } from '@lucide/svelte';
	import type { Snippet } from 'svelte';

	let {
		count,
		actionLabel,
		actionDisabled = false,
		actionBusy = false,
		destructive = false,
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
		/** Styles the action as destructive (Delete) rather than neutral (Download). */
		destructive?: boolean;
		onaction: () => void;
		onselectall: () => void;
		onclose: () => void;
		icon?: Snippet;
	} = $props();

	const pill =
		'inline-flex min-h-9 cursor-pointer items-center gap-1.5 rounded-full px-3.5 text-[0.84375rem] font-semibold focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-foreground disabled:cursor-default disabled:opacity-50';
</script>

<div data-slot="select-pill" class="flex flex-1 flex-wrap items-center gap-3 text-foreground">
	<button
		type="button"
		class="{pill} grid size-9 place-items-center px-0 hover:bg-border/60"
		aria-label="Cancel selection"
		onclick={onclose}
	>
		<X size={16} aria-hidden="true" />
	</button>
	<strong class="text-sm font-semibold">{count} selected</strong>
	<span class="flex-1"></span>
	<button
		type="button"
		class="{pill} border border-border bg-panel hover:bg-border/60"
		onclick={onselectall}>Select all</button
	>
	<button
		type="button"
		class="{pill} {destructive ? 'bg-danger text-inverse' : 'bg-foreground text-inverse'}"
		disabled={actionDisabled || actionBusy}
		aria-busy={actionBusy}
		onclick={onaction}
	>
		{@render icon?.()}
		{actionBusy && actionLabel === 'Download' ? 'Downloading…' : actionLabel}
	</button>
</div>
