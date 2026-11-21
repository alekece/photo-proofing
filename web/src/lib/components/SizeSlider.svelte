<script lang="ts">
	import { SIZE_STEPS } from '#lib/settings/settings.js';

	let { value = $bindable(), label }: { value: number; label: string } = $props();
</script>

<label class="grid min-w-0 gap-1 text-sm font-semibold">
	{label}
	<input
		type="range"
		min="0"
		max={SIZE_STEPS - 1}
		step="1"
		bind:value
		class="h-11 w-24 accent-foreground"
	/>
</label>
