<script lang="ts">
	import { ChevronDown } from '@lucide/svelte';
	import Checkbox from '#lib/components/ui/checkbox/checkbox.svelte';
	import Popover from '#lib/components/ui/popover/popover.svelte';
	import { statusDots } from '#lib/components/ui/status-chip/status-chip.svelte';
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
		isDefault || selected.length === 0
			? 'Status'
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
	class="inline-flex min-h-10 items-center gap-2 rounded-full border border-border-strong bg-panel px-4 text-sm disabled:opacity-50 {isDefault
		? ''
		: 'font-semibold'}"
	panelClass="w-60"
>
	{#snippet trigger()}
		{summary}
		<ChevronDown size={16} aria-hidden="true" />
	{/snippet}
	<ul class="grid">
		{#each ALL_STATES as state (state)}
			<li>
				<label
					class="flex min-h-10 cursor-pointer items-center gap-2 rounded-control px-2 text-sm hover:bg-border/60"
				>
					<Checkbox
						checked={selected.includes(state)}
						onchange={(event) => toggle(state, event.currentTarget.checked)}
					/>
					<span aria-hidden="true" class="size-2 rounded-full {statusDots[state]}"></span>
					<span class="grow">{STATE_LABELS[state]}</span>
					<span class="text-muted-foreground">{counts[state]}</span>
				</label>
			</li>
		{/each}
	</ul>
	<div class="mt-1 flex justify-between border-t border-border px-2 pt-1 text-sm">
		<button type="button" class="min-h-9 underline" onclick={() => onchange([...ALL_STATES])}>
			Select all
		</button>
		<button type="button" class="min-h-9 underline" onclick={() => onchange([...DEFAULT_STATES])}>
			Reset
		</button>
	</div>
</Popover>
