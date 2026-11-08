<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { CircleCheck, ImagePlus, LoaderCircle, Star, Trash2, Upload } from '@lucide/svelte';
	import { goto } from '$app/navigation';
	import { onDestroy, tick, untrack } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { scale } from 'svelte/transition';
	import type { GalleryLayout } from '#lib/gallery/api.js';
	import PhotoTile, {
		GRID_CLASSES,
		ITEM_CLASSES,
		type TileState
	} from '#lib/components/PhotoTile.svelte';
	import ProofViewer from '#lib/components/ProofViewer.svelte';
	import ViewerButton from '#lib/components/ViewerButton.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import ConfirmDialog from '#lib/components/ui/ConfirmDialog/ConfirmDialog.svelte';
	import EmptyState from '#lib/components/ui/EmptyState/EmptyState.svelte';
	import SelectActions from '#lib/components/ui/SelectActions/SelectActions.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import PhotoStack from '#lib/components/ui/PhotoStack/PhotoStack.svelte';
	import { getLiveEvents, type PhotoEvent } from '#lib/events.js';
	import { GalleryApiError, updateGallery } from '#lib/gallery/api.js';
	import {
		BULK_DELETE_LIMIT,
		deletePhoto,
		deletePhotos,
		listPhotos,
		previewUrl,
		hasCleanPhoto,
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
		heroRevision = $bindable(),
		layout = 'masonry',
		oncount,
		set = 'photos'
	}: {
		galleryId: string;
		heroId?: string | null;
		heroRevision?: number;
		/** How the tiles are laid out: the gallery's setting. */
		layout?: GalleryLayout;
		/** How many photos the gallery has, for the tab. */
		oncount?: (count: number | null) => void;
		/** Whether the gallery has watermarking on. */
		/** The client proofs, or the photographer's finished photos, which have no hero or favorites. */
		set?: PhotoSet;
	} = $props();

	const finals = $derived(set === 'finals');

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
	let deletingSelection = $state<Photo[] | null>(null);
	/** A one-off message, and the summary it went out with: it is cleared when that changes. */
	let announcement = $state<{ text: string; summary: string } | null>(null);
	let selectButton = $state<HTMLButtonElement | null>(null);
	let uploadButton = $state<HTMLElement | null>(null);
	let grid = $state<HTMLUListElement | null>(null);
	const selected = new SvelteSet<string>();

	// Bookkeeping that nothing renders. `shown` is bumped whenever the panel stops showing a
	// gallery, by a change of gallery or unmounting, so a late list answer can tell it is stale.
	let shown = 0;
	let destroyed = false;
	let fetching = false;
	let outdated = false;

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

	const queued = $derived(queue.forGallery(galleryId, set));
	// Photos deleted meanwhile drop out, so the confirmed count is what is sent.
	const selection = $derived(photos.filter(({ id }) => selected.has(id)));
	// Every photo, so one still processing or failed shows its state in the viewer.
	const proofs = $derived<Proof[]>(
		photos.map((photo) => ({
			...photo,
			alt: photo.original_name,
			thumbnailUrl: thumbnailUrl(galleryId, photo.id, set, photo.revision),
			previewUrl: previewUrl(galleryId, photo.id, set, photo.revision),
			status:
				!finals && hasCleanPhoto(photo)
					? 'ready'
					: (PROOF_STATES[tileState(photo.status)] ?? 'processing')
		}))
	);
	const summary = $derived.by(() => {
		const states = [
			...photos.map(({ status }) => tileState(status)),
			...queued.map(({ state }) => QUEUE_STATES[state])
		];
		const count = (...kinds: TileState[]) => states.filter((state) => kinds.includes(state)).length;
		const parts = [
			[selecting ? selection.length : 0, m.notification_selected],
			[count('waiting-upload', 'uploading'), m.photos_status_uploading],
			[count('waiting-process', 'processing'), m.photos_status_processing],
			[count('upload-failed', 'rejected', 'process-failed'), m.photos_status_failed]
		] as const;

		return parts
			.filter(([n]) => n > 0)
			.map(([n, label]) => label({ count: n }))
			.join(', ');
	});

	const updatingWatermarks = $derived(
		!finals &&
			photos.some(
				(photo) =>
					photo.clean_ready && (photo.status === 'waiting' || photo.status === 'processing')
			)
	);
	const failedUploads = $derived(
		queued.filter(({ state }) => state === 'failed' || state === 'rejected').length
	);
	let reportedFailures = 0;

	$effect(() => {
		oncount?.(loaded ? photos.length : null);
	});

	$effect(() => {
		const failures = failedUploads;

		if (failures > reportedFailures) {
			const fresh = failures - reportedFailures;

			toasts.error(m.photos_failed_upload({ count: fresh }));
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
			toasts.saved();
		} catch (cause) {
			if (cause instanceof GalleryApiError && cause.status === 401) {
				await signedOut();

				return;
			}

			if (galleryId === id) alert = m.photos_cover_failed();
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
	$effect(() => {
		const revision = photos.find(({ id }) => id === heroId)?.revision;
		if (heroRevision !== revision) heroRevision = revision;
	});

	async function signedOut() {
		await goto('/login?expired=1', { refreshAll: true });
	}

	async function load() {
		const id = galleryId;
		const view = shown;

		fetching = true;
		outdated = false;

		try {
			const listed = await listPhotos(id, fetch, set);

			if (view === shown) photos = listed;
		} catch (cause) {
			if (view !== shown) return;

			if (cause instanceof GalleryApiError && cause.status === 401) {
				await signedOut();

				return;
			}

			alert = m.photos_load_failed();
		} finally {
			if (view === shown) {
				loaded = true;
				fetching = false;

				if (outdated) void load();
			}
		}
	}

	/** Marks the list outdated: one fetch runs at a time, and one more follows if this came during it. */
	function refresh() {
		outdated = true;

		if (!fetching) void load();
	}

	function onPhoto(event: PhotoEvent) {
		if (event.gallery_id === galleryId && (event.kind === 'final') === finals) refresh();
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
			fetching = false;
			outdated = false;
			alert = '';
			announcement = null;
			leaveSelectMode();
			void load();
		});

		const unsubscribe = [
			events.onPhoto(onPhoto),
			events.onResync(refresh),
			events.onFavorites((event) => !finals && event.gallery_id === id && refresh()),
			queue.onUploaded((gallery, _photo, photoSet) => {
				if (gallery === id && photoSet === set) refresh();
			})
		];

		return () => {
			shown++;

			for (const off of unsubscribe) off();
		};
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

		photos.splice(index, 1);
		alert = '';
		void focusAfterRemoving(index);

		try {
			await deletePhoto(galleryId, photo.id, fetch, set);
		} catch (cause) {
			if (cause instanceof GalleryApiError && cause.status === 401) {
				await signedOut();

				return;
			}

			if (!(cause instanceof GalleryApiError && cause.status === 404)) {
				alert = m.photos_delete_failed({ name: photo.original_name });
			}
		}

		// The list says what is left, whether the photo is gone or was put back.
		refresh();
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
			alert = m.photos_retry_failed({ name: photo.original_name });
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
	 * moves on; on a failure, stops. The list is refetched either way.
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

		deletingSelection = removed;
		photos = photos.filter(({ id }) => !ids.includes(id));
		alert = '';
		announcement = null;

		try {
			while (deletedCount < ids.length) {
				const chunk = ids.slice(deletedCount, deletedCount + BULK_DELETE_LIMIT);

				await deletePhotos(confirmedGallery, chunk, fetch, set);

				for (const id of chunk) {
					if (stillShown()) selected.delete(id);
				}

				deletedCount += chunk.length;
			}

			if (showsGallery()) refresh();

			if (!stillShown()) return;

			await finishSelecting();

			announcement = {
				text: m.photos_deleted({ count: ids.length }),
				summary
			};
		} catch (cause) {
			if (cause instanceof GalleryApiError && cause.status === 401) {
				await signedOut();

				return;
			}

			if (!showsGallery()) return;

			alert = m.photos_partly_deleted({ deleted: deletedCount, total: ids.length });
			refresh();
		}
	}

	function open(photo: Photo) {
		viewIndex = photos.findIndex(({ id }) => id === photo.id);
		viewing = true;
	}
</script>

<svelte:window {onkeydown} />

<section
	aria-label={finals ? m.tab_final() : m.common_photos()}
	class={['grid gap-3 rounded-sm', dragging && 'outline-2 outline-offset-4 outline-foreground/40']}
	{ondragover}
	ondragleave={() => (dragging = false)}
	{ondrop}
>
	<div class="flex flex-wrap items-center justify-end gap-2">
		{#if updatingWatermarks}
			<p class="mr-auto flex items-center gap-2 text-sm text-muted-foreground">
				<LoaderCircle size={16} class="animate-spin-slow" aria-hidden="true" />
				{m.photos_updating_watermarks()}
			</p>
		{/if}
		{#if selecting}
			<SelectActions
				count={selection.length}
				onunselectall={() => selected.clear()}
				onleave={() => void finishSelecting()}
			/>
		{/if}
		<Button
			bind:ref={selectButton}
			variant="outline"
			size="sm"
			class="min-h-9! gap-1.5"
			disabled={!selecting && photos.length === 0}
			onclick={() => (selecting ? selectAll() : (selecting = true))}
			><CircleCheck size={16} aria-hidden="true" />{selecting
				? m.action_select_all()
				: m.action_select()}</Button
		>
		<Button
			bind:ref={uploadButton}
			variant={selecting ? 'destructive' : 'default'}
			size="sm"
			class="min-h-9! gap-1.5"
			disabled={selecting && selection.length === 0}
			onclick={() => (selecting ? (confirmingSelection = true) : picker?.click())}
			>{#if selecting}<Trash2 size={16} aria-hidden="true" />{m.action_delete()}{:else}<Upload
					size={16}
					aria-hidden="true"
				/>{m.action_upload()}{/if}</Button
		>
		<input
			bind:this={picker}
			type="file"
			multiple
			accept="image/jpeg,.jpg,.jpeg"
			class="hidden"
			aria-label={finals ? m.photos_choose_finals() : m.photos_choose()}
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
			aria-label={finals ? m.photos_list_finals() : m.common_photos()}
			class={GRID_CLASSES[layout]}
		>
			{#each photos as photo (photo.id)}
				{@const state = tileState(photo.status)}
				<li
					class={ITEM_CLASSES[layout]}
					out:scale={{
						start: 0.94,
						duration:
							typeof HTMLElement === 'undefined' ||
							typeof HTMLElement.prototype.animate !== 'function' ||
							(typeof matchMedia === 'function' &&
								matchMedia('(prefers-reduced-motion: reduce)').matches)
								? 0
								: 160
					}}
				>
					<PhotoTile
						name={photo.original_name}
						{state}
						cleanReady={!finals && hasCleanPhoto(photo)}
						ratio={layout === 'masonry' ? photo.width / photo.height : 3 / 2}
						thumbnailUrl={(finals ? state === 'ready' : hasCleanPhoto(photo))
							? thumbnailUrl(galleryId, photo.id, set, photo.revision)
							: undefined}
						selected={selected.has(photo.id)}
						downloaded={finals && !!photo.downloaded_at}
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
			<h3>{finals ? m.photos_drop_finals() : m.photos_drop()}</h3>
			<Button size="sm" onclick={() => picker?.click()}>{m.action_upload()}</Button>
		</EmptyState>
	{/if}
</section>

{#if confirming}
	{@const photo = confirming}
	<ConfirmDialog
		title={m.photos_delete_one({ name: photo.original_name })}
		confirmLabel={m.action_delete()}
		cancelLabel={m.common_cancel()}
		destructive
		onclose={() => (confirming = null)}
		onconfirm={() => remove(photo)}
	>
		{#snippet visual()}
			<PhotoStack {galleryId} {set} photoIds={[photo.id]} />
		{/snippet}
		{#snippet description()}
			<p class="text-sm text-muted-foreground">
				{finals ? m.photos_delete_one_body_finals() : m.photos_delete_one_body()}
			</p>
			{#if !finals && photo.favorite}
				<p class="text-sm font-bold text-warning">
					{m.photos_delete_one_favorite()}
				</p>
			{/if}
		{/snippet}
	</ConfirmDialog>
{/if}

{#if confirmingSelection}
	{@const confirmed = deletingSelection ?? selection}
	{@const count = confirmed.length}
	{@const favoriteCount = confirmed.filter(({ favorite }) => favorite).length}
	<ConfirmDialog
		title={m.photos_delete_many({ count })}
		confirmLabel={m.action_delete()}
		cancelLabel={m.common_cancel()}
		destructive
		onclose={() => {
			confirmingSelection = false;
			deletingSelection = null;
		}}
		onconfirm={removeSelection}
	>
		{#snippet visual()}
			<PhotoStack {galleryId} {set} photoIds={confirmed.map(({ id }) => id)} />
		{/snippet}
		{#snippet description()}
			<p class="text-sm text-muted-foreground">{m.photos_delete_undone()}</p>
			{#if !finals && favoriteCount > 0}
				<p class="text-sm font-bold text-warning">
					{favoriteCount === count
						? m.photos_delete_all_favorites()
						: m.photos_delete_some_favorites({ count: favoriteCount })}
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
			{#if !finals && photo && hasCleanPhoto(photo)}
				<ViewerButton
					pressed={photo.id === heroId}
					aria-label={photo.id === heroId ? m.cover_remove() : m.cover_set()}
					onclick={() => void toggleHero(photo)}
				>
					<Star
						size={18}
						fill={photo.id === heroId ? 'currentColor' : 'none'}
						aria-hidden="true"
						class="text-[#f2c063]"
					/>
					<span class="max-[700px]:hidden"
						>{photo.id === heroId ? m.cover_remove() : m.cover_set()}</span
					>
				</ViewerButton>
			{/if}
			{#if photo}
				<ViewerButton
					tone="warn"
					aria-label={m.action_delete()}
					onclick={() => (confirming = photo)}
				>
					<Trash2 size={18} aria-hidden="true" />
					<span class="max-[700px]:hidden">{m.action_delete()}</span>
				</ViewerButton>
			{/if}
		{/snippet}
	</ProofViewer>
{/if}
