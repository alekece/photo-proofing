<script lang="ts">
	import type { Component } from 'svelte';
	import { cn } from '#lib/utils.js';

	type Item = { id: string; label: string; icon?: Component<{ size?: number }> };

	let {
		value = $bindable(),
		items,
		label,
		class: className,
		onchange
	}: {
		value: string;
		items: readonly Item[];
		label: string;
		class?: string;
		onchange?: (value: string) => void;
	} = $props();
</script>

<div
	role="group"
	aria-label={label}
	class={cn('inline-flex overflow-hidden rounded-full border border-border bg-white', className)}
>
	{#each items as { id, label: itemLabel, icon: Icon } (id)}
		<button
			type="button"
			aria-pressed={value === id}
			class="inline-flex min-h-9 items-center gap-1.5 px-3.5 text-sm font-semibold transition-colors {value ===
			id
				? 'bg-[#efe7dc] text-foreground'
				: 'text-muted-foreground hover:text-foreground'}"
			onclick={() => {
				value = id;
				onchange?.(id);
			}}
		>
			{#if Icon}<Icon size={16} />{/if}{itemLabel}
		</button>
	{/each}
</div>
