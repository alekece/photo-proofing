<script lang="ts">
	import { page } from '$app/state';
	import { Activity, Bell, Image, Images, LogOut, Settings, User, Users } from '@lucide/svelte';
	import { onMount } from 'svelte';
	import LanguageMenu from '#lib/components/LanguageMenu.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import NotificationBell from '#lib/components/NotificationBell.svelte';
	import { getNotifications } from '#lib/gallery/notifications.svelte.js';
	import Popover from '#lib/components/ui/Popover/Popover.svelte';
	import { SettingsApi } from '#lib/settings/settings.js';
	import { cn } from '#lib/utils.js';
	import type { Snippet } from 'svelte';

	let {
		actions,
		onsignout,
		studioName
	}: {
		/** Replaces the notification bell. */
		actions?: Snippet;
		onsignout?: () => void | Promise<void>;
		/** Workspace studio name shown in the bottom account area. */
		studioName?: string;
	} = $props();

	const notifications = getNotifications();
	let identityName = $state('');
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

	$effect(() => {
		if (studioName?.trim()) identityName = studioName.trim();
	});

	onMount(() => {
		if (studioName?.trim()) {
			identityName = studioName.trim();

			return;
		}

		let active = true;

		void new SettingsApi()
			.load()
			.then((settings) => {
				if (active) identityName = settings.studioName.trim();
			})
			.catch(() => {});

		return () => {
			active = false;
		};
	});
</script>

<header
	class={cn(
		'relative z-30 flex flex-wrap items-center gap-x-2.5 gap-y-2 border-b border-border px-3.5 py-2.5 text-foreground sm:gap-4 sm:px-8 sm:py-3.5',
		'sm:fixed sm:inset-y-0 sm:left-0 sm:w-[clamp(15rem,16vw,17.5rem)] sm:flex-col sm:items-stretch sm:justify-start sm:gap-2 sm:overflow-y-auto sm:border-r sm:border-b-0 sm:px-3 sm:py-5'
	)}
>
	<a href="/dashboard" class="brand text-lg whitespace-nowrap sm:text-[1.3125rem]">PhotoProofing</a>
	<nav
		aria-label={m.nav_sections()}
		class={cn(
			'order-last flex w-full flex-wrap items-center gap-1 border-t border-border/60 pt-2',
			'sm:order-none sm:mt-3 sm:flex-col sm:items-stretch sm:gap-1 sm:border-t-0 sm:pt-0'
		)}
	>
		<a
			href="/dashboard"
			aria-current={galleriesActive ? 'page' : undefined}
			class={linkClass(galleriesActive)}
			><Images size={16} aria-hidden="true" />{m.nav_galleries()}</a
		>
		<a
			href="/photos"
			aria-current={allPhotosActive ? 'page' : undefined}
			class={linkClass(allPhotosActive)}
			><Image size={16} aria-hidden="true" />{m.nav_all_photos()}</a
		>
		<a
			href="/contacts"
			aria-current={contactsActive ? 'page' : undefined}
			class={linkClass(contactsActive)}
			><Users size={16} aria-hidden="true" />{m.contacts_title()}</a
		>
		<a
			href="/activity"
			aria-current={activityActive ? 'page' : undefined}
			class={linkClass(activityActive)}
			><Activity size={16} aria-hidden="true" />{m.nav_activity()}</a
		>
		<a
			href="/settings"
			aria-current={settingsActive ? 'page' : undefined}
			class={cn(linkClass(settingsActive), 'sm:mt-2 sm:border-t sm:border-border/60 sm:pt-2')}
			><Settings size={16} aria-hidden="true" />{m.header_settings()}</a
		>
	</nav>
	<span class="flex-1"></span>
	<div
		class={cn(
			'flex items-center gap-1.5',
			'sm:mt-auto sm:flex-col sm:items-stretch sm:gap-1 sm:border-t sm:border-border/60 sm:pt-3'
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
		<Popover
			label={m.header_account_menu()}
			align="end"
			class="flex min-h-10 w-full items-center gap-2 rounded-control px-2 text-left text-foreground hover:bg-border/60"
			panelClass="w-48 text-foreground"
		>
			{#snippet trigger()}
				<span
					aria-hidden="true"
					class="grid size-[2.125rem] shrink-0 place-items-center rounded-full border-2 border-panel bg-border-strong text-foreground ring-1 ring-border"
				>
					<User size={16} aria-hidden="true" />
				</span>
				<span class="min-w-0 truncate text-sm font-medium">
					{identityName || m.account_photographer()}
				</span>
			{/snippet}
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
