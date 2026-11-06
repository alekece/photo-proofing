<script lang="ts">
	import { Lock } from '@lucide/svelte';
	import type { Snippet } from 'svelte';

	type Item = {
		id: string;
		label: string;
		/** Shown after the label once known. */
		count?: number | null;
		/** Makes the count bold and accent-coloured, e.g. over an allowance. */
		accent?: boolean;
		/** Shows a padlock: the tab's content can no longer change. */
		locked?: boolean;
	};

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

<div role="tablist" aria-label={label} class="mb-4 flex gap-4 border-b border-border sm:gap-6">
	{#each items as { id, label, count, accent, locked } (id)}
		<button
			type="button"
			role="tab"
			id={`tab-${id}`}
			aria-selected={value === id}
			aria-controls={`panel-${id}`}
			tabindex={value === id ? 0 : -1}
			class="-mb-px flex items-center gap-2 border-b-2 pt-1.5 pb-2.5 text-[0.9rem] font-medium sm:text-[0.9375rem] {value ===
			id
				? 'border-foreground text-foreground'
				: 'border-transparent text-muted-foreground'}"
			onclick={() => (value = id)}
			onkeydown={moveTab}
		>
			{label}
			{#if count != null}
				<em
					class="rounded-full border px-1.5 py-px text-[0.65625rem] font-bold not-italic {value ===
					id
						? 'bg-white'
						: 'bg-panel'} {accent
						? 'border-danger/40 text-danger'
						: 'border-border-strong text-muted-foreground'}">{count}</em
				>
			{/if}
			{#if locked}
				<Lock size={12} aria-label="Locked" />
			{/if}
		</button>
	{/each}
</div>
<div role="tabpanel" id={`panel-${value}`} aria-labelledby={`tab-${value}`} class="grid gap-4">
	{@render panel(value)}
</div>
