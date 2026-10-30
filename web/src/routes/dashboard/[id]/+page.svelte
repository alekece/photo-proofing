<script lang="ts">
	import { onMount } from 'svelte';
	import { afterNavigate, beforeNavigate, goto } from '$app/navigation';
	import { page } from '$app/state';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import { authRequest } from '#lib/auth.js';
	import AppHeader from '#lib/components/AppHeader.svelte';
	import PressAgainButton from '#lib/components/PressAgainButton.svelte';
	import ShareLink from '#lib/components/ShareLink.svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import {
		GalleryApiError,
		shareGallery,
		updateGallery,
		type GalleryChanges,
		type GalleryLayout
	} from '#lib/gallery/api.js';
	import { endOfLocalDay, TITLE_MAX, toLocalDate, UNTITLED_GALLERY } from '#lib/gallery/format.js';
	import type { PageData } from './$types';

	const DEBOUNCE = 600;
	const TABS = [
		{ id: 'photos', label: 'Photos' },
		{ id: 'favorites', label: 'Favorites' },
		{ id: 'finals', label: 'Finals' }
	] as const;

	type TabId = (typeof TABS)[number]['id'];

	let { data }: { data: PageData } = $props();
	// svelte-ignore state_referenced_locally
	let saved = $state(data.gallery);
	// svelte-ignore state_referenced_locally
	let title = $state(data.gallery.title ?? '');
	// svelte-ignore state_referenced_locally
	let expiryDay = $state(data.gallery.expires_at ? toLocalDate(data.gallery.expires_at) : '');
	// svelte-ignore state_referenced_locally
	let included = $state<number | null>(data.gallery.included_photos);
	// svelte-ignore state_referenced_locally
	let layout = $state<GalleryLayout>(data.gallery.layout);
	// svelte-ignore state_referenced_locally
	let watermark = $state(data.gallery.watermark_enabled);
	// svelte-ignore state_referenced_locally
	let shareToken = $state(data.gallery.share_token);
	let sharing = $state(false);
	let shareFailed = $state(false);
	let signOutError = $state('');
	let tab = $state<TabId>('photos');
	let inflight = $state(0);
	let justSaved = $state(false);
	// Attempted values of fields whose last save failed; cleared per field on that field's success.
	let failed = $state<GalleryChanges>({});
	let hasFailures = $derived(Object.keys(failed).length > 0);
	let includedInvalid = $state(false);
	const timers = new Map<string, { timer: ReturnType<typeof setTimeout>; run: () => void }>();

	let activity = $derived(createSessionActivity(data.idleTimeoutMs));
	let expired = $derived(
		expiryDay !== '' && new Date(endOfLocalDay(expiryDay)).getTime() < Date.now()
	);
	afterNavigate(() => {
		void activity();
	});
	beforeNavigate(flush);
	onMount(() => {
		const stop = listenForActivity(() => activity());
		window.addEventListener('pagehide', flush);
		return () => {
			stop();
			window.removeEventListener('pagehide', flush);
			flush();
		};
	});

	// Latest save number per field: only the newest save of a field may update its state.
	const latestSave: Record<string, number> = {};
	let saveCount = 0;

	async function save(changes: GalleryChanges) {
		const keys = Object.keys(changes) as (keyof GalleryChanges)[];
		const number = ++saveCount;
		for (const key of keys) latestSave[key] = number;
		const isLatest = (key: keyof GalleryChanges) => latestSave[key] === number;
		inflight++;
		justSaved = false;
		void activity();
		try {
			const result = await updateGallery(saved.id, changes);
			const current = keys.filter(isLatest);
			for (const key of current) {
				saved = { ...saved, [key]: result[key], updated_at: result.updated_at };
				delete failed[key];
			}
			if (current.length > 0) justSaved = true;
		} catch (error) {
			if (error instanceof GalleryApiError && error.status === 401) {
				await goto('/login?expired=1', { refreshAll: true });
				return;
			}
			for (const key of keys.filter(isLatest)) failed = { ...failed, [key]: changes[key] };
		} finally {
			inflight--;
		}
	}
	function debounce(field: string, run: () => void) {
		clearTimeout(timers.get(field)?.timer);
		timers.set(field, {
			run,
			timer: setTimeout(() => {
				timers.delete(field);
				run();
			}, DEBOUNCE)
		});
	}
	function cancel(field: string) {
		clearTimeout(timers.get(field)?.timer);
		timers.delete(field);
	}
	/** Sends every debounced change now; used on blur, navigation and page hide. */
	function flush() {
		for (const [field, { timer, run }] of [...timers]) {
			clearTimeout(timer);
			timers.delete(field);
			run();
		}
	}
	function saveTitle() {
		const value = title.trim() || null;
		if (value === saved.title) return;
		void save({ title: value });
	}
	function editTitle() {
		debounce('title', saveTitle);
	}
	/** Focus moving to Retry is not leaving for good: the user is about to retry the attempt. */
	const goingToRetry = (event: FocusEvent) =>
		event.relatedTarget instanceof HTMLElement && event.relatedTarget.dataset.retry !== undefined;
	function leaveTitle(event?: FocusEvent) {
		cancel('title');
		if (failed.title !== undefined && failed.title === (title.trim() || null)) {
			if (event && goingToRetry(event)) return;
			delete failed.title;
			title = saved.title ?? '';
		} else saveTitle();
	}
	function setLayout(value: GalleryLayout) {
		layout = value;
		void save({ layout });
	}
	function setExpiry() {
		void save({ expires_at: expiryDay ? endOfLocalDay(expiryDay) : null });
	}
	function clearExpiry() {
		expiryDay = '';
		setExpiry();
	}
	function saveIncluded() {
		const count = included;
		includedInvalid = count !== null && (!Number.isInteger(count) || count < 1);
		if (includedInvalid || count === saved.included_photos) return;
		void save({ included_photos: count });
	}
	function editIncluded() {
		debounce('included_photos', saveIncluded);
	}
	function leaveIncluded(event: FocusEvent) {
		cancel('included_photos');
		if (failed.included_photos !== undefined && failed.included_photos === included) {
			if (goingToRetry(event)) return;
			delete failed.included_photos;
			included = saved.included_photos;
			includedInvalid = false;
		} else saveIncluded();
	}
	async function startSharing() {
		sharing = true;
		shareFailed = false;
		try {
			shareToken = (await shareGallery(saved.id)).share_token;
		} catch (error) {
			if (error instanceof GalleryApiError && error.status === 401) {
				await goto('/login?expired=1', { refreshAll: true });

				return;
			}

			shareFailed = true;
		} finally {
			sharing = false;
		}
	}
	async function logout() {
		signOutError = '';
		try {
			await authRequest('logout');
			await goto('/login', { refreshAll: true });
		} catch (error) {
			signOutError = error instanceof Error ? error.message : 'Unable to sign out. Please try again.';
		}
	}
	function moveTab(event: KeyboardEvent) {
		const step = { ArrowRight: 1, ArrowLeft: -1 }[event.key];
		if (step === undefined) return;

		event.preventDefault();

		const index = TABS.findIndex(({ id }) => id === tab);
		const next = TABS[(index + step + TABS.length) % TABS.length].id;

		tab = next;
		document.getElementById(`tab-${next}`)?.focus();
	}
	function retry() {
		for (const [key, value] of Object.entries(failed)) void save({ [key]: value });
	}
