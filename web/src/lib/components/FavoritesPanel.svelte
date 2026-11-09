<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { CircleCheck, Download, Heart, LoaderCircle } from '@lucide/svelte';
	import PhotoTile, { GRID_CLASSES, ITEM_CLASSES } from '#lib/components/PhotoTile.svelte';
	import ProofViewer from '#lib/components/ProofViewer.svelte';
	import ViewerButton from '#lib/components/ViewerButton.svelte';
	import { GalleryApiError, type GalleryLayout } from '#lib/gallery/api.js';
	import {
		latestExport,
		requestExport,
		exportUrl,
		startDownload,
		type Export
	} from '#lib/gallery/export.js';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import EmptyState from '#lib/components/ui/EmptyState/EmptyState.svelte';
	import SelectActions from '#lib/components/ui/SelectActions/SelectActions.svelte';
	import { getLiveEvents } from '#lib/events.js';
	import { apiErrorMessage } from '#lib/i18n/errors.js';
	import { listPhotos, previewUrl, thumbnailUrl, type Photo } from '#lib/gallery/photos.js';
	import { SelectMode } from '#lib/gallery/select-mode.svelte.js';
	import type { Proof } from '#lib/gallery/types.js';

	let {
		galleryId,
		layout = 'masonry',
		oncount,
		ondownload = async (photos: Photo[]) => {
			const current = generation;
			const url = `/api/v1/galleries/${encodeURIComponent(galleryId)}/photos/${encodeURIComponent(photos[0].id)}/download`;
			const response = await fetch(url, { method: 'HEAD', credentials: 'same-origin' });
			if (current !== generation) return;
			if (!response.ok) throw new GalleryApiError(response.status, m.favorites_download_failed());
			startDownload(url);
		}
	}: {
		galleryId: string;
		/** How the tiles are laid out: the gallery's setting. */
		layout?: GalleryLayout;
		/** How many favorites the client has picked, for the tab. */
		oncount?: (count: number | null) => void;
		/**
		 * Downloads an individual original from a tile or viewer. Selected subsets use ZIP export.
		 */
		ondownload?: (photos: Photo[]) => void | Promise<void>;
	} = $props();

	const events = getLiveEvents();

	let favorites = $state<Photo[]>([]);
	let loaded = $state(false);
	let failed = $state(false);
	let loads = 0;
	let viewIndex = $state(0);
	let viewing = $state(false);
	let downloading = $state<string | null>(null);
	let currentExport = $state<Export | null>(null);
	let zipRequesting = $state(false);
	let zipError = $state<string | null>(null);
	let zipLoads = 0;
	let generation = 0;
	// The ZIP this page asked for downloads by itself once ready; one found later is only offered.
	let awaited: string | null = null;
	const mode = new SelectMode();

	async function load(id: string) {
		const current = ++loads;

		try {
			const photos = await listPhotos(id);

			if (current !== loads) return;

			favorites = photos.filter(({ favorite }) => favorite);
			failed = false;
		} catch {
			if (current === loads) failed = true;
		} finally {
			if (current === loads) loaded = true;
		}
	}

	async function loadZip(id: string) {
		const current = ++zipLoads;

		try {
			const latest = await latestExport(id);

			if (current !== zipLoads) return;

			currentExport = latest;

			if (latest?.status.kind === 'ready' && latest.id === awaited) {
				awaited = null;
				startDownload(exportUrl(id, latest.id));
			} else if (latest?.status.kind !== 'preparing') {
				awaited = null;
			}
		} catch {
			if (current === zipLoads) zipError = m.favorites_zip_check_failed();
		}
	}

	async function requestZip(photoIds?: string[]) {
		if (zipRequesting || zipPreparing) return;

		zipRequesting = true;
		zipError = null;
		const current = generation;
		const id = galleryId;

		try {
			const created = await requestExport(id, fetch, photoIds);
			if (current !== generation) return;

			currentExport = created;
			awaited = created.id;
			// The build may already have finished: take the state from the server, not the response.
			await loadZip(id);
		} catch (error) {
			if (current === generation) {
				zipError =
					error instanceof GalleryApiError && error.status !== 502
						? error.message
						: m.favorites_zip_failed();
			}
		} finally {
			if (current === generation) zipRequesting = false;
		}
	}

	const exportFailure = $derived(
		currentExport?.status.kind === 'failed' ? apiErrorMessage(409, currentExport.status.code) : null
	);
	const zipPreparing = $derived(currentExport?.status.kind === 'preparing');

	$effect(() => {
		const id = galleryId;

		generation++;
		favorites = [];
		loaded = false;
		downloading = null;
		viewing = false;
		currentExport = null;
		zipRequesting = false;
		zipError = null;
		awaited = null;
		void load(id);
		void loadZip(id);

		const unsubscribe = [
			events.onFavorites((event) => event.gallery_id === id && void load(id)),
			events.onPhoto(
				(event) =>
					event.gallery_id === id &&
					(event.status === 'deleted' || event.status === 'ready') &&
					void load(id)
			),
			events.onExport((event) => event.gallery_id === id && void loadZip(id)),
			events.onResync(() => {
				void load(id);
				void loadZip(id);
			})
		];

		return () => {
			generation++;
			loads++;
			zipLoads++;

			for (const off of unsubscribe) off();
		};
	});

	const proofs = $derived<Proof[]>(
		favorites.map(({ id, original_name, width, height, revision }) => ({
			id,
			alt: original_name,
			thumbnailUrl: thumbnailUrl(galleryId, id, 'photos', revision),
			previewUrl: previewUrl(galleryId, id, 'photos', revision),
			width,
			height
		}))
	);

	$effect(() => {
		oncount?.(loaded && !failed ? favorites.length : null);
	});

	// Photos that stop being favorites leave a selection with them.
	$effect(() => {
		for (const id of [...mode.ids]) {
			if (!favorites.some((photo) => photo.id === id)) mode.ids.delete(id);
		}
	});

	async function download(photos: Photo[]) {
		if (downloading) return;

		const current = generation;
		downloading = photos[0].id;

		try {
			await ondownload(photos);
		} catch {
			if (current === generation) zipError = m.favorites_download_failed();
		} finally {
			if (current === generation) downloading = null;
		}
	}

	function onkeydown(event: KeyboardEvent) {
		if (mode.active && !viewing && event.key === 'Escape' && !event.defaultPrevented) mode.leave();
	}
