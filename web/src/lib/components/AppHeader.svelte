<script lang="ts">
	import { ArrowLeft, Bell, LogOut, Settings, User } from '@lucide/svelte';
	import Popover from '#lib/components/ui/popover/popover.svelte';
	import { cn } from '#lib/utils.js';
	import type { Snippet } from 'svelte';

	let {
		back,
		crumb,
		overlay = false,
		actions,
		onsignout
	}: {
		back?: { href: string; label: string };
		/** The page's own name beside the brand, e.g. the gallery title. */
		crumb?: string;
		/** Floats the bar over the hero banner below it instead of taking a row of its own. */
		overlay?: boolean;
		/** Replaces the bell, once #72 provides its data. */
		actions?: Snippet;
		onsignout?: () => void | Promise<void>;
	} = $props();

	const round = 'grid size-[2.375rem] place-items-center rounded-full';
	const hover = $derived(overlay ? 'hover:bg-panel/15' : 'hover:bg-border/60');
</script>

<header
	class={cn(
		'z-20 flex items-center gap-2.5 px-3.5 py-3 sm:gap-4 sm:px-8 sm:py-3.5',
		overlay
			? 'absolute inset-x-0 top-0 text-panel'
			: 'relative border-b border-border text-foreground'
	)}
>
	<a href="/dashboard" class="brand text-lg whitespace-nowrap sm:text-[1.3125rem]">PhotoProofing</a>
	{#if crumb}
		<span
			class={cn(
				'hidden min-w-0 truncate text-sm sm:block',
				overlay ? 'text-panel/80' : 'text-muted-foreground'
			)}>{crumb}</span
		>
	{/if}
	{#if back}
		<a
			href={back.href}
			class="inline-flex items-center gap-1 text-sm text-muted-foreground underline"
		>
			<ArrowLeft size={14} aria-hidden="true" />
			{back.label}
		</a>
	{/if}
	<span class="flex-1"></span>
	<button
		type="button"
		disabled
		title="More languages arrive with a later release"
		aria-label="Language: English"
		class="{round} text-xs font-semibold tracking-wide opacity-70">EN</button
	>
	{#if actions}
		{@render actions()}
	{:else}
		<button
			type="button"
			disabled
			title="Notifications arrive with a later release"
			aria-label="Notifications"
			class="{round} opacity-70"><Bell size={18} aria-hidden="true" /></button
		>
	{/if}
	<Popover
		label="Account menu"
		align="end"
		class="grid place-items-center rounded-full {hover} size-[2.125rem] border-2 border-panel bg-border-strong text-foreground ring-1 ring-border"
		panelClass="w-48 text-foreground"
	>
		{#snippet trigger()}<User size={16} aria-hidden="true" />{/snippet}
		<a
			href="/settings"
			class="flex min-h-10 w-full items-center gap-2 rounded-control px-3 text-sm text-foreground hover:bg-border/60"
		>
			<Settings size={16} aria-hidden="true" />
			Settings
		</a>
		<button
			type="button"
			class="flex min-h-10 w-full cursor-pointer items-center gap-2 rounded-control px-3 text-left text-sm text-foreground hover:bg-border/60"
			onclick={() => onsignout?.()}
		>
			<LogOut size={16} aria-hidden="true" />
			Sign out
		</button>
	</Popover>
</header>
