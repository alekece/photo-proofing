<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onDestroy, type Snippet } from 'svelte';
	import { Check, Eye, ImagePlus, Maximize2, X } from '@lucide/svelte';
	import ClientGallery from '#lib/components/ClientGallery.svelte';
	import ClientPreview from '#lib/components/ClientPreview.svelte';
	import PreviewFrame from '#lib/components/PreviewFrame.svelte';
	import LayoutRowsIcon from '#lib/components/icons/LayoutRowsIcon.svelte';
	import LayoutStaggeredIcon from '#lib/components/icons/LayoutStaggeredIcon.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import Card from '#lib/components/ui/Card/Card.svelte';
	import Dialog from '#lib/components/ui/Dialog/Dialog.svelte';
	import IconButtonGroup from '#lib/components/ui/IconButtonGroup/IconButtonGroup.svelte';
	import { getLiveEvents } from '#lib/events.js';
	import type {
		CoverLayout,
		FocalPoint,
		Gallery,
		GalleryChanges,
		GalleryLayout,
		GridDensity,
		GridSpacing,
		HeadingFont
	} from '#lib/gallery/api.js';
	import { hasCleanPhoto, previewUrl, thumbnailUrl, type Photo } from '#lib/gallery/photos.js';
	import { loadClientPreviewData, type PreviewViewport } from '#lib/gallery/preview.js';
	import { DEFAULT_FOCAL_POINT, HEADING_FONT_FAMILIES } from '#lib/gallery/schemas.js';
	import type { Proof } from '#lib/gallery/types.js';
	import { DEFAULT_SETTINGS, type WorkspaceSettings } from '#lib/settings/settings.js';

	interface DesignFields {
		layout: GalleryLayout;
		cover_layout: CoverLayout;
		heading_font: HeadingFont;
		grid_density: GridDensity;
		grid_spacing: GridSpacing;
	}

	const DESIGN_KEYS: (keyof DesignFields)[] = [
		'layout',
		'cover_layout',
		'heading_font',
		'grid_density',
		'grid_spacing'
	];

	/**
	 * Studio `Design` view: compact controls (cover with focal point, then the
	 * cover style/typography/grid sections) alongside a live client preview. At `lg` and up this is
	 * a one-third/two-thirds split with the inline preview growing with the page (no fixed height
	 * or own scrollbar); below `lg` only "Open full preview" in Design remains available.
	 */
	let {
		gallery,
		settings = DEFAULT_SETTINGS,
		heroId = $bindable(),
		heroRevision,
		focalPoint = $bindable(),
		focalReady = true,
		active = true,
		onsave,
		onsavefocal = async (point: FocalPoint) =>
			(await onsave({ focal_point: point })) ? point : undefined,
		onbeforepreview,
		onheader
	}: {
		gallery: Gallery;
		settings?: WorkspaceSettings;
		heroId?: string | null;
		heroRevision?: number;
		focalPoint?: FocalPoint;
		focalReady?: boolean;
		active?: boolean;
		onsave: (changes: GalleryChanges) => Promise<boolean>;
		onsavefocal?: (point: FocalPoint) => Promise<FocalPoint | undefined>;
		onbeforepreview?: () => void;
		onheader?: (action: Snippet) => void;
	} = $props();

	$effect(() => onheader?.(previewAction));

	const events = getLiveEvents();

	const savedDesign = $derived<DesignFields>({
		layout: gallery.layout,
		cover_layout: gallery.cover_layout ?? 'full_bleed',
		heading_font: gallery.heading_font ?? 'editorial',
		grid_density: gallery.grid_density ?? 'compact',
		grid_spacing: gallery.grid_spacing ?? 'compact'
	});
	const savedHeroId = $derived<string | null>(heroId !== undefined ? heroId : gallery.hero_id);

	let committed = $state<Partial<DesignFields>>({});
	let overrides = $state<Partial<DesignFields>>({});
	let fieldSeq: Partial<Record<keyof DesignFields, number>> = {};
	let nextSeq = 0;

	const design = $derived<DesignFields>({
		...savedDesign,
		...committed,
		...overrides
	});
	let localHeroId = $derived<string | null>(savedHeroId);
	const savedFocal = $derived<FocalPoint>(gallery.focal_point ?? { ...DEFAULT_FOCAL_POINT });
	// svelte-ignore state_referenced_locally
	let localFocal = $state<FocalPoint>(focalPoint ?? savedFocal);
	let pendingFocal = false;
	let latestFocal = 0;
	let galleryRevision = 0;
	let disposed = false;
	onDestroy(() => (disposed = true));
	const focalHelpId = $props.id();
	let drag: { pointerId: number; coverId: string; initial: FocalPoint } | null = null;
	// svelte-ignore state_referenced_locally
	let lastGalleryId = gallery.id;
	// svelte-ignore state_referenced_locally
	let lastSavedDesign = savedDesign;
	// svelte-ignore state_referenced_locally
	let lastSavedFocal = savedFocal;
	// svelte-ignore state_referenced_locally
	let committedFocal = savedFocal;

	$effect.pre(() => {
		if (gallery.id !== lastGalleryId) {
			lastGalleryId = gallery.id;
			lastSavedDesign = savedDesign;
			lastSavedFocal = savedFocal;
			committed = {};
			overrides = {};
			fieldSeq = {};
			pendingFocal = false;
			galleryRevision++;
			drag = null;
			committedFocal = savedFocal;
			localFocal = savedFocal;
			focalPoint = savedFocal;
			proofPhotos = [];
			previewProofs = [];
			previewFavorites = [];
			photosLoadFailed = false;

			return;
		}

		for (const key of DESIGN_KEYS) {
			if (savedDesign[key] !== lastSavedDesign[key]) {
				lastSavedDesign = { ...lastSavedDesign, [key]: savedDesign[key] };

				if (overrides[key] === undefined && committed[key] !== undefined) {
					const nextCommitted = { ...committed };
					delete nextCommitted[key];
					committed = nextCommitted;
				}
			}
		}

		if (savedFocal.x !== lastSavedFocal.x || savedFocal.y !== lastSavedFocal.y) {
			lastSavedFocal = savedFocal;

			if (!pendingFocal) {
				committedFocal = savedFocal;
			}
			if (!pendingFocal && !drag) {
				localFocal = savedFocal;
				focalPoint = savedFocal;
			}
		}
	});

	let pickingCover = $state(false);
	let expanded = $state(false);
	let previewMode = $state<PreviewViewport>('desktop');
	// svelte-ignore state_referenced_locally
	let opened = $state(active);
	let proofPhotos = $state<Photo[]>([]);
	let photosLoadFailed = $state(false);
	let previewProofs = $state<Proof[]>([]);
	let previewFavorites = $state<string[]>([]);
	let refreshTick = $state(0);

	$effect(() => {
		if (active) opened = true;
	});

	const delivered = $derived(gallery.states.includes('delivered'));
	const readyPhotos = $derived(proofPhotos.filter(hasCleanPhoto));
	const hasCover = $derived(localHeroId !== null);
	const coverImageUrl = $derived(
		localHeroId ? previewUrl(gallery.id, localHeroId, 'photos', heroRevision) : undefined
	);
	// Carries unsaved local edits (layout, cover, focal point...) into both the embedded live
	// preview and the "Open full preview" overlay, so neither waits on a PATCH to reflect them.
	const previewGallery = $derived<Gallery>({
		...gallery,
		layout: design.layout,
		cover_layout: design.cover_layout,
		focal_point: localFocal,
		heading_font: design.heading_font,
		grid_density: design.grid_density,
		grid_spacing: design.grid_spacing,
		hero_id: localHeroId
	});

	const layouts = $derived([
		{ id: 'masonry', label: m.settings_layout_masonry(), icon: LayoutStaggeredIcon },
		{ id: 'grid', label: m.settings_layout_grid(), icon: LayoutRowsIcon }
	]);

	const coverTemplates = $derived<{ id: CoverLayout; label: string }[]>([
		{ id: 'full_bleed', label: m.design_cover_full_bleed() },
		{ id: 'split', label: m.design_cover_split() },
		{ id: 'framed', label: m.design_cover_framed() },
		{ id: 'banner', label: m.design_cover_banner() }
	]);

	const headingFonts = $derived<{ id: HeadingFont; label: string }[]>([
		{ id: 'editorial', label: m.design_font_editorial() },
		{ id: 'fine_art', label: m.design_font_fine_art() },
		{ id: 'timeless', label: m.design_font_timeless() },
		{ id: 'modern', label: m.design_font_modern() }
	]);

	const gridDensities = $derived<{ id: GridDensity; label: string }[]>([
		{ id: 'compact', label: m.design_density_compact() },
		{ id: 'large', label: m.design_density_large() }
	]);

	const gridSpacings = $derived<{ id: GridSpacing; label: string }[]>([
		{ id: 'compact', label: m.design_spacing_compact() },
		{ id: 'generous', label: m.design_spacing_generous() }
	]);

	const sectionCard = 'grid gap-3 rounded-[0.875rem] border border-border bg-panel p-4 sm:p-5';
	const sectionHeading =
		'font-sans! text-[0.6875rem] leading-none font-semibold! tracking-[0.08em] text-muted-foreground uppercase';
	const pillGroup = 'inline-flex rounded-full border border-border bg-background p-[3px]';
	const pillButton =
		'cursor-pointer rounded-full px-3 py-1 text-xs font-medium text-muted-foreground transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus aria-pressed:bg-foreground aria-pressed:text-panel';

	// Load preview data when the Design tab is shown or when gallery media changes.
	$effect(() => {
		if (!active && !pickingCover) return;

		const id = gallery.id;
		const watermarked = gallery.watermark_enabled;
		const finals = delivered;
		void refreshTick;
		let alive = true;

		loadClientPreviewData(id, {
			delivered: finals,
			watermarked,
			includeOwnerProofs: true
		})
			.then((data) => {
				if (!alive) return;

				photosLoadFailed = false;
				proofPhotos = data.proofPhotos;
				previewProofs = data.proofs;
				previewFavorites = data.favorites;
			})
			.catch(() => {
				if (!alive) return;

				photosLoadFailed = true;
			});

		return () => {
			alive = false;
		};
	});

	$effect(() => {
		if (!active || !events) return;

		const id = gallery.id;
		const stopPhoto = events.onPhoto((event) => {
			if (event.gallery_id === id) refreshTick++;
		});
		const stopFavorites = events.onFavorites((event) => {
			if (event.gallery_id === id) refreshTick++;
		});

		return () => {
			stopPhoto();
			stopFavorites();
		};
	});

	function setHeroState(next: string | null) {
		localHeroId = next;
		heroId = next;
	}

	function setFocalState(next: FocalPoint) {
		localFocal = next;
		focalPoint = next;
	}

	async function setDesignField<K extends keyof DesignFields>(key: K, next: DesignFields[K]) {
		if (next === design[key]) return;

		const targetGalleryId = gallery.id;
		const seq = ++nextSeq;
		fieldSeq[key] = seq;
		overrides = { ...overrides, [key]: next };

		const ok = await onsave({ [key]: next } as GalleryChanges).catch(() => false);

		if (gallery.id !== targetGalleryId || fieldSeq[key] !== seq) return;

		if (ok) {
			committed = { ...committed, [key]: next };
		}

		const nextOverrides = { ...overrides };
		delete nextOverrides[key];
		overrides = nextOverrides;
	}

	async function chooseCover(nextId: string | null) {
		const previous = localHeroId;

		pickingCover = false;
		if (nextId === previous) return;

		setHeroState(nextId);

		if (!(await onsave({ hero_id: nextId }))) {
			setHeroState(previous);
		}
	}

	async function persistFocalPoint(next: FocalPoint) {
		const galleryId = gallery.id;
		const revision = galleryRevision;
		const sequence = ++latestFocal;
		pendingFocal = true;
		setFocalState(next);
		const confirmed = await onsavefocal(next).catch(() => undefined);

		if (disposed || gallery.id !== galleryId || galleryRevision !== revision) return;

		// An older successful write is still the rollback point for newer edits.
		if (confirmed) committedFocal = confirmed;
		if (sequence !== latestFocal) return;

		pendingFocal = false;
		if (!drag) setFocalState(committedFocal);
	}

	function clampPercent(value: number): number {
		return Math.min(100, Math.max(0, Math.round(value)));
	}

	function updateFocalFromPointer(event: PointerEvent & { currentTarget: HTMLButtonElement }) {
		const rect = event.currentTarget.getBoundingClientRect();

		if (rect.width <= 0 || rect.height <= 0) return;

		const x = clampPercent(((event.clientX - rect.left) / rect.width) * 100);
		const y = clampPercent(((event.clientY - rect.top) / rect.height) * 100);

		if (x === localFocal.x && y === localFocal.y) return;

		setFocalState({ x, y });
	}

	function onFocalPointerDown(event: PointerEvent & { currentTarget: HTMLButtonElement }) {
		if (!focalReady || !localHeroId || drag || event.button !== 0) return;

		drag = { pointerId: event.pointerId, coverId: localHeroId, initial: localFocal };
		event.currentTarget.setPointerCapture?.(event.pointerId);
		updateFocalFromPointer(event);
	}

	function onFocalPointerMove(event: PointerEvent & { currentTarget: HTMLButtonElement }) {
		if (!drag || drag.pointerId !== event.pointerId || drag.coverId !== localHeroId) return;

		updateFocalFromPointer(event);
	}

	function onFocalPointerUp(event: PointerEvent & { currentTarget: HTMLButtonElement }) {
		if (!drag || drag.pointerId !== event.pointerId) return;

		if (drag.coverId !== localHeroId) {
			cancelFocalDrag();
			return;
		}

		updateFocalFromPointer(event);
		if (drag.initial.x === localFocal.x && drag.initial.y === localFocal.y) {
			cancelFocalDrag();
			return;
		}
		drag = null;
		void persistFocalPoint(localFocal);
	}

	function cancelFocalDrag() {
		if (!drag) return;

		const { initial } = drag;
		drag = null;
		setFocalState(pendingFocal ? initial : committedFocal);
	}

	function onFocalPointerCancel(event: PointerEvent & { currentTarget: HTMLButtonElement }) {
		if (drag?.pointerId === event.pointerId) cancelFocalDrag();
	}

	$effect.pre(() => {
		const cover = localHeroId;
		if (!active || (drag && cover !== drag.coverId)) cancelFocalDrag();
	});

	function onFocalKeydown(event: KeyboardEvent) {
		if (!focalReady || !hasCover || drag) return;

		const step = event.shiftKey ? 10 : 1;
		const deltas: Record<string, [number, number]> = {
			ArrowLeft: [-step, 0],
			ArrowRight: [step, 0],
			ArrowUp: [0, -step],
			ArrowDown: [0, step]
		};
		const delta = deltas[event.key];

		if (!delta) return;

		event.preventDefault();
		const x = clampPercent(localFocal.x + delta[0]);
		const y = clampPercent(localFocal.y + delta[1]);

		if (x === localFocal.x && y === localFocal.y) return;

		void persistFocalPoint({ x, y });
	}
