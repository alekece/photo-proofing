<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Lock } from '@lucide/svelte';
	import type { Component, Snippet } from 'svelte';
	import { MediaQuery } from 'svelte/reactivity';

	type Item = {
		id: string;
		label: string;
		/** Shown in the vertical sidebar, where it stays alone once the sidebar is collapsed. */
		icon?: Component<{ size?: number; class?: string; 'aria-hidden'?: boolean | 'true' | 'false' }>;
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
		listClass = '',
		sidebar = false,
		header,
		panel
	}: {
		value: string;
		items: readonly Item[];
		label?: string;
		listClass?: string;
		sidebar?: boolean;
		header?: Snippet;
		panel: Snippet<[string]>;
	} = $props();

	const desktop = typeof matchMedia === 'function' ? new MediaQuery('min-width: 48rem') : null;
	const vertical = $derived(sidebar && desktop?.current === true);

	function moveTab(event: KeyboardEvent) {
		const available = items.filter(({ disabled }) => !disabled);
		if (available.length === 0) return;

		const index = available.findIndex(({ id }) => id === value);
		const next = {
			[vertical ? 'ArrowDown' : 'ArrowRight']: (index + 1) % available.length,
			[vertical ? 'ArrowUp' : 'ArrowLeft']: (index - 1 + available.length) % available.length,
			Home: 0,
			End: available.length - 1
		}[event.key];
		if (next === undefined) return;

		event.preventDefault();

		const id = available[next].id;

		value = id;
		const tab = document.getElementById(`tab-${id}`);
		tab?.focus();
		const strip = tab?.parentElement;
		if (tab && strip && strip.scrollWidth > strip.clientWidth) {
			tab.scrollIntoView({ block: 'nearest', inline: 'nearest' });
		}
	}
</script>

{#snippet tablist()}
	<div
		role="tablist"
		aria-label={label}
		aria-orientation={vertical ? 'vertical' : undefined}
		class={sidebar
			? 'flex min-w-0 items-center gap-1.5 px-3.5 py-2 max-md:[scrollbar-width:none] max-md:overflow-x-auto md:min-h-0 md:flex-1 md:flex-col md:items-stretch md:gap-0.5 md:overflow-x-hidden md:overflow-y-auto md:px-3 md:pt-4 md:pb-6'
			: `mb-4 flex gap-4 border-b border-border sm:gap-6 ${listClass}`}
	>
		{#each items as { id, label, icon: Icon, count, accent, locked, disabled, title } (id)}
			<button
				type="button"
				role="tab"
				id={`tab-${id}`}
				aria-selected={value === id}
				aria-controls={`panel-${id}`}
				disabled={disabled ?? false}
				{title}
				tabindex={value === id ? 0 : -1}
				class={sidebar
					? 'menu-choice min-h-8 flex-auto shrink-0 cursor-pointer justify-center px-2 text-center leading-[1.2] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-45 md:w-full md:flex-none md:justify-start md:text-left md:whitespace-normal [&:not([aria-selected=true])]:enabled:hover:bg-(--studio-hover,var(--border))'
					: `-mb-px flex items-baseline gap-1.5 border-b-2 pt-1.5 pb-[0.5625rem] text-[0.90625rem] leading-[1.2] enabled:hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50 sm:gap-2 sm:pb-2.5 sm:text-[0.9375rem] ${
							value === id
								? 'border-foreground font-semibold text-foreground'
								: 'border-transparent font-medium text-muted-foreground'
						}`}
				onclick={() => (value = id)}
				onkeydown={moveTab}
			>
				{#if sidebar && Icon}
					<Icon size={16} aria-hidden="true" class="shrink-0 max-md:hidden" />
				{/if}
				<span class={sidebar && Icon ? '' : undefined}>{label}</span>
				{#if count != null}
					<em
						class="pill border text-[0.65625rem] font-bold tracking-[0.04em] not-italic {value ===
						id
							? 'bg-white'
							: 'bg-panel'} {accent
							? 'border-danger/40 font-extrabold text-danger'
							: value === id
								? 'border-[#cdbda9] text-foreground'
								: 'border-border text-muted-foreground'}"
					>
						{count}
					</em>
				{/if}
				{#if locked}
					<Lock size={12} aria-label={m.tab_locked()} />
				{/if}
			</button>
		{/each}
	</div>
{/snippet}

<div
	class={sidebar
		? `grid min-w-0 flex-1 grid-cols-[minmax(0,1fr)] motion-reduce:transition-none md:grid-cols-[var(--rail-expanded-width)_minmax(0,1fr)] md:transition-[grid-template-columns] md:duration-200 md:ease-out ${
				header
					? 'grid-rows-[auto_auto_1fr] md:grid-rows-[auto_1fr]'
					: 'grid-rows-[auto_1fr] md:grid-rows-[1fr]'
			}`
		: 'contents'}
>
	{#if header}
		{@render header()}
	{/if}
	{#if sidebar}
		<div
			class="relative flex min-w-0 flex-col border-b border-border bg-panel md:border-r md:border-b-0 {header
				? 'md:col-start-1 md:row-span-2 md:row-start-1'
				: ''} {listClass}"
		>
			{@render tablist()}
		</div>
	{:else}
		{@render tablist()}
	{/if}
	<div
		role="tabpanel"
		id={`panel-${value}`}
		aria-labelledby={`tab-${value}`}
		class="grid min-w-0 content-start gap-4 {sidebar
			? 'workspace-inset pt-4 pb-10 sm:pt-5 sm:pb-12'
			: ''}"
	>
		{@render panel(value)}
	</div>
</div>
