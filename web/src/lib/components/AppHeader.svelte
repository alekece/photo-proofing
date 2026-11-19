<script lang="ts">
	import { page } from '$app/state';
	import { Activity, Bell, Image, Images, LogOut, Settings, User, Users } from '@lucide/svelte';
	import LanguageMenu from '#lib/components/LanguageMenu.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import NotificationBell from '#lib/components/NotificationBell.svelte';
	import { getNotifications } from '#lib/gallery/notifications.svelte.js';
	import Popover from '#lib/components/ui/Popover/Popover.svelte';
	import { cn } from '#lib/utils.js';
	import type { Snippet } from 'svelte';

	let {
		actions,
		onsignout,
		legacy = false
	}: {
		/** Replaces the notification bell. */
		actions?: Snippet;
		onsignout?: () => void | Promise<void>;
		/**
		 * Keeps the previous compact top bar, with no All Photos link and no icons, for gallery
		 * pages until Phase 2 replaces it with the gallery-specific identity rail.
		 */
		legacy?: boolean;
	} = $props();

	const notifications = getNotifications();
	const round = 'grid size-[2.375rem] place-items-center rounded-full';
	const hover = 'hover:bg-border/60';
	const pill =
		'inline-flex min-h-8 items-center gap-2 rounded-full px-3 py-1 text-[0.84375rem] leading-tight whitespace-nowrap transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus';

	const pathname = $derived.by(() => {
		try {
			return page?.url?.pathname ?? '';
		} catch {
			return typeof window === 'undefined' ? '' : window.location.pathname;
		}
	});
	const galleriesActive = $derived(pathname === '/dashboard' || pathname.startsWith('/dashboard/'));
	const allPhotosActive = $derived(pathname === '/photos' || pathname.startsWith('/photos/'));
	const contactsActive = $derived(pathname === '/contacts' || pathname.startsWith('/contacts/'));
	const activityActive = $derived(pathname === '/activity' || pathname.startsWith('/activity/'));
	const settingsActive = $derived(pathname === '/settings' || pathname.startsWith('/settings/'));

	function linkClass(active: boolean) {
		return cn(
			pill,
			active
				? 'bg-foreground font-semibold text-background'
				: 'font-medium text-muted-foreground hover:bg-border/60 hover:text-foreground'
		);
	}
</script>

<header
	class={cn(
		'relative z-30 flex flex-wrap items-center gap-x-2.5 gap-y-2 border-b border-border px-3.5 py-2.5 text-foreground sm:gap-4 sm:px-8 sm:py-3.5',
		legacy
			? 'sm:flex-nowrap'
			: 'sm:fixed sm:inset-y-0 sm:left-0 sm:w-60 sm:flex-col sm:items-stretch sm:justify-start sm:gap-2 sm:overflow-y-auto sm:border-r sm:border-b-0 sm:px-3 sm:py-5'
	)}
>
	<a href="/dashboard" class="brand text-lg whitespace-nowrap sm:text-[1.3125rem]">PhotoProofing</a>
	<nav
		aria-label={m.nav_sections()}
		class={cn(
			'order-last flex w-full flex-wrap items-center gap-1 border-t border-border/60 pt-2',
			legacy
				? 'sm:order-none sm:ml-2 sm:w-auto sm:border-t-0 sm:pt-0'
				: 'sm:order-none sm:mt-3 sm:flex-col sm:items-stretch sm:gap-1 sm:border-t-0 sm:pt-0'
		)}
	>
		<a
			href="/dashboard"
			aria-current={galleriesActive ? 'page' : undefined}
			class={linkClass(galleriesActive)}
			>{#if !legacy}<Images size={16} aria-hidden="true" />{/if}{m.nav_galleries()}</a
		>
		{#if !legacy}
			<a
				href="/photos"
				aria-current={allPhotosActive ? 'page' : undefined}
				class={linkClass(allPhotosActive)}
				><Image size={16} aria-hidden="true" />{m.nav_all_photos()}</a
			>
		{/if}
		<a
			href="/contacts"
			aria-current={contactsActive ? 'page' : undefined}
			class={linkClass(contactsActive)}
			>{#if !legacy}<Users size={16} aria-hidden="true" />{/if}{m.contacts_title()}</a
		>
		<a
			href="/activity"
			aria-current={activityActive ? 'page' : undefined}
			class={linkClass(activityActive)}
			>{#if !legacy}<Activity size={16} aria-hidden="true" />{/if}{m.nav_activity()}</a
		>
	</nav>
	<span class="flex-1"></span>
	<div
		class={cn(
			'flex items-center gap-1.5',
			!legacy && 'sm:mt-auto sm:flex-col sm:items-stretch sm:gap-1 sm:pt-4'
		)}
	>
		{#if actions}
			{@render actions()}
		{:else if notifications}
			<NotificationBell center={notifications} class="{round} {hover}" />
		{:else}
			<button
				type="button"
				disabled
				aria-label={m.header_notifications()}
				class="{round} opacity-70"><Bell size={18} aria-hidden="true" /></button
			>
		{/if}
		<LanguageMenu class="{round} {hover}" />
		{#if !legacy}
			<a
				href="/settings"
				aria-current={settingsActive ? 'page' : undefined}
				class={linkClass(settingsActive)}
				><Settings size={16} aria-hidden="true" />{m.header_settings()}</a
			>
		{/if}
		<Popover
			label={m.header_account_menu()}
			align="end"
			class="grid place-items-center rounded-full {hover} size-[2.125rem] border-2 border-panel bg-border-strong text-foreground ring-1 ring-border"
			panelClass="w-48 text-foreground"
		>
			{#snippet trigger()}<User size={16} aria-hidden="true" />{/snippet}
			{#if legacy}
				<a
					href="/settings"
					class="flex min-h-10 w-full items-center gap-2 rounded-control px-3 text-sm text-foreground hover:bg-border/60"
				>
					<Settings size={16} aria-hidden="true" />
					{m.header_settings()}
				</a>
			{/if}
			<button
				type="button"
				class="flex min-h-10 w-full cursor-pointer items-center gap-2 rounded-control px-3 text-left text-sm text-foreground hover:bg-border/60"
				onclick={() => onsignout?.()}
			>
				<LogOut size={16} aria-hidden="true" />
				{m.header_sign_out()}
			</button>
		</Popover>
	</div>
</header>