</script>

{#snippet templateDiagram(kind: CoverLayout, selected: boolean)}
	{@const photoFill = selected ? 'bg-panel/35' : 'bg-foreground/20'}
	{@const lineFill = selected ? 'bg-panel/80' : 'bg-foreground/65'}
	{@const subFill = selected ? 'bg-panel/45' : 'bg-foreground/30'}
	<span
		aria-hidden="true"
		class="relative block aspect-[16/10] w-full overflow-hidden rounded-[0.375rem] border {selected
			? 'border-panel/30 bg-panel/10'
			: 'border-border bg-background'}"
	>
		{#if kind === 'full_bleed'}
			<span class="absolute inset-0 {photoFill}"></span>
			<span class="absolute top-1.5 left-1/2 h-0.5 w-4 -translate-x-1/2 rounded-full {subFill}"
			></span>
			<span class="absolute bottom-2 left-1/2 h-1 w-9 -translate-x-1/2 rounded-full {lineFill}"
			></span>
			<span class="absolute bottom-1 left-1/2 h-0.5 w-5 -translate-x-1/2 rounded-full {subFill}"
			></span>
		{:else if kind === 'split'}
			<span class="absolute inset-y-0 left-0 w-1/2 {photoFill}"></span>
			<span
				class="absolute top-1/2 right-2 flex w-[calc(50%-0.75rem)] -translate-y-1/2 flex-col items-center gap-1"
			>
				<span class="h-1 w-7 rounded-full {lineFill}"></span>
				<span class="h-0.5 w-5 rounded-full {subFill}"></span>
			</span>
		{:else if kind === 'framed'}
			<span class="absolute inset-x-3 top-2 bottom-4 rounded-[0.1875rem] {photoFill}"></span>
			<span class="absolute bottom-2 left-1/2 h-1 w-8 -translate-x-1/2 rounded-full {lineFill}"
			></span>
			<span class="absolute bottom-1 left-1/2 h-0.5 w-5 -translate-x-1/2 rounded-full {subFill}"
			></span>
		{:else}
			<span class="absolute top-2 left-1/2 h-1 w-8 -translate-x-1/2 rounded-full {lineFill}"></span>
			<span class="absolute top-3.5 left-1/2 h-0.5 w-5 -translate-x-1/2 rounded-full {subFill}"
			></span>
			<span class="absolute inset-x-2 top-5 bottom-1.5 rounded-[0.1875rem] {photoFill}"></span>
		{/if}
	</span>
{/snippet}

{#snippet previewAction()}
	<Button
		variant="outline"
		size="sm"
		onclick={() => {
			onbeforepreview?.();
			expanded = true;
		}}
	>
		<Maximize2 size={14} aria-hidden="true" />
		{m.preview_open_full()}
	</Button>
{/snippet}

<div class="grid gap-4">
	<div class="flex flex-wrap items-center justify-between gap-2">
		<h2 class="text-2xl leading-tight">{m.tab_design()}</h2>
		{#if !onheader}{@render previewAction()}{/if}
	</div>

	<div
		data-slot="design-split"
		class="grid items-start gap-5 lg:grid-cols-[minmax(20rem,1fr)_minmax(0,2fr)]"
	>
		<div class="grid gap-4">
			<Card as="section" class="gap-4 {sectionCard}">
				<h3 class={sectionHeading}>
					{m.design_cover()}
				</h3>

				<button
					data-slot="design-cover-image"
					type="button"
					disabled={!hasCover || !focalReady}
					aria-label={m.design_focal_point_pad()}
					aria-describedby={focalHelpId}
					onpointerdown={onFocalPointerDown}
					onpointermove={onFocalPointerMove}
					onpointerup={onFocalPointerUp}
					onpointercancel={onFocalPointerCancel}
					onlostpointercapture={onFocalPointerCancel}
					onkeydown={onFocalKeydown}
					class="relative isolate aspect-[3/2] w-full cursor-crosshair touch-none overflow-hidden rounded-[0.625rem] border border-border bg-dark text-panel focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-70"
				>
					{#if coverImageUrl}
						<img
							src={coverImageUrl}
							alt=""
							style:object-position="{localFocal.x}% {localFocal.y}%"
							class="pointer-events-none size-full object-cover select-none"
						/>
						<span aria-hidden="true" class="pointer-events-none absolute inset-0 bg-dark/15"></span>
					{:else}
						<span
							aria-hidden="true"
							class="absolute inset-0 bg-[radial-gradient(80%_120%_at_20%_100%,#3a2a20,transparent_70%),linear-gradient(160deg,#2b211b,#16120f_70%)]"
						></span>
						<span
							class="relative z-10 flex size-full items-center justify-center px-4 text-center text-xs font-medium text-panel/85"
						>
							{m.design_focal_point_empty()}
						</span>
					{/if}

					<span
						aria-hidden="true"
						class="pointer-events-none absolute inset-y-0 left-1/3 w-px bg-panel/30"
					></span>
					<span
						aria-hidden="true"
						class="pointer-events-none absolute inset-y-0 left-2/3 w-px bg-panel/30"
					></span>
					<span
						aria-hidden="true"
						class="pointer-events-none absolute inset-x-0 top-1/3 h-px bg-panel/30"
					></span>
					<span
						aria-hidden="true"
						class="pointer-events-none absolute inset-x-0 top-2/3 h-px bg-panel/30"
					></span>

					{#if hasCover}
						<span
							aria-hidden="true"
							style:left="{localFocal.x}%"
							style:top="{localFocal.y}%"
							class="pointer-events-none absolute size-6 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-panel bg-foreground/55 shadow-[0_2px_8px_rgb(0_0_0/0.55)]"
						>
							<span
								class="absolute top-1/2 left-1/2 size-1.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-panel"
							></span>
						</span>
					{/if}
				</button>
				<span id={focalHelpId} class="sr-only">
					{hasCover ? m.design_focal_point_keyboard() : m.design_focal_point_empty()}
				</span>
				<div data-slot="design-cover-actions" class="flex flex-wrap items-center gap-1">
					<Button variant="outline" size="sm" onclick={() => (pickingCover = true)}>
						<ImagePlus size={15} aria-hidden="true" class="mr-1" />
						{hasCover ? m.design_change_cover() : m.design_select_cover()}
					</Button>
					{#if hasCover}
						<Button
							variant="outline"
							size="sm"
							class="border-danger text-danger hover:bg-danger/5"
							onclick={() => void chooseCover(null)}
						>
							{m.cover_remove()}
						</Button>
					{/if}
				</div>
			</Card>

			<Card as="section" class={sectionCard}>
				<h3 class={sectionHeading}>
					{m.design_cover_style()}
				</h3>
				<div role="group" aria-label={m.design_cover_style()} class="grid grid-cols-2 gap-2">
					{#each coverTemplates as item (item.id)}
						{@const selected = design.cover_layout === item.id}
						<button
							type="button"
							aria-pressed={selected}
							onclick={() => void setDesignField('cover_layout', item.id)}
							class="flex cursor-pointer flex-col gap-1.5 rounded-[0.625rem] border p-2 text-left text-xs font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus {selected
								? 'border-foreground bg-foreground text-panel'
								: 'border-border bg-background text-foreground hover:border-border-strong'}"
						>
							{@render templateDiagram(item.id, selected)}
							<span class="truncate">{item.label}</span>
						</button>
					{/each}
				</div>
			</Card>

			<Card as="section" class={sectionCard}>
				<h3 class={sectionHeading}>
					{m.design_typography()}
				</h3>
				<div role="group" aria-label={m.design_heading_font()} class="grid grid-cols-2 gap-2">
					{#each headingFonts as item (item.id)}
						{@const selected = design.heading_font === item.id}
						{@const fontFamily = HEADING_FONT_FAMILIES[item.id]}
						<button
							type="button"
							aria-pressed={selected}
							onclick={() => void setDesignField('heading_font', item.id)}
							class="flex cursor-pointer flex-col gap-1.5 rounded-[0.625rem] border px-3 py-2.5 text-left transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus {selected
								? 'border-foreground bg-foreground text-panel'
								: 'border-border bg-background text-foreground hover:border-border-strong'}"
						>
							<span aria-hidden="true" style:font-family={fontFamily} class="text-2xl leading-none"
								>Aa</span
							>
							<span style:font-family={fontFamily} class="truncate text-sm">{item.label}</span>
						</button>
					{/each}
				</div>
			</Card>

			<Card as="section" class={sectionCard}>
				<h3 class={sectionHeading}>
					{m.design_grid_and_spacing()}
				</h3>
				<div class="flex flex-wrap items-center justify-between gap-2.5 text-sm">
					<span class="text-foreground">{m.settings_layout()}</span>
					<IconButtonGroup
						value={design.layout}
						items={layouts}
						label={m.settings_layout()}
						onchange={(value) => void setDesignField('layout', value as GalleryLayout)}
					/>
				</div>
				<div class="flex flex-wrap items-center justify-between gap-2.5 text-sm">
					<span class="text-foreground">{m.design_grid_density()}</span>
					<div role="group" aria-label={m.design_grid_density()} class={pillGroup}>
						{#each gridDensities as item (item.id)}
							<button
								type="button"
								aria-pressed={design.grid_density === item.id}
								onclick={() => void setDesignField('grid_density', item.id)}
								class={pillButton}
							>
								{item.label}
							</button>
						{/each}
					</div>
				</div>
				<div class="flex flex-wrap items-center justify-between gap-2.5 text-sm">
					<span class="text-foreground">{m.design_grid_spacing()}</span>
					<div role="group" aria-label={m.design_grid_spacing()} class={pillGroup}>
						{#each gridSpacings as item (item.id)}
							<button
								type="button"
								aria-pressed={design.grid_spacing === item.id}
								onclick={() => void setDesignField('grid_spacing', item.id)}
								class={pillButton}
							>
								{item.label}
							</button>
						{/each}
					</div>
				</div>
			</Card>
		</div>

		<section
			aria-label={m.preview_label()}
			class="hidden min-w-0 flex-col overflow-hidden rounded-[0.875rem] border border-border bg-[#1d1815] shadow-[0_18px_40px_-24px_rgb(43_36_32/0.45)] lg:flex"
		>
			<PreviewFrame bind:mode={previewMode} stageClass="flex-1" scrollable={false}>
				{#snippet header(toggle)}
					<div
						class="flex flex-wrap items-center justify-between gap-2.5 border-b border-[#3b322b] bg-foreground px-4 py-2.5 text-sm font-medium text-[#efe6da]"
					>
						<span class="flex items-center gap-2 font-semibold">
							<Eye size={16} aria-hidden="true" />
							<span>{m.preview_label()}</span>
						</span>
						{@render toggle()}
					</div>
				{/snippet}

				{#if opened}
					<ClientGallery
						gallery={{ ...previewGallery, delivered }}
						coverUrl={coverImageUrl}
						proofs={previewProofs}
						favorites={previewFavorites}
						studio={settings.studioName}
						studioLogo={settings.logo}
						signature={settings.signature}
						signatureSize={settings.signatureSize}
						signatureTextColor={settings.signatureTextColor}
						preview
						embedded
						showCoverWhenEmpty
						framed={previewMode === 'phone'}
					/>
				{/if}
			</PreviewFrame>
		</section>
	</div>
</div>

{#if expanded}
	<ClientPreview
		gallery={previewGallery}
		{settings}
		{heroRevision}
		onclose={() => (expanded = false)}
	/>
{/if}

{#if pickingCover}
	<Dialog
		onclose={() => (pickingCover = false)}
		label={m.design_cover_picker_title()}
		aria-modal="true"
		class="m-auto w-[min(42rem,calc(100vw-1.5rem))] rounded-[1.125rem] border border-border bg-panel p-5 text-foreground shadow-xl sm:p-6"
	>
		{#snippet children(close)}
			<div class="mb-4 flex items-center justify-between gap-3">
				<h3 class="font-serif text-xl leading-tight">{m.design_cover_picker_title()}</h3>
				<button
					type="button"
					aria-label={m.action_close()}
					onclick={close}
					class="inline-flex size-8 cursor-pointer items-center justify-center rounded-full border border-border bg-background text-foreground hover:bg-border/45 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus"
				>
					<X size={16} aria-hidden="true" />
				</button>
			</div>

			{#if photosLoadFailed && readyPhotos.length === 0}
				<p role="alert" class="py-10 text-center text-sm text-danger">
					{m.design_preview_load_failed()}
				</p>
			{:else if readyPhotos.length === 0}
				<p class="py-10 text-center text-sm text-muted-foreground">
					{m.design_cover_picker_empty()}
				</p>
			{:else}
				<div class="grid max-h-[60dvh] grid-cols-2 gap-2.5 overflow-y-auto p-0.5 sm:grid-cols-3">
					{#each readyPhotos as photo (photo.id)}
						{@const selected = localHeroId === photo.id}
						<button
							type="button"
							aria-label={m.design_cover_pick_photo({ name: photo.original_name })}
							aria-pressed={selected}
							onclick={() => void chooseCover(photo.id)}
							class="group relative aspect-[3/2] cursor-pointer overflow-hidden rounded-[0.625rem] border-2 bg-dark focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus {selected
								? 'border-foreground ring-2 ring-foreground/25'
								: 'border-transparent hover:border-border-strong'}"
						>
							<img
								src={thumbnailUrl(gallery.id, photo.id, 'photos', photo.revision)}
								alt=""
								loading="lazy"
								class="size-full object-cover transition-transform duration-200 group-hover:scale-[1.03]"
							/>
							{#if selected}
								<span
									aria-hidden="true"
									class="absolute top-2 right-2 inline-flex size-6 items-center justify-center rounded-full bg-foreground text-panel shadow"
								>
									<Check size={14} />
								</span>
							{/if}
						</button>
					{/each}
				</div>
			{/if}
		{/snippet}
	</Dialog>
{/if}
