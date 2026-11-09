<script lang="ts">
	import { SIZE_STEPS } from '#lib/settings/settings.js';

	let { value = $bindable(), label }: { value: number; label: string } = $props();
</script>

<div
	class="flex min-w-[12.5rem] flex-1 items-center border-l border-border px-2 max-sm:basis-full max-sm:border-l-0"
>
	<div class="flex w-full items-center gap-3 px-2">
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
			class="h-7 flex-1 accent-foreground"
		/>
		<span aria-hidden="true" class="font-serif text-2xl leading-none text-muted-foreground">A</span>
	</div>
</div>
