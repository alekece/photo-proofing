<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onMount } from 'svelte';
	import { afterNavigate } from '$app/navigation';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import { ActivityFeed } from '#lib/activityLog/activityFeed.svelte.js';
	import ActivityFilters from '#lib/components/ActivityFilters.svelte';
	import ActivityTimeline from '#lib/components/ActivityTimeline.svelte';
	import WorkspaceHeading from '#lib/components/ui/WorkspaceHeading/WorkspaceHeading.svelte';
	import { getLiveEvents } from '#lib/events.js';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const events = getLiveEvents();
	const feed = new ActivityFeed();

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
</script>

<svelte:head><title>{m.activity_title()} — PhotoProofing</title></svelte:head>

<main class="flex flex-1 flex-col workspace-inset pt-4 pb-12 sm:pt-6">
	<div class="mb-4 sm:mb-6">
		<WorkspaceHeading title={m.activity_title()} />
	</div>

	<ActivityFilters {feed} searchLabel={m.activity_search_workspace()} {resultsHeight} />
	<ActivityTimeline {feed} {filtered} bind:resultsHeight onclear={() => feed.clearFilters()} />
</main>
