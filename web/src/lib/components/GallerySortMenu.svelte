<script lang="ts">
	import { ArrowUpDown, Check, ChevronDown } from '@lucide/svelte';
	import Popover from '#lib/components/ui/popover/popover.svelte';
	import { menuButton } from '#lib/components/ui/popover/menu.js';
	import type { GallerySort } from '#lib/gallery/schemas.js';

	const SORTS: [GallerySort, string][] = [
		['updated', 'Recently updated'],
		['created', 'Newest created'],
		['oldest', 'Oldest created'],
		['title', 'Title A–Z']
	];

	let {
		value,
		disabled = false,
		onchange
	}: {
		value: GallerySort;
		disabled?: boolean;
		onchange: (next: GallerySort) => void;
	} = $props();

	let open = $state(false);
	let label = $derived(SORTS.find(([sort]) => sort === value)?.[1]);
</script>

<Popover
	bind:open
	{disabled}
	align="end"
	rootClass="max-sm:flex-1"
	class={menuButton}
	panelClass="mt-1.5 w-56 max-sm:right-0"
>
	{#snippet trigger()}
		<ArrowUpDown size={16} aria-hidden="true" />
		<span class="max-sm:grow max-sm:text-left">{label}</span>
		<ChevronDown size={12} aria-hidden="true" />
	{/snippet}
	<div role="radiogroup" aria-label="Sort by" class="grid">
		{#each SORTS as [sort, name] (sort)}
			<button
				type="button"
				role="radio"
				aria-checked={sort === value}
				class="flex min-h-10 cursor-pointer items-center gap-2.5 rounded-control px-3 text-left text-[0.8125rem] hover:bg-border/60"
				onclick={() => {
					open = false;
					onchange(sort);
				}}
			>
				<span class="grid w-4 place-items-center {sort === value ? '' : 'invisible'}">
					<Check size={14} aria-hidden="true" />
				</span>
				{name}
			</button>
		{/each}
	</div>
</Popover>
