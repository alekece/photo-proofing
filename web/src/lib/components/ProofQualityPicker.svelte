<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { QUALITIES, type Quality } from '#lib/settings/settings.js';

	let { value = $bindable() }: { value: Quality } = $props();

	const LABELS: Record<Quality, { name: () => string; detail: () => string }> = {
		low: { name: m.quality_low, detail: m.quality_low_detail },
		medium: { name: m.quality_medium, detail: m.quality_medium_detail },
		high: { name: m.quality_high, detail: m.quality_high_detail }
	};
</script>

<div
	role="radiogroup"
	aria-label={m.quality_label()}
	class="grid grid-cols-[repeat(auto-fit,minmax(11rem,1fr))] gap-[1.125rem]"
>
	{#each QUALITIES as quality (quality)}
		<button
			type="button"
			role="radio"
			aria-checked={value === quality}
			class="grid cursor-pointer gap-1 rounded-xl border-[1.5px] px-4 py-3 text-left transition-colors {value ===
			quality
				? 'border-foreground bg-foreground text-inverse'
				: 'border-border bg-input hover:border-[#cdbda8]'}"
			onclick={() => (value = quality)}
		>
			<span class="font-semibold">{LABELS[quality].name()}</span>
			<span class="text-sm opacity-75">{LABELS[quality].detail()}</span>
		</button>
	{/each}
</div>
