<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import ChoiceCards from '#lib/components/ui/ChoiceCards/ChoiceCards.svelte';
	import { QUALITIES, type Quality } from '#lib/settings/settings.js';

	let { value = $bindable(), describedBy }: { value: Quality; describedBy?: string } = $props();

	const LABELS: Record<Quality, { name: () => string; detail: () => string }> = {
		low: { name: m.quality_low, detail: m.quality_low_detail },
		medium: { name: m.quality_medium, detail: m.quality_medium_detail },
		high: { name: m.quality_high, detail: m.quality_high_detail }
	};
	const options = $derived(
		QUALITIES.map((quality) => ({
			value: quality,
			label: LABELS[quality].name(),
			detail: LABELS[quality].detail()
		}))
	);
</script>

<ChoiceCards bind:value label={m.quality_label()} {describedBy} {options} />
