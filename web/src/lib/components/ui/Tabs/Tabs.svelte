<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
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
		/** Visible but unavailable at the current stage of the gallery. */
		disabled?: boolean;
		/** Explains an unavailable tab on hover. */
		title?: string;
	};

	let {
		value = $bindable(),
		items,
		label,
		panel
	}: { value: string; items: readonly Item[]; label?: string; panel: Snippet<[string]> } = $props();

	function moveTab(event: KeyboardEvent) {
		const available = items.filter(({ disabled }) => !disabled);
		if (available.length === 0) return;

		const index = available.findIndex(({ id }) => id === value);
		const next = {
			ArrowRight: (index + 1) % available.length,
			ArrowLeft: (index - 1 + available.length) % available.length,
			Home: 0,
			End: available.length - 1
		}[event.key];
		if (next === undefined) return;

		event.preventDefault();

		const id = available[next].id;

		value = id;
		document.getElementById(`tab-${id}`)?.focus();
	}
</script>

<div role="tablist" aria-label={label} class="mb-4 flex gap-4 border-b border-border sm:gap-6">
	{#each items as { id, label, count, accent, locked, disabled, title } (id)}
		<button
			type="button"
			role="tab"
			id={`tab-${id}`}
			aria-selected={value === id}
			aria-controls={`panel-${id}`}
			disabled={disabled ?? false}
			{title}
			tabindex={value === id ? 0 : -1}
			class="-mb-px flex items-baseline gap-1.5 border-b-2 pt-1.5 pb-[0.5625rem] text-[0.90625rem] leading-[1.2] enabled:hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50 sm:gap-2 sm:pb-2.5 sm:text-[0.9375rem] {value ===
			id
				? 'border-foreground font-semibold text-foreground'
				: 'border-transparent font-medium text-muted-foreground'}"
			onclick={() => (value = id)}
			onkeydown={moveTab}
		>
			{label}
			{#if count != null}
				<em
					class="rounded-full border px-1.5 py-px text-[0.65625rem] font-bold tracking-[0.04em] not-italic {value ===
					id
						? 'bg-white'
						: 'bg-panel'} {accent
						? 'border-danger/40 font-extrabold text-danger'
						: value === id
							? 'border-[#cdbda9] text-foreground'
							: 'border-border text-muted-foreground'}">{count}</em
				>
			{/if}
			{#if locked}
				<Lock size={12} aria-label={m.tab_locked()} />
			{/if}
		</button>
	{/each}
</div>
<div role="tabpanel" id={`panel-${value}`} aria-labelledby={`tab-${value}`} class="grid gap-4">
	{@render panel(value)}
</div>
