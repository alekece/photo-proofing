<script lang="ts">
	import { afterNavigate, goto, snapshot } from '$app/navigation';
	import { page } from '$app/state';
	import { onMount, type ComponentProps, type Snippet } from 'svelte';
	import { authRequest } from '#lib/auth.js';
	import { createLiveEvents, setLiveEvents } from '#lib/events.js';
	import { setGalleryGridReturn } from '#lib/gallery/dashboardNavigation.svelte.js';
	import { NotificationCenter, setNotifications } from '#lib/gallery/notifications.svelte.js';
	import { m } from '#lib/paraglide/messages.js';
	import { rails } from '#lib/rails.svelte.js';
	import AppHeader from './AppHeader.svelte';

	let {
		children,
		studioName,
		counts
	}: {
		children: Snippet;
		studioName?: string;
		counts?: ComponentProps<typeof AppHeader>['counts'];
	} = $props();
	let signingOut = $state(false);
	let signOutError = $state('');

	const events = createLiveEvents();
	const notifications = new NotificationCenter();
	const gridReturn = setGalleryGridReturn();
	setLiveEvents(events);
	setNotifications(notifications);

	onMount(() => {
		const stop = notifications.start(events);
		const stopRails = rails.start();
		events.start();

		return () => {
			stop();
			stopRails();
			events.close();
		};
	});

	afterNavigate(({ from, to }) => {
		signOutError = '';

		if (
			to?.route.id === '/dashboard/[id]' &&
			from?.route.id !== '/dashboard' &&
			to.url.pathname !== from?.url.pathname
		) {
			gridReturn.view = undefined;
		}
	});

	snapshot({
		id: 'studio-gallery-grid-return',
		capture: () =>
			page.route.id === '/dashboard/[id]' ? $state.snapshot(gridReturn.view) : undefined,
		restore: (view) => {
			if (page.route.id === '/dashboard/[id]') gridReturn.view = view;
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

<AppHeader onsignout={logout} {studioName} {counts} />
<div
	class="flex min-w-0 flex-1 flex-col motion-reduce:transition-none sm:ml-(--workspace-rail-width) sm:transition-[margin-left] sm:duration-200 sm:ease-out"
>
	{#if signOutError}
		<p role="alert" class="workspace-inset py-2 text-sm text-danger">{signOutError}</p>
	{/if}
	{@render children()}
</div>
