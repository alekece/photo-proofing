<script lang="ts">
	import { page } from '$app/state';
	import { Bell, LogOut, Settings, User } from '@lucide/svelte';
	import LanguageMenu from '#lib/components/LanguageMenu.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import NotificationBell from '#lib/components/NotificationBell.svelte';
	import { getNotifications } from '#lib/gallery/notifications.svelte.js';
	import Popover from '#lib/components/ui/Popover/Popover.svelte';
	import { cn } from '#lib/utils.js';
	import type { Snippet } from 'svelte';

	let {
		actions,
		onsignout
	}: {
		/** Replaces the notification bell. */
		actions?: Snippet;
		onsignout?: () => void | Promise<void>;
	} = $props();

	const notifications = getNotifications();
	const round = 'grid size-[2.375rem] place-items-center rounded-full';
	const hover = 'hover:bg-border/60';
	const pill =
		'inline-flex min-h-8 items-center rounded-full px-3 py-1 text-[0.84375rem] leading-tight whitespace-nowrap transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus';

	const pathname = $derived.by(() => {
		try {
			return page?.url?.pathname ?? '';
		} catch {
			return typeof window === 'undefined' ? '' : window.location.pathname;
		}
	});
	const galleriesActive = $derived(pathname === '/dashboard' || pathname.startsWith('/dashboard/'));
	const contactsActive = $derived(pathname === '/contacts' || pathname.startsWith('/contacts/'));
</script>

<header
	class={cn(
		'relative z-30 flex flex-wrap items-center gap-x-2.5 gap-y-2 border-b border-border px-3.5 py-2.5 text-foreground sm:flex-nowrap sm:gap-4 sm:px-8 sm:py-3.5'
	)}
>
	<a href="/dashboard" class="brand text-lg whitespace-nowrap sm:text-[1.3125rem]">PhotoProofing</a>
	<nav
		aria-label={m.nav_sections()}
		class="order-last flex w-full items-center gap-1 border-t border-border/60 pt-2 sm:order-none sm:ml-2 sm:w-auto sm:border-t-0 sm:pt-0"
	>
		<a
			href="/dashboard"
			aria-current={galleriesActive ? 'page' : undefined}
			class={cn(
				pill,
				galleriesActive
					? 'bg-foreground font-semibold text-background'
					: 'font-medium text-muted-foreground hover:bg-border/60 hover:text-foreground'
			)}>{m.nav_galleries()}</a
		>
		<a
			href="/contacts"
			aria-current={contactsActive ? 'page' : undefined}
			class={cn(
				pill,
				contactsActive
					? 'bg-foreground font-semibold text-background'
					: 'font-medium text-muted-foreground hover:bg-border/60 hover:text-foreground'
			)}>{m.contacts_title()}</a
		>
		<button
			type="button"
			disabled
			title={m.nav_coming_soon()}
			class={cn(pill, 'cursor-not-allowed font-medium text-muted-foreground/70 opacity-60')}
			>{m.nav_activity()}</button
		>
	</nav>
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
