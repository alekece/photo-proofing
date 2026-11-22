<script lang="ts">
	import { page } from '$app/state';
	import { Activity, Bell, Image, Images, LogOut, Settings, User, Users } from '@lucide/svelte';
	import { m } from '#lib/paraglide/messages.js';
	import NotificationBell from '#lib/components/NotificationBell.svelte';
	import { getGalleryGridReturn } from '#lib/gallery/dashboardNavigation.svelte.js';
	import { getNotifications } from '#lib/gallery/notifications.svelte.js';
	import { buildVersion, displayVersion } from '#lib/build.js';
	import { legalNotices } from '#lib/footer.js';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import Popover from '#lib/components/ui/Popover/Popover.svelte';
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
	const gridReturn = getGalleryGridReturn();
	const identityName = $derived(studioName?.trim() || m.account_photographer());
	const round = 'grid size-[2.375rem] place-items-center rounded-full';
	const pill =
		'menu-choice min-h-8 py-1 leading-tight focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus sm:focus-visible:-outline-offset-2';

	const pathname = $derived.by(() => {
		try {
			return page?.url?.pathname ?? '';
		} catch {
			return typeof window === 'undefined' ? '' : window.location.pathname;
		}
	});
	const galleriesActive = $derived(pathname === '/dashboard' || pathname.startsWith('/dashboard/'));
	const galleriesHref = $derived(
		(pathname.startsWith('/dashboard/') && gridReturn?.href) || '/dashboard'
	);
	const allPhotosActive = $derived(pathname === '/photos' || pathname.startsWith('/photos/'));
	const contactsActive = $derived(pathname === '/contacts' || pathname.startsWith('/contacts/'));
	const activityActive = $derived(pathname === '/activity' || pathname.startsWith('/activity/'));
	const settingsActive = $derived(pathname === '/settings' || pathname.startsWith('/settings/'));
</script>

<header
	class={cn(
		'relative z-30 flex flex-wrap items-center gap-x-2.5 gap-y-2 border-b border-border px-3.5 py-2.5 text-foreground sm:gap-4 sm:px-8 sm:py-3.5',
		'sm:fixed sm:inset-y-0 sm:left-0 sm:w-(--gallery-rail-width) sm:flex-col sm:flex-nowrap sm:items-stretch sm:justify-start sm:gap-2 sm:border-r sm:border-b-0 sm:bg-(--studio-rail) sm:px-3 sm:py-5'
	)}
>
	<div class="flex min-w-0 items-center gap-1 sm:shrink-0">
		<a href={galleriesHref} class="brand text-lg whitespace-nowrap sm:text-[1.3125rem]"
			>PhotoProofing</a
		>
		{#if actions}
			{@render actions()}
		{:else if notifications}
			<NotificationBell center={notifications} class={round} />
		{:else}
			<button
				type="button"
				disabled
				aria-label={m.header_notifications()}
				class="{round} opacity-70"><Bell size={18} aria-hidden="true" /></button
			>
		{/if}
	</div>
	<nav
		aria-label={m.nav_sections()}
		class={cn(
			'order-last flex w-full flex-wrap items-center gap-1 border-t border-border/60 pt-2',
			'sm:order-none sm:mt-3 sm:min-h-0 sm:flex-col sm:flex-nowrap sm:items-stretch sm:gap-1 sm:overflow-y-auto sm:border-t-0 sm:pt-0 [&>a]:shrink-0 sm:[&>a]:rounded-control sm:[&>a[aria-current=page]]:bg-(--studio-choice) sm:[&>a[aria-current=page]]:text-foreground sm:[&>a[aria-current=page]]:shadow-[inset_2px_0_currentColor]'
		)}
	>
		<a href={galleriesHref} aria-current={galleriesActive ? 'page' : undefined} class={pill}
			><Images size={16} aria-hidden="true" />{m.nav_galleries()}</a
		>
		<a href="/photos" aria-current={allPhotosActive ? 'page' : undefined} class={pill}
			><Image size={16} aria-hidden="true" />{m.nav_all_photos()}</a
		>
		<a href="/contacts" aria-current={contactsActive ? 'page' : undefined} class={pill}
			><Users size={16} aria-hidden="true" />{m.contacts_title()}</a
		>
		<a href="/activity" aria-current={activityActive ? 'page' : undefined} class={pill}
			><Activity size={16} aria-hidden="true" />{m.nav_activity()}</a
		>
		<a
			href="/settings"
			aria-current={settingsActive ? 'page' : undefined}
			class={cn(pill, 'border-border/60 max-sm:border-l sm:mt-2 sm:border-t sm:pt-2')}
			><Settings size={16} aria-hidden="true" />{m.header_settings()}</a
		>
	</nav>
	<span class="flex-1"></span>
	<div
		class={cn(
			'flex items-center gap-1.5',
			'sm:mt-auto sm:shrink-0 sm:flex-col sm:items-stretch sm:gap-1 sm:border-t sm:border-border/60 sm:pt-3'
		)}
	>
		<Popover
			label={m.header_account_menu()}
			align="end"
			placement="responsive"
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
			<Button variant="ghost" size="sm" class="w-full justify-start" onclick={() => onsignout?.()}>
				<LogOut size={16} aria-hidden="true" />
				{m.header_sign_out()}
			</Button>
			<div class="sm:hidden">{@render metadata()}</div>
		</Popover>
		<div class="hidden sm:block">{@render metadata()}</div>
	</div>
</header>

{#snippet metadata()}
	<small class="block px-2 py-1 text-xs text-muted-foreground" title={buildVersion || undefined}>
		© {new Date().getFullYear()} PhotoProofing · {buildVersion
			? m.footer_build({ version: displayVersion(buildVersion) })
			: m.footer_development_build()}
	</small>
	{#if legalNotices.length}
		<nav aria-label={m.footer_legal()} class="flex flex-wrap gap-2 px-2 text-xs">
			{#each legalNotices as notice (notice.href)}<a href={notice.href}>{notice.label}</a>{/each}
		</nav>
	{/if}
{/snippet}
