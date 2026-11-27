<script lang="ts" generics="T extends string">
	import { ChevronDown } from '@lucide/svelte';
	import type { Snippet } from 'svelte';
	import Popover from '#lib/components/ui/Popover/Popover.svelte';
	import { menuButton } from '#lib/components/ui/Popover/menu.js';
	import Select, { type SelectSurface } from '#lib/components/ui/Select/Select.svelte';

	let {
		label,
		summary,
		icon: TriggerIcon,
		disabled = false,
		activeCount = 0,
		value,
		surface = 'panel',
		options,
		onchange,
		children: content
	}: {
		label: string;
		summary: string;
		icon: typeof ChevronDown;
		disabled?: boolean;
		activeCount?: number;
	} & (
		| {
				value: T;
				surface?: SelectSurface;
				options: readonly { value: T; label: string }[];
				onchange: (value: T) => void;
				children?: never;
		  }
		| { children: Snippet; value?: never; surface?: never; options?: never; onchange?: never }
	) = $props();
</script>

{#if options && value !== undefined}
	<Select
		toolbar
		{surface}
		{label}
		{summary}
		icon={TriggerIcon}
		{disabled}
		{activeCount}
		{value}
		{options}
		{onchange}
	/>
{:else}
	<Popover
		{disabled}
		label={summary === label ? label : `${label}: ${summary}`}
		align="end"
		rootClass="w-fit max-w-full max-sm:static max-sm:flex-[1_1_7rem]"
		class={menuButton}
		panelClass="sm:w-56 max-sm:inset-x-0 max-sm:w-auto"
	>
		{#snippet trigger()}
			<TriggerIcon size={16} aria-hidden="true" class="shrink-0" />
			<span class="min-w-0 flex-1 truncate text-left" title={summary}>{summary}</span>
			{#if activeCount > 0}
				<span
					aria-hidden="true"
					class="grid min-w-5 shrink-0 place-items-center rounded-full bg-foreground px-1 text-xs text-background sm:hidden"
				>
					{activeCount}
				</span>
			{/if}
			<ChevronDown size={12} aria-hidden="true" class="shrink-0" />
		{/snippet}

		{@render content?.()}
	</Popover>
{/if}
