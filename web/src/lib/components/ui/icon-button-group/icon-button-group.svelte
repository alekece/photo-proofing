<script lang="ts">
	import type { Component } from 'svelte';

	type Item = { id: string; label: string; icon: Component<{ size?: number }> };

	let {
		value = $bindable(),
		items,
		label
	}: { value: string; items: readonly Item[]; label: string } = $props();
</script>

<div role="group" aria-label={label} class="inline-flex gap-1">
	{#each items as { id, label: itemLabel, icon: Icon } (id)}
		<button
			type="button"
			aria-label={itemLabel}
			aria-pressed={value === id}
			title={itemLabel}
			class="grid size-10 place-items-center rounded-control border {value === id
				? 'border-foreground bg-foreground text-inverse'
				: 'border-border-strong bg-transparent text-foreground'}"
			onclick={() => (value = id)}
		>
			<Icon size={18} />
		</button>
	{/each}
</div>
