<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Check, ChevronDown, ListFilter } from '@lucide/svelte';
	import Popover from '#lib/components/ui/Popover/Popover.svelte';
	import { menuButton } from '#lib/components/ui/Popover/menu.js';

	export type PhotoFilter = 'all' | 'commented';

	let {
		value,
		counts,
		onchange
	}: {
		value: PhotoFilter;
		counts: Record<PhotoFilter, number>;
		onchange: (next: PhotoFilter) => void;
	} = $props();

	const FILTERS: [PhotoFilter, () => string][] = [
		['all', m.photos_filter_all],
		['commented', m.photos_filter_commented]
	];

	let open = $state(false);
	const label = $derived(
		value === 'all' ? m.photos_filter_label() : FILTERS.find(([id]) => id === value)?.[1]()
	);
</script>

<Popover
	bind:open
	align="end"
	rootClass="w-40 shrink-0"
	class="{menuButton} {value === 'all' ? '' : 'font-semibold'}"
	panelClass="mt-1.5 w-56"
>
	{#snippet trigger()}
		<ListFilter size={16} aria-hidden="true" />
		<span class="min-w-0 flex-1 truncate text-left">{label}</span>
		<ChevronDown size={12} aria-hidden="true" />
	{/snippet}
	<div role="radiogroup" aria-label={m.photos_filter_label()} class="grid">
		{#each FILTERS as [id, name] (id)}
			<button
				type="button"
				role="radio"
				aria-checked={id === value}
				class="flex min-h-10 cursor-pointer items-center gap-2.5 rounded-control px-3 text-left text-[0.8125rem] hover:bg-border/60"
				onclick={() => {
					open = false;
					onchange(id);
				}}
			>
				<span class="grid w-4 place-items-center {id === value ? '' : 'invisible'}">
					<Check size={14} aria-hidden="true" />
				</span>
				<span class="grow">{name()}</span>
				<span class="text-xs text-muted-foreground">{counts[id]}</span>
			</button>
		{/each}
	</div>
</Popover>
