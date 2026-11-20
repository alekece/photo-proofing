<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onMount } from 'svelte';
	import { afterNavigate, goto } from '$app/navigation';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import { ActivityFeed } from '#lib/activityLog/activityFeed.svelte.js';
	import { authRequest } from '#lib/auth.js';
	import ActivityFilters from '#lib/components/ActivityFilters.svelte';
	import ActivityTimeline from '#lib/components/ActivityTimeline.svelte';
	import AppHeader from '#lib/components/AppHeader.svelte';
	import { getLiveEvents } from '#lib/events.js';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const events = getLiveEvents();
	const feed = new ActivityFeed();

	let signingOut = $state(false);
	let signOutError = $state('');
	let resultsHeight = $state(0);

	const activity = $derived(createSessionActivity(data.idleTimeoutMs));
	const filtered = $derived(Object.keys(feed.filters).length > 0);

	afterNavigate(() => {
		void activity();
	});
	onMount(() => {
		const stopListening = listenForActivity(() => activity());
		const stopFeed = feed.start(events);

		void feed.load();

		return () => {
			stopListening();
			stopFeed();
		};
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

<svelte:head><title>{m.activity_title()} — PhotoProofing</title></svelte:head>

<AppHeader onsignout={logout} studioName={data.settings?.studioName} />

<main class="flex flex-1 flex-col px-3.5 pt-4 pb-12 sm:px-8 sm:pt-6">
	{#if signOutError}
		<p role="alert" class="mb-3 text-sm text-danger">{signOutError}</p>
	{/if}

	<div class="mb-4 sm:mb-6">
		<h1 class="text-[1.875rem] leading-[1.08] sm:text-[2.5rem]">{m.activity_title()}</h1>
		<p class="mt-1 text-sm text-muted-foreground">{m.activity_subtitle()}</p>
	</div>

	<ActivityFilters {feed} galleries={data.galleries} contacts={data.contacts} {resultsHeight} />
	<ActivityTimeline {feed} {filtered} bind:resultsHeight onclear={() => feed.clearFilters()} />
</main>
