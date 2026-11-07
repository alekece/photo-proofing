<script lang="ts">
	import { CircleCheck, ImagePlus, LoaderCircle, Star, Trash2, Upload } from '@lucide/svelte';
	import { goto } from '$app/navigation';
	import { onDestroy, tick, untrack } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import type { GalleryLayout } from '#lib/gallery/api.js';
	import PhotoTile, {
		GRID_CLASSES,
		ITEM_CLASSES,
		type TileState
	} from '#lib/components/PhotoTile.svelte';
	import ProofViewer from '#lib/components/ProofViewer.svelte';
	import ViewerButton from '#lib/components/ViewerButton.svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import ConfirmDialog from '#lib/components/ui/confirm-dialog/confirm-dialog.svelte';
	import EmptyState from '#lib/components/ui/empty-state/empty-state.svelte';
	import SelectPill from '#lib/components/ui/select-pill/select-pill.svelte';
	import { toasts } from '#lib/components/ui/toast/toasts.svelte.js';
	import PhotoStack from '#lib/components/ui/photo-stack/photo-stack.svelte';
	import { getLiveEvents, type PhotoChange, type PhotoEvent } from '#lib/events.js';
	import { GalleryApiError, updateGallery } from '#lib/gallery/api.js';
	import {
		BULK_DELETE_LIMIT,
		deletePhoto,
		deletePhotos,
		listPhotos,
		previewUrl,
		retryPhoto,
		thumbnailUrl,
		type Photo,
		type PhotoSet,
		type PhotoStatus
	} from '#lib/gallery/photos.js';
	import type { Proof } from '#lib/gallery/types.js';
	import { getUploadQueue, type QueueState } from '#lib/gallery/uploads.svelte.js';

	let {
		galleryId,
		heroId = $bindable(null),
		layout = 'masonry',
		oncount,
		watermarked = false,
		set = 'photos'
	}: {
		galleryId: string;
		heroId?: string | null;
		/** How the tiles are laid out: the gallery's setting. */
		layout?: GalleryLayout;
		/** How many photos the gallery has, for the tab. */
		oncount?: (count: number | null) => void;
		/** Whether the gallery has watermarking on. */
		watermarked?: boolean;
		/** The client proofs, or the photographer's finished photos, which have no hero or favorites. */
		set?: PhotoSet;
	} = $props();

	const finals = $derived(set === 'finals');

	/** Event storms and resyncs refetch the list at most this often. */
	const REFRESH_INTERVAL_MS = 1000;

	const queue = getUploadQueue();
	const events = getLiveEvents();

	let photos = $state<Photo[]>([]);
	let loaded = $state(false);
	let alert = $state('');
	let confirming = $state<Photo | null>(null);
	let viewing = $state(false);
	let viewIndex = $state(0);
	let dragging = $state(false);
	let picker = $state<HTMLInputElement | null>(null);
	let selecting = $state(false);
	let confirmingSelection = $state(false);
	/** A one-off message, and the summary it went out with: it is cleared when that changes. */
	let announcement = $state<{ text: string; summary: string } | null>(null);
	let selectButton = $state<HTMLButtonElement | null>(null);
	let uploadButton = $state<HTMLElement | null>(null);
	let grid = $state<HTMLUListElement | null>(null);
	const selected = new SvelteSet<string>();

	// Bookkeeping that nothing renders, so plain collections on purpose.
	// Photos whose deletion is pending, as last known, or `null` once known to be gone.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const deleting = new Map<string, Photo | null>();
	// Changes since the last list request started, which an answer arriving later may predate.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const recentChanges = new Map<string, PhotoChange>();
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const uploadedDuringLoad = new Set<string>();
	let loads = 0;
	// Bumped whenever the panel stops showing a gallery, by a change of gallery or unmounting.
	let shown = 0;
	let destroyed = false;
	let lastLoadAt = -Infinity;
	let refreshTimer: ReturnType<typeof setTimeout> | undefined;
	let wasInFlight = 0;

	const SERVER_STATES: Record<Exclude<PhotoStatus, { failed: string }>, TileState> = {
		waiting: 'waiting-process',
		processing: 'processing',
		ready: 'ready'
	};
	const PROOF_STATES: Partial<Record<TileState, Proof['status']>> = {
		ready: 'ready',
		'process-failed': 'failed'
	};
	const QUEUE_STATES: Record<QueueState, TileState> = {
		waiting: 'waiting-upload',
		uploading: 'uploading',
		failed: 'upload-failed',
		rejected: 'rejected'
	};

	const tileState = (status: PhotoStatus) =>
		typeof status === 'object' ? 'process-failed' : SERVER_STATES[status];

	const statusOf = (change: Exclude<PhotoChange, 'deleted'>): PhotoStatus =>
		change === 'failed' ? { failed: 'Processing failed' } : change;

	/** The photo as the latest change seen says, or `null` once deleted. */
	function changed(photo: Photo): Photo | null {
		const change = recentChanges.get(photo.id);

		if (change === 'deleted') return null;

		return change === undefined ? photo : { ...photo, status: statusOf(change) };
	}

	/** The photo to show, or `null` once deleted or while its deletion is pending. */
	const withRecentChange = (photo: Photo) => (deleting.has(photo.id) ? null : changed(photo));

	const queued = $derived(queue.forGallery(galleryId, set));
	// Photos deleted meanwhile drop out, so the confirmed count is what is sent.
	const selection = $derived(photos.filter(({ id }) => selected.has(id)));
	const ready = $derived(photos.filter(({ status }) => status === 'ready'));
	// Every photo, so one still processing or failed shows its state in the viewer.
	const proofs = $derived<Proof[]>(
		photos.map(({ id, original_name, width, height, status }) => ({
			id,
			alt: original_name,
			thumbnailUrl: thumbnailUrl(galleryId, id, set),
			previewUrl: previewUrl(galleryId, id, set),
			width,
			height,
			status: PROOF_STATES[tileState(status)] ?? 'processing'
		}))
	);
	const summary = $derived.by(() => {
		const states = [
			...photos.map(({ status }) => tileState(status)),
			...queued.map(({ state }) => QUEUE_STATES[state])
		];
		const count = (...kinds: TileState[]) => states.filter((state) => kinds.includes(state)).length;
		const parts = [
			[selecting ? selection.length : 0, 'selected'],
			[count('waiting-upload', 'uploading'), 'uploading'],
			[count('waiting-process', 'processing'), 'processing'],
			[count('upload-failed', 'rejected', 'process-failed'), 'failed']
		] as const;

		return parts
			.filter(([n]) => n > 0)
			.map(([n, label]) => `${n} ${label}`)
			.join(', ');
	});

	// Photos that were ready: one that is not any more is being regenerated, not freshly uploaded.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const everReady = new Set<string>();
	const updatingWatermarks = $derived(
		watermarked && photos.some(({ id, status }) => status !== 'ready' && everReady.has(id))
	);
	const failedUploads = $derived(
		queued.filter(({ state }) => state === 'failed' || state === 'rejected').length
	);
	let reportedFailures = 0;

	$effect(() => {
		for (const { id } of ready) everReady.add(id);
	});

	$effect(() => {
		oncount?.(loaded ? photos.length : null);
	});

	$effect(() => {
		const failures = failedUploads;

		if (failures > reportedFailures) {
			const fresh = failures - reportedFailures;

			toasts.error(`${fresh} ${fresh === 1 ? 'photo' : 'photos'} failed to upload.`);
		}

		reportedFailures = failures;
	});

	// Deleting from the viewer moves it on to the neighbouring photo, or closes it after the last.
	$effect.pre(() => {
		if (!viewing || viewIndex < proofs.length) return;

		if (proofs.length === 0) viewing = false;
		else viewIndex = proofs.length - 1;
	});

	let savingHero = false;

	/** Stars `photo`, or clears the star when it already is the hero; the old hero stays on failure. */
	async function toggleHero(photo: Photo) {
		if (savingHero) return;

		const target = photo.id === heroId ? null : photo.id;
		const id = galleryId;

		savingHero = true;
		alert = '';

		try {
			const gallery = await updateGallery(id, { hero_id: target });

			if (galleryId === id) heroId = gallery.hero_id;
		} catch (cause) {
			if (cause instanceof GalleryApiError && cause.status === 401) {
				await signedOut();

				return;
			}

			if (galleryId === id) alert = 'Couldn’t update the hero photo. Please try again.';
		} finally {
			savingHero = false;
		}
	}

	// A deleted photo cannot stay the hero; the server clears it the same way.
	$effect(() => {
		if (loaded && heroId !== null && !photos.some(({ id }) => id === heroId)) {
			heroId = null;
		}
	});

	async function signedOut() {
		await goto('/login?expired=1', { refreshAll: true });
	}

	async function load() {
		const id = galleryId;
		const current = ++loads;

		lastLoadAt = Date.now();

		// Changes are cleared below, so a pending deletion keeps what they said about its photo.
		for (const [id, photo] of deleting) deleting.set(id, photo && changed(photo));

		recentChanges.clear();
		uploadedDuringLoad.clear();

		try {
			const listed = await listPhotos(id, fetch, set);

			if (current !== loads) return;

			// A photo uploaded after the list was requested may be missing from it, not gone.
			for (const [id, known] of deleting) {
				const fresh = listed.find((photo) => photo.id === id);

				deleting.set(id, fresh ?? (uploadedDuringLoad.has(id) ? known : null));
			}

			const missing = photos.filter(
				(photo) => uploadedDuringLoad.has(photo.id) && !listed.some(({ id }) => id === photo.id)
			);

			photos = withRecentChanges([...listed, ...missing]);
		} catch (cause) {
			if (current !== loads) return;

			if (cause instanceof GalleryApiError && cause.status === 401) {
				await signedOut();

				return;
			}

			alert = 'Couldn’t load the photos. Please reload the page.';
		} finally {
			if (current === loads) loaded = true;
		}
	}

	/** Refetches now, or once the interval since the last fetch has passed. */
	function refresh() {
		if (refreshTimer !== undefined) return;

		const wait = lastLoadAt + REFRESH_INTERVAL_MS - Date.now();

		if (wait <= 0) {
			void load();

			return;
		}

		refreshTimer = setTimeout(() => {
			refreshTimer = undefined;
			void load();
		}, wait);
	}

	function onPhoto(event: PhotoEvent) {
		if (event.gallery_id !== galleryId || (event.kind === 'final') !== finals) return;

		recentChanges.set(event.photo_id, event.status);

		const photo = photos.find(({ id }) => id === event.photo_id);

		if (event.status === 'deleted') {
			photos = photos.filter(({ id }) => id !== event.photo_id);
		} else if (photo !== undefined) {
			photo.status = statusOf(event.status);
		} else if (queue.inFlight(galleryId, set) === 0 && !deleting.has(event.photo_id)) {
			// While uploads are in flight, an unknown photo is usually one of them; its answer brings it in.
			refresh();
		}
	}

	onDestroy(() => (destroyed = true));

	$effect(() => {
		if (announcement && announcement.summary !== summary) announcement = null;
	});

	$effect(() => {
		const id = galleryId;

		untrack(() => {
			photos = [];
			loaded = false;
			alert = '';
			announcement = null;
			leaveSelectMode();
			void load();
		});

		const unsubscribe = [
			events.onPhoto(onPhoto),
			events.onResync(refresh),
			events.onFavorites((event) => !finals && event.gallery_id === id && refresh()),
			queue.onUploaded((gallery, photo, photoSet) => {
				const current = withRecentChange(photo);

				if (gallery !== id || photoSet !== set || current === null) return;

				uploadedDuringLoad.add(photo.id);

				if (!photos.some(({ id }) => id === photo.id)) photos.push(current);
			})
		];

		return () => {
			shown++;
			clearTimeout(refreshTimer);
			refreshTimer = undefined;

			for (const off of unsubscribe) off();
		};
	});

	// When the gallery's uploads finish, one refetch catches a photo whose answer was lost.
	$effect(() => {
		const inFlight = queue.inFlight(galleryId, set);

		if (wasInFlight > 0 && inFlight === 0) void untrack(load);

		wasInFlight = inFlight;
	});

	function addFiles(files: FileList | null | undefined) {
		if (files && files.length > 0) queue.add(galleryId, [...files], set);
	}

	function ondrop(event: DragEvent) {
		event.preventDefault();
		dragging = false;
		addFiles(event.dataTransfer?.files);
	}

	function ondragover(event: DragEvent) {
		event.preventDefault();
		dragging = true;
	}

	function onpick(event: Event & { currentTarget: HTMLInputElement }) {
		addFiles(event.currentTarget.files);
		event.currentTarget.value = '';
	}

	async function remove(photo: Photo) {
		const index = photos.findIndex(({ id }) => id === photo.id);

		if (index === -1) return;

		deleting.set(photo.id, photos[index]);
		photos.splice(index, 1);
		alert = '';
		void focusAfterRemoving(index);

		try {
			await deletePhoto(galleryId, photo.id, fetch, set);
			recentChanges.set(photo.id, 'deleted');
		} catch (cause) {
			if (cause instanceof GalleryApiError && cause.status === 401) {
				await signedOut();
			} else if (cause instanceof GalleryApiError && cause.status === 404) {
				recentChanges.set(photo.id, 'deleted');
			} else {
				restore(photo.id, index);
			}
		} finally {
			deleting.delete(photo.id);
		}
	}

	/** Puts back a photo whose deletion failed, as last known, unless it is gone meanwhile. */
	function restore(photoId: string, index: number) {
		const known = deleting.get(photoId);
		const current = known && changed(known);

		deleting.delete(photoId);

		if (!current || photos.some(({ id }) => id === photoId)) return;

		photos.splice(Math.min(index, photos.length), 0, current);
		alert = `Couldn’t delete ${current.original_name}. Please try again.`;
	}

	/** Once the tile at `position` is gone, focuses the one now there, else the previous, else Upload. */
	async function focusAfterRemoving(position: number) {
		await tick();

		const tiles = grid?.querySelectorAll('li') ?? [];
		const neighbour = tiles[position] ?? tiles[position - 1];

		(neighbour?.querySelector('button') ?? uploadButton)?.focus();
	}

	async function retry(photo: Photo) {
		const before = photo.status;

		photo.status = 'waiting';
		alert = '';

		try {
			await retryPhoto(galleryId, photo.id, fetch, set);
		} catch (cause) {
			if (cause instanceof GalleryApiError && cause.status === 401) {
				await signedOut();

				return;
			}

			photo.status = before;
			alert = `Couldn’t retry ${photo.original_name}. Please try again.`;
		}
	}

	function leaveSelectMode() {
		selecting = false;
		selected.clear();
	}

	/** Leaves select mode from its own controls, which vanish, so focus lands back on Select. */
	async function finishSelecting() {
		leaveSelectMode();
		await tick();
		(selectButton?.disabled ? uploadButton : selectButton)?.focus();
	}

	function toggle(id: string) {
		if (!selected.delete(id)) selected.add(id);
	}

	/** Selects the server photos shown now; later arrivals stay unselected. */
	function selectAll() {
		for (const { id } of photos) selected.add(id);
	}

	/** Whether the key belongs to a field being typed in; tile checkboxes keep the shortcuts. */
	function typing(target: EventTarget | null) {
		if (!(target instanceof HTMLElement)) return false;

		if (target instanceof HTMLInputElement) return target.type !== 'checkbox';

		return (
			target instanceof HTMLTextAreaElement ||
			target instanceof HTMLSelectElement ||
			target.isContentEditable
		);
	}

	function onkeydown(event: KeyboardEvent) {
		// The confirmation dialog owns the keyboard; its Escape closes only the dialog.
		if (!selecting || confirmingSelection || event.defaultPrevented || typing(event.target)) {
			return;
		}

		if (event.key === 'Escape') {
			void finishSelecting();
		} else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'a') {
			event.preventDefault();
			selectAll();
		}
	}

	/**
	 * Deletes the selection chunk by chunk in the gallery it was confirmed in, even if the panel
	 * moves on; on a failure, stops and refetches what the server kept.
	 */
	async function removeSelection() {
		const confirmedGallery = galleryId;
		const view = shown;
		// Selection, focus and announcement belong to the view the deletion was confirmed in; the
		// photos and alert to the gallery, which may be shown again after switching away and back.
		const stillShown = () => view === shown;
		const showsGallery = () => !destroyed && galleryId === confirmedGallery;
		const removed = selection;
		const ids = removed.map(({ id }) => id);
		let deletedCount = 0;

		for (const photo of removed) deleting.set(photo.id, photo);

		photos = photos.filter(({ id }) => !deleting.has(id));
		alert = '';
		announcement = null;

		try {
			while (deletedCount < ids.length) {
				const chunk = ids.slice(deletedCount, deletedCount + BULK_DELETE_LIMIT);

				await deletePhotos(confirmedGallery, chunk, fetch, set);

				for (const id of chunk) {
					recentChanges.set(id, 'deleted');
					deleting.delete(id);

					if (stillShown()) selected.delete(id);
				}

				deletedCount += chunk.length;
			}

			if (!stillShown()) return;

			await finishSelecting();

			announcement = {
				text: `${ids.length} ${ids.length === 1 ? 'photo' : 'photos'} deleted`,
				summary
			};
		} catch (cause) {
			if (cause instanceof GalleryApiError && cause.status === 401) {
				await signedOut();

				return;
			}

			if (!showsGallery()) return;

			alert = `Deleted ${deletedCount} of ${ids.length} photos. Please try again.`;
			await load();

			if (showsGallery()) restoreSurvivors(ids.slice(deletedCount));
		} finally {
			for (const id of ids) deleting.delete(id);
		}
	}

	/** The photos as the latest changes seen say, without those deleted. */
	function withRecentChanges(records: Photo[]) {
		return records.map(withRecentChange).filter((photo) => photo !== null);
	}

	/** Puts back photos whose deletion failed, as last known, unless shown already or gone since. */
	function restoreSurvivors(photoIds: string[]) {
		const back = photoIds
			.map((photoId) => deleting.get(photoId))
			.map((known) => known && changed(known))
			.filter((photo): photo is Photo => !!photo && !photos.some(({ id }) => id === photo.id));

		for (const photo of back) {
			const index = photos.findIndex((other) => listedBefore(photo, other));

			photos.splice(index === -1 ? photos.length : index, 0, photo);
		}
	}

	/** Whether `a` comes before `b` in the server's order, by creation then id. */
	function listedBefore(a: Photo, b: Photo) {
		const [atA, atB] = [instant(a.created_at), instant(b.created_at)];

		return atA === atB ? a.id < b.id : atA < atB;
	}

	/** A UTC RFC 3339 timestamp, with its fraction padded so timestamps compare as strings. */
	function instant(timestamp: string) {
		return timestamp.replace(
			/(?:\.(\d+))?Z$/,
			(_, fraction = '') => `.${fraction.padEnd(9, '0')}Z`
		);
	}

	function open(photo: Photo) {
		viewIndex = photos.findIndex(({ id }) => id === photo.id);
		viewing = true;
	}
