<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onMount } from 'svelte';
	import { SvelteMap, SvelteSet } from 'svelte/reactivity';
	import { afterNavigate, goto } from '$app/navigation';
	import { authRequest } from '#lib/auth.js';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import AppHeader from '#lib/components/AppHeader.svelte';
	import PhotoTile from '#lib/components/PhotoTile.svelte';
	import { GRID_CLASSES } from '#lib/components/PhotoTile.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import ConfirmDialog from '#lib/components/ui/ConfirmDialog/ConfirmDialog.svelte';
	import EmptyState from '#lib/components/ui/EmptyState/EmptyState.svelte';
	import SelectActions from '#lib/components/ui/SelectActions/SelectActions.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import { Download, Images, Search, Trash2 } from '@lucide/svelte';
	import {
		BULK_DELETE_LIMIT,
		deletePhotos,
		downloadOriginal,
		thumbnailUrl,
		type PhotoSet,
		type WorkspacePhoto
	} from '#lib/gallery/photos.js';
	import { ApiError } from '#lib/http.js';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	// svelte-ignore state_referenced_locally
	let photos = $state<WorkspacePhoto[]>(data.photos ? [...data.photos] : []);
	let query = $state('');
	let galleryFilter = $state('all');
	let typeFilter = $state<'all' | 'proof' | 'final'>('all');
	let sortOrder = $state<'newest' | 'oldest'>('newest');
	const selectedIds = new SvelteSet<string>();
	let selectionMode = $state(false);
	let deleteConfirmOpen = $state(false);
	let downloadBusy = $state(false);
	let signingOut = $state(false);
	let signOutError = $state('');

	const loadFailed = $derived(data.photos === null);

	/** The gallery label to show, and to search and build tile names from. */
	function galleryLabel(entry: Pick<WorkspacePhoto, 'gallery_title'>) {
		return entry.gallery_title ?? m.gallery_untitled();
	}

	const galleryOptions = $derived.by(() => {
		const byId = new SvelteMap<string, string>();

		for (const entry of photos) byId.set(entry.gallery_id, galleryLabel(entry));

		return [...byId.entries()].sort((a, b) => a[1].localeCompare(b[1]));
	});

	const matching = $derived.by(() => {
		const needle = query.trim().toLowerCase();

		return photos.filter((entry) => {
			if (galleryFilter !== 'all' && entry.gallery_id !== galleryFilter) return false;
			if (typeFilter !== 'all' && entry.kind !== typeFilter) return false;
			if (needle === '') return true;

			return (
				galleryLabel(entry).toLowerCase().includes(needle) ||
				entry.original_name.toLowerCase().includes(needle) ||
				entry.id.toLowerCase().includes(needle)
			);
		});
	});

	const visible = $derived(
		[...matching].sort((a, b) =>
			sortOrder === 'newest'
				? b.created_at.localeCompare(a.created_at)
				: a.created_at.localeCompare(b.created_at)
		)
	);

	const selection = $derived(visible.filter((entry) => selectedIds.has(entry.id)));

	// A photo hidden by a filter change is no longer offered among the selection's actions.
	$effect(() => {
		const visibleIds = new Set(visible.map((entry) => entry.id));

		for (const id of [...selectedIds]) {
			if (!visibleIds.has(id)) selectedIds.delete(id);
		}
	});

	function toggleSelect(id: string) {
		selectionMode = true;

		if (!selectedIds.delete(id)) selectedIds.add(id);
	}

	function clearSelection() {
		selectedIds.clear();
		selectionMode = false;
	}

	function clearSelectedOnly() {
		selectedIds.clear();
	}

	function handleWindowKeydown(event: KeyboardEvent) {
		if (
			event.key === 'Escape' &&
			!event.defaultPrevented &&
			selectionMode &&
			!deleteConfirmOpen &&
			!document.querySelector('[data-slot="popover"] > button[aria-expanded="true"]')
		) {
			event.preventDefault();
			clearSelection();
		}
	}

	function clearFilters() {
		query = '';
		galleryFilter = 'all';
		typeFilter = 'all';
	}

	function photoSet(kind: WorkspacePhoto['kind']): PhotoSet {
		return kind === 'final' ? 'finals' : 'photos';
	}

	async function downloadSelected() {
		if (downloadBusy) return;

		const targets = [...selection];

		if (targets.length === 0) return;

		downloadBusy = true;
		let failed = 0;

		try {
			for (const entry of targets) {
				try {
					await downloadOriginal(entry.gallery_id, entry.id, fetch, photoSet(entry.kind));
				} catch (cause) {
					if (cause instanceof ApiError && cause.status === 401) {
						await goto('/login', { refreshAll: true });

						return;
					}

					failed += 1;
				}
			}
		} finally {
			downloadBusy = false;
		}

		if (failed > 0) toasts.error(m.photos_download_some_failed());
	}

	/**
	 * Deletes the selection grouped by the gallery and kind it belongs to, in bounded chunks; on
	 * the first failing chunk it stops, leaving that chunk and every not-yet-processed group
	 * selected so the photographer can retry just what is left.
	 */
	async function confirmBulkDelete() {
		const targets = [...selection];
		const groups = new SvelteMap<string, { galleryId: string; set: PhotoSet; ids: string[] }>();

		for (const entry of targets) {
			const set = photoSet(entry.kind);
			const key = `${entry.gallery_id}:${set}`;
			const group = groups.get(key) ?? { galleryId: entry.gallery_id, set, ids: [] };

			group.ids.push(entry.id);
			groups.set(key, group);
		}

		let deleted = 0;

		for (const group of groups.values()) {
			for (let at = 0; at < group.ids.length; at += BULK_DELETE_LIMIT) {
				const chunk = group.ids.slice(at, at + BULK_DELETE_LIMIT);

				try {
					await deletePhotos(group.galleryId, chunk, fetch, group.set);
				} catch (cause) {
					if (cause instanceof ApiError && cause.status === 401) {
						await goto('/login', { refreshAll: true });

						return;
					}

					throw new Error(
						deleted > 0
							? m.photos_partly_deleted({ deleted, total: targets.length })
							: cause instanceof Error
								? cause.message
								: m.api_error_generic(),
						{ cause }
					);
				}

				for (const id of chunk) {
					selectedIds.delete(id);
					deleted += 1;
				}
				photos = photos.filter((entry) => !chunk.includes(entry.id));
			}
		}

		deleteConfirmOpen = false;
		if (selectedIds.size === 0) selectionMode = false;
	}

	let activity = $derived(createSessionActivity(data.idleTimeoutMs));
	afterNavigate(() => {
		void activity();
	});
	onMount(() => listenForActivity(() => activity()));

	async function logout() {
		if (signingOut) return;

		signingOut = true;
		signOutError = '';

		try {
			await authRequest('logout');
			await goto('/login', { refreshAll: true });
		} catch (error) {
			signOutError = error instanceof Error ? error.message : m.sign_out_failed();
		} finally {
			signingOut = false;
		}
	}
