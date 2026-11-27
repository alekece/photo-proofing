<script module lang="ts">
	export const STUDIO_SECTIONS = [
		'photos',
		'favorites',
		'comments',
		'final',
		'activity',
		'design',
		'contacts',
		'settings'
	] as const;

	export type StudioSection = (typeof STUDIO_SECTIONS)[number];
</script>

<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { getGalleryGridReturn } from '#lib/gallery/dashboardNavigation.svelte.js';
	import {
		Activity as ActivityIcon,
		CheckCircle2,
		Heart,
		Image as ImageIcon,
		ChevronLeft,
		MessageSquare,
		Palette,
		SlidersHorizontal,
		Users
	} from '@lucide/svelte';
	import type { Component, Snippet } from 'svelte';
	import CommentsPanel from '#lib/components/CommentsPanel.svelte';
	import CoverBackdrop from '#lib/components/CoverBackdrop.svelte';
	import GalleryActivityPanel from '#lib/components/GalleryActivityPanel.svelte';
	import FavoritesPanel from '#lib/components/FavoritesPanel.svelte';
	import GalleryActionBar from '#lib/components/GalleryActionBar.svelte';
	import GalleryContactsPanel from '#lib/components/GalleryContactsPanel.svelte';
	import GalleryDesignPanel from '#lib/components/GalleryDesignPanel.svelte';
	import GallerySettingsPanel from '#lib/components/GallerySettingsPanel.svelte';
	import GalleryFavorite from '#lib/components/GalleryFavorite.svelte';
	import PhotosPanel from '#lib/components/PhotosPanel.svelte';
	import StatusChip from '#lib/components/ui/StatusChip/StatusChip.svelte';
	import type { FocalPoint, Gallery, GalleryChanges } from '#lib/gallery/api.js';
	import { FavoritesProgress } from '#lib/gallery/favoritesProgress.js';
	import { formatDateTime, formatLongDate, galleryStatus } from '#lib/gallery/format.js';
	import { previewUrl } from '#lib/gallery/photos.js';
	import { DEFAULT_FOCAL_POINT } from '#lib/gallery/schemas.js';
	import { DEFAULT_SETTINGS, type WorkspaceSettings } from '#lib/settings/settings.js';

	type NavItem = {
		id: StudioSection;
		label: string;
		icon: Component<{ size?: number; class?: string; 'aria-hidden'?: boolean | 'true' | 'false' }>;
		count?: number | null;
		accent?: boolean;
		disabled?: boolean;
		title?: string;
	};

	const gridReturn = getGalleryGridReturn();

	let {
		gallery,
		shareToken,
		origin,
		settings = DEFAULT_SETTINGS,
		contactsRevision = 0,
		heroId = $bindable(),
		heroRevision = $bindable(),
		focalReady = true,
		expectedAccessCode = $bindable(),
		heading,
		onsave,
		onsavefocal,
		schedule,
		cancel,
		onbeforepreview,
		onshare,
		onpublish,
		onarchive,
		onrestore,
		ondelete,
		requestedPhoto,
		requestedSection,
		requestedThread,
		requestKey
	}: {
		gallery: Gallery;
		shareToken: string | null;
		origin: string;
		settings?: WorkspaceSettings;
		contactsRevision?: number;
		heroId?: string | null;
		heroRevision?: number;
		focalReady?: boolean;
		expectedAccessCode?: string | null | 'invalid';
		heading: Snippet;
		onsave: (changes: GalleryChanges) => Promise<boolean>;
		onsavefocal?: (point: FocalPoint) => Promise<FocalPoint | undefined>;
		schedule: (field: string, run: () => void) => void;
		cancel: (field: string) => void;
		onbeforepreview?: () => void;
		onshare: () => void;
		onpublish: () => void;
		onarchive: () => void;
		onrestore: () => void;
		ondelete: () => void;
		requestedPhoto?: { id: string; kind: 'proof' | 'final' };
		/** A tab named by the link that opened the gallery, such as one from Activity. */
		requestedSection?: StudioSection;
		/** The photo whose comment thread opens in the Comments tab. */
		requestedThread?: string;
		/** Changes with every navigation, so a link to the tab already requested still applies. */
		requestKey?: unknown;
	} = $props();

	let section = $state<StudioSection>('photos');
	const requestedId = $derived(requestedPhoto?.id);
	const requestedKind = $derived(requestedPhoto?.kind);
	$effect(() => {
		void requestKey;

		if (requestedId) section = requestedKind === 'final' ? 'final' : 'photos';
		else if (requestedSection) selectSection(requestedSection);
	});
	// A new object per request lets the same thread open again after it was closed.
	const linkedThread = $derived.by(() => {
		void requestKey;

		return requestedThread ? { photoId: requestedThread } : undefined;
	});
	let chosenThread = $state<{ photoId: string }>();
	const openThread = $derived(chosenThread ?? linkedThread);

	function openDestination(place: { section?: string; thread?: string }) {
		const target = STUDIO_SECTIONS.find((candidate) => candidate === place.section);

		if (!target) return;

		chosenThread = place.thread ? { photoId: place.thread } : undefined;
		selectSection(target);
	}
	let photoCount = $state<number | null>(null);
	let favoriteCount = $state<number | null>(null);
	let commentCount = $state<number | null>(null);
	let finalCount = $state<number | null>(null);
	let contactCount = $state<number | null>(null);

	// Saving replaces the parent gallery object; equal IDs must not reset loaded panels.
	const id = $derived(gallery.id);
	const favoritesProgress = new FavoritesProgress();
	let favoriteIdsGallery = $state<string | null>(null);
	let favoriteIds = $state<string[] | null>(null);
	let favoriteProgressRevision = $state(0);
	const favoritesCollected = $derived.by(() => {
		void favoriteProgressRevision;
		return (
			favoriteIdsGallery === id &&
			favoriteIds !== null &&
			favoritesProgress.matches(id, favoriteIds)
		);
	});

	function favoritesChanged(ids: string[] | null) {
		favoriteIdsGallery = id;
		favoriteIds = ids;
	}

	function fullFavoritesDownloaded(ids: string[]) {
		favoritesProgress.record(id, ids);
		favoriteProgressRevision++;
	}

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
	const archived = $derived(status === 'archived');
	function selectSection(target: StudioSection) {
		if (allItems.find((item) => item.id === target)?.disabled) return;

		section = target;
	}

	const day = $derived(gallery.shoot_date ? formatLongDate(gallery.shoot_date) : null);
	const expiry = $derived(gallery.expires_at ? formatLongDate(gallery.expires_at) : null);

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
		{
			id: 'activity',
			label: m.nav_activity(),
			icon: ActivityIcon,
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

		selectSection(target);
		if (section === target) document.getElementById(`tab-${target}`)?.focus();
	}
