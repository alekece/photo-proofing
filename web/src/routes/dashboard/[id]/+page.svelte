<script lang="ts">
	import { onMount } from 'svelte';
	import { afterNavigate, beforeNavigate, goto } from '$app/navigation';
	import { page } from '$app/state';
	import { getUploadQueue } from '#lib/gallery/uploads.svelte.js';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import { authRequest } from '#lib/auth.js';
	import AppHeader from '#lib/components/AppHeader.svelte';
	import ClientPreview from '#lib/components/ClientPreview.svelte';
	import GalleryActions from '#lib/components/GalleryActions.svelte';
	import GallerySettings from '#lib/components/GallerySettings.svelte';
	import GalleryTabs from '#lib/components/GalleryTabs.svelte';
	import PublishDialog from '#lib/components/PublishDialog.svelte';
	import ShareLink from '#lib/components/ShareLink.svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import Input from '#lib/components/ui/input/input.svelte';
	import Label from '#lib/components/ui/label/label.svelte';
	import PageShell from '#lib/components/ui/page-shell/page-shell.svelte';
	import {
		GalleryApiError,
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
	let previewing = $state(false);
	let signOutError = $state('');
	let inflight = $state(0);
	let justSaved = $state(false);
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
<AppHeader back={{ href: '/dashboard', label: 'All galleries' }} onsignout={logout} />
<PageShell width="wide">
	{#if signOutError}<p role="alert" class="text-danger">{signOutError}</p>{/if}

	<div class="flex flex-wrap items-start justify-between gap-4">
		<div class="grid min-w-0 flex-1 gap-1">
			<div class="flex flex-wrap items-center gap-3">
				<Label srOnly for="title">Gallery title</Label>
				<Input
					id="title"
					class="min-h-0 flex-1 border-transparent bg-transparent px-2 py-1 font-serif text-3xl hover:border-border-strong focus:border-foreground"
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
							onclick={retry}>Retry</Button
						>
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
			<Button variant="outline" onclick={() => (previewing = true)}>Preview</Button>
			{#if shareToken}
				<Button onclick={() => (publishing = 'deliver')}>Deliver to client</Button>
			{:else}
				<Button onclick={() => (publishing = 'share')}>Share gallery</Button>
			{/if}
		</div>
	</div>
	{#if shareToken}
		<div class="grid gap-1">
			<h2 class="text-sm font-medium">Client link</h2>
			<ShareLink token={shareToken} origin={page.url.origin} />
		</div>
	{/if}

	<div class="grid gap-6 md:grid-cols-[20rem_1fr]">
		<div class="grid content-start gap-4">
			<GallerySettings
				gallery={saved}
				bind:failed
				onsave={(changes) => void save(changes)}
				schedule={debounce}
				{cancel}
			/>
			<GalleryActions gallery={saved} onchange={(gallery) => (saved = gallery)} />
		</div>

		<GalleryTabs
			galleryId={saved.id}
			included={saved.included_photos}
			bind:heroId={saved.hero_id}
		/>
	</div>
</PageShell>
{#if previewing}
	<ClientPreview gallery={saved} onclose={() => (previewing = false)} />
{/if}
{#if publishing}
	<PublishDialog
		kind={publishing}
		galleryId={saved.id}
		expiresAt={saved.expires_at}
		uploading={queue?.inFlight(saved.id) ?? 0}
		onconfirm={publishing === 'share' ? share : deliver}
		onclose={() => (publishing = null)}
	/>
{/if}