</script>

<svelte:window {onkeydown} />

{#snippet downloadButton()}
	<Button
		size="sm"
		class="min-h-9! gap-1.5"
		disabled={zipPreparing || zipRequesting || (mode.active && mode.count === 0)}
		aria-busy={zipPreparing || zipRequesting}
		onclick={() =>
			void requestZip(
				mode.count > 0 ? favorites.filter(({ id }) => mode.has(id)).map(({ id }) => id) : undefined
			)}
	>
		<Download size={16} aria-hidden="true" />
		<span class={[mode.active && 'max-sm:sr-only']}>
			{zipPreparing || zipRequesting
				? m.favorites_downloading()
				: mode.count > 0
					? m.favorites_download_count({ count: mode.count })
					: m.favorites_download_all()}
		</span>
	</Button>
{/snippet}

{#if !mode.active}
	<div class="flex flex-wrap items-center justify-end gap-2">
		<Button
			variant="outline"
			size="sm"
			class="min-h-9! gap-1.5"
			disabled={favorites.length === 0}
			onclick={() => mode.enter()}
			><CircleCheck size={16} aria-hidden="true" />{m.action_select()}</Button
		>
		{#if favorites.length > 0}{@render downloadButton()}{/if}
	</div>
{:else}
	<SelectActions
		count={mode.count}
		total={favorites.length}
		onselectall={() => mode.selectAll(favorites.map(({ id }) => id))}
		onunselectall={() => mode.unselectAll()}
		onexit={() => mode.leave()}
		actions={downloadButton}
	/>
{/if}

{#if zipError || exportFailure}
	<p role="alert" class="text-right text-sm text-danger">
		{zipError ?? exportFailure}
		{m.favorites_try_again()}
	</p>
{/if}

{#if failed}
	<p role="alert" class="text-sm text-danger">{m.favorites_load_failed()}</p>
{:else if loaded && favorites.length === 0}
	<EmptyState>
		<span class="grid size-14 place-items-center rounded-full bg-background"
			><Heart size={26} aria-hidden="true" /></span
		>
		<h3>{m.favorites_empty()}</h3>
		<p class="text-sm">{m.favorites_empty_body()}</p>
	</EmptyState>
{:else}
	<ul aria-label={m.tab_favorites()} class={GRID_CLASSES[layout]}>
		{#each favorites as photo, index (photo.id)}
			<li class={ITEM_CLASSES[layout]}>
				<PhotoTile
					name={photo.original_name}
					state="ready"
					ratio={layout === 'masonry' ? photo.width / photo.height : 3 / 2}
					thumbnailUrl={thumbnailUrl(galleryId, photo.id, 'photos', photo.revision)}
					selected={mode.has(photo.id)}
					onselect={mode.active ? () => mode.toggle(photo.id) : undefined}
					onopen={() => {
						viewIndex = index;
						viewing = true;
					}}
					ondownload={() => void download([photo])}
					downloadBusy={downloading === photo.id}
				/>
			</li>
		{/each}
	</ul>
{/if}

{#if viewing && proofs[viewIndex]}
	<ProofViewer
		{proofs}
		bind:index={viewIndex}
		readonly
		isSelected={() => true}
		onclose={() => (viewing = false)}
	>
		{#snippet actions(proof)}
			<ViewerButton
				aria-label={m.tile_download({ name: proof.alt })}
				disabled={downloading !== null}
				aria-busy={downloading === proof.id}
				onclick={() => {
					const photo = favorites.find(({ id }) => id === proof.id);
					if (photo) void download([photo]);
				}}
			>
				{#if downloading === proof.id}
					<LoaderCircle
						size={18}
						class="animate-spin motion-reduce:animate-none"
						aria-hidden="true"
					/>
				{:else}
					<Download size={18} aria-hidden="true" />
				{/if}
			</ViewerButton>
		{/snippet}
	</ProofViewer>
{/if}
