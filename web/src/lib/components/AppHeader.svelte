<script lang="ts">
	import { page } from '$app/state';
	import {
		Activity,
		Bell,
		Image,
		Images,
		LogOut,
		PanelLeftClose,
		PanelLeftOpen,
		Settings,
		User,
		Users
	} from '@lucide/svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { getLocale } from '#lib/paraglide/runtime.js';
	import NotificationBell from '#lib/components/NotificationBell.svelte';
	import { getGalleryGridReturn } from '#lib/gallery/dashboardNavigation.svelte.js';
	import { rails } from '#lib/rails.svelte.js';
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
		studioName,
		counts
	}: {
		/** Replaces the notification bell. */
		actions?: Snippet;
		onsignout?: () => void | Promise<void>;
		/** Workspace studio name shown in the bottom account area. */
		studioName?: string;
		counts?: Partial<Record<'galleries' | 'photos' | 'contacts', number>>;
	} = $props();

	const notifications = getNotifications();
	const gridReturn = getGalleryGridReturn();
	const identityName = $derived(studioName?.trim() || m.account_photographer());
	const railIcon = 'sm:size-11 sm:[--icon-hover-color:var(--border)]';
	const round = `grid size-[2.375rem] place-items-center rounded-full ${railIcon}`;
	const pill =
		'menu-choice min-h-8 py-1 leading-tight workspace-collapsed:mx-auto workspace-collapsed:size-10 workspace-collapsed:justify-center workspace-collapsed:px-0 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus sm:focus-visible:-outline-offset-2';

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
		'motion-reduce:transition-none sm:fixed sm:inset-y-0 sm:left-0 sm:w-(--workspace-rail-width) sm:flex-col sm:flex-nowrap sm:items-stretch sm:justify-start sm:gap-2 sm:border-r sm:border-b-0 sm:bg-(--studio-rail) sm:px-3 sm:py-5 sm:transition-[width] sm:duration-200 sm:ease-out'
	)}
