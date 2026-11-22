<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { ListFilter } from '@lucide/svelte';
	import ToolbarMenu from '#lib/components/ui/ToolbarMenu/ToolbarMenu.svelte';
	import type { GalleryState } from '#lib/gallery/schemas.js';
	import { ALL_STATES, STATE_LABELS } from '#lib/gallery/states.js';

	let {
		selected,
		counts,
		disabled = false,
		onchange
	}: {
		selected: GalleryState | null;
		counts: Record<GalleryState, number>;
		disabled?: boolean;
		onchange: (next: GalleryState | null) => void;
	} = $props();

	const options = $derived([
		{ value: '' as const, label: m.filter_all() },
		...ALL_STATES.map((state) => ({
			value: state,
			label: `${STATE_LABELS[state]()} (${counts[state]})`
		}))
	]);
</script>

<ToolbarMenu
	{disabled}
	label={m.filter_status()}
	summary={selected ? STATE_LABELS[selected]() : m.filter_status()}
	icon={ListFilter}
	value={selected ?? ''}
	{options}
	onchange={(value) => onchange(value || null)}
/>
