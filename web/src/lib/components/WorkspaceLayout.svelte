<script lang="ts" module>
	import { getContext } from 'svelte';

	const studioNameKey = Symbol('workspace studio name');

	export function getStudioNamePreview() {
		return getContext<((name: string | undefined) => void) | undefined>(studioNameKey);
	}
</script>

<script lang="ts">
	import { afterNavigate, beforeNavigate, goto } from '$app/navigation';
	import { onMount, setContext, type Snippet } from 'svelte';
	import { authRequest } from '#lib/auth.js';
	import { createLiveEvents, setLiveEvents } from '#lib/events.js';
	import { setGalleryGridReturn } from '#lib/gallery/dashboardNavigation.svelte.js';
	import { NotificationCenter, setNotifications } from '#lib/gallery/notifications.svelte.js';
	import { m } from '#lib/paraglide/messages.js';
	import AppHeader from './AppHeader.svelte';
	import SiteFooter from './SiteFooter.svelte';

	let { children, studioName }: { children: Snippet; studioName?: string } = $props();
	let previewName = $state<string>();
	let signingOut = $state(false);
	let signOutError = $state('');

	const events = createLiveEvents();
	const notifications = new NotificationCenter();
	const gridReturn = setGalleryGridReturn();
	setLiveEvents(events);
	setNotifications(notifications);
	setContext(studioNameKey, (name: string | undefined) => (previewName = name));

	onMount(() => {
		const stop = notifications.start(events);
		events.start();

		return () => {
			stop();
			events.close();
		};
	});

	afterNavigate(() => (signOutError = ''));
	beforeNavigate(({ from, to }) => {
		if (
			to?.route.id === '/dashboard/[id]' &&
			from?.route.id !== '/dashboard' &&
			to.url.pathname !== from?.url.pathname
		) {
			gridReturn.view = undefined;
		}
	});

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

<AppHeader onsignout={logout} studioName={previewName ?? studioName} />
<div class="flex min-w-0 flex-1 flex-col sm:ml-(--gallery-rail-width)">
	{#if signOutError}
		<p role="alert" class="workspace-inset py-2 text-sm text-danger">{signOutError}</p>
	{/if}
	{@render children()}
	<SiteFooter compact />
</div>