</script>

<svelte:head><title>{m.photos_head()} — PhotoProofing</title></svelte:head>
<svelte:window onkeydown={handleWindowKeydown} />
<AppHeader onsignout={logout} studioName={data.settings?.studioName} />
<main class="flex flex-1 flex-col px-3.5 pt-4 pb-24 sm:px-8 sm:pt-6">
	{#if signOutError}<p role="alert" class="mb-3 text-sm text-danger">{signOutError}</p>{/if}

	<h1 class="mb-4 text-[1.875rem] leading-[1.08] sm:mb-6 sm:text-[2.5rem]">{m.photos_head()}</h1>

	{#if !loadFailed}
		{@const none = photos.length === 0}
		<div
			class="sticky top-2 z-[6] mb-4 flex flex-wrap items-center gap-2 rounded-[0.875rem] border border-border bg-panel p-2 sm:mb-[1.375rem] sm:gap-3.5 sm:py-2.5 sm:pr-3 sm:pl-4 {none
				? 'opacity-55'
				: ''}"
		>
			<label
				class="flex min-w-40 flex-1 items-center gap-2 rounded-md has-focus-visible:outline-2 has-focus-visible:outline-offset-4 has-focus-visible:outline-focus/40 max-sm:basis-full"
			>
				<Search size={16} aria-hidden="true" class="shrink-0" />
				<input
					type="search"
					aria-label={m.photos_search()}
					placeholder={m.photos_search()}
					bind:value={query}
					disabled={none}
					class="w-full border-0 bg-transparent text-[0.9375rem] outline-none placeholder:text-muted-foreground focus-visible:shadow-none focus-visible:outline-none [&::-webkit-search-cancel-button]:hidden"
				/>
			</label>
			<label class="flex items-center gap-1.5 text-sm">
				<span class="sr-only">{m.photos_filter_gallery()}</span>
				<select
					aria-label={m.photos_filter_gallery()}
					bind:value={galleryFilter}
					disabled={none}
					class="rounded-md border border-border bg-panel px-2 py-1.5 text-sm"
				>
					<option value="all">{m.photos_filter_gallery_all()}</option>
					{#each galleryOptions as [id, label] (id)}
						<option value={id}>{label}</option>
					{/each}
				</select>
			</label>
			<label class="flex items-center gap-1.5 text-sm">
				<span class="sr-only">{m.photos_filter_type()}</span>
				<select
					aria-label={m.photos_filter_type()}
					bind:value={typeFilter}
					disabled={none}
					class="rounded-md border border-border bg-panel px-2 py-1.5 text-sm"
				>
					<option value="all">{m.photos_filter_type_all()}</option>
					<option value="proof">{m.photos_kind_proof()}</option>
					<option value="final">{m.photos_kind_final()}</option>
				</select>
			</label>
			<label class="flex items-center gap-1.5 text-sm">
				<span class="sr-only">{m.photos_sort()}</span>
				<select
					aria-label={m.photos_sort()}
					bind:value={sortOrder}
					disabled={none}
					class="rounded-md border border-border bg-panel px-2 py-1.5 text-sm"
				>
					<option value="newest">{m.photos_sort_newest()}</option>
					<option value="oldest">{m.photos_sort_oldest()}</option>
				</select>
			</label>
		</div>
	{/if}

	{#if loadFailed}
		<p role="alert" class="px-3.5 sm:px-0">{m.photos_load_failed()}</p>
	{:else if photos.length === 0}
		<EmptyState variant="plain" class="rounded-card py-14">
			<Images size={28} aria-hidden="true" class="text-muted-foreground" />
			<h2 class="text-[1.625rem] text-foreground sm:text-[1.75rem]">{m.photos_empty()}</h2>
			<p class="max-w-sm text-sm">{m.photos_empty_body()}</p>
		</EmptyState>
	{:else if visible.length === 0}
		<EmptyState variant="plain" class="rounded-card py-14">
			<h2 class="text-[1.625rem] text-foreground sm:text-[1.75rem]">{m.photos_no_match()}</h2>
			<Button variant="outline" onclick={clearFilters}>{m.photos_clear_filters()}</Button>
		</EmptyState>
	{:else}
		<ul aria-label={m.photos_head()} class={GRID_CLASSES.grid}>
			{#each visible as entry (entry.id)}
				{@const name = `${galleryLabel(entry)} — ${entry.original_name} — ${
					entry.kind === 'final' ? m.photos_kind_final() : m.photos_kind_proof()
				}`}
				<li>
					<PhotoTile
						{name}
						state="ready"
						cleanReady
						persistentSelect
						ratio={entry.width / entry.height}
						thumbnailUrl={thumbnailUrl(
							entry.gallery_id,
							entry.id,
							photoSet(entry.kind),
							entry.revision
						)}
						selected={selectedIds.has(entry.id)}
						onopen={() =>
							void goto(
								`/dashboard/${encodeURIComponent(entry.gallery_id)}?photo=${encodeURIComponent(entry.id)}&kind=${entry.kind}`
							)}
						onselect={() => toggleSelect(entry.id)}
						source={{ title: galleryLabel(entry), kind: entry.kind }}
					/>
					<p class="truncate px-1 pt-1 text-xs text-muted-foreground" title={entry.original_name}>
						{entry.original_name}
					</p>
				</li>
			{/each}
		</ul>
	{/if}
</main>

{#if selectionMode}
	<SelectActions
		count={selection.length}
		total={visible.length}
		rail="dashboard"
		onselectall={() => {
			for (const entry of visible) selectedIds.add(entry.id);
		}}
		onunselectall={clearSelectedOnly}
		onexit={clearSelection}
	>
		{#snippet actions()}
			<Button
				variant="outline"
				size="sm"
				class="min-h-9! gap-1.5"
				disabled={selection.length === 0 || downloadBusy}
				onclick={() => void downloadSelected()}
				><Download size={16} aria-hidden="true" /><span class="max-sm:sr-only"
					>{m.action_download()}</span
				></Button
			>
			<Button
				variant="destructive"
				size="sm"
				class="min-h-9! gap-1.5"
				disabled={selection.length === 0}
				onclick={() => (deleteConfirmOpen = true)}
				><Trash2 size={16} aria-hidden="true" /><span class="max-sm:sr-only"
					>{m.action_delete()}</span
				></Button
			>
		{/snippet}
	</SelectActions>
{/if}

{#if deleteConfirmOpen}
	<ConfirmDialog
		title={m.photos_delete_many({ count: selection.length })}
		confirmLabel={m.action_delete()}
		destructive
		onconfirm={confirmBulkDelete}
		onclose={() => (deleteConfirmOpen = false)}
	>
		{#snippet description()}
			<p class="text-sm text-muted-foreground">{m.photos_delete_undone()}</p>
		{/snippet}
	</ConfirmDialog>
{/if}
