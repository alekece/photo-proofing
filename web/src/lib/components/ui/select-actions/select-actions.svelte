<script lang="ts">
	import type { Snippet } from 'svelte';
	import Button from '#lib/components/ui/button/button.svelte';

	let {
		count,
		onselectall,
		onunselectall,
		children
	}: {
		count: number;
		onselectall: () => void;
		onunselectall: () => void;
		/** The tab's own selection actions, such as Delete. */
		children?: Snippet;
	} = $props();

	let selectAll = $state<HTMLButtonElement | null>(null);

	// The button disables itself once the selection is empty, so focus moves on with it.
	function unselectAll() {
		onunselectall();
		selectAll?.focus();
	}
</script>

<div data-slot="select-actions" class="mr-auto flex flex-wrap items-center gap-2">
	<strong class="mr-1 text-sm font-semibold">{count} selected</strong>
	<Button bind:ref={selectAll} variant="outline" size="sm" class="min-h-9!" onclick={onselectall}
		>Select all</Button
	>
	<Button variant="outline" size="sm" class="min-h-9!" disabled={count === 0} onclick={unselectAll}
		>Unselect all</Button
	>
	{@render children?.()}
</div>
