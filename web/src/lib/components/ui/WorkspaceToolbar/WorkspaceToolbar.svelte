<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { cn } from '#lib/utils.js';
	import { Search } from '@lucide/svelte';
	import type { Snippet } from 'svelte';

	let {
		search,
		filters,
		sort,
		actions,
		onclear,
		resultsHeight = 0,
		class: className
	}: {
		search?: {
			label: string;
			value: string;
			disabled?: boolean;
			oninput: (value: string) => void;
		};
		filters?: Snippet;
		sort?: Snippet;
		actions?: Snippet;
		onclear?: () => void;
		resultsHeight?: number;
		class?: string;
	} = $props();

	let viewportHeight = $state(0);
</script>

<svelte:window bind:innerHeight={viewportHeight} />

<div
	data-slot="workspace-toolbar"
	class={cn(
		'flex flex-wrap items-center gap-2 rounded-[0.875rem] border border-border bg-panel p-2 sm:gap-3 sm:px-3 sm:py-2.5',
		viewportHeight > 0 && resultsHeight > viewportHeight ? 'sticky top-2 z-[6]' : 'relative',
		className
	)}
>
	{#if search}
		<label
			class="flex min-h-9 min-w-32 flex-1 items-center gap-2 rounded-control px-2 text-muted-foreground has-focus-visible:outline-2 has-focus-visible:outline-focus/40 max-sm:basis-full"
		>
			<Search size={16} aria-hidden="true" class="shrink-0" />
			<input
				type="search"
				aria-label={search.label}
				placeholder={search.label}
				value={search.value}
				disabled={search.disabled}
				oninput={(event) => search?.oninput(event.currentTarget.value)}
				class="w-full min-w-0 border-0 bg-transparent text-[0.9375rem] text-foreground outline-none placeholder:text-muted-foreground focus-visible:shadow-none focus-visible:outline-none"
			/>
		</label>
	{/if}

	<div class="ml-auto flex min-w-0 flex-wrap items-center justify-end gap-2 max-sm:grow">
		{@render filters?.()}
		{@render sort?.()}
		{#if onclear}
			<button
				type="button"
				class="min-h-9 shrink-0 cursor-pointer rounded-control px-2 text-sm text-danger hover:underline"
				onclick={onclear}>{m.filter_reset()}</button
			>
		{/if}
		{@render actions?.()}
	</div>
</div>
