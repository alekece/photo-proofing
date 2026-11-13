<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Check, Eye, ImagePlus, Monitor, RotateCcw, Smartphone, X } from '@lucide/svelte';
	import ClientGallery from '#lib/components/ClientGallery.svelte';
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
	import {
		hasCleanPhoto,
		listClientPreviewPhotos,
		listPhotos,
		previewUrl,
		thumbnailUrl,
		type Photo
	} from '#lib/gallery/photos.js';
	import { toClientPreviewProofs, toOwnerProofs } from '#lib/gallery/proofs.js';
	import { DEFAULT_FOCAL_POINT, HEADING_FONT_FAMILIES } from '#lib/gallery/schemas.js';
	import type { Proof } from '#lib/gallery/types.js';
	import { parseSettings, type WorkspaceSettings } from '#lib/settings/settings.js';
	import { cn } from '#lib/utils.js';

	/**
	 * Studio `Design` view: two-column workspace with cover picker, four cover templates,
	 * rule-of-thirds focal-point pad, heading typography presets, photo grid layout/density/spacing
	 * controls, and a live scrollable client preview.
	 */
	let {
		gallery,
		heroId = $bindable(),
		heroRevision,
		focalPoint = $bindable(),
		active = true,
		onsave,
		schedule,
		cancel
	}: {
		gallery: Gallery;
		heroId?: string | null;
		heroRevision?: number;
		focalPoint?: FocalPoint;
		active?: boolean;
		onsave: (changes: GalleryChanges) => Promise<boolean>;
		schedule?: (field: string, run: () => void) => void;
		cancel?: (field: string) => void;
	} = $props();

	const events = getLiveEvents();

	let currentLayout = $derived<GalleryLayout>(gallery.layout);
	let currentCoverLayout = $derived<CoverLayout>(gallery.cover_layout ?? 'full_bleed');
	let currentHeadingFont = $derived<HeadingFont>(gallery.heading_font ?? 'editorial');
	let currentGridDensity = $derived<GridDensity>(gallery.grid_density ?? 'compact');
	let currentGridSpacing = $derived<GridSpacing>(gallery.grid_spacing ?? 'compact');
	let localHeroId = $derived<string | null>(heroId !== undefined ? heroId : gallery.hero_id);
	const savedFocal = $derived<FocalPoint>(gallery.focal_point ?? { ...DEFAULT_FOCAL_POINT });
	// svelte-ignore state_referenced_locally
	let localFocal = $state<FocalPoint>(focalPoint ?? savedFocal);
	let focalDirty = false;
	// svelte-ignore state_referenced_locally
	let lastGalleryId = gallery.id;
	// svelte-ignore state_referenced_locally
	let lastSavedFocal = savedFocal;

	$effect.pre(() => {
		if (gallery.id !== lastGalleryId) {
			lastGalleryId = gallery.id;
			lastSavedFocal = savedFocal;
			focalDirty = false;
			localFocal = savedFocal;
			focalPoint = savedFocal;
			proofPhotos = [];
			previewProofs = [];
			previewFavorites = [];
			photosLoadFailed = false;

			return;
		}

		if (savedFocal.x !== lastSavedFocal.x || savedFocal.y !== lastSavedFocal.y) {
			lastSavedFocal = savedFocal;

			if (!focalDirty) {
				localFocal = savedFocal;
				focalPoint = savedFocal;
			}
		}
	});

	let pickingCover = $state(false);
	let draggingFocal = $state(false);
	let previewMode = $state<'desktop' | 'phone'>('desktop');
	// svelte-ignore state_referenced_locally
	let opened = $state(active);
	let proofPhotos = $state<Photo[]>([]);
	let photosLoadFailed = $state(false);
	let previewProofs = $state<Proof[]>([]);
	let previewFavorites = $state<string[]>([]);
	let studioSettings = $state<WorkspaceSettings | null>(null);
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
	const isDefaultFocal = $derived(
		localFocal.x === DEFAULT_FOCAL_POINT.x && localFocal.y === DEFAULT_FOCAL_POINT.y
	);

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

	const previewViews = $derived([
		{ id: 'desktop' as const, label: m.preview_desktop(), icon: Monitor },
		{ id: 'phone' as const, label: m.preview_phone(), icon: Smartphone }
	]);

	// Load preview data when the Design tab is shown or when gallery media changes.
	$effect(() => {
		if (!active && !pickingCover) return;

		const id = gallery.id;
		const watermarked = gallery.watermark_enabled;
		const finals = delivered;
		void refreshTick;
		let alive = true;

		Promise.all([
			listPhotos(id),
			finals ? listPhotos(id, fetch, 'finals') : Promise.resolve([] as Photo[]),
			finals ? Promise.resolve([]) : listClientPreviewPhotos(id, watermarked),
			fetch('/api/v1/settings')
				.then(async (response) => (response.ok ? parseSettings(await response.json()) : null))
				.catch(() => null)
		])
			.then(([allProofs, finalPhotos, visibleProofs, studio]) => {
				if (!alive) return;

				photosLoadFailed = false;
				proofPhotos = allProofs;
				if (studio) studioSettings = studio;

				if (finals) {
					previewProofs = toOwnerProofs(id, finalPhotos, 'finals').map((proof) => ({
						...proof,
						downloadUrl: proof.previewUrl
					}));
					previewFavorites = [];

					return;
				}

				previewProofs = toClientPreviewProofs(id, visibleProofs, watermarked);
				previewFavorites = allProofs
					.filter(({ favorited_at }) => favorited_at)
					.map(({ id: photoId }) => photoId);
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

	async function setLayout(value: string) {
		const next = value as GalleryLayout;

		if (next === currentLayout) return;

		const previous = currentLayout;

		currentLayout = next;

		if (!(await onsave({ layout: next }))) {
			currentLayout = previous;
		}
	}

	async function setCoverLayout(next: CoverLayout) {
		if (next === currentCoverLayout) return;

		const previous = currentCoverLayout;

		currentCoverLayout = next;

		if (!(await onsave({ cover_layout: next }))) {
			currentCoverLayout = previous;
		}
	}

	async function setHeadingFont(next: HeadingFont) {
		if (next === currentHeadingFont) return;

		const previous = currentHeadingFont;

		currentHeadingFont = next;

		if (!(await onsave({ heading_font: next }))) {
			currentHeadingFont = previous;
		}
	}

	async function setGridDensity(next: GridDensity) {
		if (next === currentGridDensity) return;

		const previous = currentGridDensity;

		currentGridDensity = next;

		if (!(await onsave({ grid_density: next }))) {
			currentGridDensity = previous;
		}
	}

	async function setGridSpacing(next: GridSpacing) {
		if (next === currentGridSpacing) return;

		const previous = currentGridSpacing;

		currentGridSpacing = next;

		if (!(await onsave({ grid_spacing: next }))) {
			currentGridSpacing = previous;
		}
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
		const saved = await onsave({ focal_point: next });

		if (localFocal.x !== next.x || localFocal.y !== next.y) return;

		focalDirty = false;

		if (saved) {
			lastSavedFocal = next;
		} else {
			setFocalState(gallery.focal_point ?? { ...DEFAULT_FOCAL_POINT });
		}
	}

	function queueFocalSave(next: FocalPoint) {
		focalDirty = true;
		setFocalState(next);

		if (schedule) {
			schedule('focal_point', () => void persistFocalPoint(next));
		} else {
			void persistFocalPoint(next);
		}
	}

	function resetFocalPoint() {
		cancel?.('focal_point');
		const reset = { ...DEFAULT_FOCAL_POINT };

		focalDirty = true;
		setFocalState(reset);
		void persistFocalPoint(reset);
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

		queueFocalSave({ x, y });
	}

	function onFocalPointerDown(event: PointerEvent & { currentTarget: HTMLButtonElement }) {
		if (!hasCover) return;

		draggingFocal = true;
		event.currentTarget.setPointerCapture?.(event.pointerId);
		updateFocalFromPointer(event);
	}

	function onFocalPointerMove(event: PointerEvent & { currentTarget: HTMLButtonElement }) {
		if (!draggingFocal || !hasCover) return;

		updateFocalFromPointer(event);
	}

	function onFocalPointerUp(event: PointerEvent & { currentTarget: HTMLButtonElement }) {
		if (!draggingFocal) return;

		draggingFocal = false;
		event.currentTarget.releasePointerCapture?.(event.pointerId);
	}

	function onFocalKeydown(event: KeyboardEvent) {
		if (!hasCover) return;

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

		queueFocalSave({ x, y });
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

<div class="grid gap-4">
	<h2 class="text-2xl leading-tight">{m.tab_design()}</h2>

	<div
		class="grid items-start gap-5 lg:grid-cols-[22rem_minmax(0,1fr)] xl:grid-cols-[24rem_minmax(0,1fr)]"
	>
		<div class="grid gap-4">
			<Card
				as="section"
				class="grid gap-4 rounded-[0.875rem] border border-border bg-panel p-4 sm:p-5"
			>
				<h3
					class="font-sans! text-[0.6875rem] leading-none font-semibold! tracking-[0.08em] text-muted-foreground uppercase"
				>
					{m.design_cover()}
				</h3>

				<div class="flex flex-wrap items-center gap-2">
					<Button variant="outline" size="sm" onclick={() => (pickingCover = true)}>
						<ImagePlus size={15} aria-hidden="true" class="mr-1.5" />
						{hasCover ? m.design_change_cover() : m.design_select_cover()}
					</Button>
					{#if hasCover}
						<Button variant="ghost" size="sm" onclick={() => void chooseCover(null)}>
							{m.cover_remove()}
						</Button>
					{/if}
				</div>

				<div class="grid gap-2">
					<span class="text-sm text-foreground">{m.design_cover_style()}</span>
					<div role="group" aria-label={m.design_cover_style()} class="grid grid-cols-2 gap-2">
						{#each coverTemplates as item (item.id)}
							{@const selected = currentCoverLayout === item.id}
							<button
								type="button"
								aria-pressed={selected}
								onclick={() => void setCoverLayout(item.id)}
								class="flex cursor-pointer flex-col gap-1.5 rounded-[0.625rem] border p-2 text-left text-xs font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus {selected
									? 'border-foreground bg-foreground text-panel'
									: 'border-border bg-background text-foreground hover:border-border-strong'}"
							>
								{@render templateDiagram(item.id, selected)}
								<span class="truncate">{item.label}</span>
							</button>
						{/each}
					</div>
				</div>

				<div class="grid gap-2">
					<div class="flex items-center justify-between gap-2 text-sm">
						<span class="text-foreground">{m.design_focal_point()}</span>
						<div class="flex items-center gap-2">
							<span class="text-xs text-muted-foreground tabular-nums">
								{localFocal.x}%, {localFocal.y}%
							</span>
							<button
								type="button"
								disabled={!hasCover || isDefaultFocal}
								onclick={resetFocalPoint}
								class="inline-flex cursor-pointer items-center gap-1 rounded-full border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus enabled:hover:bg-border/45 disabled:cursor-not-allowed disabled:opacity-45"
							>
								<RotateCcw size={12} aria-hidden="true" />
								{m.design_focal_point_reset()}
							</button>
						</div>
					</div>

					<button
						type="button"
						disabled={!hasCover}
						aria-label={m.design_focal_point_pad({ x: localFocal.x, y: localFocal.y })}
						onpointerdown={onFocalPointerDown}
						onpointermove={onFocalPointerMove}
						onpointerup={onFocalPointerUp}
						onpointercancel={onFocalPointerUp}
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
							<span aria-hidden="true" class="pointer-events-none absolute inset-0 bg-dark/15"
							></span>
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
				</div>
			</Card>

			<Card
				as="section"
				class="grid gap-3 rounded-[0.875rem] border border-border bg-panel p-4 sm:p-5"
			>
				<h3
					class="font-sans! text-[0.6875rem] leading-none font-semibold! tracking-[0.08em] text-muted-foreground uppercase"
				>
					{m.design_typography()}
				</h3>
				<div role="group" aria-label={m.design_heading_font()} class="grid grid-cols-2 gap-2">
					{#each headingFonts as item (item.id)}
						{@const selected = currentHeadingFont === item.id}
						{@const fontFamily = HEADING_FONT_FAMILIES[item.id]}
						<button
							type="button"
							aria-pressed={selected}
							onclick={() => void setHeadingFont(item.id)}
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

			<Card
				as="section"
				class="grid gap-3 rounded-[0.875rem] border border-border bg-panel p-4 sm:p-5"
			>
				<h3
					class="font-sans! text-[0.6875rem] leading-none font-semibold! tracking-[0.08em] text-muted-foreground uppercase"
				>
					{m.design_grid_and_spacing()}
				</h3>
				<div class="flex flex-wrap items-center justify-between gap-2.5 text-sm">
					<span class="text-foreground">{m.settings_layout()}</span>
					<IconButtonGroup
						value={currentLayout}
						items={layouts}
						label={m.settings_layout()}
						onchange={(value) => void setLayout(value)}
					/>
				</div>
				<div class="flex flex-wrap items-center justify-between gap-2.5 text-sm">
					<span class="text-foreground">{m.design_grid_density()}</span>
					<div
						role="group"
						aria-label={m.design_grid_density()}
						class="inline-flex rounded-full border border-border bg-background p-[3px]"
					>
						{#each gridDensities as item (item.id)}
							<button
								type="button"
								aria-pressed={currentGridDensity === item.id}
								onclick={() => void setGridDensity(item.id)}
								class="cursor-pointer rounded-full px-3 py-1 text-xs font-medium text-muted-foreground transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus aria-pressed:bg-foreground aria-pressed:text-panel"
							>
								{item.label}
							</button>
						{/each}
					</div>
				</div>
				<div class="flex flex-wrap items-center justify-between gap-2.5 text-sm">
					<span class="text-foreground">{m.design_grid_spacing()}</span>
					<div
						role="group"
						aria-label={m.design_grid_spacing()}
						class="inline-flex rounded-full border border-border bg-background p-[3px]"
					>
						{#each gridSpacings as item (item.id)}
							<button
								type="button"
								aria-pressed={currentGridSpacing === item.id}
								onclick={() => void setGridSpacing(item.id)}
								class="cursor-pointer rounded-full px-3 py-1 text-xs font-medium text-muted-foreground transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus aria-pressed:bg-foreground aria-pressed:text-panel"
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
			class="flex min-w-0 flex-col overflow-hidden rounded-[0.875rem] border border-border bg-[#1d1815] shadow-[0_18px_40px_-24px_rgb(43_36_32/0.45)]"
		>
			<div
				class="flex flex-wrap items-center justify-between gap-2.5 border-b border-[#3b322b] bg-foreground px-4 py-2.5 text-sm font-medium text-[#efe6da]"
			>
				<span class="flex items-center gap-2 font-semibold">
					<Eye size={16} aria-hidden="true" />
					<span>{m.preview_label()}</span>
				</span>
				<div
					role="group"
					aria-label={m.preview_size()}
					class="flex rounded-full bg-[#3a302a] p-[3px]"
				>
					{#each previewViews as { id, label, icon: Icon } (id)}
						<button
							type="button"
							aria-pressed={previewMode === id}
							onclick={() => (previewMode = id)}
							class="flex cursor-pointer items-center gap-1.5 rounded-full px-3 py-1 text-[0.8125rem] font-semibold focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-viewer-focus {previewMode ===
							id
								? 'bg-panel text-foreground'
								: 'text-[#cdbfae]'}"
						>
							<Icon size={15} aria-hidden="true" />
							<span>{label}</span>
						</button>
					{/each}
				</div>
			</div>

			<div
				class={cn(
					'h-[min(740px,calc(100dvh-11rem))] min-h-[520px]',
					previewMode === 'phone'
						? 'grid place-items-center overflow-hidden bg-[radial-gradient(circle_at_50%_40%,#3a302a,#1d1815_70%)] p-4 sm:p-6'
						: 'overflow-auto bg-background'
				)}
			>
				<div
					data-frame={previewMode}
					class={cn(
						previewMode === 'phone' &&
							'h-full max-h-[660px] w-[340px] max-w-full rounded-[40px] bg-black p-2.5 shadow-[0_30px_80px_rgb(0_0_0/0.5)]'
					)}
				>
					<div
						class={cn(
							'relative bg-background text-foreground',
							previewMode === 'phone' &&
								'h-full translate-z-0 [scrollbar-width:none] overflow-auto rounded-[30px]'
						)}
					>
						{#if opened}
							<ClientGallery
								gallery={{
									...gallery,
									layout: currentLayout,
									cover_layout: currentCoverLayout,
									focal_point: localFocal,
									heading_font: currentHeadingFont,
									grid_density: currentGridDensity,
									grid_spacing: currentGridSpacing,
									hero_id: localHeroId,
									delivered
								}}
								coverUrl={coverImageUrl}
								proofs={previewProofs}
								favorites={previewFavorites}
								studio={studioSettings?.studioName}
								studioLogo={studioSettings?.logo}
								signature={studioSettings?.signature}
								signatureSize={studioSettings?.signatureSize}
								preview
								embedded
								showCoverWhenEmpty
								framed={previewMode === 'phone'}
							/>
						{/if}
					</div>
				</div>
			</div>
		</section>
	</div>
</div>

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
