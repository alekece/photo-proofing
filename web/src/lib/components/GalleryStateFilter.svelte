<script lang="ts">
	import { ChevronDown, ListFilter } from '@lucide/svelte';
	import Checkbox from '#lib/components/ui/checkbox/checkbox.svelte';
	import Popover from '#lib/components/ui/popover/popover.svelte';
	import { statusDots } from '#lib/components/ui/status-chip/status-chip.svelte';
	import { menuButton } from '#lib/components/ui/popover/menu.js';
	import type { GalleryState } from '#lib/gallery/schemas.js';
	import { ALL_STATES, DEFAULT_STATES, STATE_LABELS, sameStates } from '#lib/gallery/states.js';

	let {
		selected,
		counts,
		disabled = false,
		onchange
	}: {
		selected: GalleryState[];
		counts: Record<GalleryState, number>;
		disabled?: boolean;
		onchange: (next: GalleryState[]) => void;
	} = $props();

	let isDefault = $derived(sameStates(selected, DEFAULT_STATES));
	let summary = $derived(
		isDefault
			? 'Status'
			: selected.length === 0
				? 'None'
				: selected.length === ALL_STATES.length
					? 'All statuses'
					: ALL_STATES.filter((state) => selected.includes(state))
							.map((state) => STATE_LABELS[state])
							.join(', ')
	);

	function toggle(state: GalleryState, checked: boolean) {
		onchange(
			checked
				? ALL_STATES.filter((chosen) => chosen === state || selected.includes(chosen))
				: selected.filter((chosen) => chosen !== state)
		);
	}
</script>

<Popover
	{disabled}
	rootClass="max-sm:flex-1"
	class="{menuButton} {isDefault ? '' : 'font-semibold'}"
	panelClass="mt-1.5 w-60 max-sm:left-0"
>
	{#snippet trigger()}
		<ListFilter size={16} aria-hidden="true" />
		<span class="max-sm:grow max-sm:text-left">{summary}</span>
		<ChevronDown size={12} aria-hidden="true" />
	{/snippet}
	<ul class="grid">
		{#each ALL_STATES as state (state)}
			<li>
				<label
					class="flex min-h-10 cursor-pointer items-center gap-2 rounded-control px-2 text-[0.8125rem] hover:bg-border/60"
				>
					<Checkbox
						class="size-4 accent-foreground"
						checked={selected.includes(state)}
						onchange={(event) => toggle(state, event.currentTarget.checked)}
					/>
					<span aria-hidden="true" class="size-[7px] rounded-full {statusDots[state]}"></span>
					<span class="grow">{STATE_LABELS[state]}</span>
					<span class="text-xs text-muted-foreground">{counts[state]}</span>
				</label>
			</li>
		{/each}
	</ul>
	<div
		class="mx-1 mt-1.5 flex justify-between border-t border-border px-2 pt-2 pb-1 text-[0.8125rem]"
	>
		<button
			type="button"
			class="cursor-pointer text-danger"
			onclick={() => onchange([...ALL_STATES])}
		>
			Select all
		</button>
		<button
			type="button"
			class="cursor-pointer text-danger"
			onclick={() => onchange([...DEFAULT_STATES])}
		>
			Reset
		</button>
	</div>
</Popover>
