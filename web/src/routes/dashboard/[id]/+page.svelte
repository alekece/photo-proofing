<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onMount, untrack } from 'svelte';
	import { afterNavigate, beforeNavigate, goto } from '$app/navigation';
	import { page } from '$app/state';
	import { getUploadQueue } from '#lib/gallery/uploads.svelte.js';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import { authRequest } from '#lib/auth.js';
	import AppHeader from '#lib/components/AppHeader.svelte';
	import ArchiveGalleryDialog from '#lib/components/ArchiveGalleryDialog.svelte';
	import ClientPreview from '#lib/components/ClientPreview.svelte';
	import DeleteGalleryDialog from '#lib/components/DeleteGalleryDialog.svelte';
	import GalleryStudio from '#lib/components/GalleryStudio.svelte';
	import RestoreExpiredDialog from '#lib/components/RestoreExpiredDialog.svelte';
	import PublishDialog from '#lib/components/PublishDialog.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import Input from '#lib/components/ui/Input/Input.svelte';
	import Label from '#lib/components/ui/Label/Label.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import {
		archiveGallery,
		deliverGallery,
		deleteGallery,
		GalleryApiError,
		restoreGallery,
		shareGallery,
		updateGallery,
		type GalleryChanges
	} from '#lib/gallery/api.js';
	import { goingToRetry } from '#lib/gallery/focus.js';
	import { getNotifications } from '#lib/gallery/notifications.svelte.js';
	import { TITLE_MAX } from '#lib/gallery/format.js';
	import { DEFAULT_SETTINGS } from '#lib/settings/settings.js';
	import type { PageData } from './$types';

	const DEBOUNCE = 600;

	let { data }: { data: PageData } = $props();
	const queue = getUploadQueue();
	const notifications = getNotifications();

	// Keyed on the id only: the center's own state must not re-run this after its refresh.
	$effect(() => {
		const id = data.gallery.id;

		untrack(() => void notifications?.markGalleryRead(id));
	});
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
	let heroRevision = $state<number | undefined>();
	let expectedAccessCode = $state<string | null | 'invalid' | undefined>();
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
	// Active PATCH promises; never read by the template.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const pendingSaves = new Set<Promise<boolean>>();
	let saveCount = 0;

	function save(changes: GalleryChanges): Promise<boolean> {
		const promise = performSave(changes);
		pendingSaves.add(promise);
		void promise.finally(() => pendingSaves.delete(promise));
		return promise;
	}

	async function performSave(changes: GalleryChanges): Promise<boolean> {
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
				if (key === 'access_code') {
					saved = { ...saved, access_code_protected: result.access_code_protected };
				}
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
				toasts.error(error instanceof GalleryApiError ? error.message : m.gallery_save_failed());
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
	/** Drops pending saves; the gallery they would update is going away. */
	function cancelAll() {
		for (const { timer } of timers.values()) clearTimeout(timer);
		timers.clear();
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
	/** Flushes debounced edits and verifies the access code is persisted before locking sharing. */
	async function prepareForShare(): Promise<boolean> {
		flush();

		while (pendingSaves.size > 0) {
			const outcomes = await Promise.all([...pendingSaves]);

			if (outcomes.includes(false)) {
				return false;
			}
		}

		if (
			expectedAccessCode === 'invalid' ||
			(expectedAccessCode !== undefined && (saved.access_code ?? null) !== expectedAccessCode)
		) {
			return false;
		}

		return true;
	}

	/** Creates the client link; awaits any pending setting saves first so access codes lock safely. */
	async function share() {
		if (!(await prepareForShare())) {
			throw new Error(m.gallery_share_failed());
		}

		try {
			const shared = await shareGallery(saved.id);

			shareToken = shared.share_token;
			saved = {
				...saved,
				share_token: shared.share_token,
				access_code: shared.access_code,
				access_code_protected: shared.access_code_protected,
				states: shared.states,
				updated_at: shared.updated_at
			};
		} catch (error) {
			if (error instanceof GalleryApiError && error.status === 401) {
				await goto('/login?expired=1', { refreshAll: true });
			}

			throw new Error(m.gallery_share_failed(), { cause: error });
		}
	}
	/** Sends the photographer to sign in on a 401; otherwise rethrows with a retryable message. */
	async function failAction(error: unknown, message: string): Promise<never> {
		if (error instanceof GalleryApiError && error.status === 401) {
			await goto('/login?expired=1', { refreshAll: true });
		}

		throw new Error(message, { cause: error });
	}
	async function deliver(): Promise<void> {
		flush();

		try {
			const delivered = await deliverGallery(saved.id);

			saved = { ...saved, states: delivered.states, updated_at: delivered.updated_at };
		} catch (error) {
			await failAction(error, m.gallery_deliver_failed());
		}
	}
	async function archive(): Promise<void> {
		flush();

		try {
			const archived = await archiveGallery(saved.id);

			saved = { ...saved, states: archived.states, updated_at: archived.updated_at };
		} catch (error) {
			await failAction(error, m.gallery_archive_failed());
		}
	}
	async function remove(): Promise<void> {
		try {
			await deleteGallery(saved.id);
		} catch (error) {
			await failAction(error, m.gallery_delete_failed());
		}

		cancelAll();
		await goto('/dashboard', { refreshAll: true });
	}
	/** Restore never changes the expiry, so an expired gallery gets a prompt to set a new date. */
	async function restore(): Promise<void> {
		flush();

		try {
			const restored = await restoreGallery(saved.id);

			saved = { ...saved, states: restored.states, updated_at: restored.updated_at };
			expiredSince = restored.states.includes('expired') ? restored.expires_at : null;
		} catch (error) {
			if (error instanceof GalleryApiError && error.status === 401) {
				await goto('/login?expired=1', { refreshAll: true });

				return;
			}

			toasts.error(error instanceof GalleryApiError ? error.message : m.gallery_restore_failed());
		}
	}
	async function logout() {
		signOutError = '';
		try {
			await authRequest('logout');
			await goto('/login', { refreshAll: true });
		} catch (error) {
			signOutError = error instanceof Error ? error.message : m.sign_out_failed();
		}
	}
	function retry() {
		for (const [key, value] of Object.entries(failed)) void save({ [key]: value });
	}
</script>

<svelte:head><title>{saved.title ?? m.gallery_untitled()} — PhotoProofing</title></svelte:head>
<AppHeader onsignout={logout} />
{#if signOutError}
	<p
		role="alert"
		class="border-b border-border bg-panel px-3.5 py-2 text-center text-sm text-danger sm:px-8"
	>
		{signOutError}
	</p>
{/if}

<GalleryStudio
	gallery={saved}
	{shareToken}
	origin={page.url.origin}
	settings={data.settings ?? DEFAULT_SETTINGS}
	uploading={queue?.inFlight(saved.id) ?? 0}
	bind:heroId={saved.hero_id}
	bind:heroRevision
	bind:expectedAccessCode
	onsave={save}
	schedule={debounce}
	{cancel}
	onpreview={() => {
		flush();
		previewing = true;
	}}
	onshare={() => {
		flush();
		if (expectedAccessCode === 'invalid') {
			toasts.error(m.settings_access_code_invalid());
			return;
		}
		publishing = 'share';
	}}
	onbeforesend={async () => {
		const ok = await prepareForShare();

		if (!ok) {
			toasts.error(
				expectedAccessCode === 'invalid'
					? m.settings_access_code_invalid()
					: m.gallery_share_failed()
			);
		}

		return ok;
	}}
	oninvitationsent={({ gallery: updated }) => {
		shareToken = updated.share_token;
		saved = {
			...saved,
			share_token: updated.share_token,
			access_code: updated.access_code,
			access_code_protected: updated.access_code_protected,
			states: updated.states,
			updated_at: updated.updated_at
		};
	}}
	onpublish={() => (publishing = 'deliver')}
	onarchive={() => (confirming = 'archive')}
	onrestore={restore}
	ondelete={() => (confirming = 'delete')}
>
	{#snippet heading()}
		<Label srOnly for="title">{m.gallery_title_label()}</Label>
		<Input
			id="title"
			class="min-h-0 min-w-28 flex-1 rounded-none border-0 border-b border-transparent bg-transparent p-0 font-serif text-[1.0625rem] leading-tight text-foreground placeholder:text-muted-foreground/70 hover:border-dotted hover:border-border-strong focus:border-solid focus:border-foreground md:text-[1.25rem]"
			bind:value={title}
			placeholder={m.gallery_untitled()}
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
		<div
			class={inflight === 0 && hasFailures
				? 'order-last flex basis-full items-center gap-1.5 pt-0.5 text-xs text-danger'
				: 'sr-only'}
			role="status"
			aria-live="polite"
		>
			{#if inflight === 0 && hasFailures}
				<span>{m.gallery_not_saved()}</span>
				<!-- mousedown would blur the edited field, which reverts a failed value before the click -->
				<Button
					variant="outline"
					size="sm"
					data-retry
					class="min-h-7! px-2.5! text-xs"
					onmousedown={(event) => event.preventDefault()}
					onclick={retry}>{m.action_retry()}</Button
				>
			{/if}
		</div>
	{/snippet}
</GalleryStudio>
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
		title={saved.title ?? m.gallery_untitled()}
		expiresAt={saved.expires_at}
		uploading={queue?.inFlight(saved.id) ?? 0}
		onconfirm={publishing === 'share' ? share : deliver}
		onclose={() => (publishing = null)}
	/>
{/if}
