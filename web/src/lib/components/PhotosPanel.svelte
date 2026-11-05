<script lang="ts">
	import { goto } from '$app/navigation';
	import { tick, untrack } from 'svelte';
	import PhotoTile, { type TileState } from '#lib/components/PhotoTile.svelte';
	import ProofViewer from '#lib/components/ProofViewer.svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import Dialog from '#lib/components/ui/dialog/dialog.svelte';
	import EmptyState from '#lib/components/ui/empty-state/empty-state.svelte';
	import { getLiveEvents, type PhotoChange, type PhotoEvent } from '#lib/events.js';
	import { GalleryApiError } from '#lib/gallery/api.js';
	import {
		deletePhoto,
		listPhotos,
		previewUrl,
		retryPhoto,
		thumbnailUrl,
		type Photo,
		type PhotoStatus
	} from '#lib/gallery/photos.js';
	import type { Proof } from '#lib/gallery/types.js';
	import { getUploadQueue, type QueueState } from '#lib/gallery/uploads.svelte.js';

	let { galleryId }: { galleryId: string } = $props();

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
	let uploadButton = $state<HTMLElement | null>(null);
	let grid = $state<HTMLUListElement | null>(null);

	// Bookkeeping that nothing renders, so plain collections on purpose.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const deleting = new Set<string>();
	// Changes since the last list request started, which an answer arriving later may predate.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const recentChanges = new Map<string, PhotoChange>();
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const uploadedDuringLoad = new Set<string>();
	let loads = 0;
	let lastLoadAt = -Infinity;
	let refreshTimer: ReturnType<typeof setTimeout> | undefined;
	let wasInFlight = 0;

	const SERVER_STATES: Record<Exclude<PhotoStatus, { failed: string }>, TileState> = {
		waiting: 'waiting-process',
		processing: 'processing',
		ready: 'ready'
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
	function withRecentChange(photo: Photo): Photo | null {
		const change = recentChanges.get(photo.id);

		if (deleting.has(photo.id) || change === 'deleted') return null;

		return change === undefined ? photo : { ...photo, status: statusOf(change) };
	}

	const queued = $derived(queue.forGallery(galleryId));
	const ready = $derived(photos.filter(({ status }) => status === 'ready'));
	const proofs = $derived<Proof[]>(
		ready.map(({ id, original_name, width, height }) => ({
			id,
			alt: original_name,
			thumbnailUrl: thumbnailUrl(galleryId, id),
			previewUrl: previewUrl(galleryId, id),
			width,
			height
		}))
	);
	const summary = $derived.by(() => {
		const states = [
			...photos.map(({ status }) => tileState(status)),
			...queued.map(({ state }) => QUEUE_STATES[state])
		];
		const count = (...kinds: TileState[]) => states.filter((state) => kinds.includes(state)).length;
		const parts = [
			[count('waiting-upload', 'uploading'), 'uploading'],
			[count('waiting-process', 'processing'), 'processing'],
			[count('upload-failed', 'rejected', 'process-failed'), 'failed']
		] as const;

		return parts
			.filter(([n]) => n > 0)
			.map(([n, label]) => `${n} ${label}`)
			.join(', ');
	});

	async function signedOut() {
		await goto('/login?expired=1', { refreshAll: true });
	}

	async function load() {
		const id = galleryId;
		const current = ++loads;

		lastLoadAt = Date.now();
		recentChanges.clear();
		uploadedDuringLoad.clear();

		try {
			const listed = await listPhotos(id);

			if (current !== loads) return;

			const missing = photos.filter(
				(photo) => uploadedDuringLoad.has(photo.id) && !listed.some(({ id }) => id === photo.id)
			);

			photos = [...listed, ...missing].map(withRecentChange).filter((photo) => photo !== null);
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
		if (event.gallery_id !== galleryId) return;

		recentChanges.set(event.photo_id, event.status);

		const photo = photos.find(({ id }) => id === event.photo_id);

		if (event.status === 'deleted') {
			photos = photos.filter(({ id }) => id !== event.photo_id);
		} else if (photo !== undefined) {
			photo.status = statusOf(event.status);
		} else if (queue.inFlight(galleryId) === 0 && !deleting.has(event.photo_id)) {
			// While uploads are in flight, an unknown photo is usually one of them; its answer brings it in.
			refresh();
		}
	}

	$effect(() => {
		const id = galleryId;

		untrack(() => {
			photos = [];
			loaded = false;
			alert = '';
			void load();
		});

		const unsubscribe = [
			events.onPhoto(onPhoto),
			events.onResync(refresh),
			queue.onUploaded((gallery, photo) => {
				const current = withRecentChange(photo);

				if (gallery !== id || current === null) return;

				uploadedDuringLoad.add(photo.id);

				if (!photos.some(({ id }) => id === photo.id)) photos.push(current);
			})
		];

		return () => {
			clearTimeout(refreshTimer);
			refreshTimer = undefined;

			for (const off of unsubscribe) off();
		};
	});

	// When the gallery's uploads finish, one refetch catches a photo whose answer was lost.
	$effect(() => {
		const inFlight = queue.inFlight(galleryId);

		if (wasInFlight > 0 && inFlight === 0) void untrack(load);

		wasInFlight = inFlight;
	});

	function addFiles(files: FileList | null | undefined) {
		if (files && files.length > 0) queue.add(galleryId, [...files]);
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

		deleting.add(photo.id);
		photos.splice(index, 1);
		alert = '';
		void focusAfterRemoving(index);

		try {
			await deletePhoto(galleryId, photo.id);
			recentChanges.set(photo.id, 'deleted');
		} catch (cause) {
			if (cause instanceof GalleryApiError && cause.status === 401) {
				await signedOut();
			} else if (cause instanceof GalleryApiError && cause.status === 404) {
				recentChanges.set(photo.id, 'deleted');
			} else {
				restore(photo, index);
			}
		} finally {
			deleting.delete(photo.id);
		}
	}

	/** Puts back a photo whose deletion failed, as the events since say, unless they deleted it. */
	function restore(photo: Photo, index: number) {
		deleting.delete(photo.id);

		const current = withRecentChange(photo);

		if (current === null || photos.some(({ id }) => id === photo.id)) return;

		photos.splice(Math.min(index, photos.length), 0, current);
		alert = `Couldn’t delete ${photo.original_name}. Please try again.`;
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
			await retryPhoto(galleryId, photo.id);
		} catch (cause) {
			if (cause instanceof GalleryApiError && cause.status === 401) {
				await signedOut();

				return;
			}

			photo.status = before;
			alert = `Couldn’t retry ${photo.original_name}. Please try again.`;
		}
	}

	function open(photo: Photo) {
		viewIndex = ready.indexOf(photo);
		viewing = true;
	}
</script>

<section
	aria-label="Photos"
	class={['grid gap-3 rounded-sm', dragging && 'outline-2 outline-offset-4 outline-foreground/40']}
	{ondragover}
	ondragleave={() => (dragging = false)}
	{ondrop}
>
	<div class="flex justify-end">
		<Button bind:ref={uploadButton} variant="outline" size="sm" onclick={() => picker?.click()}
			>+ Upload</Button
		>
		<input
			bind:this={picker}
			type="file"
			multiple
			accept="image/jpeg,.jpg,.jpeg"
			class="hidden"
			aria-label="Choose photos to upload"
			onchange={onpick}
		/>
	</div>

	{#if alert}
		<p role="alert" class="text-sm text-danger">{alert}</p>
	{/if}
	<p role="status" aria-live="polite" class="sr-only">{summary}</p>

	{#if photos.length > 0 || queued.length > 0}
		<ul
			bind:this={grid}
			aria-label="Photos"
			class="grid grid-cols-[repeat(auto-fill,minmax(8rem,1fr))] gap-2"
		>
			{#each photos as photo (photo.id)}
				{@const state = tileState(photo.status)}
				<li>
					<PhotoTile
						name={photo.original_name}
						{state}
						thumbnailUrl={state === 'ready' ? thumbnailUrl(galleryId, photo.id) : undefined}
						onremove={() => (confirming = photo)}
						onretry={() => retry(photo)}
						onopen={() => open(photo)}
					/>
				</li>
			{/each}
			{#each queued as item, position (item.key)}
				<li>
					<PhotoTile
						name={item.file.name}
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
		<EmptyState class="text-muted-foreground">No photos yet. Drop JPEGs here to start.</EmptyState>
	{/if}
</section>

{#if confirming}
	<Dialog
		onclose={() => (confirming = null)}
		labelledby="delete-photo-title"
		class="m-auto w-[min(24rem,calc(100vw-2rem))] rounded-dialog border border-border bg-panel p-6 text-foreground shadow-xl"
	>
		{#snippet children(close)}
			<div class="grid gap-4">
				<div class="grid gap-1">
					<h2 id="delete-photo-title" class="text-xl">Delete {confirming?.original_name}?</h2>
					<p class="text-sm text-muted-foreground">
						The photo and its previews are removed for good, wherever it is in processing.
					</p>
				</div>
				<div class="flex justify-end gap-2">
					<Button variant="ghost" onclick={close}>Cancel</Button>
					<Button
						onclick={() => {
							const photo = confirming;

							close();
							if (photo) void remove(photo);
						}}>Delete</Button
					>
				</div>
			</div>
		{/snippet}
	</Dialog>
{/if}

{#if viewing && proofs[viewIndex]}
	<ProofViewer {proofs} bind:index={viewIndex} onclose={() => (viewing = false)} />
{/if}
