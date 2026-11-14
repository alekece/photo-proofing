<script module lang="ts">
	export type StudioSection =
		'photos' | 'favorites' | 'comments' | 'final' | 'design' | 'contacts' | 'settings';
</script>

<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import {
		ArrowLeft,
		CheckCircle2,
		Heart,
		Image as ImageIcon,
		Lock,
		MessageSquare,
		Palette,
		SlidersHorizontal,
		Star,
		Users
	} from '@lucide/svelte';
	import type { Component, Snippet } from 'svelte';
	import CommentsPanel from '#lib/components/CommentsPanel.svelte';
	import CoverBackdrop from '#lib/components/CoverBackdrop.svelte';
	import FavoritesPanel from '#lib/components/FavoritesPanel.svelte';
	import GalleryActionBar from '#lib/components/GalleryActionBar.svelte';
	import GalleryContactsPanel from '#lib/components/GalleryContactsPanel.svelte';
	import GalleryDesignPanel from '#lib/components/GalleryDesignPanel.svelte';
	import GallerySettingsPanel from '#lib/components/GallerySettingsPanel.svelte';
	import PhotosPanel from '#lib/components/PhotosPanel.svelte';
	import StatusChip from '#lib/components/ui/StatusChip/StatusChip.svelte';
	import type { FocalPoint, Gallery, GalleryChanges } from '#lib/gallery/api.js';
	import { formatDateTime, formatLongDate, galleryStatus } from '#lib/gallery/format.js';
	import { previewUrl } from '#lib/gallery/photos.js';
	import { DEFAULT_FOCAL_POINT } from '#lib/gallery/schemas.js';

	type NavItem = {
		id: StudioSection;
		label: string;
		icon: Component<{ size?: number; class?: string; 'aria-hidden'?: boolean | 'true' | 'false' }>;
		count?: number | null;
		accent?: boolean;
		locked?: boolean;
		disabled?: boolean;
		title?: string;
	};

	let {
		gallery,
		shareToken,
		origin,
		heroId = $bindable(),
		heroRevision = $bindable(),
		expectedAccessCode = $bindable(),
		heading,
		onsave,
		schedule,
		cancel,
		onpreview,
		onshare,
		onpublish,
		onarchive,
		onrestore,
		ondelete
	}: {
		gallery: Gallery;
		shareToken: string | null;
		origin: string;
		heroId?: string | null;
		heroRevision?: number;
		expectedAccessCode?: string | null | 'invalid';
		heading: Snippet;
		onsave: (changes: GalleryChanges) => Promise<boolean>;
		schedule: (field: string, run: () => void) => void;
		cancel: (field: string) => void;
		onpreview: () => void;
		onshare: () => void;
		onpublish: () => void;
		onarchive: () => void;
		onrestore: () => void;
		ondelete: () => void;
	} = $props();

	let section = $state<StudioSection>('photos');
	let photoCount = $state<number | null>(null);
	let favoriteCount = $state<number | null>(null);
	let commentCount = $state<number | null>(null);
	let finalCount = $state<number | null>(null);
	let contactCount = $state<number | null>(null);

	// Saving replaces the parent gallery object; equal IDs must not reset loaded panels.
	const id = $derived(gallery.id);
	const coverId = $derived(heroId !== undefined ? heroId : gallery.hero_id);
	// svelte-ignore state_referenced_locally
	let liveFocalPoint = $state<FocalPoint>(gallery.focal_point ?? DEFAULT_FOCAL_POINT);
	const savedFavorite = $derived(gallery.favorite ?? false);
	let favorite = $derived(savedFavorite);
	const status = $derived(galleryStatus(gallery.states));

	async function toggleFavorite() {
		const previous = favorite;
		const next = !favorite;

		favorite = next;

		if (!(await onsave({ favorite: next }))) {
			favorite = previous;
		}
	}
	const shared = $derived(shareToken !== null);
	const delivered = $derived(gallery.states.includes('delivered'));
	const archived = $derived(status === 'archived');

	// A shoot date is a calendar day: read at local noon so no time zone moves it.
	const day = $derived(
		gallery.shoot_date
			? formatLongDate(`${gallery.shoot_date}T12:00`)
			: formatLongDate(gallery.created_at)
	);
	const dayLabel = $derived(gallery.shoot_date ? m.details_shot() : m.details_created());
	const openedValue = $derived(
		gallery.last_opened_at === null
			? m.details_not_opened()
			: formatDateTime(gallery.last_opened_at)
	);

	const collectionItems = $derived<NavItem[]>([
		{ id: 'photos', label: m.common_photos(), icon: ImageIcon, count: photoCount },
		{
			id: 'favorites',
			label: m.tab_favorites(),
			icon: Heart,
			count: favoriteCount,
			accent:
				gallery.included_photos !== null &&
				favoriteCount !== null &&
				favoriteCount > gallery.included_photos,
			locked: delivered,
			disabled: !shared,
			title: shared ? undefined : m.tab_after_sharing()
		},
		{
			id: 'comments',
			label: m.tab_comments(),
			icon: MessageSquare,
			count: commentCount,
			disabled: !shared,
			title: shared ? undefined : m.tab_after_sharing()
		},
		{
			id: 'final',
			label: m.tab_final(),
			icon: CheckCircle2,
			count: finalCount,
			disabled: !shared,
			title: shared ? undefined : m.tab_after_sharing()
		}
	]);

	const studioItems = $derived<NavItem[]>([
		{ id: 'design', label: m.tab_design(), icon: Palette },
		{ id: 'contacts', label: m.contacts_title(), icon: Users, count: contactCount },
		{ id: 'settings', label: m.tab_settings(), icon: SlidersHorizontal }
	]);

	const allItems = $derived([...collectionItems, ...studioItems]);

	function moveTab(event: KeyboardEvent & { currentTarget: HTMLButtonElement }) {
		const available = allItems.filter(({ disabled }) => !disabled);

		if (available.length === 0) return;

		const focusedId = event.currentTarget.id.replace(/^tab-/, '');
		const currentIndex = available.findIndex(({ id: itemId }) => itemId === focusedId);
		const index =
			currentIndex !== -1
				? currentIndex
				: available.findIndex(({ id: itemId }) => itemId === section);
		const next = {
			ArrowDown: (index + 1) % available.length,
			ArrowRight: (index + 1) % available.length,
			ArrowUp: (index - 1 + available.length) % available.length,
			ArrowLeft: (index - 1 + available.length) % available.length,
			Home: 0,
			End: available.length - 1
		}[event.key];

		if (next === undefined) return;

		event.preventDefault();

		const target = available[next].id;

		section = target;
		document.getElementById(`tab-${target}`)?.focus();
	}
