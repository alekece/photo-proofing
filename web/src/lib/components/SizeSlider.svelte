<script lang="ts">
	import { SIZE_STEPS } from '#lib/settings/settings.js';

	let { value = $bindable(), label }: { value: number; label: string } = $props();
</script>

<div class="flex w-full min-w-0 items-center gap-3">
	<span aria-hidden="true" class="font-serif text-[0.8125rem] leading-none text-muted-foreground"
		>A</span
	>
	<input
		type="range"
		min="0"
		max={SIZE_STEPS - 1}
		step="1"
		aria-label={label}
		bind:value
		class="h-8 min-w-0 flex-1 accent-foreground"
	/>
	<span aria-hidden="true" class="font-serif text-2xl leading-none text-muted-foreground">A</span>
</div>