</script>

<svelte:window {onkeydown} />

<section
	aria-label={finals ? 'Final' : 'Photos'}
	class={['grid gap-3 rounded-sm', dragging && 'outline-2 outline-offset-4 outline-foreground/40']}
	{ondragover}
	ondragleave={() => (dragging = false)}
	{ondrop}
>
	<div class="flex flex-wrap items-center justify-end gap-2">
		{#if updatingWatermarks}
			<p class="mr-auto flex items-center gap-2 text-sm text-muted-foreground">
				<LoaderCircle size={16} class="animate-spin-slow" aria-hidden="true" />
				Updating watermarks…
			</p>
		{/if}
		{#if selecting}
			<SelectPill
				count={selection.length}
				actionLabel="Delete"
				destructive
				actionDisabled={selection.length === 0}
				onaction={() => (confirmingSelection = true)}
				onselectall={selectAll}
				onclose={finishSelecting}
			>
				{#snippet icon()}<Trash2 size={16} aria-hidden="true" />{/snippet}
			</SelectPill>
		{:else}
			<Button
				bind:ref={selectButton}
				variant="outline"
				size="sm"
				class="min-h-9! gap-1.5"
				disabled={photos.length === 0}
				onclick={() => (selecting = true)}
				><CircleCheck size={16} aria-hidden="true" />Select</Button
			>
		{/if}
		{#if !selecting}
			<Button
				bind:ref={uploadButton}
				size="sm"
				class="min-h-9! gap-1.5"
				onclick={() => picker?.click()}><Upload size={16} aria-hidden="true" />Upload</Button
			>
		{/if}
		<input
			bind:this={picker}
			type="file"
			multiple
			accept="image/jpeg,.jpg,.jpeg"
			class="hidden"
			aria-label={finals ? 'Choose finished photos to upload' : 'Choose photos to upload'}
			onchange={onpick}
		/>
	</div>

	{#if alert}
		<p role="alert" class="text-sm text-danger">{alert}</p>
	{/if}
	<p role="status" aria-live="polite" class="sr-only">
		{[announcement?.text, summary].filter(Boolean).join('. ')}
	</p>

	{#if photos.length > 0 || queued.length > 0}
		<ul
			bind:this={grid}
			aria-label={finals ? 'Final photos' : 'Photos'}
			class={GRID_CLASSES[layout]}
		>
			{#each photos as photo (photo.id)}
				{@const state = tileState(photo.status)}
				<li class={ITEM_CLASSES[layout]}>
					<PhotoTile
						name={photo.original_name}
						{state}
						ratio={layout === 'masonry' ? photo.width / photo.height : 3 / 2}
						thumbnailUrl={state === 'ready' ? thumbnailUrl(galleryId, photo.id, set) : undefined}
						selected={selected.has(photo.id)}
						onremove={() => (confirming = photo)}
						onretry={() => retry(photo)}
						onopen={() => open(photo)}
						hero={!finals && photo.id === heroId}
						onhero={finals ? undefined : () => toggleHero(photo)}
						onselect={selecting ? () => toggle(photo.id) : undefined}
					/>
				</li>
			{/each}
			{#each queued as item, position (item.key)}
				<li class={ITEM_CLASSES[layout]}>
					<PhotoTile
						name={item.file.name}
						ratio={3 / 2}
						state={QUEUE_STATES[item.state]}
						error={item.error}
						onremove={() => {
							queue.cancel(item.key);
							void focusAfterRemoving(photos.length + position);
						}}
						onretry={() => queue.retry(item.key)}
					/>
				</li>
			{/each}
		</ul>
	{:else if loaded}
		<EmptyState>
			<ImagePlus size={32} aria-hidden="true" />
			<h3>{finals ? 'Drop the finished photos here' : 'Drop the shoot here'}</h3>
			<Button size="sm" onclick={() => picker?.click()}>Upload</Button>
		</EmptyState>
	{/if}
</section>

{#if confirming}
	{@const photo = confirming}
	<ConfirmDialog
		title="Delete {photo.original_name}?"
		confirmLabel="Delete"
		cancelLabel="Cancel"
		destructive
		onclose={() => (confirming = null)}
		onconfirm={() => void remove(photo)}
	>
		{#snippet visual()}
			<PhotoStack {galleryId} {set} photoIds={[photo.id]} />
		{/snippet}
		{#snippet description()}
			<p class="text-sm text-muted-foreground">
				{finals
					? 'The finished photo and its previews are removed from the draft, wherever it is in processing.'
					: 'The photo and its previews are removed for good, wherever it is in processing.'}
			</p>
			{#if !finals && photo.favorite}
				<p class="text-sm font-bold text-warning">
					This photo is one of the client’s favorites. It leaves their selection too.
				</p>
			{/if}
		{/snippet}
	</ConfirmDialog>
{/if}

{#if confirmingSelection}
	{@const count = selection.length}
	{@const favoriteCount = selection.filter(({ favorite }) => favorite).length}
	<ConfirmDialog
		title="Delete {count} {count === 1 ? 'photo' : 'photos'}?"
		confirmLabel="Delete"
		cancelLabel="Cancel"
		destructive
		onclose={() => (confirmingSelection = false)}
		onconfirm={() => void removeSelection()}
	>
		{#snippet visual()}
			<PhotoStack {galleryId} {set} photoIds={selection.map(({ id }) => id)} />
		{/snippet}
		{#snippet description()}
			<p class="text-sm text-muted-foreground">This can’t be undone.</p>
			{#if !finals && favoriteCount > 0}
				<p class="text-sm font-bold text-warning">
					{favoriteCount === count ? 'All' : favoriteCount} of them {favoriteCount === 1
						? 'is'
						: 'are'} the client’s favorites. They leave the client’s selection too.
				</p>
			{/if}
		{/snippet}
	</ConfirmDialog>
{/if}

{#if viewing && proofs[viewIndex]}
	{@const current = (id: string) => photos.find((photo) => photo.id === id)}
	<ProofViewer
		{proofs}
		bind:index={viewIndex}
		onclose={() => (viewing = false)}
		onretry={(id) => {
			const photo = current(id);

			if (photo) void retry(photo);
		}}
	>
		{#snippet actions(proof)}
			{@const photo = current(proof.id)}
			{#if !finals && photo && photo.status === 'ready'}
				<ViewerButton
					pressed={photo.id === heroId}
					aria-label={photo.id === heroId ? 'Remove hero' : 'Set as hero'}
					onclick={() => void toggleHero(photo)}
				>
					<Star
						size={18}
						fill={photo.id === heroId ? 'currentColor' : 'none'}
						aria-hidden="true"
						class="text-[#f2c063]"
					/>
					<span class="max-[700px]:hidden"
						>{photo.id === heroId ? 'Remove hero' : 'Set as hero'}</span
					>
				</ViewerButton>
			{/if}
			{#if photo}
				<ViewerButton tone="warn" aria-label="Delete" onclick={() => (confirming = photo)}>
					<Trash2 size={18} aria-hidden="true" />
					<span class="max-[700px]:hidden">Delete</span>
				</ViewerButton>
			{/if}
		{/snippet}
	</ProofViewer>
{/if}
