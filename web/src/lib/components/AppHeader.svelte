<script lang="ts">
	import { Bell, LogOut, Settings, User } from '@lucide/svelte';
	import LanguageMenu from '#lib/components/LanguageMenu.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import NotificationBell from '#lib/components/NotificationBell.svelte';
	import { getNotifications } from '#lib/gallery/notifications.svelte.js';
	import Popover from '#lib/components/ui/Popover/Popover.svelte';
	import { cn } from '#lib/utils.js';
	import type { Snippet } from 'svelte';

	let {
		overlay = false,
		actions,
		onsignout
	}: {
		/** Floats the bar over the hero banner below it instead of taking a row of its own. */
		overlay?: boolean;
		/** Replaces the notification bell. */
		actions?: Snippet;
		onsignout?: () => void | Promise<void>;
	} = $props();

	const notifications = getNotifications();
	const round = 'grid size-[2.375rem] place-items-center rounded-full';
	const hover = $derived(overlay ? 'hover:bg-panel/15' : 'hover:bg-border/60');
</script>

<header
	class={cn(
		'z-30 flex items-center gap-2.5 px-3.5 py-3 sm:gap-4 sm:px-8 sm:py-3.5',
		overlay
			? 'absolute inset-x-0 top-0 text-panel'
			: 'relative border-b border-border text-foreground'
	)}
>
	<a href="/dashboard" class="brand text-lg whitespace-nowrap sm:text-[1.3125rem]">PhotoProofing</a>
	<span class="flex-1"></span>
	<LanguageMenu class="{round} {hover}" />
	{#if actions}
		{@render actions()}
	{:else if notifications}
		<NotificationBell center={notifications} class="{round} {hover}" />
	{:else}
		<button type="button" disabled aria-label={m.header_notifications()} class="{round} opacity-70"
			><Bell size={18} aria-hidden="true" /></button
		>
	{/if}
	<Popover
		label={m.header_account_menu()}
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
			{m.header_settings()}
		</a>
		<button
			type="button"
			class="flex min-h-10 w-full cursor-pointer items-center gap-2 rounded-control px-3 text-left text-sm text-foreground hover:bg-border/60"
			onclick={() => onsignout?.()}
		>
			<LogOut size={16} aria-hidden="true" />
			{m.header_sign_out()}
		</button>
	</Popover>
</header>