</script>

{#snippet tabButton(item: NavItem)}
	{@const active = section === item.id}
	{@const Icon = item.icon}
	<button
		type="button"
		role="tab"
		id={`tab-${item.id}`}
		aria-selected={active}
		aria-controls={`panel-${item.id}`}
		disabled={item.disabled ?? false}
		title={item.title}
		tabindex={active ? 0 : -1}
		class="flex shrink-0 cursor-pointer items-center gap-2 rounded-full px-3 py-1.5 text-[0.84375rem] leading-[1.2] whitespace-nowrap transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-45 md:w-full md:rounded-[0.5625rem] md:px-2.5 md:py-2 {active
			? 'bg-foreground font-semibold text-background'
			: 'bg-border/45 font-medium text-foreground enabled:hover:bg-border/70 md:bg-transparent'}"
		onclick={() => (section = item.id)}
		onkeydown={moveTab}
	>
		<Icon size={16} aria-hidden="true" class="shrink-0 max-md:hidden" />
		<span>{item.label}</span>
		{#if item.count != null}
			<em
				class="ml-auto rounded-full px-1.5 py-px text-[0.6875rem] font-bold tracking-[0.02em] not-italic tabular-nums {item.accent
					? active
						? 'bg-danger text-inverse'
						: 'bg-danger/15 font-extrabold text-danger'
					: active
						? 'text-background/80'
						: 'text-muted-foreground'}">{item.count}</em
			>
		{/if}
		{#if item.locked}
			<Lock
				size={12}
				aria-label={m.tab_locked()}
				class={item.count == null ? 'ml-auto' : undefined}
			/>
		{/if}
	</button>
{/snippet}

<div
	data-slot="gallery-studio"
	class="grid min-h-[calc(100dvh-3.75rem)] grid-cols-1 content-start [grid-template-areas:'identity''journey''nav''stage'] md:grid-cols-[16.5rem_1fr] md:grid-rows-[auto_auto_1fr] md:content-stretch md:[grid-template-areas:'identity_journey''identity_stage''nav_stage']"
>
	<div
		class="flex items-center gap-2.5 bg-panel px-3.5 pt-2.5 pb-2 [grid-area:identity] md:flex-col md:items-stretch md:gap-3 md:border-r md:border-border md:px-4 md:pt-4 md:pb-3"
	>
		<a
			href="/dashboard"
			aria-label={m.header_back_to_galleries()}
			class="inline-flex size-8 shrink-0 items-center justify-center rounded-full border border-border bg-background text-xs font-medium text-muted-foreground hover:text-foreground md:size-auto md:w-fit md:justify-start md:gap-1.5 md:border-0 md:bg-transparent md:px-0.5 md:py-0.5"
		>
			<ArrowLeft class="size-3.5 shrink-0" aria-hidden="true" />
			<span class="max-md:sr-only">{m.header_back_to_galleries()}</span>
		</a>

		<button
			type="button"
			aria-label={m.studio_open_design()}
			onclick={() => (section = 'design')}
			class="relative isolate h-[2.125rem] w-11 shrink-0 cursor-pointer overflow-hidden rounded-[0.4375rem] bg-dark shadow-[0_8px_20px_-14px_rgb(43_36_32/0.55)] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus md:aspect-[3/2] md:h-auto md:w-full md:rounded-card"
		>
			{#if coverId}
				<img
					src={previewUrl(gallery.id, coverId, 'photos', heroRevision)}
					alt=""
					style:object-position="{liveFocalPoint.x}% {liveFocalPoint.y}%"
					class="size-full object-cover {archived ? 'brightness-[0.85] grayscale-[0.7]' : ''}"
				/>
			{:else}
				<CoverBackdrop title={gallery.title} compact />
			{/if}
		</button>

		<div class="grid min-w-0 flex-1 gap-0.5 md:gap-1">
			<div class="flex flex-wrap items-center gap-2">
				{@render heading()}
				<button
					type="button"
					aria-label={favorite ? m.studio_favorite_remove() : m.studio_favorite_add()}
					title={favorite ? m.studio_favorite_remove() : m.studio_favorite_add()}
					aria-pressed={favorite}
					onclick={() => void toggleFavorite()}
					class="inline-grid size-7 shrink-0 cursor-pointer place-items-center rounded-full border border-transparent transition-colors hover:bg-border/60 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus {favorite
						? 'text-[#c9921a]'
						: 'text-muted-foreground hover:text-foreground'}"
				>
					<Star size={15} fill={favorite ? 'currentColor' : 'none'} aria-hidden="true" />
				</button>
				<StatusChip {status} dot class="shrink-0" />
			</div>
			<p class="truncate text-xs text-muted-foreground">
				<span>{dayLabel}</span>
				<span>{day}</span>
				{#if shared}
					<span aria-hidden="true"> · </span>
					<span>{m.details_last_opened()}</span>
					<span>{openedValue}</span>
				{/if}
			</p>
		</div>
	</div>

	<div class="min-w-0 [grid-area:journey]">
		<GalleryActionBar
			gallery={{ ...gallery, share_token: shareToken }}
			{photoCount}
			{favoriteCount}
			{onpreview}
			{onshare}
			{onpublish}
			{onrestore}
		/>
	</div>

	<div
		role="tablist"
		aria-label={m.tabs_label()}
		class="flex [scrollbar-width:none] items-center gap-1.5 overflow-x-auto border-b border-border bg-background px-3.5 py-2 [grid-area:nav] md:flex-col md:items-stretch md:gap-4 md:overflow-visible md:border-r md:border-b-0 md:bg-panel md:px-3 md:pt-1 md:pb-6 [&::-webkit-scrollbar]:hidden"
	>
		<div role="none" class="contents md:grid md:gap-0.5">
			<span
				aria-hidden="true"
				class="hidden px-2.5 pb-1 text-[0.65625rem] font-semibold tracking-[0.09em] text-muted-foreground uppercase md:block"
				>{m.studio_group_collection()}</span
			>
			{#each collectionItems as item (item.id)}
				{@render tabButton(item)}
			{/each}
		</div>

		<span aria-hidden="true" class="mx-0.5 h-4 w-px shrink-0 bg-border md:hidden"></span>

		<div role="none" class="contents md:grid md:gap-0.5">
			<span
				aria-hidden="true"
				class="hidden px-2.5 pb-1 text-[0.65625rem] font-semibold tracking-[0.09em] text-muted-foreground uppercase md:block"
				>{m.studio_group_studio()}</span
			>
			{#each studioItems as item (item.id)}
				{@render tabButton(item)}
			{/each}
		</div>
	</div>

	<main class="min-w-0 px-3.5 pt-4 pb-10 [grid-area:stage] sm:px-7 sm:pt-5 sm:pb-12">
		<div role="tabpanel" id={`panel-${section}`} aria-labelledby={`tab-${section}`}>
			<!-- All panels stay mounted so counts load on open and in-flight edits survive tab switches. -->
			<div hidden={section !== 'photos'}>
				<PhotosPanel
					galleryId={id}
					bind:heroId={() => coverId, (next) => (heroId = next)}
					bind:heroRevision
					layout={gallery.layout}
					oncount={(n) => (photoCount = n)}
				/>
			</div>
			<div hidden={section !== 'favorites'} class="grid gap-4">
				<FavoritesPanel
					galleryId={id}
					layout={gallery.layout}
					oncount={(n) => (favoriteCount = n)}
				/>
			</div>
			<div hidden={section !== 'comments'} class="grid gap-4">
				<CommentsPanel galleryId={id} layout={gallery.layout} oncount={(n) => (commentCount = n)} />
			</div>
			<div hidden={section !== 'final'}>
				<PhotosPanel
					set="finals"
					galleryId={id}
					layout={gallery.layout}
					oncount={(n) => (finalCount = n)}
				/>
			</div>
			<div hidden={section !== 'design'}>
				<GalleryDesignPanel
					{gallery}
					bind:heroId={() => coverId, (next) => (heroId = next)}
					{heroRevision}
					bind:focalPoint={liveFocalPoint}
					active={section === 'design'}
					{onsave}
					{schedule}
					{cancel}
				/>
			</div>
			<div hidden={section !== 'contacts'}>
				<GalleryContactsPanel galleryId={id} oncount={(n) => (contactCount = n)} />
			</div>
			<div hidden={section !== 'settings'}>
				<GallerySettingsPanel
					{gallery}
					{shareToken}
					{origin}
					bind:expectedAccessCode
					{onsave}
					{schedule}
					{cancel}
					{onarchive}
					{onrestore}
					{ondelete}
				/>
			</div>
		</div>
	</main>
</div>
