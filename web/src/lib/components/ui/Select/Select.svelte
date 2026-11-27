<script module lang="ts">
	export type SelectSurface = 'input' | 'panel' | 'transparent';

	const triggerSurface: Record<SelectSurface, string> = {
		input: 'bg-input aria-expanded:hover:bg-input',
		panel: 'bg-panel aria-expanded:hover:bg-panel',
		transparent: 'bg-transparent aria-expanded:hover:bg-transparent'
	};
</script>

<script lang="ts" generics="T extends string">
	import { Check, ChevronDown } from '@lucide/svelte';
	import { cn } from '#lib/utils.js';
	import Popover from '#lib/components/ui/Popover/Popover.svelte';
	import { menuButton } from '#lib/components/ui/Popover/menu.js';

	let {
		id,
		label,
		'aria-describedby': describedBy,
		summary,
		icon: Icon,
		disabled = false,
		activeCount = 0,
		toolbar = false,
		surface = 'input',
		value = $bindable(),
		options,
		onchange
	}: {
		id?: string;
		label: string;
		'aria-describedby'?: string;
		summary?: string;
		icon?: typeof ChevronDown;
		disabled?: boolean;
		activeCount?: number;
		toolbar?: boolean;
		surface?: SelectSurface;
		value: T;
		options: readonly { value: T; label: string }[];
		onchange?: (value: T) => void;
	} = $props();

	const caption = $derived(
		summary ?? options.find((option) => option.value === value)?.label ?? label
	);
	let menu = $state<HTMLDivElement | null>(null);

	$effect(() => {
		if (!menu) return;

		(
			menu.querySelector<HTMLButtonElement>('[aria-checked="true"]') ??
			menu.querySelector<HTMLButtonElement>('button')
		)?.focus();
	});

	function navigate(event: KeyboardEvent, close: () => void) {
		const { key, currentTarget } = event;

		if (key === 'Escape' || (key === 'Tab' && event.shiftKey)) {
			event.preventDefault();
			event.stopPropagation();
			close();
			return;
		}

		if (!menu || !['ArrowUp', 'ArrowDown', 'Home', 'End'].includes(key)) return;

		const items = [...menu.querySelectorAll<HTMLButtonElement>('button')];
		const index = items.indexOf(currentTarget as HTMLButtonElement);
		const next =
			key === 'Home'
				? 0
				: key === 'End'
					? items.length - 1
					: (index + (key === 'ArrowDown' ? 1 : -1) + items.length) % items.length;

		event.preventDefault();
		items[next]?.focus();
	}

	function choose(next: T, close: () => void) {
		close();
		if (next === value) return;

		if (onchange) onchange(next);
		else value = next;
	}
</script>

<Popover
	{id}
	{disabled}
	aria-describedby={describedBy}
	label={caption === label ? label : `${label}: ${caption}`}
	align={toolbar ? 'end' : 'start'}
	placement="auto"
	rootClass={toolbar ? 'w-fit max-w-full max-sm:static max-sm:flex-[1_1_7rem]' : 'w-full min-w-0'}
	class={cn(
		menuButton,
		'hover:bg-background',
		triggerSurface[surface],
		!toolbar && 'disabled:opacity-50'
	)}
	panelClass={cn(
		surface === 'input' && 'bg-input',
		toolbar ? 'sm:w-56 max-sm:inset-x-0 max-sm:w-auto' : 'w-full min-w-0'
	)}
>
	{#snippet trigger()}
		{#if Icon}<Icon size={16} aria-hidden="true" class="shrink-0" />{/if}
		<span class="min-w-0 flex-1 truncate text-left" title={caption}>{caption}</span>
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

	{#snippet children({ close })}
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
					class="flex min-h-10 cursor-pointer items-center gap-2.5 rounded-control px-3 text-left text-[0.8125rem] hover:bg-background"
					onkeydown={(event) => navigate(event, close)}
					onclick={() => choose(option.value, close)}
				>
					<span class="grid w-4 shrink-0 place-items-center">
						{#if option.value === value}<Check size={14} aria-hidden="true" />{/if}
					</span>
					{option.label}
				</button>
			{/each}
		</div>
	{/snippet}
</Popover>
