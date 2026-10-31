<script lang="ts">
	import type { Snippet } from 'svelte';

	type Item = { id: string; label: string };

	let {
		value = $bindable(),
		items,
		label,
		panel
	}: { value: string; items: readonly Item[]; label?: string; panel: Snippet<[string]> } = $props();

	function moveTab(event: KeyboardEvent) {
		const index = items.findIndex(({ id }) => id === value);
		const next = {
			ArrowRight: (index + 1) % items.length,
			ArrowLeft: (index - 1 + items.length) % items.length,
			Home: 0,
			End: items.length - 1
		}[event.key];
		if (next === undefined) return;

		event.preventDefault();

		const id = items[next].id;

		value = id;
		document.getElementById(`tab-${id}`)?.focus();
	}
</script>

<div role="tablist" aria-label={label} class="mb-4 flex gap-6 border-b border-border-strong">
	{#each items as { id, label } (id)}
		<button
			type="button"
			role="tab"
			id={`tab-${id}`}
			aria-selected={value === id}
			aria-controls={`panel-${id}`}
			tabindex={value === id ? 0 : -1}
			class="border-b-2 px-0 pb-2 text-xs tracking-[0.2em] uppercase {value === id
				? 'border-foreground text-foreground'
				: 'border-transparent text-muted-foreground'}"
			onclick={() => (value = id)}
			onkeydown={moveTab}
		>
			{label}
		</button>
	{/each}
</div>
<div role="tabpanel" id={`panel-${value}`} aria-labelledby={`tab-${value}`} class="grid gap-4">
	{@render panel(value)}
</div>
