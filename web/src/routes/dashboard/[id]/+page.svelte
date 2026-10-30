<script lang="ts">
	import { onMount } from 'svelte';
	import { afterNavigate, beforeNavigate, goto } from '$app/navigation';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import Placeholder from '#lib/components/Placeholder.svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import {
		GalleryApiError,
		updateGallery,
		type GalleryChanges,
		type GalleryLayout
	} from '#lib/gallery/api.js';
	import { endOfLocalDay, toLocalDate } from '#lib/gallery/format.js';
	import type { PageData } from './$types';

	const DEBOUNCE = 600;
	const TITLE_MAX = 255;

	let { data }: { data: PageData } = $props();
	// svelte-ignore state_referenced_locally
	let saved = $state(data.gallery);
	// svelte-ignore state_referenced_locally
	let title = $state(data.gallery.title);
	// svelte-ignore state_referenced_locally
	let expiryDay = $state(data.gallery.expires_at ? toLocalDate(data.gallery.expires_at) : '');
	// svelte-ignore state_referenced_locally
	let included = $state<number | null>(data.gallery.included_photos);
	// svelte-ignore state_referenced_locally
	let layout = $state<GalleryLayout>(data.gallery.layout);
	// svelte-ignore state_referenced_locally
	let watermark = $state(data.gallery.watermark_enabled);
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

	async function save(changes: GalleryChanges) {
		inflight++;
		justSaved = false;
		void activity();
		try {
			const result = await updateGallery(saved.id, changes);
			saved = { ...saved, updated_at: result.updated_at };
			for (const key of Object.keys(changes) as (keyof GalleryChanges)[]) {
				saved = { ...saved, [key]: result[key] };
			}
			for (const key of Object.keys(changes) as (keyof GalleryChanges)[]) delete failed[key];
			justSaved = true;
		} catch (error) {
			if (error instanceof GalleryApiError && error.status === 401) {
				await goto('/login?expired=1', { refreshAll: true });
				return;
			}
			failed = { ...failed, ...changes };
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
		const value = title.trim();
		if (value === '' || value === saved.title) return;
		void save({ title: value });
	}
	function editTitle() {
		if (title.trim() === '') cancel('title');
		else debounce('title', saveTitle);
	}
	function leaveTitle() {
		cancel('title');
		if ('title' in failed) {
			delete failed.title;
			title = saved.title;
		} else if (title.trim() === '') title = saved.title;
		else saveTitle();
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
	function leaveIncluded() {
		cancel('included_photos');
		if ('included_photos' in failed) {
			delete failed.included_photos;
			included = saved.included_photos;
			includedInvalid = false;
		} else saveIncluded();
	}
	function retry() {
		for (const [key, value] of Object.entries(failed)) void save({ [key]: value });
	}
</script>

<svelte:head><title>{saved.title} — PhotoProofing</title></svelte:head>
<main class="mx-auto grid max-w-5xl gap-6 p-6">
	<a href="/dashboard" class="text-sm underline underline-offset-4">← All galleries</a>

	<div class="aspect-[3/1] overflow-hidden rounded-xl">
		<Placeholder background="sheet" icon label="Your hero photo will appear here" />
	</div>

	<div class="flex flex-wrap items-center gap-3">
		<label class="sr-only" for="title">Gallery title</label>
		<input
			id="title"
			class="min-w-0 flex-1 rounded-md border border-transparent bg-transparent px-2 py-1 text-2xl font-semibold hover:border-input focus:border-ring focus:outline-none"
			bind:value={title}
			maxlength={TITLE_MAX}
			oninput={editTitle}
			onblur={leaveTitle}
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
					onmousedown={(event) => event.preventDefault()}
					onclick={retry}
				>Retry</Button>
			{:else if justSaved}
				<span>Saved</span>
			{/if}
		</div>
	</div>

	<div class="grid gap-6 md:grid-cols-[1fr_20rem]">
		<section
			class="grid min-h-48 place-items-center rounded-xl border border-dashed p-8 text-center text-muted-foreground"
			aria-label="Photos"
		>
			Photos will be added here
		</section>

		<aside class="grid content-start gap-5 rounded-xl border p-4" aria-label="Gallery settings">
			<h2 class="font-medium">Settings</h2>

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

			<div class="grid gap-2">
				<label for="expiry" class="text-sm font-medium">Expiry</label>
				<div class="flex gap-2">
					<input
						id="expiry"
						type="date"
						class="min-w-0 flex-1 rounded-md border bg-background px-2 py-1 text-sm"
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
					<p class="text-xs text-amber-700">This date is in the past, so the gallery is already expired.</p>
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
					class="rounded-md border bg-background px-2 py-1 text-sm"
					bind:value={included}
					oninput={editIncluded}
					onblur={leaveIncluded}
					aria-invalid={includedInvalid}
					aria-describedby="included-help"
				/>
				<p id="included-help" class="text-xs {includedInvalid ? 'text-destructive' : 'text-muted-foreground'}">
					{includedInvalid ? 'Enter a whole number of at least 1.' : 'Leave empty for unlimited.'}
				</p>
			</div>

			<label class="flex items-center gap-2 text-sm font-medium">
				<input
					type="checkbox"
					bind:checked={watermark}
					onchange={() => void save({ watermark_enabled: watermark })}
				/>
				Watermark
			</label>

			<p class="text-sm text-muted-foreground">Hero photo: set once photos are added.</p>
		</aside>
	</div>
</main>