</script>

{#snippet railBar()}
	<a
		href={gridReturn?.href ?? '/dashboard'}
		aria-label={m.header_back_to_galleries()}
		class="group/back mb-1 inline-flex min-h-9 w-fit cursor-pointer items-center gap-1.5 px-2 text-sm font-medium text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus"
	>
		<ChevronLeft size={16} aria-hidden="true" />
		<span aria-hidden="true" class="grid overflow-hidden leading-6">
			<span
				class="col-start-1 row-start-1 transition-transform duration-200 group-hover/back:-translate-y-full group-focus-visible/back:-translate-y-full motion-reduce:transition-none"
			>
				{m.nav_galleries()}
			</span>
			<span
				class="col-start-1 row-start-1 translate-y-full transition-transform duration-200 group-hover/back:translate-y-0 group-focus-visible/back:translate-y-0 motion-reduce:transition-none"
			>
				{m.action_back()}
			</span>
		</span>
	</a>
{/snippet}

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
		class="menu-choice shrink-0 cursor-pointer leading-[1.2] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-45 md:w-full [&:not([aria-selected=true])]:enabled:hover:bg-(--studio-hover,var(--border))"
		onclick={() => selectSection(item.id)}
		onkeydown={moveTab}
	>
		<Icon size={16} aria-hidden="true" class="shrink-0 max-md:hidden" />
		<span class="">{item.label}</span>
		{#if item.count != null}
			<em
				class="ml-auto pill text-[0.6875rem] font-bold tracking-[0.02em] not-italic tabular-nums {item.accent
					? active
						? 'bg-danger text-inverse'
						: 'bg-danger/15 font-extrabold text-danger'
					: active
						? 'text-background/80'
						: 'text-muted-foreground'}"
			>
				{item.count}
			</em>
		{/if}
	</button>
{/snippet}

<div
	data-slot="gallery-studio"
	class="relative grid min-h-[calc(100dvh-3.75rem)] grid-cols-[minmax(0,1fr)] content-start [grid-template-areas:'identity''journey''nav''stage'] motion-reduce:transition-none sm:min-h-dvh md:h-dvh md:grid-cols-[var(--rail-expanded-width)_minmax(0,1fr)] md:grid-rows-[auto_minmax(0,1fr)] md:content-stretch md:overflow-hidden md:transition-[grid-template-columns] md:duration-200 md:ease-out md:[grid-template-areas:'side_journey''side_stage']"
>
	<div
		data-slot="gallery-side"
		class="contents md:row-span-2 md:flex md:min-h-0 md:flex-col md:overflow-y-auto md:border-r md:border-border md:bg-panel md:[grid-area:side]"
	>
		<div
			data-slot="gallery-identity"
			class="grid min-w-0 grid-cols-[auto_minmax(0,1fr)] items-center gap-x-2.5 gap-y-1.5 bg-panel px-3.5 pt-2.5 pb-2 [grid-area:identity] md:grid-cols-1 md:items-stretch md:gap-3 md:px-4 md:pt-4 md:pb-5"
		>
			<div class="hidden md:block">{@render railBar()}</div>
			<div class="group relative isolate aspect-[3/2] w-[8.75rem] min-w-0 md:h-auto md:w-full">
				<button
					type="button"
					aria-label={m.studio_open_design()}
					onclick={() => (section = 'design')}
					class="absolute inset-0 size-full cursor-pointer overflow-hidden rounded-card bg-dark shadow-[0_8px_20px_-14px_rgb(43_36_32/0.55)] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus"
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

				<div
					data-slot="gallery-cover-controls"
					class="pointer-events-none absolute bottom-2 left-2 z-[3] flex items-center gap-1.5"
				>
					<StatusChip {status} workspace />
					<GalleryFavorite
						{favorite}
						interactive
						label={favorite ? m.studio_favorite_remove() : m.studio_favorite_add()}
						onclick={() => void toggleFavorite()}
						class="pointer-events-auto {favorite
							? ''
							: 'opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100 focus-visible:opacity-100 [@media(hover:none)]:opacity-100'}"
					/>
				</div>
			</div>
			<div class="grid min-w-0 gap-0.5 max-md:col-start-2 md:gap-1">
				<div class="flex min-w-0 flex-wrap items-center gap-2">
					{@render heading()}
				</div>
				{#if day}
					<p class="truncate text-xs text-muted-foreground">
						<span>{m.details_shot()}</span>
						<time datetime={gallery.shoot_date ?? undefined}>{day}</time>
					</p>
				{/if}
				{#if expiry}
					<p class="truncate text-xs text-muted-foreground">
						<span>{m.expiry_open_until()}</span>
						<time datetime={gallery.expires_at ?? undefined}>{expiry}</time>
					</p>
				{/if}
				{#if shared && gallery.last_opened_at}
					<p class="truncate text-xs text-muted-foreground">
						<span>{m.details_last_opened()}</span>
						<span>{formatDateTime(gallery.last_opened_at)}</span>
					</p>
				{/if}
			</div>
		</div>

		<div class="flex min-w-0 flex-col [grid-area:nav] md:flex-1">
			<div
				role="tablist"
				aria-label={m.tabs_label()}
				class="flex [scrollbar-width:none] items-center gap-1.5 overflow-x-auto border-b border-border bg-background px-3.5 py-2 md:flex-1 md:flex-col md:items-stretch md:gap-4 md:overflow-visible md:border-b-0 md:bg-transparent md:px-3 md:pt-3 md:pb-3 [&::-webkit-scrollbar]:hidden"
			>
				<div role="none" class="contents md:grid md:gap-0.5">
					<span
						aria-hidden="true"
						class="hidden px-2.5 pb-1 text-[0.65625rem] font-semibold tracking-[0.09em] text-muted-foreground uppercase md:block"
					>
						{m.studio_group_collection()}
					</span>
					{#each collectionItems as item (item.id)}
						{@render tabButton(item)}
					{/each}
				</div>

				<span aria-hidden="true" class="mx-0.5 h-4 w-px shrink-0 bg-border md:hidden"></span>

				<div role="none" class="contents md:grid md:gap-0.5">
					<span
						aria-hidden="true"
						class="hidden px-2.5 pb-1 text-[0.65625rem] font-semibold tracking-[0.09em] text-muted-foreground uppercase md:block"
					>
						{m.studio_group_studio()}
					</span>
					{#each studioItems as item (item.id)}
						{@render tabButton(item)}
					{/each}
				</div>
			</div>
		</div>
	</div>
	<div class="relative z-20 min-w-0 [grid-area:journey]" data-slot="gallery-content-header">
		<GalleryActionBar
			gallery={{ ...gallery, share_token: shareToken }}
			{photoCount}
			{favoriteCount}
			{finalCount}
			{favoritesCollected}
			onupload={() => selectSection('photos')}
			{onshare}
			oncollect={() => selectSection('favorites')}
			onretouch={() => selectSection('final')}
			ondelivery={onpublish}
			onrestore={section === 'settings' ? undefined : onrestore}
		/>
	</div>

	<main
		class="min-w-0 workspace-inset pt-4 pb-10 [grid-area:stage] sm:pt-5 sm:pb-12 md:min-h-0 md:overflow-y-auto"
	>
		<!-- All panels stay mounted so counts load on open and in-flight edits survive tab switches. -->
		<div
			role="tabpanel"
			id="panel-photos"
			aria-labelledby="tab-photos"
			hidden={section !== 'photos'}
		>
			<PhotosPanel
				galleryId={id}
				openPhotoId={requestedPhoto?.kind === 'proof' ? requestedPhoto.id : undefined}
				bind:heroId={() => coverId, (next) => (heroId = next)}
				bind:heroRevision
				active={section === 'photos'}
				oncount={(n) => (photoCount = n)}
			/>
		</div>
		<div
			role="tabpanel"
			id="panel-favorites"
			aria-labelledby="tab-favorites"
			hidden={section !== 'favorites'}
			class={section === 'favorites' ? 'grid gap-4' : undefined}
		>
			<FavoritesPanel
				galleryId={id}
				active={section === 'favorites'}
				oncount={(n) => (favoriteCount = n)}
				onfavorites={favoritesChanged}
				onfulldownload={fullFavoritesDownloaded}
			/>
		</div>
		<div
			role="tabpanel"
			id="panel-comments"
			aria-labelledby="tab-comments"
			hidden={section !== 'comments'}
			class={section === 'comments' ? 'grid gap-4' : undefined}
		>
			<CommentsPanel
				galleryId={id}
				{openThread}
				active={section === 'comments'}
				oncount={(n) => (commentCount = n)}
			/>
		</div>
		<div role="tabpanel" id="panel-final" aria-labelledby="tab-final" hidden={section !== 'final'}>
			<PhotosPanel
				set="finals"
				openPhotoId={requestedPhoto?.kind === 'final' ? requestedPhoto.id : undefined}
				galleryId={id}
				active={section === 'final'}
				oncount={(n) => (finalCount = n)}
			/>
		</div>
		<div
			role="tabpanel"
			id="panel-activity"
			aria-labelledby="tab-activity"
			hidden={section !== 'activity'}
		>
			<GalleryActivityPanel galleryId={id} onopen={openDestination} />
		</div>
		<div
			role="tabpanel"
			id="panel-design"
			aria-labelledby="tab-design"
			hidden={section !== 'design'}
		>
			<GalleryDesignPanel
				{gallery}
				{settings}
				bind:heroId={() => coverId, (next) => (heroId = next)}
				{heroRevision}
				bind:focalPoint={liveFocalPoint}
				{focalReady}
				active={section === 'design'}
				{onsave}
				{onsavefocal}
				{onbeforepreview}
			/>
		</div>
		<div
			role="tabpanel"
			id="panel-contacts"
			aria-labelledby="tab-contacts"
			hidden={section !== 'contacts'}
		>
			<GalleryContactsPanel
				galleryId={id}
				revision={contactsRevision}
				oncount={(n) => {
					contactCount = n;
				}}
			/>
		</div>
		<div
			role="tabpanel"
			id="panel-settings"
			aria-labelledby="tab-settings"
			hidden={section !== 'settings'}
		>
			<GallerySettingsPanel
				{gallery}
				{shareToken}
				{origin}
				bind:expectedAccessCode
				{onsave}
				{schedule}
				{cancel}
				onsharelink={onshare}
				{onarchive}
				{onrestore}
				{ondelete}
			/>
		</div>
	</main>
</div>
