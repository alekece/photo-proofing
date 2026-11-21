<script module lang="ts">
	export type StudioSection =
		| 'photos'
		| 'favorites'
		| 'comments'
		| 'final'
		| 'activity'
		| 'design'
		| 'share'
		| 'contacts'
		| 'settings';
</script>

<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import {
		Activity as ActivityIcon,
		ArrowLeft,
		CheckCircle2,
		Heart,
		Image as ImageIcon,
		Lock,
		MessageSquare,
		Palette,
		Send,
		SlidersHorizontal,
		Star,
		Users
	} from '@lucide/svelte';
	import { tick, type Component, type Snippet } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import CommentsPanel from '#lib/components/CommentsPanel.svelte';
	import CoverBackdrop from '#lib/components/CoverBackdrop.svelte';
	import GalleryActivityPanel from '#lib/components/GalleryActivityPanel.svelte';
	import FavoritesPanel from '#lib/components/FavoritesPanel.svelte';
	import GalleryActionBar from '#lib/components/GalleryActionBar.svelte';
	import GalleryContactsPanel from '#lib/components/GalleryContactsPanel.svelte';
	import GalleryDesignPanel from '#lib/components/GalleryDesignPanel.svelte';
	import GallerySettingsPanel from '#lib/components/GallerySettingsPanel.svelte';
	import GallerySharePanel from '#lib/components/GallerySharePanel.svelte';
	import PhotosPanel from '#lib/components/PhotosPanel.svelte';
	import StatusChip from '#lib/components/ui/StatusChip/StatusChip.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import type { FocalPoint, Gallery, GalleryChanges, QueuedInvitations } from '#lib/gallery/api.js';
	import { getGalleryGridReturn } from '#lib/gallery/dashboardNavigation.svelte.js';
	import { formatDateTime, formatLongDate, galleryStatus } from '#lib/gallery/format.js';
	import { previewUrl } from '#lib/gallery/photos.js';
	import { DEFAULT_FOCAL_POINT } from '#lib/gallery/schemas.js';
	import type { ShareAttempt } from '#lib/gallery/types.js';
	import { DEFAULT_SETTINGS, type WorkspaceSettings } from '#lib/settings/settings.js';

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
		settings = DEFAULT_SETTINGS,
		uploading = 0,
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
		onstartsend,
		oninvitationsent,
		requestedPhoto
	}: {
		gallery: Gallery;
		shareToken: string | null;
		origin: string;
		settings?: WorkspaceSettings;
		uploading?: number;
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
		onstartsend?: () => ShareAttempt;
		oninvitationsent?: (result: QueuedInvitations) => void;
		requestedPhoto?: { id: string; kind: 'proof' | 'final' };
	} = $props();

	let section = $state<StudioSection>('photos');
	const gridReturn = getGalleryGridReturn();
	const headerActions = new SvelteMap<StudioSection, Snippet>();
	const requestedId = $derived(requestedPhoto?.id);
	const requestedKind = $derived(requestedPhoto?.kind);
	$effect(() => {
		if (requestedId) section = requestedKind === 'final' ? 'final' : 'photos';
	});
	let previousSection = $state<StudioSection>('photos');
	let photoCount = $state<number | null>(null);
	let favoriteCount = $state<number | null>(null);
	let commentCount = $state<number | null>(null);
	let finalCount = $state<number | null>(null);
	let contactCount = $state<number | null>(null);
	let shareOpened = $state(false);
	let contactsPanelRevision = $state(0);
	let shareContactsRevision = $state(0);

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
	const previewAccessCode = $derived(
		shared
			? (gallery.access_code ?? null)
			: expectedAccessCode !== undefined && expectedAccessCode !== 'invalid'
				? expectedAccessCode
				: (gallery.access_code ?? null)
	);

	$effect(() => {
		if (section === 'share') {
			shareOpened = true;
		}
	});

	$effect(() => {
		if (archived && section === 'share') {
			section = 'photos';
		}
	});

	function selectSection(target: StudioSection) {
		if (allItems.find((item) => item.id === target)?.disabled) return;

		if (target === 'share') {
			if (expectedAccessCode === 'invalid') {
				toasts.error(m.settings_access_code_invalid());

				return;
			}

			if (section !== 'share') {
				previousSection = section;
			}
		}

		section = target;
	}

	function leaveShare() {
		const target =
			previousSection !== 'share' && !allItems.find((item) => item.id === previousSection)?.disabled
				? previousSection
				: 'photos';

		section = target;
		void tick().then(() => {
			if (section === target) document.getElementById(`tab-${target}`)?.focus();
		});
	}

	const day = $derived(gallery.shoot_date ? formatLongDate(gallery.shoot_date) : null);
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
		{
			id: 'share',
			label: m.tab_share(),
			icon: Send,
			disabled: archived,
			title: archived ? m.tab_share_archived() : undefined
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

{#snippet tabButton(item: NavItem)}
	{@const active = section === item.id}
	{@const Icon = item.icon}
	<button
		type="button"
		role="tab"
		id={`tab-${item.id}`}
		data-share-trigger={item.id === 'share' ? true : undefined}
		aria-selected={active}
		aria-controls={`panel-${item.id}`}
		disabled={item.disabled ?? false}
		title={item.title}
		tabindex={active ? 0 : -1}
		class="flex shrink-0 cursor-pointer items-center gap-2 rounded-full px-3 py-1.5 text-[0.84375rem] leading-[1.2] whitespace-nowrap transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-45 md:w-full md:rounded-[0.5625rem] md:px-2.5 md:py-2 {active
			? 'bg-foreground font-semibold text-background'
			: 'bg-border/45 font-medium text-foreground enabled:hover:bg-border/70 md:bg-transparent'}"
		onclick={() => selectSection(item.id)}
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
	class="grid min-h-[calc(100dvh-3.75rem)] grid-cols-[minmax(0,1fr)] content-start [grid-template-areas:'identity''journey''nav''stage'] md:grid-cols-[var(--gallery-rail-width)_minmax(0,1fr)] md:grid-rows-[auto_auto_1fr] md:content-stretch md:[grid-template-areas:'identity_journey''identity_stage''nav_stage']"
>
	<div
		data-slot="gallery-identity"
		class="grid min-w-0 grid-cols-[auto_minmax(0,1fr)] items-center gap-x-2.5 gap-y-1.5 bg-panel px-3.5 pt-2.5 pb-2 [grid-area:identity] md:grid-cols-1 md:items-stretch md:gap-3 md:border-r md:border-border md:px-4 md:pt-4 md:pb-3"
	>
		<a
			href={gridReturn?.href ?? '/dashboard'}
			aria-label={m.header_back_to_galleries()}
			class="inline-flex size-8 shrink-0 items-center justify-center rounded-full border border-border bg-background text-xs font-medium text-muted-foreground hover:text-foreground md:size-auto md:w-fit md:justify-start md:gap-1.5 md:border-0 md:bg-transparent md:px-0.5 md:py-0.5"
		>
			<ArrowLeft class="size-3.5 shrink-0" aria-hidden="true" />
			<span class="max-md:sr-only">{m.header_back_to_galleries()}</span>
		</a>

		<div class="relative isolate h-20 min-w-0 md:aspect-[3/2] md:h-auto">
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

			<button
				type="button"
				aria-label={favorite ? m.studio_favorite_remove() : m.studio_favorite_add()}
				title={favorite ? m.studio_favorite_remove() : m.studio_favorite_add()}
				aria-pressed={favorite}
				onclick={() => void toggleFavorite()}
				class="absolute top-2 left-2 grid size-8 cursor-pointer place-items-center rounded-full bg-panel transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus {favorite
					? 'text-hero'
					: 'text-muted-foreground hover:text-foreground'}"
			>
				<Star size={15} fill={favorite ? 'currentColor' : 'none'} aria-hidden="true" />
			</button>
			<StatusChip {status} dot workspace class="pointer-events-none absolute top-2 right-2" />
			{#if day}
				<p
					class="pointer-events-none absolute bottom-2 left-2 rounded-full bg-dark/80 px-2 py-0.5 text-xs text-inverse"
				>
					<span>{m.details_shot()}</span>
					<span>{day}</span>
				</p>
			{/if}
		</div>
		<div class="grid min-w-0 gap-0.5 max-md:col-start-2 md:gap-1">
			<div class="flex min-w-0 flex-wrap items-center gap-2">
				{@render heading()}
			</div>
			{#if shared}
				<p class="truncate text-xs text-muted-foreground">
					<span>{m.details_last_opened()}</span>
					<span>{openedValue}</span>
				</p>
			{/if}
		</div>
	</div>

	<div class="min-w-0 [grid-area:journey]" data-slot="gallery-content-header">
		<GalleryActionBar
			gallery={{ ...gallery, share_token: shareToken }}
			{photoCount}
			{favoriteCount}
			action={headerActions.get(section)}
			onupload={() => selectSection('photos')}
			onshare={() => selectSection('share')}
			oncollect={() => selectSection('favorites')}
			ondelivery={() => selectSection('share')}
			onrestore={section === 'settings' ? undefined : onrestore}
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

	<main class="min-w-0 workspace-inset pt-4 pb-10 [grid-area:stage] sm:pt-5 sm:pb-12">
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
				onheader={(action) => headerActions.set('photos', action)}
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
				onheader={(action) => headerActions.set('final', action)}
			/>
		</div>
		<div
			role="tabpanel"
			id="panel-activity"
			aria-labelledby="tab-activity"
			hidden={section !== 'activity'}
		>
			<GalleryActivityPanel galleryId={id} />
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
				onheader={(action) => headerActions.set('design', action)}
			/>
		</div>
		<div role="tabpanel" id="panel-share" aria-labelledby="tab-share" hidden={section !== 'share'}>
			{#if shareOpened}
				<GallerySharePanel
					gallery={{ ...gallery, share_token: shareToken }}
					active={section === 'share'}
					{settings}
					{coverId}
					{heroRevision}
					focalPoint={liveFocalPoint}
					{photoCount}
					{finalCount}
					{uploading}
					accessCode={previewAccessCode}
					contactsRevision={shareContactsRevision}
					{onstartsend}
					oncancel={leaveShare}
					{onpublish}
					onheader={(action) =>
						action ? headerActions.set('share', action) : headerActions.delete('share')}
					onopensettings={() => selectSection('settings')}
					onsent={(result) => {
						oninvitationsent?.(result);
						contactsPanelRevision += 1;
						shareContactsRevision += 1;
						toasts.success(m.share_invitations_queued({ count: result.queued_count }));
						leaveShare();
					}}
				/>
			{/if}
		</div>
		<div
			role="tabpanel"
			id="panel-contacts"
			aria-labelledby="tab-contacts"
			hidden={section !== 'contacts'}
		>
			<GalleryContactsPanel
				galleryId={id}
				revision={contactsPanelRevision}
				onheader={(action) => headerActions.set('contacts', action)}
				oncount={(n) => {
					if (contactCount !== null && contactCount !== n) {
						shareContactsRevision += 1;
					}
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