>
	<div
		class="flex min-w-0 items-center gap-1 sm:shrink-0 sm:gap-2 workspace-collapsed:justify-center"
	>
		<a
			href={galleriesHref}
			class="brand text-lg whitespace-nowrap sm:me-auto sm:text-[1.3125rem] workspace-collapsed:hidden"
		>
			PhotoProofing
		</a>
		<div class="sm:hidden">
			{#if actions}
				{@render actions()}
			{:else if notifications}
				<NotificationBell center={notifications} class={round} />
			{:else}
				<button
					type="button"
					disabled
					aria-label={m.header_notifications()}
					class="{round} opacity-70"
				>
					<Bell size={18} aria-hidden="true" />
				</button>
			{/if}
		</div>
		<button
			type="button"
			aria-expanded={!rails.collapsed}
			aria-label={rails.collapsed ? m.rail_expand() : m.rail_collapse()}
			title={rails.collapsed ? m.rail_expand() : m.rail_collapse()}
			class="group/toggle relative hidden size-10 shrink-0 cursor-pointer place-items-center rounded-full text-foreground transition-colors hover:bg-border focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus motion-reduce:transition-none sm:grid"
			onclick={() => rails.toggle()}
		>
			{#if rails.collapsed}
				<span
					aria-hidden="true"
					class="brand col-start-1 row-start-1 text-[1.375rem] leading-none text-foreground transition-opacity duration-150 group-hover/toggle:opacity-0 group-focus-visible/toggle:opacity-0 motion-reduce:transition-none"
				>
					P
				</span>
				<PanelLeftOpen
					size={17}
					strokeWidth={1.75}
					aria-hidden="true"
					class="col-start-1 row-start-1 opacity-0 transition-opacity duration-150 group-hover/toggle:opacity-100 group-focus-visible/toggle:opacity-100 motion-reduce:transition-none"
				/>
			{:else}
				<PanelLeftClose size={17} strokeWidth={1.75} aria-hidden="true" />
			{/if}
		</button>
	</div>
	<nav
		aria-label={m.nav_sections()}
		class={cn(
			'order-last flex w-full flex-wrap items-center gap-1 border-t border-border/60 pt-2',
			'sm:order-none sm:mt-3 sm:min-h-0 sm:flex-col sm:flex-nowrap sm:items-stretch sm:gap-1 sm:overflow-y-auto sm:border-t-0 sm:pt-0 [&>a]:shrink-0'
		)}
	>
		<a
			href={galleriesHref}
			aria-current={galleriesActive ? 'page' : undefined}
			title={rails.collapsed ? m.nav_galleries() : undefined}
			class={pill}
		>
			<Images size={16} aria-hidden="true" /><span class="workspace-collapsed:sr-only">
				{m.nav_galleries()}
			</span>
			{@render total(counts?.galleries)}
		</a>
		<a
			href="/photos"
			aria-current={allPhotosActive ? 'page' : undefined}
			title={rails.collapsed ? m.nav_all_photos() : undefined}
			class={pill}
		>
			<Image size={16} aria-hidden="true" /><span class="workspace-collapsed:sr-only">
				{m.nav_all_photos()}
			</span>
			{@render total(counts?.photos)}
		</a>
		<a
			href="/contacts"
			aria-current={contactsActive ? 'page' : undefined}
			title={rails.collapsed ? m.contacts_title() : undefined}
			class={pill}
		>
			<Users size={16} aria-hidden="true" /><span class="workspace-collapsed:sr-only">
				{m.contacts_title()}
			</span>
			{@render total(counts?.contacts)}
		</a>
		<a
			href="/activity"
			aria-current={activityActive ? 'page' : undefined}
			title={rails.collapsed ? m.nav_activity() : undefined}
			class={pill}
		>
			<Activity size={16} aria-hidden="true" /><span class="workspace-collapsed:sr-only">
				{m.nav_activity()}
			</span>
		</a>
	</nav>
	<span class="flex-1"></span>
	<div
		class="flex items-center gap-1.5 sm:shrink-0 sm:flex-col sm:items-stretch sm:gap-1 workspace-collapsed:items-center"
	>
		<a
			href="/settings"
			aria-current={settingsActive ? 'page' : undefined}
			title={rails.collapsed ? m.header_settings() : undefined}
			class={pill}
		>
			<Settings size={16} aria-hidden="true" /><span class="workspace-collapsed:sr-only">
				{m.header_settings()}
			</span>
		</a>
		{#if notifications}
			<div class="hidden sm:block">
				<NotificationBell center={notifications} row class={cn(pill, 'w-full')} />
			</div>
		{/if}
	</div>
	<div
		class="flex items-center gap-1.5 sm:shrink-0 sm:flex-col sm:items-stretch sm:gap-1 sm:border-t sm:border-border/60 sm:pt-3 workspace-collapsed:items-center"
	>
		<Popover
			label={m.header_account_menu()}
			align="end"
			placement="responsive"
			rootClass="block w-full sm:w-full"
			class="flex min-h-10 w-full cursor-pointer items-center gap-2 rounded-control px-2 text-left text-foreground hover:bg-border workspace-collapsed:justify-center workspace-collapsed:px-0"
			panelClass="w-48 text-foreground sm:top-auto sm:right-auto sm:bottom-0 sm:left-full sm:mb-0 sm:ml-2 sm:w-52"
		>
			{#snippet trigger()}
				{@render identity()}
			{/snippet}
			<Button variant="ghost" size="sm" class="w-full justify-start" onclick={() => onsignout?.()}>
				<LogOut size={16} aria-hidden="true" />
				{m.header_sign_out()}
			</Button>
			<div class="mt-1 border-t border-border pt-1 sm:hidden workspace-collapsed:block">
				{@render metadata()}
			</div>
		</Popover>
		<div class="hidden sm:block workspace-collapsed:hidden">{@render metadata()}</div>
	</div>
</header>

{#snippet identity()}
	<span
		aria-hidden="true"
		class="grid size-[2.125rem] shrink-0 place-items-center rounded-full border-2 border-panel bg-border-strong text-foreground ring-1 ring-border"
	>
		<User size={16} aria-hidden="true" />
	</span>
	<span class="min-w-0 truncate text-sm font-medium workspace-collapsed:sr-only">
		{identityName}
	</span>
{/snippet}

{#snippet total(value: number | undefined)}
	{#if value !== undefined}
		<span class="ms-auto text-xs font-semibold tabular-nums workspace-collapsed:hidden">
			{value.toLocaleString(getLocale())}
		</span>
	{/if}
{/snippet}

{#snippet metadata()}
	<small class="block px-2 py-1 text-xs text-muted-foreground" title={buildVersion || undefined}>
		{displayVersion(buildVersion || 'dev')} · © {new Date().getFullYear()} PhotoProofing
	</small>
	{#if legalNotices.length}
		<nav aria-label={m.footer_legal()} class="flex flex-wrap gap-2 px-2 text-xs">
			{#each legalNotices as notice (notice.href)}<a href={notice.href}>{notice.label}</a>{/each}
		</nav>
	{/if}
{/snippet}
