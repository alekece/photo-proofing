<script lang="ts" generics="T extends string">
	import { Check, ChevronDown } from '@lucide/svelte';
	import type { Snippet } from 'svelte';
	import Popover from '#lib/components/ui/Popover/Popover.svelte';
	import { menuButton } from '#lib/components/ui/Popover/menu.js';

	let {
		label,
		summary,
		icon: TriggerIcon,
		disabled = false,
		activeCount = 0,
		value,
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
				options: readonly { value: T; label: string }[];
				onchange: (value: T) => void;
				children?: never;
		  }
		| { children: Snippet; value?: never; options?: never; onchange?: never }
	) = $props();

	let menu = $state<HTMLDivElement | null>(null);

	$effect(() => {
		if (!menu) return;

		(
			menu.querySelector<HTMLButtonElement>('[aria-checked="true"]') ??
			menu.querySelector<HTMLButtonElement>('button')
		)?.focus();
	});

	function navigate(
		event: KeyboardEvent & { currentTarget: HTMLButtonElement },
		close: () => void
	) {
		const { key, currentTarget } = event;

		if (key === 'Tab' && event.shiftKey) {
			event.preventDefault();
			close();
			return;
		}

		if (!menu || !['ArrowUp', 'ArrowDown', 'Home', 'End'].includes(key)) return;

		const items = [...menu.querySelectorAll<HTMLButtonElement>('button')];
		const index = items.indexOf(currentTarget);
		const next =
			key === 'Home'
				? 0
				: key === 'End'
					? items.length - 1
					: (index + (key === 'ArrowDown' ? 1 : -1) + items.length) % items.length;

		event.preventDefault();
		items[next]?.focus();
	}
</script>

<Popover
	{disabled}
	label={summary === label ? label : `${label}: ${summary}`}
	align="end"
	placement={options ? 'auto' : 'below'}
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
				>{activeCount}</span
			>
		{/if}
		<ChevronDown size={12} aria-hidden="true" class="shrink-0" />
	{/snippet}

	{#snippet children({ close })}
		{#if options}
			<div
				bind:this={menu}
				role="menu"
				aria-label={label}
				class="grid max-h-[min(18rem,var(--popover-space,40dvh))] overflow-y-auto overscroll-contain"
			>
				{#each options as option (option.value)}
					<button
						type="button"
						role="menuitemradio"
						aria-checked={option.value === value}
						tabindex="-1"
						class="flex min-h-10 cursor-pointer items-center gap-2.5 rounded-control px-3 text-left text-[0.8125rem] hover:bg-(--studio-choice)"
						onkeydown={(event) => navigate(event, close)}
						onclick={() => {
							close();
							if (option.value !== value) onchange?.(option.value);
						}}
					>
						<span class="grid w-4 shrink-0 place-items-center">
							{#if option.value === value}<Check size={14} aria-hidden="true" />{/if}
						</span>
						{option.label}
					</button>
				{/each}
			</div>
		{:else}
			{@render content?.()}
		{/if}
	{/snippet}
</Popover>
