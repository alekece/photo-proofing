<script lang="ts">
	import { onMount } from 'svelte';
	import { afterNavigate, beforeNavigate, goto } from '$app/navigation';
	import { page } from '$app/state';
	import { getUploadQueue } from '#lib/gallery/uploads.svelte.js';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import { authRequest } from '#lib/auth.js';
	import AppHeader from '#lib/components/AppHeader.svelte';
	import ArchiveGalleryDialog from '#lib/components/ArchiveGalleryDialog.svelte';
	import ClientPreview from '#lib/components/ClientPreview.svelte';
	import DeleteGalleryDialog from '#lib/components/DeleteGalleryDialog.svelte';
	import GalleryHeader from '#lib/components/GalleryHeader.svelte';
	import GallerySettings from '#lib/components/GallerySettings.svelte';
	import GalleryTabs from '#lib/components/GalleryTabs.svelte';
	import RestoreExpiredDialog from '#lib/components/RestoreExpiredDialog.svelte';
	import PublishDialog from '#lib/components/PublishDialog.svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import Input from '#lib/components/ui/input/input.svelte';
	import Label from '#lib/components/ui/label/label.svelte';
	import { toasts } from '#lib/components/ui/toast/toasts.svelte.js';
	import {
		GalleryApiError,
		restoreGallery,
		shareGallery,
		updateGallery,
		type GalleryChanges
	} from '#lib/gallery/api.js';
	import { goingToRetry } from '#lib/gallery/focus.js';
	import { TITLE_MAX, UNTITLED_GALLERY } from '#lib/gallery/format.js';
	import type { PageData } from './$types';

	const DEBOUNCE = 600;

	let { data }: { data: PageData } = $props();
	const queue = getUploadQueue();
	// svelte-ignore state_referenced_locally
	let saved = $state(data.gallery);
	// svelte-ignore state_referenced_locally
	let title = $state(data.gallery.title ?? '');
	// svelte-ignore state_referenced_locally
	let shareToken = $state(data.gallery.share_token);
	let publishing = $state<'share' | 'deliver' | null>(null);
	let confirming = $state<'archive' | 'delete' | null>(null);
	// The expiry the restored gallery is still past; the prompt stays open while a new one is saved.
	let expiredSince = $state<string | null>(null);
	let photoCount = $state<number | null>(null);
	let favoriteCount = $state<number | null>(null);
	let heroRevision = $state<number | undefined>();
	let previewing = $state(false);
	let signOutError = $state('');
	let inflight = $state(0);
	// Attempted values of fields whose last save failed; cleared per field on that field's success.
	let failed = $state<GalleryChanges>({});
	let hasFailures = $derived(Object.keys(failed).length > 0);
	// Non-reactive timer bookkeeping; never read by the template.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const timers = new Map<string, { timer: ReturnType<typeof setTimeout>; run: () => void }>();

	let activity = $derived(createSessionActivity(data.idleTimeoutMs));
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

	async function save(changes: GalleryChanges): Promise<boolean> {
		const keys = Object.keys(changes) as (keyof GalleryChanges)[];
		const number = ++saveCount;
		for (const key of keys) latestSave[key] = number;
		const isLatest = (key: keyof GalleryChanges) => latestSave[key] === number;
		// Title failures stay beside the field; setting failures use toasts.
		const inTitle = keys.length === 1 && keys[0] === 'title';

		inflight++;
		void activity();
		try {
			const result = await updateGallery(saved.id, changes);
			const current = keys.filter(isLatest);
			for (const key of current) {
				saved = { ...saved, [key]: result[key], updated_at: result.updated_at };
				delete failed[key];
			}
			// A response for another field may still describe an older expiry.
			if (current.length > 0 && result.expires_at === saved.expires_at) {
				saved = { ...saved, states: result.states };
			}

			return true;
		} catch (error) {
			if (error instanceof GalleryApiError && error.status === 401) {
				await goto('/login?expired=1', { refreshAll: true });
				return false;
			}
			if (inTitle) {
				for (const key of keys.filter(isLatest)) failed = { ...failed, [key]: changes[key] };
			} else {
				toasts.error(
					error instanceof GalleryApiError ? error.message : 'Couldn’t save. Please try again.'
				);
			}

			return false;
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
	function leaveTitle(event?: FocusEvent) {
		cancel('title');
		if (failed.title !== undefined && failed.title === (title.trim() || null)) {
			if (event && goingToRetry(event)) return;
			delete failed.title;
			title = saved.title ?? '';
		} else saveTitle();
	}
	/** Creates the client link and returns it for the dialog to offer. */
	async function share() {
		try {
			shareToken = (await shareGallery(saved.id)).share_token;
		} catch (error) {
			if (error instanceof GalleryApiError && error.status === 401) {
				await goto('/login?expired=1', { refreshAll: true });
			}

			throw new Error('Couldn’t share the gallery. Please try again.', { cause: error });
		}

		return `${page.url.origin}/g/${shareToken}`;
	}
	/** The dialog is ready; the endpoint arrives with #71, so confirming says so for now. */
	async function deliver(): Promise<void> {
		throw new Error('Delivering finished photos arrives with a later release.');
	}
	/** Archiving, restoring and deleting arrive with #73; the dialogs are ready for it. */
	async function archive(): Promise<void> {
		throw new Error('Archiving galleries arrives with a later release.');
	}
	async function remove(): Promise<void> {
		throw new Error('Deleting galleries arrives with a later release.');
	}
	/** Restore never changes the expiry, so an expired gallery gets a prompt to set a new date. */
	async function restore(): Promise<void> {
		flush();

		try {
			const restored = await restoreGallery(saved.id);

			saved = { ...saved, states: restored.states, updated_at: restored.updated_at };
			toasts.success('Restored');
			expiredSince = restored.states.includes('expired') ? restored.expires_at : null;
		} catch (error) {
			if (error instanceof GalleryApiError && error.status === 401) {
				await goto('/login?expired=1', { refreshAll: true });

				return;
			}

			toasts.error(
				error instanceof GalleryApiError ? error.message : 'Couldn’t restore. Please try again.'
			);
		}
	}
	async function logout() {
		signOutError = '';
		try {
			await authRequest('logout');
			await goto('/login', { refreshAll: true });
		} catch (error) {
			signOutError =
				error instanceof Error ? error.message : 'Unable to sign out. Please try again.';
		}
	}
	function retry() {
		for (const [key, value] of Object.entries(failed)) void save({ [key]: value });
	}
</script>

<svelte:head><title>{saved.title ?? UNTITLED_GALLERY} — PhotoProofing</title></svelte:head>
<div class="relative">
	<AppHeader overlay crumb={saved.title ?? UNTITLED_GALLERY} onsignout={logout} />
	{#if signOutError}<p
			role="alert"
			class="absolute inset-x-0 top-16 z-20 text-center text-warning-on-dark"
		>
			{signOutError}
		</p>{/if}

	<GalleryHeader
		{heroRevision}
		gallery={{ ...saved, share_token: shareToken }}
		{photoCount}
		{favoriteCount}
		onpreview={() => (previewing = true)}
		onshare={() => (publishing = 'share')}
		ondeliver={() => (publishing = 'deliver')}
		onrestore={restore}
	>
		{#snippet heading()}
			<div class="flex flex-wrap items-center gap-3">
				<Label srOnly for="title">Gallery title</Label>
				<Input
					id="title"
					class="mt-2 mb-1.5 min-h-0 flex-1 rounded-none border-0 border-b border-transparent bg-transparent p-0 font-serif text-[2rem] leading-none text-panel placeholder:text-panel/70 hover:border-dotted hover:border-panel/50 focus:border-solid focus:border-panel sm:text-[2.875rem]"
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
							class="border-white/60 bg-transparent text-inverse hover:bg-white/15"
							onmousedown={(event) => event.preventDefault()}
							onclick={retry}>Retry</Button
						>
					{/if}
				</div>
			</div>
		{/snippet}
	</GalleryHeader>
</div>
<div class="grid min-h-[calc(100vh-25rem)] grid-cols-[3.5rem_1fr] md:grid-cols-[18.75rem_1fr]">
	<GallerySettings
		gallery={saved}
		{shareToken}
		origin={page.url.origin}
		onsave={save}
		schedule={debounce}
		{cancel}
		onarchive={() => (confirming = 'archive')}
		onrestore={restore}
		ondelete={() => (confirming = 'delete')}
	/>

	<GalleryTabs
		galleryId={saved.id}
		included={saved.included_photos}
		layout={saved.layout}
		bind:heroId={saved.hero_id}
		bind:heroRevision
		bind:photoCount
		bind:favoriteCount
		watermarked={saved.watermark_enabled}
		delivered={saved.states.includes('delivered')}
		shared={shareToken !== null}
	/>
</div>
{#if previewing}
	<ClientPreview gallery={saved} onclose={() => (previewing = false)} />
{/if}
{#if confirming === 'archive'}
	<ArchiveGalleryDialog onconfirm={archive} onclose={() => (confirming = null)} />
{:else if confirming === 'delete'}
	<DeleteGalleryDialog
		galleryId={saved.id}
		onconfirm={remove}
		onclose={() => (confirming = null)}
	/>
{/if}
{#if expiredSince}
	<RestoreExpiredDialog
		expiresAt={expiredSince}
		onsave={(expiresAt) => save({ expires_at: expiresAt })}
		onclose={() => (expiredSince = null)}
	/>
{/if}
{#if publishing}
	<PublishDialog
		kind={publishing}
		galleryId={saved.id}
		title={saved.title ?? UNTITLED_GALLERY}
		expiresAt={saved.expires_at}
		uploading={queue?.inFlight(saved.id) ?? 0}
		onconfirm={publishing === 'share' ? share : deliver}
		onclose={() => (publishing = null)}
	/>
{/if}