</script>

<svelte:head><title>{saved.title ?? UNTITLED_GALLERY} — PhotoProofing</title></svelte:head>
<AppHeader back={{ href: '/dashboard', label: 'All galleries' }} onsignout={logout} />
<main class="mx-auto grid max-w-6xl gap-6 p-6">
	{#if signOutError}<p role="alert" class="text-danger">{signOutError}</p>{/if}

	<div class="flex flex-wrap items-start justify-between gap-4">
		<div class="grid min-w-0 flex-1 gap-1">
			<div class="flex flex-wrap items-center gap-3">
				<label class="sr-only" for="title">Gallery title</label>
				<input
					id="title"
					class="min-w-0 flex-1 rounded-md border border-transparent bg-transparent px-2 py-1 font-serif text-3xl hover:border-border-strong focus:border-foreground focus:outline-none"
					bind:value={title}
					placeholder={UNTITLED_GALLERY}
					maxlength={TITLE_MAX}
					oninput={editTitle}
					onblur={(event) => leaveTitle(event)}
					onkeydown={(event) => {
						if (event.key === 'Enter') {
							event.preventDefault();
							leaveTitle();
						}
					}}
				/>
				<div class="flex items-center gap-2 text-sm" role="status" aria-live="polite">
					{#if inflight > 0}
						<span>Saving…</span>
					{:else if hasFailures}
						<span>Couldn’t save</span>
						<!-- mousedown would blur the edited field, which reverts a failed value before the click -->
						<Button
							variant="outline"
							size="sm"
							data-retry
							onmousedown={(event) => event.preventDefault()}
							onclick={retry}
						>Retry</Button>
					{:else if justSaved}
						<span>Saved</span>
					{/if}
				</div>
			</div>
			<p class="px-2 text-sm text-muted-foreground">
				{#if shareToken}<span>Shared</span>{:else}<span>Draft · not shared</span>{/if}
			</p>
		</div>
		<div class="flex flex-wrap items-center gap-2">
			{#if shareToken}
				<Button disabled title="Delivering finished photos arrives with a later release">
					Deliver to client
				</Button>
			{:else}
				<PressAgainButton
					label="Share gallery"
					confirmLabel="Press again to share"
					onconfirm={startSharing}
					disabled={sharing}
				/>
			{/if}
		</div>
	</div>
	{#if shareFailed}
		<p class="text-sm text-danger" role="alert">Couldn’t share the gallery. Please try again.</p>
	{/if}
	{#if shareToken}
		<div class="grid gap-1">
			<h2 class="text-sm font-medium">Client link</h2>
			<ShareLink token={shareToken} origin={page.url.origin} />
		</div>
	{/if}

	<div class="grid gap-6 md:grid-cols-[20rem_1fr]">
		<aside
			class="grid content-start gap-5 rounded-lg border border-border bg-panel p-4"
			aria-label="Gallery settings"
		>
			<h2 class="text-lg">Settings</h2>

			<fieldset class="grid gap-2">
				<legend class="mb-1 text-sm font-medium">Layout</legend>
				<div class="flex gap-4 text-sm">
					<label class="flex items-center gap-2">
						<input
							type="radio"
							name="layout"
							value="grid"
							checked={layout === 'grid'}
							onchange={() => setLayout('grid')}
						/>
						Grid
					</label>
					<label class="flex items-center gap-2">
						<input
							type="radio"
							name="layout"
							value="masonry"
							checked={layout === 'masonry'}
							onchange={() => setLayout('masonry')}
						/>
						Masonry
					</label>
				</div>
			</fieldset>

			<p class="text-sm text-muted-foreground">Hero photo: set once photos are added.</p>

			<div class="grid gap-2">
				<label for="expiry" class="text-sm font-medium">Expiry</label>
				<div class="flex gap-2">
					<input
						id="expiry"
						type="date"
						class="min-w-0 flex-1 rounded-md border border-border-strong bg-white px-2 py-1 text-sm"
						value={expiryDay}
						onchange={(event) => {
							expiryDay = event.currentTarget.value;
							setExpiry();
						}}
					/>
					<Button variant="outline" size="sm" onclick={clearExpiry} disabled={expiryDay === ''}>
						Clear
					</Button>
				</div>
				<p class="text-xs text-muted-foreground">
					{expiryDay ? 'Ends at the end of this day.' : 'No expiry.'}
				</p>
				{#if expired}
					<p class="text-xs text-amber-800">
						This date is in the past, so the gallery is already expired.
					</p>
				{/if}
			</div>

			<div class="grid gap-2">
				<label for="included" class="text-sm font-medium">Included photos</label>
				<input
					id="included"
					type="number"
					min="1"
					step="1"
					placeholder="Unlimited"
					class="rounded-md border border-border-strong bg-white px-2 py-1 text-sm"
					bind:value={included}
					oninput={editIncluded}
					onblur={leaveIncluded}
					aria-invalid={includedInvalid}
					aria-describedby="included-help"
				/>
				<p
					id="included-help"
					class="text-xs {includedInvalid ? 'text-danger' : 'text-muted-foreground'}"
				>
					{includedInvalid ? 'Enter a whole number of at least 1.' : 'Leave empty for unlimited.'}
				</p>
			</div>

			<label
				class="flex items-center gap-2 border-t border-border pt-4 text-sm font-medium"
			>
				<input
					type="checkbox"
					bind:checked={watermark}
					onchange={() => void save({ watermark_enabled: watermark })}
				/>
				Watermark
			</label>
		</aside>

		<section class="min-w-0">
			<div
				role="tablist"
				aria-label="Gallery content"
				class="mb-4 flex gap-6 border-b border-border-strong"
			>
				{#each TABS as { id, label } (id)}
					<button
						type="button"
						role="tab"
						id={`tab-${id}`}
						aria-selected={tab === id}
						aria-controls={`panel-${id}`}
						tabindex={tab === id ? 0 : -1}
						class="border-b-2 px-0 pb-2 text-xs tracking-[0.2em] uppercase {tab === id
							? 'border-foreground text-foreground'
							: 'border-transparent text-muted-foreground'}"
						onclick={() => (tab = id)}
						onkeydown={moveTab}
					>
						{label}
					</button>
				{/each}
			</div>
			<div role="tabpanel" id={`panel-${tab}`} aria-labelledby={`tab-${tab}`} class="grid gap-4">
				{#if tab === 'photos'}
					<div class="flex justify-end">
						<Button
							variant="outline"
							size="sm"
							disabled
							title="Photo upload arrives with a later release"
						>
							+ Upload
						</Button>
					</div>
					<p class="rounded-lg border border-dashed border-border-strong p-12 text-center text-muted-foreground">
						No photos yet. Drop JPEGs here to start.
					</p>
				{:else if tab === 'favorites'}
					<div class="flex flex-wrap items-center justify-between gap-2">
						<span class="text-sm text-muted-foreground">Client selection</span>
						<Button size="sm" disabled title="Downloads arrive with a later release">
							Download
						</Button>
					</div>
					<p class="rounded-lg border border-dashed border-border-strong p-12 text-center text-muted-foreground">
						No favorites yet. The client’s selection will appear here.
					</p>
				{:else}
					<div class="flex justify-end">
						<Button
							variant="outline"
							size="sm"
							disabled
							title="Uploading finished photos arrives with a later release"
						>
							+ Add finished photos
						</Button>
					</div>
					<p class="rounded-lg border border-dashed border-border-strong p-12 text-center text-muted-foreground">
						No finished photos yet. Drop the edited JPEGs here.
					</p>
				{/if}
			</div>
		</section>
	</div>
</main>
