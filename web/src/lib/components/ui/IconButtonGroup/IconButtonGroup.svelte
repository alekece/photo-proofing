<script lang="ts">
	import type { Component } from 'svelte';

	type Item = { id: string; label: string; icon: Component<{ size?: number }>; disabled?: boolean };

	let {
		value = $bindable(),
		items,
		label,
		onchange
	}: {
		value: string;
		items: readonly Item[];
		label: string;
		onchange?: (value: string) => void;
	} = $props();
</script>

<div role="group" aria-label={label} class="inline-flex gap-1.5">
	{#each items as { id, label: itemLabel, icon: Icon, disabled } (id)}
		<button
			type="button"
			aria-label={itemLabel}
			aria-pressed={value === id}
			title={itemLabel}
			{disabled}
			class="grid h-[2.125rem] w-10 place-items-center rounded-lg border transition-colors {value ===
			id
				? 'border-foreground bg-foreground text-inverse'
				: 'border-border bg-white text-muted-foreground hover:border-[#cdbda9] hover:text-foreground'} disabled:cursor-not-allowed disabled:opacity-50"
			onclick={() => {
				value = id;
				onchange?.(id);
			}}
		>
			<Icon size={18} />
		</button>
	{/each}
</div>
